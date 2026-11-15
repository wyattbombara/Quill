use crate::{
    error::{Result, invalid},
    model::AgentMode,
};
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

#[derive(Serialize, Deserialize, Clone)]
pub struct FloorNote {
    pub id: i64,
    pub after_entry: Option<i64>,
    pub seat: Option<usize>,
    pub text: String,
}
pub fn floor_log(db: &Connection, turn: i64) -> rusqlite::Result<Vec<FloorNote>> {
    db.prepare("SELECT id,after_entry,seat,text FROM floor_log WHERE turn=? ORDER BY id")?
        .query_map([turn], |r| {
            Ok(FloorNote {
                id: r.get(0)?,
                after_entry: r.get(1)?,
                seat: r.get(2)?,
                text: r.get(3)?,
            })
        })?
        .collect()
}
#[derive(Serialize, Deserialize, Clone)]
pub struct Ballot {
    pub ordering: Vec<usize>,
    pub winner: Option<usize>,
}
pub fn ballot(db: &Connection, turn: i64) -> rusqlite::Result<Option<Ballot>> {
    db.query_row(
        "SELECT ordering,winner FROM ballots WHERE turn=?",
        [turn],
        |r| {
            let raw: String = r.get(0)?;
            let ordering = serde_json::from_str(&raw).map_err(|e| {
                rusqlite::Error::FromSqlConversionFailure(
                    0,
                    rusqlite::types::Type::Text,
                    Box::new(e),
                )
            })?;
            Ok(Ballot {
                ordering,
                winner: r.get(1)?,
            })
        },
    )
    .optional()
}
pub fn start_ballot(db: &Connection, turn: i64, seats: usize) -> Result<Ballot> {
    let mut order: Vec<_> = (0..seats).collect();
    order.sort_by_cached_key(|_| uuid::Uuid::new_v4());
    db.execute(
        "INSERT INTO ballots(turn,ordering) VALUES(?,?)",
        params![turn, serde_json::to_string(&order)?],
    )?;
    Ok(Ballot {
        ordering: order,
        winner: None,
    })
}
pub fn vote(db: &mut Connection, turn: i64, seat: usize) -> Result<()> {
    let changed=db.execute("UPDATE ballots SET winner=?2 WHERE turn=?1 AND winner IS NULL
        AND EXISTS (SELECT 1 FROM turns WHERE id=?1 AND status<>'streaming')
        AND EXISTS (SELECT 1 FROM debate_entries WHERE turn=?1 AND seat=?2 AND phase='parallel' AND status='complete')",params![turn,seat])?;
    if changed != 1 {
        return Err(invalid("Choose a completed answer in an open ballot."));
    }
    Ok(())
}

#[derive(Serialize)]
pub struct Project {
    pub id: i64,
    pub name: String,
}
#[derive(Serialize)]
pub struct Pin {
    pub id: i64,
    pub kind: String,
    pub text: String,
    pub source: Option<i64>,
    pub chat: Option<i64>,
}
#[derive(Serialize)]
pub struct Recent {
    pub chat: i64,
    pub turn: i64,
    pub title: String,
    pub prompt: String,
    pub reply: String,
    pub status: String,
}
#[derive(Serialize)]
pub struct Score {
    pub category: String,
    pub model: String,
    pub wins: i64,
    pub ballots: i64,
}
#[derive(Serialize)]
pub struct Workspace {
    pub project: Option<i64>,
    pub projects: Vec<Project>,
    pub members: Vec<i64>,
    pub pins: Vec<Pin>,
    pub recent: Vec<Recent>,
    pub draft: String,
    pub revision: i64,
    pub parent: Option<i64>,
    pub children: Vec<i64>,
    pub scores: Vec<Score>,
}
fn project(db: &Connection, chat: i64) -> rusqlite::Result<Option<i64>> {
    db.query_row("SELECT project FROM members WHERE chat=?", [chat], |r| {
        r.get(0)
    })
    .optional()
}
fn ensure_project(db: &Connection, chat: i64) -> Result<i64> {
    if let Some(id) = project(db, chat)? {
        return Ok(id);
    }
    let name: String = db.query_row("SELECT title FROM chats WHERE id=?", [chat], |r| r.get(0))?;
    db.execute("INSERT INTO projects(name) VALUES(?)", [name])?;
    let id = db.last_insert_rowid();
    db.execute(
        "INSERT INTO members(chat,project) VALUES(?,?)",
        params![chat, id],
    )?;
    Ok(id)
}
pub fn join(db: &mut Connection, chat: i64, other: i64) -> Result<()> {
    let tx = db.transaction()?;
    let target = ensure_project(&tx, other)?;
    if project(&tx, chat)?.is_some_and(|p| p != target) {
        return Err(invalid(
            "This chat already belongs to a project. Choose a chat outside a project.",
        ));
    }
    tx.execute(
        "INSERT OR IGNORE INTO members(chat,project) VALUES(?,?)",
        params![chat, target],
    )?;
    tx.commit()?;
    Ok(())
}
pub fn workspace(db: &Connection, chat: i64) -> Result<Workspace> {
    let project = project(db, chat)?;
    let projects = db
        .prepare("SELECT id,name FROM projects ORDER BY name,id")?
        .query_map([], |r| {
            Ok(Project {
                id: r.get(0)?,
                name: r.get(1)?,
            })
        })?
        .collect::<rusqlite::Result<_>>()?;
    let members = db
        .prepare("SELECT chat FROM members WHERE project=? ORDER BY chat")?
        .query_map([project], |r| r.get(0))?
        .collect::<rusqlite::Result<_>>()?;
    let pins=db.prepare("SELECT p.id,kind,text,source,t.chat FROM pins p LEFT JOIN turns t ON t.id=p.source WHERE project=? ORDER BY p.id")?.query_map([project],|r|Ok(Pin{id:r.get(0)?,kind:r.get(1)?,text:r.get(2)?,source:r.get(3)?,chat:r.get(4)?}))?.collect::<rusqlite::Result<_>>()?;
    let (draft, revision) = db
        .query_row(
            "SELECT draft,revision FROM projects WHERE id=?",
            [project],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?
        .unwrap_or_default();
    let recent=db.prepare("SELECT c.id,t.id,c.title,substr(t.prompt,1,260),CASE WHEN EXISTS(SELECT 1 FROM ballots b WHERE b.turn=t.id AND b.winner IS NULL) THEN '[Blind comparison awaiting vote]' ELSE substr(t.reply,1,1200) END,t.status FROM turns t JOIN chats c ON c.id=t.chat WHERE c.id=?1 OR c.id IN (SELECT chat FROM members WHERE project=?2) ORDER BY t.id DESC LIMIT 12")?.query_map(params![chat,project],|r|Ok(Recent{chat:r.get(0)?,turn:r.get(1)?,title:r.get(2)?,prompt:r.get(3)?,reply:r.get(4)?,status:r.get(5)?}))?.collect::<rusqlite::Result<_>>()?;
    let parent = db
        .query_row("SELECT parent FROM forks WHERE chat=?", [chat], |r| {
            r.get(0)
        })
        .optional()?
        .flatten();
    let children = db
        .prepare("SELECT chat FROM forks WHERE parent=? ORDER BY chat")?
        .query_map([chat], |r| r.get(0))?
        .collect::<rusqlite::Result<_>>()?;
    let scores=db.prepare("SELECT b.category,coalesce(m.model,json_extract(d.models,'$.agents['||e.seat||'].model')),sum(e.seat=b.winner),count(*) FROM ballots b JOIN debate_entries e ON b.turn=e.turn JOIN debates d ON d.turn=b.turn LEFT JOIN entry_models m ON m.entry=e.id WHERE b.winner IS NOT NULL AND e.status='complete' GROUP BY 1,2 ORDER BY 1,3 DESC,2")?.query_map([],|r|Ok(Score{category:r.get(0)?,model:r.get(1)?,wins:r.get(2)?,ballots:r.get(3)?}))?.collect::<rusqlite::Result<_>>()?;
    Ok(Workspace {
        project,
        projects,
        members,
        pins,
        recent,
        draft,
        revision,
        parent,
        children,
        scores,
    })
}
pub fn pin(
    db: &mut Connection,
    chat: i64,
    id: Option<i64>,
    kind: String,
    text: String,
    source: Option<i64>,
) -> Result<()> {
    let text = text.trim();
    if !["Decision", "Tried", "Open question", "Memory"].contains(&kind.as_str())
        || text.is_empty()
        || text.chars().count() > 2000
    {
        return Err(invalid("Use a project note of 1–2,000 characters."));
    }
    let tx = db.transaction()?;
    let p = ensure_project(&tx, chat)?;
    if let Some(source) = source {
        let belongs:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM turns t JOIN members m ON m.chat=t.chat WHERE t.id=? AND m.project=?)",params![source,p],|r|r.get(0))?;
        if !belongs {
            return Err(invalid("The source message is outside this project."));
        }
    }
    if let Some(id) = id {
        if tx.execute(
            "UPDATE pins SET kind=?,text=?,source=? WHERE id=? AND project=?",
            params![kind, text, source, id, p],
        )? != 1
        {
            return Err(invalid("This note no longer exists."));
        }
    } else {
        let count: i64 = tx.query_row("SELECT count(*) FROM pins WHERE project=?", [p], |r| {
            r.get(0)
        })?;
        if count >= 64 {
            return Err(invalid("A project can hold 64 notes."));
        }
        tx.execute(
            "INSERT INTO pins(project,kind,text,source) VALUES(?,?,?,?)",
            params![p, kind, text, source],
        )?;
    }
    tx.commit()?;
    Ok(())
}
pub fn draft(db: &mut Connection, chat: i64, text: String, revision: i64) -> Result<()> {
    if text.len() > 128_000 {
        return Err(invalid("Draft exceeds 128 KB."));
    }
    let tx = db.transaction()?;
    let p = ensure_project(&tx, chat)?;
    if tx.execute(
        "UPDATE projects SET draft=?,revision=revision+1 WHERE id=? AND revision=?",
        params![text, p, revision],
    )? != 1
    {
        return Err(invalid(
            "A newer draft exists. Reopen the workspace before saving.",
        ));
    }
    tx.commit()?;
    Ok(())
}
pub fn context(db: &Connection, chat: i64) -> Result<Vec<Value>> {
    let mut stmt = db.prepare(
        "SELECT kind,text FROM pins JOIN members USING(project) WHERE chat=? ORDER BY pins.id",
    )?;
    let notes = stmt
        .query_map([chat], |r| {
            Ok(format!(
                "{}: {}",
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?
            ))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    if notes.is_empty() {
        return Ok(Vec::new());
    }
    Ok(vec![
        json!({"role":"user","content":format!("My pinned project notes:\n{}",notes.join("\n"))}),
    ])
}
pub fn fork(db: &mut Connection, chat: i64, turn: i64) -> Result<i64> {
    let tx = db.transaction()?;
    let title:String=tx.query_row("SELECT title FROM chats JOIN turns ON turns.chat=chats.id WHERE turns.id=? AND chats.id=? AND turns.status<>'streaming'",params![turn,chat],|r|r.get(0))?;
    tx.execute(
        "INSERT INTO chats(title) VALUES(?)",
        [format!(
            "{} · branch",
            title.chars().take(88).collect::<String>()
        )],
    )?;
    let new = tx.last_insert_rowid();
    tx.execute(
        "INSERT INTO forks(chat,parent,fork_turn) VALUES(?,?,?)",
        params![new, chat, turn],
    )?;
    let p = ensure_project(&tx, chat)?;
    tx.execute("INSERT INTO members VALUES(?,?)", params![new, p])?;
    tx.execute(
        "INSERT INTO chat_goals SELECT ?,goal FROM chat_goals WHERE chat=?",
        params![new, chat],
    )?;
    let ids = tx
        .prepare("SELECT id FROM turns WHERE chat=? AND id<=? ORDER BY id")?
        .query_map(params![chat, turn], |r| r.get::<_, i64>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    for id in ids {
        tx.execute("INSERT INTO turns(chat,prompt,reply,model,status,error) SELECT ?,prompt,reply,model,status,error FROM turns WHERE id=?",params![new,id])?;
        let copy = tx.last_insert_rowid();
        let mut debate = super::debate::load(&tx, id)?;
        if let Some(d) = &debate
            && d.run.mode == AgentMode::Parallel
            && d.ballot.as_ref().is_some_and(|b| b.winner.is_none())
        {
            return Err(invalid("Vote on the blind comparison before branching."));
        }
        let snapshot = debate
            .take()
            .map(|d| serde_json::to_string(&d))
            .transpose()?;
        tx.execute(
            "INSERT INTO copied_turns VALUES(?,?)",
            params![copy, snapshot],
        )?;
        tx.execute("INSERT INTO files(turn,name,mime,data) SELECT ?,name,mime,data FROM files WHERE turn=? ORDER BY id",params![copy,id])?;
    }
    tx.commit()?;
    Ok(new)
}
