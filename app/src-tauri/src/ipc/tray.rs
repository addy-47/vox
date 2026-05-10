use tauri::{AppHandle, Manager, State, WebviewWindow, Emitter};
use crate::core::state::AppState;
use crate::core::settings::InteractionMode;
use crate::tray::{setup_linux_virtual_layer, position_tray_window};

/// Toggles the tray window visibility and updates the menu checkmark state.
pub async fn toggle_hud_visibility(app: AppHandle) {
    let state: State<'_, AppState> = app.state();
    let mut hud_lock = state.hud_visible.lock().await;
    let new_state = !*hud_lock;
    *hud_lock = new_state;

    if let Some(window) = app.get_webview_window("tray") {
        if new_state {
            position_tray_window(&window).await;
        } else {
            let _ = window.hide();
        }
        let _ = app.emit("toggle_hud", ());
    }

    let item_lock = state.hud_menu_item.lock().await;
    if let Some(item) = &*item_lock {
        let _ = item.set_checked(new_state);
    }
}
#[cfg(target_os = "linux")]
use gtk::prelude::*;

#[tauri::command]
pub fn hide_tray_window(app: AppHandle) {
    if let Some(window) = app.get_webview_window("tray") {
        let _ = window.hide();
    }
}

#[tauri::command]
pub async fn sync_hud_visibility(app: AppHandle, visible: bool) {
    let state: State<'_, AppState> = app.state();
    let mut hud_lock = state.hud_visible.lock().await;
    *hud_lock = visible;

    let item_lock = state.hud_menu_item.lock().await;
    if let Some(item) = &*item_lock {
        let _ = item.set_checked(visible);
    }
}

#[tauri::command]
pub fn set_hud_ignore_cursor(window: WebviewWindow, ignore: bool) {
    #[cfg(target_os = "linux")]
    {
        if ignore {
            if let Ok(gtk_window) = window.gtk_window() {
                gtk_window.input_shape_combine_region(None);
            }
        } else {
            setup_linux_virtual_layer(window.app_handle(), "tray");
        }
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = window.set_ignore_cursor_events(ignore);
    }
}

#[tauri::command]
pub async fn update_interaction_mode(app: AppHandle, target: String, mode: String) -> Result<(), String> {
    let state: State<'_, AppState> = app.state();
    let mut settings = state.settings.lock().await;
    
    let new_mode = match mode.to_uppercase().as_str() {
        "PASSIVE" => InteractionMode::Passive,
        "PTT" => InteractionMode::PTT,
        _ => return Err(format!("Invalid interaction mode: {}", mode)),
    };

    match target.to_lowercase().as_str() {
        "main" => {
            settings.main_app_mode = new_mode.clone();
        }
        "tray" => {
            settings.tray_mode = new_mode.clone();
        }
        _ => return Err(format!("Invalid target window: {}", target)),
    }
    
    let _ = settings.save(&state.config_dir);
    
    let event_name = format!("mode_changed_{}", target.to_lowercase());
    let _ = app.emit(&event_name, mode.clone());
    let _ = app.emit("mode_changed", mode);
    
    Ok(())
}

#[tauri::command]
pub async fn show_main_window(app: AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.set_focus();
    }
    Ok(())
}
