use crate::{
    error::{Error, Result, invalid},
    model::{Attachment, Chat, StreamEvent, Turn, UsageReport},
    services::{
        api,
        database::{self, Database},
    },
};
use rusqlite::params;
use std::{collections::HashMap, sync::Mutex};
use tauri::{State, ipc::Channel};
use tokio_util::sync::CancellationToken;

#[derive(Default)]
pub struct Runs(Mutex<HashMap<i64, CancellationToken>>);
impl Runs {
    fn entries(&self) -> std::sync::MutexGuard<'_, HashMap<i64, CancellationToken>> {
        self.0.lock().unwrap_or_else(|e| e.into_inner())
    }
}
#[tauri::command]
pub async fn list_chats(db: State<'_, Database>) -> Result<Vec<Chat>> {
    db.run(|c| database::chats(c)).await
}
#[tauri::command]
pub async fn create_chat(db: State<'_, Database>) -> Result<Chat> {
    db.run(|c| {
        Ok(c.query_row(
            "INSERT INTO chats(title) VALUES('New chat') RETURNING id,title,touched",
            [],
            |r| {
                Ok(Chat {
                    id: r.get(0)?,
                    title: r.get(1)?,
                    touched: r.get(2)?,
                    goal: None,
                })
            },
        )?)
    })
    .await
}
#[tauri::command]
pub async fn set_chat_goal(
    db: State<'_, Database>,
    chat: Option<i64>,
    goal: Option<String>,
) -> Result<Chat> {
    db.run(move |c| database::set_goal(c, chat, goal)).await
}
#[tauri::command]
pub async fn get_usage(db: State<'_, Database>, chat: Option<i64>) -> Result<UsageReport> {
    db.run(move |c| database::usage_report(c, chat)).await
}
#[tauri::command]
pub async fn rename_chat(db: State<'_, Database>, chat: i64, title: String) -> Result<()> {
    let title = title.trim().to_owned();
    if title.is_empty() || title.chars().count() > 100 {
        return Err(invalid("Use a title of 1–100 characters."));
    }
    db.run(move |c| {
        if c.execute("UPDATE chats SET title=? WHERE id=?", params![title, chat])? == 0 {
            return Err(invalid("Conversation no longer exists."));
        }
        Ok(())
    })
    .await
}
#[tauri::command]
pub async fn delete_chat(db: State<'_, Database>, chat: i64) -> Result<()> {
    db.run(move |c| {
        if c.execute(
            "DELETE FROM chats WHERE id=?1 AND NOT EXISTS
            (SELECT 1 FROM turns WHERE chat=?1 AND status='streaming')",
            [chat],
        )? == 0
        {
            return Err(invalid(
                "Stop the active reply before deleting this conversation.",
            ));
        }
        Ok(())
    })
    .await
}
#[tauri::command]
pub async fn list_turns(db: State<'_, Database>, chat: i64) -> Result<Vec<Turn>> {
    db.run(move |c| database::turns(c, chat)).await
}
#[tauri::command]
pub async fn read_attachment(
    db: State<'_, Database>,
    chat: i64,
    turn: i64,
    position: u32,
) -> Result<Attachment> {
    db.run(move |c| {
        Ok(c.query_row(
            "SELECT f.name,f.mime,f.data FROM files f JOIN turns t ON t.id=f.turn
         WHERE t.chat=? AND t.id=? ORDER BY f.id LIMIT 1 OFFSET ?",
            params![chat, turn, position],
            |r| {
                Ok(Attachment {
                    name: r.get(0)?,
                    mime: r.get(1)?,
                    data: r.get(2)?,
                })
            },
        )?)
    })
    .await
}
#[tauri::command]
pub fn stop_reply(runs: State<'_, Runs>, id: i64) {
    if let Some(token) = runs.entries().get(&id) {
        token.cancel();
    }
}
#[tauri::command]
pub async fn send_message(
    auth: State<'_, super::auth::Auth>,
    db: State<'_, Database>,
    client: State<'_, reqwest::Client>,
    runs: State<'_, Runs>,
    chat: i64,
    prompt: String,
    files: Vec<Attachment>,
    channel: Channel<StreamEvent>,
    debate: Option<crate::model::DebateOptions>,
) -> Result<()> {
    let (key, model, account) = super::auth::authorization(&db, &auth, debate.as_ref()).await?;
    let agents = debate.map(|d| d.run);
    let roster = agents.clone();
    let request = db
        .run(move |c| {
            database::begin_authorized_turn(c, chat, prompt, files, model, roster, Some(account))
        })
        .await?;
    let id = request.id;
    let token = CancellationToken::new();
    runs.entries().insert(id, token.clone());
    let mut reply = String::new();
    let result = async {
        channel.send(StreamEvent::Started {
            id,
            model: request.model.clone(),
        })?;
        if let Some(agents) = agents {
            reply = crate::services::debate::run(
                &db,
                &client,
                api::RESPONSES,
                &key,
                chat,
                request,
                &agents,
                &token,
                &channel,
            )
            .await?;
            return Ok(());
        }
        let post = client.post(api::RESPONSES).bearer_auth(key);
        api::stream(post, &db, request, &token, &channel, &mut reply).await
    }
    .await;
    runs.entries().remove(&id);
    let (status, error) = match &result {
        Ok(()) => ("complete", None),
        Err(Error::Cancelled) => ("stopped", None),
        Err(e) => ("failed", Some(e.to_string())),
    };
    db.run(move |c| {
        c.execute(
            "UPDATE turns SET reply=?,status=?,error=? WHERE id=?",
            params![reply, status, error, id],
        )?;
        Ok(())
    })
    .await?;
    // Once a turn exists its persisted status is the authoritative UI result.
    Ok(())
}
