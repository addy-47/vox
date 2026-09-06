use std::sync::{atomic::Ordering, Arc};

use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

use crate::{core::state::AppState, tray::refresh_tray_menu};

/// Returns the live "main" webview, rebuilding it if it was destroyed.
pub fn ensure_main_window(app: &AppHandle) -> Result<WebviewWindow, String> {
    if let Some(existing) = app.get_webview_window("main") {
        // [DIAG:LAUNCH-WINDOW-SEQ] Snapshot pre-show window state for jitter RCA.
        let pre_scale = existing.scale_factor().unwrap_or(1.0);
        let pre_outer_pos = existing.outer_position().ok();
        let pre_outer_size = existing.outer_size().ok();
        let pre_inner_size = existing.inner_size().ok();
        let pre_is_maximized = existing.is_maximized().unwrap_or(false);
        let pre_is_minimized = existing.is_minimized().unwrap_or(false);
        let pre_is_visible = existing.is_visible().unwrap_or(false);
        log::info!(
            "[DIAG:LAUNCH-WINDOW-SEQ] ensure_main_window(existing) PRE  scale={} maximized={} minimized={} visible={} inner={:?} outer={:?} pos={:?}",
            pre_scale, pre_is_maximized, pre_is_minimized, pre_is_visible,
            pre_inner_size, pre_outer_size, pre_outer_pos
        );

        if let Err(e) = existing.unminimize() {
            log::debug!("[MainWindow] Failed to unminimize: {}", e);
        }
        if let Err(e) = existing.show() {
            log::debug!("[MainWindow] Failed to show: {}", e);
        }
        if let Err(e) = existing.set_focus() {
            log::debug!("[MainWindow] Failed to focus: {}", e);
        }

        let post_is_maximized = existing.is_maximized().unwrap_or(false);
        let post_is_minimized = existing.is_minimized().unwrap_or(false);
        let post_is_visible = existing.is_visible().unwrap_or(false);
        let post_inner_size = existing.inner_size().ok();
        log::info!(
            "[DIAG:LAUNCH-WINDOW-SEQ] ensure_main_window(existing) POST maximized={} minimized={} visible={} inner={:?}",
            post_is_maximized, post_is_minimized, post_is_visible, post_inner_size
        );
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
    refresh_tray_menu(app);

    if let Err(e) = window.show() {
        log::debug!("[MainWindow] Failed to show newly created window: {}", e);
    }
    if let Err(e) = window.set_focus() {
        log::debug!("[MainWindow] Failed to focus newly created window: {}", e);
    }
    Ok(window)
}
