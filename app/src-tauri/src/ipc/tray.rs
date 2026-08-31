use crate::core::events::{emit_ipc, IpcEvent};
use crate::core::settings::InteractionMode;
use crate::core::state::AppState;
use crate::tray::position_tray_window;
#[cfg(target_os = "linux")]
use crate::tray::setup_linux_virtual_layer;
use tauri::{AppHandle, Manager, State, WebviewWindow};

/// Toggles the tray window visibility and updates the menu checkmark state.
#[tauri::command]
pub async fn toggle_tray_visibility(app: AppHandle) {
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
        state
            .pipeline
            .cancel_flag
            .store(true, std::sync::atomic::Ordering::Relaxed);
        if let Some(engine) = state.engine.lock().await.as_ref() {
            let turn_id = state
                .pipeline
                .turn_id
                .load(std::sync::atomic::Ordering::Relaxed);
            if let Err(e) = engine
                .pipeline_tx
                .send(crate::core::events::VoxEvent::Cancelled { turn_id })
            {
                log::warn!("[Tray] Failed to send VoxEvent::Cancelled: {}", e);
            }
            if let Err(e) = engine
                .stt_tx
                .send(crate::services::stt::SttCommand::ResetStream)
            {
                log::warn!("[Tray] Failed to send SttCommand::ResetStream: {}", e);
            }
            engine.playback_engine.cancel();
        }
    }
}

/// Hide the tray HUD overlay window and cancel active dictation turns.
#[tauri::command]
pub async fn hide_tray_window(app: AppHandle) {
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
}

/// Synchronize the tray HUD overlay visibility with interaction owner and VAD routing.
#[tauri::command]
pub async fn sync_hud_visibility(app: AppHandle, visible: bool) {
    let state: State<'_, std::sync::Arc<AppState>> = app.state();

    let dictation_enabled = state
        .settings
        .read()
        .map(|s| s.dictation.enabled)
        .unwrap_or(false);
    if !dictation_enabled && visible {
        return;
    }

    let old_visible = state
        .hud_visible
        .swap(visible, std::sync::atomic::Ordering::Relaxed);

    if old_visible != visible {
        if visible {
            log::info!("[Tray] Starting Tray user session (Tray window shown).");
        } else {
            log::info!("[Tray] Ending Tray user session (Tray window hidden).");
        }
    }

    if visible {
        state.owner.store(
            crate::core::state::InteractionOwner::Dictation as u32,
            std::sync::atomic::Ordering::Relaxed,
        );
        if let Ok(window) = crate::tray::ensure_tray_window(&app) {
            if let Err(e) = window.show() {
                log::warn!("[Tray] Failed to show tray window: {}", e);
            }
            position_tray_window(&window).await;
        }
    } else {
        cancel_active_dictation_turn(&state).await;

        if state.pipeline.state() != crate::core::state::InteractionState::Idle {
            state.owner.store(
                crate::core::state::InteractionOwner::Assistant as u32,
                std::sync::atomic::Ordering::Relaxed,
            );
        }

        if let Some(window) = app.get_webview_window("tray") {
            if let Err(e) = window.hide() {
                log::warn!("[Tray] Failed to hide tray window: {}", e);
            }
        }
    }

    let item_lock = state.hud_menu_item.lock();
    if let Some(item) = &*item_lock {
        if let Err(e) = item.set_checked(visible) {
            log::warn!("[Tray] Failed to set menu item checked state: {}", e);
        }
    }
}

/// Set whether the tray HUD overlay should ignore mouse cursor input events.
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
        if let Err(e) = window.set_ignore_cursor_events(ignore) {
            log::warn!("[Tray] Failed to set ignore cursor events: {}", e);
        }
    }
}

fn evaluate_main_mode_engine_lifecycle(app: AppHandle, state: &AppState) {
    let (dictation_enabled, interaction_mode) = state
        .settings
        .read()
        .map(|s| (s.dictation.enabled, s.interaction.mode.clone()))
        .unwrap_or((false, InteractionMode::PTT));

    if !dictation_enabled
        && state.pipeline.state() == crate::core::state::InteractionState::Idle
        && interaction_mode == InteractionMode::PTT
    {
        log::info!("[Settings] Main App mode changed to non-passive and Dictation is disabled. Stopping engine...");
        tauri::async_runtime::spawn(async move {
            if let Err(e) = crate::ipc::pipeline::stop_engine(app).await {
                log::warn!("[Tray] Failed to stop engine: {}", e);
            }
        });
    } else if interaction_mode == InteractionMode::Passive {
        log::info!("[Settings] Main App mode changed to Passive. Ensuring engine is launched...");
        tauri::async_runtime::spawn(async move {
            if let Err(e) = crate::ipc::pipeline::launch_engine(app).await {
                log::warn!("[Tray] Failed to launch engine: {}", e);
            }
        });
    }
}

/// Update interaction mode (PTT vs Passive) for main or tray windows and sync VAD.
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
                settings.interaction.mode = new_mode.clone();
            }
            "tray" | "dictation" => {
                settings.dictation.interaction_mode = match new_mode {
                    InteractionMode::Passive => {
                        crate::core::settings::DictationInteractionMode::Passive
                    }
                    InteractionMode::PTT => crate::core::settings::DictationInteractionMode::Ptt,
                };
            }
            _ => return Err(format!("Invalid target window: {}", target)),
        }
        if let Err(e) = settings.save() {
            log::warn!(
                "[Tray] Failed to save settings on interaction mode update: {}",
                e
            );
        }
    }

    let owner: crate::core::state::InteractionOwner = state
        .owner
        .load(std::sync::atomic::Ordering::Relaxed)
        .into();
    let current_target = match target.to_lowercase().as_str() {
        "main" => crate::core::state::InteractionOwner::Assistant,
        _ => crate::core::state::InteractionOwner::Dictation,
    };

    if owner == current_target {
        if let Some(engine) = state.engine.lock().await.as_ref() {
            if let Err(e) = engine
                .vad_tx
                .send(crate::services::vad::VadCommand::UpdateMode(new_mode))
            {
                log::warn!("[Tray] Failed to send VadCommand::UpdateMode: {}", e);
            }
        }
    }

    if target.to_lowercase() == "main" {
        evaluate_main_mode_engine_lifecycle(app.clone(), &state);
    }

    if let Err(e) = emit_ipc(&app, IpcEvent::SettingsUpdated) {
        log::warn!("[Tray] Failed to emit settings-updated: {}", e);
    }

    Ok(())
}

#[tauri::command]
pub async fn show_main_window(app: AppHandle) -> Result<(), String> {
    // Rebuilds the window if it was destroyed (renderer crash); otherwise
    // un-hides and focuses the existing one.
    crate::window_main::ensure_main_window(&app)?;
    Ok(())
}
