use tauri::AppHandle;

use crate::{
    core::{
        events::{PipelineError, PipelineImpact, ToastLevel},
        state::{AppState, InteractionState},
    },
    pipeline::dictation::transition_dictation,
    toast::{should_show_error_toast, show_toast},
};

/// Logs dictation errors, updates tray state, and handles recovery per 2D Error Classification.
pub fn on_error<R: tauri::Runtime>(err: PipelineError, app: &AppHandle<R>, state: &AppState) {
    log::error!(
        "[Dictation::Error] Error on turn {} (impact: {:?}, actionability: {:?}): {}",
        err.turn_id,
        err.impact,
        err.actionability,
        err.message
    );

    let was_idle = state.pipeline.dictation_state() == InteractionState::Idle;

    match err.impact {
        PipelineImpact::Degraded => {
            // Degraded dictation (e.g. transliteration dropped): No state change
        }
        PipelineImpact::TurnAborted => {
            // Transient STT recovery: if dictation remains enabled, transition back to Ready.
            if was_idle {
                transition_dictation(InteractionState::Idle, app, state);
            } else {
                transition_dictation(InteractionState::Ready, app, state);
            }
        }
        PipelineImpact::SessionHalted => {
            transition_dictation(InteractionState::Error, app, state);
        }
    }

    if should_show_error_toast(app) {
        let level = match err.impact {
            PipelineImpact::Degraded => ToastLevel::Warning,
            _ => ToastLevel::Error,
        };
        if let Err(e) = show_toast(app, "Dictation Notice", &err.message, level) {
            log::warn!("[Dictation::Error] Failed to show error toast: {}", e);
        }
    }
}

/// Handles cancellation event and resets state machine to Ready.
pub fn on_cancelled<R: tauri::Runtime>(turn_id: u32, app: &AppHandle<R>, state: &AppState) {
    log::info!(
        "[Dictation::Error] Interaction cancelled on turn {}",
        turn_id
    );
    transition_dictation(InteractionState::Ready, app, state);
}
