use crate::{
    error::{Result, invalid},
    services::{database::Database, studio},
};
use tauri::State;

#[tauri::command]
pub async fn workspace(db: State<'_, Database>, chat: i64) -> Result<studio::Workspace> {
    db.run(move |c| studio::workspace(c, chat)).await
}
#[tauri::command]
pub async fn join_project(db: State<'_, Database>, chat: i64, other: i64) -> Result<()> {
    db.run(move |c| studio::join(c, chat, other)).await
}
#[tauri::command]
pub async fn save_pin(
    db: State<'_, Database>,
    chat: i64,
    id: Option<i64>,
    kind: String,
    text: String,
    source: Option<i64>,
) -> Result<()> {
    db.run(move |c| studio::pin(c, chat, id, kind, text, source))
        .await
}
#[tauri::command]
pub async fn remove_pin(db: State<'_, Database>, chat: i64, id: i64) -> Result<()> {
    db.run(move |c| {
        if c.execute(
            "DELETE FROM pins WHERE id=? AND project=(SELECT project FROM members WHERE chat=?)",
            [id, chat],
        )? != 1
        {
            return Err(invalid("This note no longer exists."));
        }
        Ok(())
    })
    .await
}
#[tauri::command]
pub async fn save_draft(
    db: State<'_, Database>,
    chat: i64,
    text: String,
    revision: i64,
) -> Result<()> {
    db.run(move |c| studio::draft(c, chat, text, revision))
        .await
}
#[tauri::command]
pub async fn fork_chat(db: State<'_, Database>, chat: i64, turn: i64) -> Result<i64> {
    db.run(move |c| studio::fork(c, chat, turn)).await
}
#[tauri::command]
pub async fn cast_vote(db: State<'_, Database>, turn: i64, seat: usize) -> Result<()> {
    db.run(move |c| studio::vote(c, turn, seat)).await
}
