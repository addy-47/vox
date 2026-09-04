use crate::core::error::VoxIpcError;
use crate::core::events::{emit_ipc, IpcEvent};
use crate::core::state::AppState;
use crate::tray::position_tray_window;
#[cfg(target_os = "linux")]
use crate::tray::setup_linux_virtual_layer;
use tauri::{AppHandle, Manager, State};

/// Toggles the tray window visibility and updates the menu checkmark state (internal native menu callback).
pub async fn toggle_tray_visibility_internal<R: tauri::Runtime>(app: AppHandle<R>) {
    let state: State<'_, std::sync::Arc<AppState>> = app.state();

    let (setup_completed, dictation_enabled, is_tray_mode) = match state.settings.read() {
        Ok(s) => (
            s.system.setup_completed,
            s.dictation.enabled,
            s.dictation.output_mode == crate::core::settings::DictationOutputMode::Tray,
        ),
        Err(e) => {
            log::warn!("[Tray] Failed to acquire settings read lock: {}", e);
            return;
        }
    };
    if !setup_completed {
        log::warn!("[Tray] Blocked toggle_tray_visibility: Setup not completed.");
        return;
    }
    if !dictation_enabled || !is_tray_mode {
        log::warn!("[Tray] Blocked toggle_tray_visibility: Dictation is disabled or not set to Tray output mode.");
        return;
    }

    let new_state = !state.hud_visible.load(std::sync::atomic::Ordering::Relaxed);
    state
        .hud_visible
        .store(new_state, std::sync::atomic::Ordering::Relaxed);

    if new_state {
        if let Ok(window) = crate::tray::ensure_tray_window(&app) {
            if let Err(e) = window.show() {
                log::warn!("[Tray] Failed to show tray window: {}", e);
            }
            position_tray_window(&window).await;
            if let Err(e) = emit_ipc(&app, IpcEvent::ToggleTray) {
                log::warn!("[Tray] Failed to emit toggle_tray: {}", e);
            }
        }
    } else if let Some(window) = app.get_webview_window("tray") {
        if let Err(e) = window.hide() {
            log::warn!("[Tray] Failed to hide tray window: {}", e);
        }
        if let Err(e) = emit_ipc(&app, IpcEvent::ToggleTray) {
            log::warn!("[Tray] Failed to emit toggle_tray: {}", e);
        }
    }

    let item_lock = state.hud_menu_item.lock();
    if let Some(item) = &*item_lock {
        if let Err(e) = item.set_checked(new_state) {
            log::warn!("[Tray] Failed to set menu item checked state: {}", e);
        }
    }
}

#[cfg(target_os = "linux")]
use gtk::prelude::*;

async fn cancel_active_dictation_turn(state: &AppState) {
    let owner: crate::core::state::InteractionOwner = state
        .owner
        .load(std::sync::atomic::Ordering::Relaxed)
        .into();

    if owner == crate::core::state::InteractionOwner::Dictation {
        let event_tx_opt = state.event_tx.lock().clone();
        if let Some(tx) = event_tx_opt {
            if let Err(e) = tx.send(crate::core::events::VoxEvent::PttCancel) {
                log::warn!("[Tray] Failed to send VoxEvent::PttCancel: {}", e);
            }
        }
    }
}

/// Hide the tray overlay window and cancel active dictation turns.
#[tauri::command]
pub async fn hide_tray_window<R: tauri::Runtime>(app: AppHandle<R>) {
    let state: State<'_, std::sync::Arc<AppState>> = app.state();
    let was_visible = state
        .hud_visible
        .swap(false, std::sync::atomic::Ordering::Relaxed);
    if was_visible {
        log::info!("[Tray] Ending Tray user session (Tray window hidden).");
    }

    cancel_active_dictation_turn(&state).await;

    if let Some(window) = app.get_webview_window("tray") {
        if let Err(e) = window.hide() {
            log::warn!("[Tray] Failed to hide tray window: {}", e);
        }
    }

    let item_lock = state.hud_menu_item.lock();
    if let Some(item) = &*item_lock {
        if let Err(e) = item.set_checked(false) {
            log::warn!("[Tray] Failed to uncheck menu item: {}", e);
        }
    }
}

/// Set whether a transparent overlay window (tray or toast) should ignore mouse cursor input events.
#[tauri::command]
pub fn set_window_click_through<R: tauri::Runtime>(
    app: AppHandle<R>,
    window: String,
    enabled: bool,
) -> Result<(), VoxIpcError> {
    let target_window = app
        .get_webview_window(&window)
        .ok_or_else(|| VoxIpcError::NotFound(format!("Window '{}' not found", window)))?;

    #[cfg(target_os = "linux")]
    {
        if enabled {
            if let Ok(gtk_window) = target_window.gtk_window() {
                gtk_window.input_shape_combine_region(None);
            }
        } else if window == "tray" {
            setup_linux_virtual_layer(&app, "tray");
        } else if window == "toast" {
            crate::toast::setup_linux_toast_layer(&app, "toast");
        }
    }
    #[cfg(not(target_os = "linux"))]
    {
        if let Err(e) = target_window.set_ignore_cursor_events(enabled) {
            log::warn!(
                "[Tray] Failed to set ignore cursor events on '{}': {}",
                window,
                e
            );
        }
    }
    Ok(())
}

#[tauri::command]
pub async fn show_main_window(app: AppHandle) -> Result<(), VoxIpcError> {
    crate::window_main::ensure_main_window(&app).map_err(VoxIpcError::Internal)?;
    Ok(())
}
