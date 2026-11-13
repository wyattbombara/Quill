use crate::{
    error::{Result, invalid},
    model::{Attachment, Chat, Preferences, Turn, Usage, UsageReport},
};
use base64::{Engine, engine::general_purpose::STANDARD};
use rusqlite::{Connection, OptionalExtension, params};
use serde_json::{Value, json};
pub use super::encrypted_store::Database;

pub fn read_preferences(db: &Connection) -> Result<Preferences> {
    let value: Option<String> = db
        .query_row("SELECT value FROM preferences", [], |r| r.get(0))
        .optional()?;
    Ok(match value {
        Some(v) => serde_json::from_str(&v)?,
        None => Preferences::default(),
    })
}
pub fn chats(db: &Connection) -> Result<Vec<Chat>> {
    let mut q = db.prepare("SELECT id,title,goal,touched FROM chats LEFT JOIN chat_goals ON chat=id ORDER BY touched DESC,id DESC")?;
    Ok(q.query_map([], |r| {
        Ok(Chat {
            id: r.get(0)?,
            title: r.get(1)?,
            goal: r.get(2)?,
            touched: r.get(3)?,
        })
    })?
    .collect::<rusqlite::Result<_>>()?)
}
pub fn set_goal(db: &mut Connection, chat: Option<i64>, goal: Option<String>) -> Result<Chat> {
    if let Some(text) = &goal {
        if text.trim().is_empty() || text.chars().count() > 2000 {
            return Err(invalid("Use a goal of 1–2,000 characters."));
        }
    }
    let tx = db.transaction()?;
    let id = match chat {
        Some(id) => id,
        None if goal.is_some() => {
            tx.execute("INSERT INTO chats(title) VALUES('New chat')", [])?;
            tx.last_insert_rowid()
        }
        None => return Err(invalid("No conversation selected.")),
    };
    let (title, touched) =
        tx.query_row("SELECT title,touched FROM chats WHERE id=?", [id], |row| {
            Ok((row.get(0)?, row.get(1)?))
        })?;
    if let Some(text) = &goal {
        tx.execute("INSERT INTO chat_goals(chat,goal) VALUES(?,?) ON CONFLICT(chat) DO UPDATE SET goal=excluded.goal", params![id,text])?;
    } else {
        tx.execute("DELETE FROM chat_goals WHERE chat=?", [id])?;
    }
    tx.commit()?;
    Ok(Chat {
        id,
        title,
        goal,
        touched,
    })
}

