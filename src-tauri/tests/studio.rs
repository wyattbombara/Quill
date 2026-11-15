use quill_chat::{
    model::{AgentMode, AgentRun, DebateAgent},
    services::{database, floor::Floor, studio},
};
use rusqlite::{Connection, params};
use tokio_util::sync::CancellationToken;
const SCHEMA: &str = include_str!("../src/services/schema.sql");
fn setup() -> Connection {
    let db = Connection::open_in_memory().unwrap();
    db.pragma_update(None, "foreign_keys", true).unwrap();
    db.execute_batch(SCHEMA).unwrap();
    db.execute_batch("INSERT INTO chats(title) VALUES('Original'),('Other')")
        .unwrap();
    db
}
#[test]
fn branching_preserves_files_goals_and_usage_after_reopen() {
    let mut db = setup();
    database::set_goal(&mut db, Some(1), Some("Ship".into())).unwrap();
    let t = database::begin(&mut db, 1, "First".into(), vec![])
        .unwrap()
        .id;
    db.execute(
        "INSERT INTO files(turn,name,mime,data) VALUES(?,'note','text/plain','YQ==')",
        [t],
    )
    .unwrap();
    db.execute(
        "UPDATE turns SET status='complete',reply='Done' WHERE id=?",
        [t],
    )
    .unwrap();
    database::begin(&mut db, 1, "Later".into(), vec![]).unwrap();
    let branch = studio::fork(&mut db, 1, t).unwrap();
    db.execute_batch(SCHEMA).unwrap();
    let turns = database::turns(&db, branch).unwrap();
    assert_eq!(turns.len(), 1);
    assert_eq!(turns[0].files, ["note"]);
    assert_eq!(turns[0].reply, "Done");
    assert_eq!(
        database::usage_report(&mut db, Some(branch))
            .unwrap()
            .device
            .requests,
        2
    );
    assert_eq!(
        database::usage_report(&mut db, Some(branch))
            .unwrap()
            .chat
            .requests,
        0
    );
    let w = studio::workspace(&db, branch).unwrap();
    assert_eq!(w.parent, Some(1));
    assert_eq!(w.members.len(), 2);
    assert!(studio::fork(&mut db, 2, t).is_err());
    db.execute("DELETE FROM chats WHERE id=1", []).unwrap();
    assert_eq!(database::turns(&db, branch).unwrap().len(), 1);
}
#[test]
fn linked_memory_and_draft_are_scoped_and_revision_checked() {
    let mut db = setup();
    let t = database::begin(&mut db, 1, "Basis".into(), vec![])
        .unwrap()
        .id;
    db.execute("UPDATE turns SET status='complete'", [])
        .unwrap();
    studio::pin(
        &mut db,
        1,
        None,
        "Decision".into(),
        "Offline first".into(),
        Some(t),
    )
    .unwrap();
    assert!(studio::context(&db, 2).unwrap().is_empty());
    studio::join(&mut db, 2, 1).unwrap();
    let req = database::begin(&mut db, 2, "Design".into(), vec![]).unwrap();
    assert!(
        req.input[0]["content"]
            .as_str()
            .unwrap()
            .contains("Offline first")
    );
    studio::draft(&mut db, 1, "Document".into(), 0).unwrap();
    assert!(studio::draft(&mut db, 2, "Stale".into(), 0).is_err());
    let w = studio::workspace(&db, 2).unwrap();
    assert_eq!(w.draft, "Document");
    assert_eq!(w.pins[0].source, Some(t));
    assert_eq!(w.recent.len(), 2);
    db.execute("DELETE FROM chats WHERE id=1", []).unwrap();
    assert_eq!(studio::workspace(&db, 2).unwrap().pins[0].source, None);
    db.execute("DELETE FROM chats WHERE id=2", []).unwrap();
    assert_eq!(
        db.query_row("SELECT count(*) FROM projects", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        0
    );
}
#[test]
fn ballots_survive_reopen_and_exclude_failed_answers() {
    let mut db = setup();
    let agents = (0..2)
        .map(|i| DebateAgent {
            name: format!("Seat {i}"),
            model: format!("model-{i}"),
            focus: String::new(),
            personality: String::new(),
        })
        .collect();
    let t = database::begin_turn(
        &mut db,
        1,
        "Vote".into(),
        vec![],
        None,
        Some(AgentRun {
            mode: AgentMode::Parallel,
            agents,
        }),
    )
    .unwrap()
    .id;
    let b = studio::start_ballot(&db, t, 2).unwrap();
    for (seat, status) in [(0, "failed"), (1, "complete")] {
        db.execute("INSERT INTO debate_usage(chat) VALUES(1)", [])
            .unwrap();
        db.execute(
            "INSERT INTO debate_entries(id,turn,seat,phase,status) VALUES(?,?,?,'parallel',?)",
            params![db.last_insert_rowid(), t, seat, status],
        )
        .unwrap();
    }
    assert!(studio::vote(&mut db, t, 1).is_err());
    db.execute("UPDATE turns SET status='complete' WHERE id=?", [t])
        .unwrap();
    assert!(studio::fork(&mut db, 1, t).is_err());
    assert!(studio::vote(&mut db, t, 0).is_err());
    studio::vote(&mut db, t, 1).unwrap();
    assert!(studio::vote(&mut db, t, 1).is_err());
    db.execute_batch(SCHEMA).unwrap();
    let ballot = studio::ballot(&db, t).unwrap().unwrap();
    assert_eq!(ballot.ordering, b.ordering);
    assert_eq!(ballot.winner, Some(1));
    let branch = studio::fork(&mut db, 1, t).unwrap();
    let copy = database::turns(&db, branch)
        .unwrap()
        .pop()
        .unwrap()
        .debate
        .unwrap();
    assert_eq!(copy.entries.len(), 2);
    assert_eq!(copy.ballot.unwrap().winner, Some(1));
    let scores = studio::workspace(&db, 1).unwrap().scores;
    assert_eq!(scores.len(), 1);
    assert_eq!(scores[0].wins, 1);
}
#[test]
fn immediate_floor_interrupt_cancels_only_speaker_and_respects_last_turn() {
    let floor = Floor::new(3);
    let room = CancellationToken::new();
    let (_, speaker) = floor.next(&room, false);
    floor
        .interject(Some(1), "Wait until done".into(), false)
        .unwrap();
    assert!(!speaker.is_cancelled());
    assert!(floor.interject(None, "Duplicate".into(), true).is_err());
    let (note, second) = floor.next(&room, false);
    assert_eq!(note.unwrap().seat, Some(1));
    floor
        .interject(None, "New requirement".into(), true)
        .unwrap();
    assert!(second.is_cancelled());
    assert!(!room.is_cancelled());
    let (note, last) = floor.next(&room, true);
    assert_eq!(note.unwrap().text, "New requirement");
    assert!(!last.is_cancelled());
    assert!(floor.interject(None, "Late".into(), false).is_err());
    room.cancel();
    assert!(last.is_cancelled());
}
