use tauri::AppHandle;

use crate::core::state::{AppState, InteractionState};
use crate::pipeline::dictation::transition_dictation;

/// Logs dictation errors, updates tray state, and auto-recovers to Ready if enabled.
pub fn on_error<R: tauri::Runtime>(
    turn_id: u32,
    message: String,
    app: &AppHandle<R>,
    state: &AppState,
) {
    log::error!("[Dictation::Error] Error on turn {}: {}", turn_id, message);
    let was_idle = state.pipeline.dictation_state() == InteractionState::Idle;
    transition_dictation(InteractionState::Error, app, state);

    if crate::toast::should_show_error_toast(app) {
        if let Err(e) = crate::toast::show_toast(
            app,
            "Voice Error",
            &message,
            crate::core::events::ToastLevel::Error,
        ) {
            log::warn!("[Dictation::Error] Failed to show error toast: {}", e);
        }
    }

    // Transient STT recovery: if dictation remains enabled, transition back to Ready.
    // A disabled (Idle) dictation stays Idle so a hotkey press cannot enable it.
    if was_idle {
        transition_dictation(InteractionState::Idle, app, state);
    } else {
        transition_dictation(InteractionState::Ready, app, state);
    }
}

/// Handles cancellation event and resets state machine to Ready.
pub fn on_cancelled<R: tauri::Runtime>(turn_id: u32, app: &AppHandle<R>, state: &AppState) {
    log::info!("[Dictation::Error] Interaction cancelled on turn {}", turn_id);
    transition_dictation(InteractionState::Ready, app, state);
}