pub fn usage_report(db: &mut Connection, chat: Option<i64>) -> Result<UsageReport> {
    fn counts(row: &rusqlite::Row<'_>) -> rusqlite::Result<Usage> {
        Ok(Usage {
            requests: row.get(0)?,
            reported: row.get(1)?,
            input_tokens: row.get(2)?,
            output_tokens: row.get(3)?,
        })
    }
    const TOTALS: &str = "SELECT count(*),count(input_tokens),coalesce(sum(input_tokens),0),coalesce(sum(output_tokens),0) FROM (SELECT chat,input_tokens,output_tokens FROM turn_usage UNION ALL SELECT chat,input_tokens,output_tokens FROM debate_usage)";
    let tx = db.transaction()?;
    let device = tx.query_row(TOTALS, [], counts)?;
    let chat = tx.query_row(&format!("{TOTALS} WHERE chat=?"), [chat], counts)?;
    let today = tx.query_row("SELECT count(*),count(input_tokens),coalesce(sum(input_tokens),0),coalesce(sum(output_tokens),0)
        FROM (SELECT t.input_tokens,t.output_tokens FROM request_times d JOIN turn_usage t ON d.agent=0 AND t.turn=d.request WHERE d.started>=unixepoch('now','localtime','start of day','utc')
        UNION ALL SELECT t.input_tokens,t.output_tokens FROM request_times d JOIN debate_usage t ON d.agent=1 AND t.id=d.request WHERE d.started>=unixepoch('now','localtime','start of day','utc'))", [], counts)?;
    tx.commit()?;
    Ok(UsageReport {
        chat,
        device,
        today,
    })
}
pub fn turns(db: &Connection, chat: i64) -> Result<Vec<Turn>> {
    let mut q = db
        .prepare("SELECT id,prompt,reply,model,status,error FROM turns WHERE chat=? ORDER BY id")?;
    let mut files = db.prepare("SELECT name FROM files WHERE turn=? ORDER BY id")?;
    let rows = q.query_map([chat], |r| {
        let id: i64 = r.get(0)?;
        Ok(Turn {
            debate: super::debate::load(db, id)?,
            id,
            prompt: r.get(1)?,
            reply: r.get(2)?,
            model: r.get(3)?,
            status: r.get(4)?,
            error: r.get(5)?,
            files: files
                .query_map([id], |r| r.get(0))?
                .collect::<rusqlite::Result<_>>()?,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

pub const MAX_FILE_BYTES: usize = 8 * 1024 * 1024;
pub const MAX_TURN_BYTES: usize = 16 * 1024 * 1024;
fn validate_files(files: &[Attachment]) -> Result<()> {
    if files.len() > 6 {
        return Err(invalid("Choose up to six attachments per message."));
    }
    let mut total = 0;
    for f in files {
        let allowed = matches!(
            f.mime.as_str(),
            "application/pdf"
                | "image/png"
                | "image/jpeg"
                | "image/webp"
                | "text/plain"
                | "text/markdown"
                | "text/csv"
                | "application/json"
        );
        if !allowed
            || f.name.is_empty()
            || f.name.len() > 255
            || f.name.contains(['/', '\\'])
            || f.data.len() > MAX_FILE_BYTES.div_ceil(3) * 4
        {
            return Err(invalid("Unsupported attachment or file exceeds 8 MiB."));
        }
        let bytes = STANDARD
            .decode(&f.data)
            .map_err(|_| invalid("Invalid attachment encoding."))?;
        total += bytes.len();
        if bytes.is_empty() || bytes.len() > MAX_FILE_BYTES || total > MAX_TURN_BYTES {
            return Err(invalid(
                "Choose nonempty files up to 8 MiB each and 16 MiB total.",
            ));
        }
    }
    Ok(())
}

pub struct Request {
    pub id: i64,
    pub model: String,
    pub input: Vec<Value>,
}
pub fn begin(
    db: &mut Connection,
    chat: i64,
    prompt: String,
    files: Vec<Attachment>,
) -> Result<Request> {
    begin_with_model(db, chat, prompt, files, None)
}
pub fn begin_with_model(
    db: &mut Connection,
    chat: i64,
    prompt: String,
    files: Vec<Attachment>,
    model: Option<String>,
) -> Result<Request> {
    begin_turn(db, chat, prompt, files, model, None)
}

pub fn begin_turn(
    db: &mut Connection,
    chat: i64,
    prompt: String,
    files: Vec<Attachment>,
    model: Option<String>,
    debate: Option<crate::model::AgentRun>,
) -> Result<Request> {
    begin_authorized_turn(db, chat, prompt, files, model, debate, None)
}
pub fn begin_authorized_turn(
    db: &mut Connection,
    chat: i64,
    prompt: String,
    files: Vec<Attachment>,
    model: Option<String>,
    debate: Option<crate::model::AgentRun>,
    expected_account: Option<Option<String>>,
) -> Result<Request> {
    if prompt.trim().is_empty() && files.is_empty() {
        return Err(invalid("Enter a message or attach a file."));
    }
    if prompt.len() > 512_000 {
        return Err(invalid("Message exceeds 512 KB."));
    }
    validate_files(&files)?;
    let tx = db.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
    if let Some(expected) = expected_account {
        let active: Option<String> = tx
            .query_row("SELECT active FROM oauth_host", [], |r| r.get(0))
            .optional()?
            .flatten();
        if expected != active {
            return Err(invalid(
                "The billing account changed. Try again with the current account.",
            ));
        }
    }
    if let Some(run) = &debate {
        super::debate::validate_agents(&run.agents)?;
        let limit = match run.mode {
            crate::model::AgentMode::Parallel => super::accounts::parallel_capacity(&tx)?.seats,
            crate::model::AgentMode::Roundtable => read_preferences(&tx)?
                .workspace
                .roundtable_budget
                .clamp(2, 8),
        };
        if run.agents.len() > limit {
            return Err(invalid(format!(
                "This run allows up to {limit} agents. Adjust the lineup before starting."
            )));
        }
    }
    let model = match model {
        Some(model) => model,
        None => read_preferences(&tx)?.model,
    };
    let previous: i64 = tx.query_row("SELECT count(*) FROM turns WHERE chat=?", [chat], |r| {
        r.get(0)
    })?;
    tx.execute(
        "INSERT INTO turns(chat,prompt,model,status) VALUES(?,?,?,'streaming')",
        params![chat, prompt, model],
    )?;
    let id = tx.last_insert_rowid();
    if let Some(run) = debate {
        tx.execute(
            "INSERT INTO debates(turn,models) VALUES(?,?)",
            params![id, serde_json::to_string(&run)?],
        )?;
    } else {
        tx.execute(
            "INSERT INTO turn_usage(turn,chat) VALUES(?,?)",
            params![id, chat],
        )?;
    }
    for f in &files {
        tx.execute(
            "INSERT INTO files(turn,name,mime,data) VALUES(?,?,?,?)",
            params![id, f.name, f.mime, f.data],
        )?;
    }
    let title: String = prompt
        .trim()
        .lines()
        .next()
        .unwrap_or("")
        .chars()
        .take(55)
        .collect();
    let title = if title.is_empty() {
        files.first().map(|f| f.name.as_str()).unwrap_or("New chat")
    } else {
        &title
    };
    tx.execute("UPDATE chats SET touched=unixepoch(),title=CASE WHEN ?=0 AND title='New chat' THEN ? ELSE title END WHERE id=?",
        params![previous,title,chat])?;
    // Limit the encoded file payload before reading historical blobs into memory.
    let size: i64 = tx.query_row(
        "SELECT coalesce(sum(length(f.data)),0) FROM files f JOIN turns t ON t.id=f.turn
        WHERE t.chat=? AND (t.status='complete' OR t.id=?)",
        params![chat, id],
        |r| r.get(0),
    )?;
    if size > 48 * 1024 * 1024 {
        return Err(invalid(
            "Attachment history is full. Start a new chat to send more files.",
        ));
    }
    let mut input = Vec::new();
    let goal: Option<String> = tx
        .query_row("SELECT goal FROM chat_goals WHERE chat=?", [chat], |r| {
            r.get(0)
        })
        .optional()?;
    {
        let mut q = tx.prepare("SELECT id,prompt,reply FROM turns WHERE chat=? AND (status='complete' OR id=?) ORDER BY id")?;
        let mut rows = q.query(params![chat, id])?;
        let mut fq = tx.prepare("SELECT name,mime,data FROM files WHERE turn=? ORDER BY id")?;
        while let Some(row) = rows.next()? {
            let turn: i64 = row.get(0)?;
            if turn == id {
                if let Some(goal) = &goal {
                    input.push(json!({"role":"user", "content":format!("My goal for this conversation: {goal}")}));
                }
            }
            let text: String = row.get(1)?;
            let mut parts = Vec::new();
            if !text.is_empty() {
                parts.push(json!({"type":"input_text","text":text}));
            }
            let mut fr = fq.query([turn])?;
            while let Some(f) = fr.next()? {
                let name: String = f.get(0)?;
                let mime: String = f.get(1)?;
                let data: String = f.get(2)?;
                let url = format!("data:{mime};base64,{data}");
                parts.push(if mime.starts_with("image/") {
                    json!({"type":"input_image","image_url":url})
                } else {
                    json!({"type":"input_file","filename":name,"file_data":url})
                });
            }
            input.push(json!({"role":"user","content":parts}));
            if turn != id {
                input.push(json!({"role":"assistant","content":row.get::<_,String>(2)?}));
            }
        }
    }
    tx.commit()?;
    Ok(Request { id, model, input })
}
