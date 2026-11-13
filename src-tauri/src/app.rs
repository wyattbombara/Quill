use crate::commands::{auth::*, chat::*, settings::*};
use crate::services::{database::Database, desktop, updates::*};
use std::time::Duration;
use tauri::{Manager, WindowEvent};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

pub fn run() {
    let context = tauri::generate_context!();
    let builder = tauri::Builder::default();
    #[cfg(target_os = "windows")]
    let (context, builder) = {
        let mut context = context;
        let main = context
            .config_mut()
            .app
            .windows
            .iter_mut()
            .find(|win| win.label == "main")
            .expect("main window configuration");
        main.title = "Quill".into();
        main.decorations = false;
        (
            context,
            builder.append_invoke_initialization_script(";window.__QUILL_WINDOWS_FRAME__ = true;"),
        )
    };
    builder
        .plugin(tauri_plugin_single_instance::init(|app, _, _| {
            desktop::show(app)
        }))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, _, event| {
                    if event.state() == ShortcutState::Pressed {
                        desktop::show(app);
                    }
                })
                .build(),
        )
        .setup(|app| {
            let dir = app.path().app_local_data_dir()?;
            std::fs::create_dir_all(&dir)?;
            let opened = Database::from_keychain(dir.join("quill.sqlite3")).and_then(|db| {
                let settings = db.initialize()?;
                Ok((db, settings))
            });
            let (db, settings) = match opened {
                Ok(opened) => opened,
                Err(error) => {
                    use tauri_plugin_dialog::DialogExt;
                    if let Some(window) = app.get_webview_window("main") {
                        let _ = window.hide();
                    }
                    let handle = app.handle().clone();
                    app.dialog()
                        .message(format!(
                            "Quill could not open your local history.\n\n{error}"
                        ))
                        .title("Quill storage")
                        .kind(tauri_plugin_dialog::MessageDialogKind::Error)
                        .show(move |_| handle.exit(1));
                    return Ok(());
                }
            };
            app.manage(desktop::CloseBehavior(
                settings.workspace.quit_on_close.into(),
            ));
            app.set_theme(settings.theme.native());
            let key = settings
                .shortcut
                .parse::<Shortcut>()
                .ok()
                .filter(|s| app.global_shortcut().register(*s).is_ok());
            app.manage(Hotkey(tokio::sync::Mutex::new(key)));
            app.manage(db);
            app.manage(Runs::default());
            app.manage(Auth::default());
            app.manage(
                reqwest::Client::builder()
                    .redirect(reqwest::redirect::Policy::none())
                    .connect_timeout(Duration::from_secs(20))
                    .read_timeout(Duration::from_secs(120))
                    .build()?,
            );
            desktop::tray(app.handle())?;
            Ok(())
        })
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                let Some(behavior) = window.try_state::<desktop::CloseBehavior>() else {
                    return;
                };
                if behavior.0.load(std::sync::atomic::Ordering::Acquire) {
                    window.app_handle().exit(0);
                } else if window.hide().is_ok() {
                    api.prevent_close();
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            list_chats,
            auth_status,
            chatgpt_login,
            cancel_login,
            chatgpt_logout,
            use_api_key,
            plan_models,
            select_plan_model,
            welcome_seen,
            set_chat_goal,
            get_usage,
            create_chat,
            rename_chat,
            delete_chat,
            list_turns,
            read_attachment,
            debate_models,
            send_message,
            stop_reply,
            get_preferences,
            save_preferences,
            shortcut_active,
            key_present,
            save_key,
            remove_key,
            desktop::open_link,
            desktop::show_chat_window,
            check_update,
            install_update
        ])
        .run(context)
        .expect("Quill could not start");
}
