use tauri::AppHandle;

use crate::{
    core::{
        error::{PipelineError, PipelineImpact},
        events::Severity,
        state::{AppState, InteractionState},
    },
    pipeline::dictation::transition_dictation,
    services::{
        self,
        notifications::{Action, NotificationCategory, NotificationParams},
    },
};

/// Logs dictation errors, updates tray state, and handles recovery per 2D Error Classification.
pub fn on_error<R: tauri::Runtime>(err: PipelineError, app: &AppHandle<R>, state: &AppState) {
    log::error!(
        "[Dictation::Error] Error on turn {} (impact: {:?}): {}",
        err.turn_id,
        err.impact,
        err.message
    );

    let was_idle = state.pipeline.dictation_state() == InteractionState::Idle;

    match err.impact {
        PipelineImpact::None | PipelineImpact::Degraded => {
            // Degraded dictation or None (e.g. transliteration dropped): No state change
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

    let app_handle = app.clone();
    let db = state.db.clone();
    let message = err.message.clone();
    let impact = err.impact;
    let severity = match impact {
        PipelineImpact::None => Severity::Info,
        PipelineImpact::Degraded => Severity::Warning,
        _ => Severity::Critical,
    };
    let action = match impact {
        PipelineImpact::SessionHalted => {
            Action::Interactive(services::notifications::ActionPayload::Navigate {
                target: "settings/dictation".to_string(),
            })
        }
        _ => Action::Transient,
    };

    tauri::async_runtime::spawn(async move {
        let params = NotificationParams {
            category: NotificationCategory::Dictation,
            severity,
            impact: Some(impact),
            action,
            title: "Dictation Notice",
            message: &message,
            group_key: Some("dictation:error"),
            session_id: None,
            metadata: None,
            duration_ms: None,
        };
        if let Err(e) = services::notifications::notify(&app_handle, &db, params).await {
            log::warn!("[Dictation::Error] Failed to dispatch notification: {}", e);
        }
    });
}

/// Handles cancellation event and resets state machine to Ready.
pub fn on_cancelled<R: tauri::Runtime>(turn_id: u32, app: &AppHandle<R>, state: &AppState) {
    log::info!(
        "[Dictation::Error] Interaction cancelled on turn {}",
        turn_id
    );
    transition_dictation(InteractionState::Ready, app, state);
}
