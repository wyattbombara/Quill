use crate::error::{Result, invalid};
use serde::Serialize;
use tauri::AppHandle;
use tauri_plugin_updater::UpdaterExt;

#[derive(Serialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum UpdateStatus {
    Unconfigured,
    Current,
    Available { version: String },
}

#[tauri::command]
pub async fn check_update(app: AppHandle) -> Result<UpdateStatus> {
    if !app.config().plugins.0.get("updater").is_some_and(|v| {
        v["pubkey"].as_str().is_some_and(|s| !s.is_empty())
            && v["endpoints"].as_array().is_some_and(|a| !a.is_empty())
    }) {
        return Ok(UpdateStatus::Unconfigured);
    }
    let updater = app.updater().map_err(|e| invalid(e.to_string()))?;
    Ok(
        match updater.check().await.map_err(|e| invalid(e.to_string()))? {
            Some(update) => UpdateStatus::Available {
                version: update.version,
            },
            None => UpdateStatus::Current,
        },
    )
}
#[tauri::command]
pub async fn install_update(app: AppHandle, version: String) -> Result<()> {
    let updater = app.updater().map_err(|e| invalid(e.to_string()))?;
    let update = updater
        .check()
        .await
        .map_err(|e| invalid(e.to_string()))?
        .ok_or_else(|| invalid("The update is no longer available."))?;
    if update.version != version {
        return Err(invalid("The release changed. Check for updates again."));
    }
    update
        .download_and_install(|_, _| {}, || {})
        .await
        .map_err(|e| invalid(e.to_string()))?;
    app.restart()
}
