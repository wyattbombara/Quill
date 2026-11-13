use quill_chat::{
    model::Attachment,
    services::database::{begin, chats, set_goal, turns, usage_report},
};
use rusqlite::Connection;

fn setup() -> Connection {
    let db = Connection::open_in_memory().unwrap();
    db.pragma_update(None, "foreign_keys", true).unwrap();
    db.execute_batch(include_str!("../src/services/schema.sql"))
        .unwrap();
    db.execute("INSERT INTO chats(title) VALUES('New chat')", [])
        .unwrap();
    db
}
#[test]
fn goals_follow_the_chat_and_usage_survives_deletion() {
    let mut db = setup();
    let chat = set_goal(&mut db, None, Some("Learn Rust".into())).unwrap();
    assert_eq!(chats(&db).unwrap()[0].goal.as_deref(), Some("Learn Rust"));
    let first = begin(&mut db, chat.id, "First".into(), vec![]).unwrap();
    assert_eq!(first.input[0]["role"], "user");
    assert_eq!(
        first.input[0]["content"],
        "My goal for this conversation: Learn Rust"
    );
    let other = begin(&mut db, 1, "Other".into(), vec![]).unwrap();
    assert_eq!(other.input.len(), 1);
    db.execute("UPDATE turns SET status='complete'", [])
        .unwrap();
    set_goal(&mut db, Some(chat.id), None).unwrap();
    let next = begin(&mut db, chat.id, "Next".into(), vec![]).unwrap();
    assert_eq!(next.input.len(), 3);
    db.execute(
        "UPDATE turn_usage SET input_tokens=12, output_tokens=4 WHERE turn=?",
        [first.id],
    )
    .unwrap();
    // Simulate an older turn that predates usage tracking, then reopen twice.
    db.execute("DELETE FROM turn_usage WHERE turn=?", [other.id])
        .unwrap();
    for _ in 0..2 {
        db.execute_batch(include_str!("../src/services/schema.sql"))
            .unwrap();
    }
    let totals = usage_report(&mut db, Some(chat.id)).unwrap();
    assert_eq!((totals.chat.requests, totals.chat.reported), (2, 1));
    assert_eq!(
        (
            totals.device.requests,
            totals.device.input_tokens,
            totals.device.output_tokens
        ),
        (3, 12, 4)
    );
    db.execute("DELETE FROM chats WHERE id=?", [chat.id])
        .unwrap();
    assert_eq!(usage_report(&mut db, None).unwrap().device.requests, 3);
}
fn file() -> Attachment {
    Attachment {
        name: "notes.txt".into(),
        mime: "text/plain".into(),
        data: "SGVsbG8=".into(),
    }
}
#[test]
fn replays_only_completed_pairs_and_their_files() {
    let mut db = setup();
    let first = begin(&mut db, 1, "Review".into(), vec![file()]).unwrap();
    assert_eq!(
        first.input[0]["content"][1]["file_data"],
        "data:text/plain;base64,SGVsbG8="
    );
    assert!(begin(&mut db, 1, "overlap".into(), vec![]).is_err());
    db.execute(
        "UPDATE turns SET reply='OK',status='complete' WHERE id=?",
        [first.id],
    )
    .unwrap();
    let aborted = begin(&mut db, 1, "Aborted".into(), vec![]).unwrap();
    db.execute(
        "UPDATE turns SET reply='part',status='stopped' WHERE id=?",
        [aborted.id],
    )
    .unwrap();
    let next = begin(&mut db, 1, "Next".into(), vec![]).unwrap();
    assert_eq!(next.input.len(), 3);
    assert_eq!(next.input[1]["content"], "OK");
    let visible = turns(&db, 1).unwrap();
    assert_eq!(visible[0].files, ["notes.txt"]);
    assert_eq!(visible[1].reply, "part");
    assert_eq!(visible[1].status, "stopped");
}
#[test]
fn rejects_invalid_attachments_without_creating_a_turn() {
    let mut db = setup();
    let mut bad = file();
    bad.data = "invalid!".into();
    assert!(begin(&mut db, 1, "".into(), vec![bad]).is_err());
    assert!(turns(&db, 1).unwrap().is_empty());
}
#[test]
fn restart_recovers_stream_and_deletion_cascades() {
    let mut db = setup();
    begin(&mut db, 1, "First line\nSecond line".into(), vec![file()]).unwrap();
    db.execute("UPDATE turns SET reply='checkpoint'", [])
        .unwrap();
    db.execute_batch(include_str!("../src/services/schema.sql"))
        .unwrap();
    let recovered = turns(&db, 1).unwrap();
    assert_eq!(recovered[0].status, "interrupted");
    assert_eq!(recovered[0].reply, "checkpoint");
    let title: String = db
        .query_row("SELECT title FROM chats", [], |r| r.get(0))
        .unwrap();
    assert_eq!(title, "First line");
    db.execute("DELETE FROM chats", []).unwrap();
    for table in ["turns", "files"] {
        let count: i64 = db
            .query_row(&format!("SELECT count(*) FROM {table}"), [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 0);
    }
}

#[test]
fn motion_preferences_upgrade_and_round_trip() {
    use quill_chat::model::{InterfaceFont, Motion};
    use quill_chat::services::database::read_preferences;

    let db = setup();
    let legacy = r#"{"model":"custom-model","theme":"dark","shortcut":"Alt+Q"}"#;
    db.execute("INSERT INTO preferences(id,value) VALUES(1,?)", [legacy])
        .unwrap();
    let mut prefs = read_preferences(&db).unwrap();
    assert!(matches!(prefs.motion, Motion::System));
    assert_eq!(prefs.model, "custom-model");
    assert_eq!(prefs.shortcut, "Alt+Q");

    assert!(matches!(prefs.font, InterfaceFont::Quill));
    use quill_chat::model::Palette;
    assert!(matches!(prefs.palette, Palette::VioletClay));
    assert!(!prefs.sidebar_logo);
    prefs.palette = Palette::MossParchment;
    prefs.sidebar_logo = true;
    prefs.font = InterfaceFont::Serif;
    prefs.motion = Motion::Smooth;
    db.execute(
        "UPDATE preferences SET value=? WHERE id=1",
        [serde_json::to_string(&prefs).unwrap()],
    )
    .unwrap();
    assert!(matches!(
        read_preferences(&db).unwrap().font,
        InterfaceFont::Serif
    ));
    let restored = read_preferences(&db).unwrap();
    assert!(matches!(restored.palette, Palette::MossParchment));
    assert!(restored.sidebar_logo);
    assert!(matches!(
        read_preferences(&db).unwrap().motion,
        Motion::Smooth
    ));
}

#[test]
fn today_counts_only_dated_requests_and_survives_chat_deletion() {
    let mut db = setup();
    let id = begin(&mut db, 1, "hello".into(), vec![]).unwrap().id;
    db.execute(
        "UPDATE turn_usage SET input_tokens=4, output_tokens=7 WHERE turn=?",
        [id],
    )
    .unwrap();
    assert_eq!(
        usage_report(&mut db, Some(1)).unwrap().today.output_tokens,
        7
    );
    db.execute("DELETE FROM chats", []).unwrap();
    assert_eq!(usage_report(&mut db, None).unwrap().today.requests, 1);
    db.execute("UPDATE request_times SET started=unixepoch()-86400", [])
        .unwrap();
    let report = usage_report(&mut db, None).unwrap();
    assert_eq!(report.today.requests, 0);
    assert_eq!(report.device.requests, 1);
}
