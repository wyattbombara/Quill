use crate::{
    error::{Result, invalid},
    model::Preferences,
    services::{
        credentials,
        database::{Database, read_preferences},
    },
};
use tauri::{AppHandle, State};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut};
use tokio::sync::Mutex;

pub struct Hotkey(pub Mutex<Option<Shortcut>>);
#[tauri::command]
pub async fn get_preferences(db: State<'_, Database>) -> Result<Preferences> {
    db.run(|c| read_preferences(c)).await
}
#[tauri::command]
pub async fn shortcut_active(key: State<'_, Hotkey>) -> Result<bool> {
    Ok(key.0.lock().await.is_some())
}
#[tauri::command]
pub async fn save_preferences(
    app: AppHandle,
    db: State<'_, Database>,
    key: State<'_, Hotkey>,
    preferences: Preferences,
) -> Result<()> {
    let mut current = key.0.lock().await;
    let options = &preferences.workspace;
    if !(2..=8).contains(&options.roundtable_budget)
        || !(12..=24).contains(&options.reply_size)
        || !(10..=22).contains(&options.code_size)
    {
        return Err(invalid(
            "Choose a supported font size and a Roundtable budget of 2–8 requests.",
        ));
    }
    let model = preferences.model.trim();
    if model.is_empty() || model.len() > 128 || model.contains(char::is_whitespace) {
        return Err(invalid("Enter a valid model ID."));
    }
    let shortcut: Shortcut = preferences
        .shortcut
        .parse()
        .map_err(|_| invalid("Invalid shortcut. Try Ctrl+Alt+Space."))?;
    let changed = current.as_ref() != Some(&shortcut);
    let manager = app.global_shortcut();
    if changed {
        manager
            .register(shortcut)
            .map_err(|e| invalid(format!("Shortcut unavailable: {e}")))?;
    }
    let value = serde_json::to_string(&preferences)?;
    let written = db
        .run(move |c| {
            c.execute(
                "INSERT INTO preferences(id,value) VALUES(1,?)
        ON CONFLICT(id) DO UPDATE SET value=excluded.value",
                [value],
            )?;
            Ok(())
        })
        .await;
    if let Err(e) = written {
        if changed {
            let _ = manager.unregister(shortcut);
        }
        return Err(e);
    }
    use tauri::Manager;
    app.state::<super::super::services::desktop::CloseBehavior>()
        .0
        .store(options.quit_on_close, std::sync::atomic::Ordering::Release);
    app.set_theme(preferences.theme.native());
    if changed && let Some(old) = current.replace(shortcut) {
        manager.unregister(old).map_err(|e| {
            invalid(format!(
                "New shortcut saved. Restart to release old shortcut: {e}"
            ))
        })?;
    }
    Ok(())
}
#[tauri::command]
pub async fn key_present() -> Result<bool> {
    Ok(tokio::task::spawn_blocking(credentials::read)
        .await??
        .is_some())
}
#[tauri::command]
pub async fn save_key(key: String) -> Result<()> {
    tokio::task::spawn_blocking(move || credentials::write(key)).await?
}
#[tauri::command]
pub async fn remove_key() -> Result<()> {
    tokio::task::spawn_blocking(credentials::remove).await?
}
