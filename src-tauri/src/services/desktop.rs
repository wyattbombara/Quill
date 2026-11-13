use tauri::{
    AppHandle, Emitter, Manager,
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
};

pub fn show(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
    }
}
#[tauri::command]
pub fn open_link(app: AppHandle, url: String) -> crate::error::Result<()> {
    use tauri_plugin_opener::OpenerExt;
    let url = reqwest::Url::parse(&url).map_err(|_| crate::error::invalid("Invalid URL."))?;
    if !matches!(url.scheme(), "https" | "http") || url.host().is_none() {
        return Err(crate::error::invalid("Only web links can be opened."));
    }
    app.opener()
        .open_url(url.as_str(), None::<&str>)
        .map_err(|e| crate::error::invalid(e.to_string()))
}
pub fn tray(app: &AppHandle) -> tauri::Result<()> {
    let open = MenuItem::with_id(app, "open", "Open Quill", true, None::<&str>)?;
    let new = MenuItem::with_id(app, "new", "New chat", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit Quill", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&open, &new, &quit])?;
    let mut tray = TrayIconBuilder::new().tooltip("Quill").menu(&menu);
    if let Some(icon) = app.default_window_icon() {
        tray = tray.icon(icon.clone());
    }
    tray.on_menu_event(|app, event| match event.id.as_ref() {
        "open" => show(app),
        "new" => {
            show(app);
            let _ = app.emit_to("main", "new-chat", ());
        }
        "quit" => app.exit(0),
        _ => {}
    })
    .build(app)?;
    Ok(())
}

pub struct CloseBehavior(pub std::sync::atomic::AtomicBool);

#[tauri::command]
pub fn show_chat_window(app: AppHandle) {
    show(&app);
}
