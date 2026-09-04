use tauri::AppHandle;

use crate::core::events::{emit_ipc_to, IpcEvent, VoiceErrorPayload};
use crate::core::state::{AppState, InteractionOwner, InteractionState};
use crate::pipeline::dictation::transition_dictation;
use crate::pipeline::WINDOW_TRAY;

/// Logs dictation errors, updates tray state, and auto-recovers to Ready if enabled.
pub fn on_error<R: tauri::Runtime>(
    turn_id: u32,
    message: String,
    app: &AppHandle<R>,
    state: &AppState,
) {
    log::error!("[Dictation::Error] Error on turn {}: {}", turn_id, message);
    transition_dictation(InteractionState::Error, app, state);

    let toast_message = message.clone();
    if let Err(e) = emit_ipc_to(
        app,
        WINDOW_TRAY,
        IpcEvent::VoiceError(VoiceErrorPayload {
            message,
            source: "Dictation".to_string(),
            owner: Some(InteractionOwner::Dictation),
        }),
    ) {
        log::warn!("[Dictation::Error] Failed to emit voice_error: {}", e);
    }

    if crate::toast::should_show_error_toast(app) {
        if let Err(e) = crate::toast::show_toast(
            app,
            "Voice Error",
            &toast_message,
            crate::core::events::ToastLevel::Error,
        ) {
            log::warn!("[Dictation::Error] Failed to show error toast: {}", e);
        }
    }

    // Transient STT recovery: if dictation remains enabled, transition back to Ready.
    if state.pipeline.dictation_state() != InteractionState::Idle {
        transition_dictation(InteractionState::Ready, app, state);
    }
}

/// Handles cancellation event and resets state machine to Ready.
pub fn on_cancelled<R: tauri::Runtime>(turn_id: u32, app: &AppHandle<R>, state: &AppState) {
    log::info!("[Dictation::Error] Interaction cancelled on turn {}", turn_id);
    transition_dictation(InteractionState::Ready, app, state);
}
