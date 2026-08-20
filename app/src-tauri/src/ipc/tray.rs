use crate::core::settings::InteractionMode;
use crate::core::state::AppState;
use crate::tray::position_tray_window;
#[cfg(target_os = "linux")]
use crate::tray::setup_linux_virtual_layer;
use tauri::{AppHandle, Emitter, Manager, State, WebviewWindow};

/// Toggles the tray window visibility and updates the menu checkmark state.
pub async fn toggle_hud_visibility(app: AppHandle) {
    let state: State<'_, std::sync::Arc<AppState>> = app.state();

    // Check setup completion and dictation prerequisites
    let (setup_completed, dictation_enabled, is_tray_mode) = {
        let s = state.settings.read().unwrap();
        (
            s.setup.completed,
            s.dictation.enabled,
            s.dictation.output_mode == crate::core::settings::DictationOutputMode::Tray,
        )
    };
    if !setup_completed {
        log::warn!("[Tray] Blocked toggle_hud_visibility: Setup not completed.");
        return;
    }
    if !dictation_enabled || !is_tray_mode {
        log::warn!("[Tray] Blocked toggle_hud_visibility: Dictation is disabled or not set to Tray output mode.");
        return;
    }


    let mut hud_lock = state.hud_visible.lock().await;
    let new_state = !*hud_lock;
    *hud_lock = new_state;

    if new_state {
        if let Ok(window) = crate::tray::ensure_tray_window(&app) {
            let _ = window.show();
            position_tray_window(&window).await;
            let _ = app.emit("toggle_hud", ());
        }
    } else if let Some(window) = app.get_webview_window("tray") {
        let _ = window.hide();
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

    let owner: crate::core::state::InteractionOwner = state
        .owner
        .load(std::sync::atomic::Ordering::Relaxed)
        .into();
    if owner == crate::core::state::InteractionOwner::Dictation {
        state
            .pipeline
            .cancel_flag
            .store(true, std::sync::atomic::Ordering::Relaxed);
        if let Some(engine) = state.engine.lock().await.as_ref() {
            let turn_id = state
                .pipeline
                .turn_id
                .load(std::sync::atomic::Ordering::Relaxed);
            let _ = engine
                .pipeline_tx
                .send(crate::core::events::VoxEvent::Cancelled { turn_id });
            let _ = engine
                .stt_tx
                .send(crate::services::stt::SttCommand::ResetStream);
            engine.playback_engine.cancel();
        }
    }

    if let Some(window) = app.get_webview_window("tray") {
        let _ = window.hide();
    }
}

#[tauri::command]
pub async fn sync_hud_visibility(app: AppHandle, visible: bool) {
    let state: State<'_, std::sync::Arc<AppState>> = app.state();

    let dictation_enabled = {
        let s = state.settings.read().unwrap();
        s.dictation.enabled
    };
    if !dictation_enabled && visible {
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

    // Sync owner state and VAD actor
    if visible {
        state.owner.store(
            crate::core::state::InteractionOwner::Dictation as u32,
            std::sync::atomic::Ordering::Relaxed,
        );
        if let Some(engine) = state.engine.lock().await.as_ref() {
            let _ = engine
                .vad_tx
                .send(crate::core::state::VadCommand::UpdateOwner(
                    crate::core::state::InteractionOwner::Dictation,
                ));
        }
    } else {
        let owner: crate::core::state::InteractionOwner = state
            .owner
            .load(std::sync::atomic::Ordering::Relaxed)
            .into();
        if owner == crate::core::state::InteractionOwner::Dictation {
            state
                .pipeline
                .cancel_flag
                .store(true, std::sync::atomic::Ordering::Relaxed);
            if let Some(engine) = state.engine.lock().await.as_ref() {
                let turn_id = state
                    .pipeline
                    .turn_id
                    .load(std::sync::atomic::Ordering::Relaxed);
                let _ = engine
                    .pipeline_tx
                    .send(crate::core::events::VoxEvent::Cancelled { turn_id });
                let _ = engine
                    .stt_tx
                    .send(crate::services::stt::SttCommand::ResetStream);
                engine.playback_engine.cancel();
            }
        }

        if state
            .pipeline
            .is_engaged
            .load(std::sync::atomic::Ordering::Relaxed)
        {
            state.owner.store(
                crate::core::state::InteractionOwner::MainWindow as u32,
                std::sync::atomic::Ordering::Relaxed,
            );
            if let Some(engine) = state.engine.lock().await.as_ref() {
                let _ = engine
                    .vad_tx
                    .send(crate::core::state::VadCommand::UpdateOwner(
                        crate::core::state::InteractionOwner::MainWindow,
                    ));
            }
        }
    }

    if visible {
        if let Ok(window) = crate::tray::ensure_tray_window(&app) {
            let _ = window.show();
            let _ = position_tray_window(&window).await;
        }
    } else if let Some(window) = app.get_webview_window("tray") {
        let _ = window.hide();
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
pub async fn update_interaction_mode(
    app: AppHandle,
    target: String,
    mode: String,
) -> Result<(), String> {
    let state: State<'_, std::sync::Arc<AppState>> = app.state();
    let new_mode = match mode.to_uppercase().as_str() {
        "PASSIVE" => InteractionMode::Passive,
        "PTT" => InteractionMode::PTT,
        _ => return Err(format!("Invalid interaction mode: {}", mode)),
    };

    {
        let mut settings = state.settings.write().map_err(|e| e.to_string())?;

        match target.to_lowercase().as_str() {
            "main" => {
                settings.interaction.main_app_mode = new_mode.clone();
            }
            "tray" | "dictation" => {
                settings.dictation.interaction_mode = match new_mode {
                    InteractionMode::Passive => crate::core::settings::DictationInteractionMode::Passive,
                    InteractionMode::PTT => crate::core::settings::DictationInteractionMode::Ptt,
                };
            }
            _ => return Err(format!("Invalid target window: {}", target)),
        }

        let _ = settings.save();
    }

    // 1. VAD Actor synchronization on owner match
    let owner: crate::core::state::InteractionOwner = state
        .owner
        .load(std::sync::atomic::Ordering::Relaxed)
        .into();
    let current_target = match target.to_lowercase().as_str() {
        "main" => crate::core::state::InteractionOwner::MainWindow,
        _ => crate::core::state::InteractionOwner::Dictation,
    };

    if owner == current_target {
        if let Some(engine) = state.engine.lock().await.as_ref() {
            let _ = engine
                .vad_tx
                .send(crate::core::state::VadCommand::UpdateMode(new_mode));
        }
    }

    // 2. Engine lifecycle check for main window mode changes
    if target.to_lowercase() == "main" {
        let (dictation_enabled, is_engaged, is_passive) = {
            let s = state.settings.read().unwrap();
            (
                s.dictation.enabled,
                state
                    .pipeline
                    .is_engaged
                    .load(std::sync::atomic::Ordering::Relaxed),
                s.interaction.main_app_mode == InteractionMode::Passive,
            )
        };

        if !dictation_enabled && !is_engaged && !is_passive {
            log::info!("[Settings] Main App mode changed to non-passive and Dictation is disabled. Stopping engine...");
            let app_clone = app.clone();
            tauri::async_runtime::spawn(async move {
                let _ = crate::ipc::pipeline::stop_engine(app_clone).await;
            });
        } else if is_passive {
            log::info!(
                "[Settings] Main App mode changed to Passive. Ensuring engine is launched..."
            );
            let app_clone = app.clone();
            tauri::async_runtime::spawn(async move {
                let _ = crate::ipc::pipeline::launch_engine(app_clone).await;
            });
        }
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
    }
    Ok(())
}
