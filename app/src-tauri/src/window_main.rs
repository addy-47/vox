//! ============================================================================
//! src/window_main.rs — Main window lifecycle (lazy recreate on demand)
//! ============================================================================
//!
//! The "main" webview is defined statically in `tauri.conf.json` and normally
//! persists for the app's lifetime (hidden, never closed, on `CloseRequested`).
//! If the renderer is destroyed (crash / DevTools `window.close()`), the window
//! handle disappears from the manager. `ensure_main_window` re-creates it so the
//! tray "Launch Vox" action always yields a fresh, live window instead of
//! silently re-showing a dead one.

use crate::core::state::AppState;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

/// Returns the live "main" webview, rebuilding it if it was destroyed.
///
/// Mirrors the static entry in `tauri.conf.json` (label `"main"`). If the window
/// still exists it is simply un-hidden and focused; otherwise a new one is built
/// and the `main_window_destroyed` crash flag is cleared.
pub fn ensure_main_window(app: &AppHandle) -> Result<WebviewWindow, String> {
    if let Some(existing) = app.get_webview_window("main") {
        let _ = existing.unminimize();
        let _ = existing.show();
        let _ = existing.set_focus();
        return Ok(existing);
    }

    // Window entirely gone (renderer crash / DevTools close). Rebuild it.
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

    // A fresh window exists — clear the crash marker and hide "Restart Vox".
    let state = app.state::<Arc<AppState>>();
    state.main_window_destroyed.store(false, Ordering::Relaxed);
    crate::tray::refresh_tray_menu(app);

    let _ = window.show();
    let _ = window.set_focus();
    Ok(window)
}
