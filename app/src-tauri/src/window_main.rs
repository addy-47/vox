use crate::core::state::AppState;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

/// Returns the live "main" webview, rebuilding it if it was destroyed.
pub fn ensure_main_window(app: &AppHandle) -> Result<WebviewWindow, String> {
    if let Some(existing) = app.get_webview_window("main") {
        if let Err(e) = existing.unminimize() {
            log::debug!("[MainWindow] Failed to unminimize: {}", e);
        }
        if let Err(e) = existing.show() {
            log::debug!("[MainWindow] Failed to show: {}", e);
        }
        if let Err(e) = existing.set_focus() {
            log::debug!("[MainWindow] Failed to focus: {}", e);
        }
        return Ok(existing);
    }

    log::warn!("[MainWindow] 'main' webview absent — reconstructing fresh window.");
    let window = WebviewWindowBuilder::new(app, "main", WebviewUrl::App("/".into()))
        .title("Vox")
        .maximized(true)
        .visible(true)
        .center()
        .transparent(false)
        .decorations(false)
        .always_on_top(false)
        .resizable(true)
        .zoom_hotkeys_enabled(false)
        .build()
        .map_err(|e| format!("Failed to create main window: {}", e))?;

    let state = app.state::<Arc<AppState>>();
    state.main_window_destroyed.store(false, Ordering::Relaxed);
    crate::tray::refresh_tray_menu(app);

    if let Err(e) = window.show() {
        log::debug!("[MainWindow] Failed to show newly created window: {}", e);
    }
    if let Err(e) = window.set_focus() {
        log::debug!("[MainWindow] Failed to focus newly created window: {}", e);
    }
    Ok(window)
}
