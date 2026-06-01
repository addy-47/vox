use tauri::{AppHandle, Manager, State, WebviewWindow, Emitter};
use crate::core::state::AppState;
use crate::core::settings::InteractionMode;
#[cfg(target_os = "linux")]
use crate::tray::setup_linux_virtual_layer;
use crate::tray::position_tray_window;


/// Toggles the tray window visibility and updates the menu checkmark state.
pub async fn toggle_hud_visibility(app: AppHandle) {
    let state: State<'_, std::sync::Arc<AppState>> = app.state();
    
    // Check if setup is completed and tray is even enabled
    let (tray_enabled, setup_completed) = {
        let s = state.settings.read().unwrap();
        (s.ui.tray_enabled, s.setup.completed)
    };
    if !setup_completed || !tray_enabled {
        log::warn!("[Tray] Blocked toggle_hud_visibility: Setup not completed or Tray HUD is disabled.");
        return;
    }

    let mut hud_lock = state.hud_visible.lock().await;
    let new_state = !*hud_lock;
    *hud_lock = new_state;

    if let Some(window) = app.get_webview_window("tray") {
        if new_state {
            let _ = window.show();
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
pub async fn hide_tray_window(app: AppHandle) {
    let state: State<'_, std::sync::Arc<AppState>> = app.state();
    let mut hud_lock = state.hud_visible.lock().await;
    if *hud_lock {
        *hud_lock = false;
        log::info!("[Tray] Ending Tray user session (Tray window hidden).");
    }
    if let Some(window) = app.get_webview_window("tray") {
        let _ = window.hide();
    }
}

#[tauri::command]
pub async fn sync_hud_visibility(app: AppHandle, visible: bool) {
    let state: State<'_, std::sync::Arc<AppState>> = app.state();
    
    let tray_enabled = {
        let s = state.settings.read().unwrap();
        s.ui.tray_enabled
    };
    if !tray_enabled && visible {
        return;
    }

    let mut hud_lock = state.hud_visible.lock().await;
    let old_visible = *hud_lock;
    *hud_lock = visible;

    if old_visible != visible {
        if visible {
            log::info!("[Tray] Starting Tray user session (Tray window shown).");
        } else {
            log::info!("[Tray] Ending Tray user session (Tray window hidden).");
        }
    }

    if let Some(window) = app.get_webview_window("tray") {
        if visible {
            let _ = window.show();
            let _ = position_tray_window(&window).await;
        } else {
            let _ = window.hide();
        }
    }

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
    let state: State<'_, std::sync::Arc<AppState>> = app.state();
    {
        let mut settings = state.settings.write().map_err(|e| e.to_string())?;
        
        let new_mode = match mode.to_uppercase().as_str() {
            "PASSIVE" => InteractionMode::Passive,
            "PTT" => InteractionMode::PTT,
            _ => return Err(format!("Invalid interaction mode: {}", mode)),
        };

        match target.to_lowercase().as_str() {
            "main" => {
                settings.interaction.main_app_mode = new_mode.clone();
            }
            "tray" => {
                settings.interaction.tray_mode = new_mode.clone();
            }
            _ => return Err(format!("Invalid target window: {}", target)),
        }
        
        let _ = settings.save();
    }
    
    let event_name = format!("mode_changed_{}", target.to_lowercase());
    let _ = app.emit(&event_name, mode.clone());
    let _ = app.emit("mode_changed", mode);
    let _ = app.emit("settings-updated", ());
    
    Ok(())
}

#[tauri::command]
pub async fn show_main_window(app: AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();

        // Lazy Launch: If we are in Passive mode, we need the engine running.
        let state: State<'_, std::sync::Arc<AppState>> = app.state();
        let is_passive = {
            let s = state.settings.read().unwrap();
            s.interaction.main_app_mode == InteractionMode::Passive
        };

        if is_passive {
            let app_clone = app.clone();
            tauri::async_runtime::spawn(async move {
                let state: State<'_, std::sync::Arc<AppState>> = app_clone.state();
                let engine_running = state.engine.lock().await.is_some();
                if !engine_running {
                    log::info!("[Window] Main window shown in Passive mode. Launching engine...");
                    if let Err(e) = crate::ipc::pipeline::launch_engine(app_clone).await {
                        log::error!("[Window] Lazy launch failed: {}", e);
                    }
                }
            });
        }
    }
    Ok(())
}
