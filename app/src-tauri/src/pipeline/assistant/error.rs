use std::sync::atomic::Ordering;

use tauri::AppHandle;

use crate::{
    core::{
        error::{PipelineError, PipelineImpact},
        events::Severity,
        state::{AppState, InteractionState},
    },
    pipeline::{transition, RoutingContext},
    services::notifications::{
        notify, Action, ActionPayload, NotificationCategory, NotificationParams,
    },
};

/// Handles pipeline subsystem errors according to the 2D Error Classification Matrix.
pub fn on_error<R: tauri::Runtime + 'static>(
    err: PipelineError,
    app: &AppHandle<R>,
    state: &AppState,
    ctx: &RoutingContext,
) {
    log::error!(
        "[Pipeline::Error] Error on turn {} (source: {}, impact: {:?}): {}",
        err.turn_id,
        err.source,
        err.impact,
        err.message
    );

    match err.impact {
        PipelineImpact::None | PipelineImpact::Degraded => {
            // Degraded fidelity or None: Turn continues without stopping. No state transition, no token cancellation.
        }
        PipelineImpact::TurnAborted => {
            // Turn fails cleanly: Cancel turn token and synthesis jobs, return state machine directly to Ready.
            if let Ok(guard) = state.engine.try_lock() {
                if let Some(ref engine) = *guard {
                    engine.playback_engine.cancel();
                }
            }
            state.pipeline.turn_token().cancel();
            state
                .pipeline
                .pending_synthesis_jobs
                .store(0, Ordering::Relaxed);
            state.pipeline.reset_turn_guards();
            transition(InteractionState::Ready, ctx, app, state);
        }
        PipelineImpact::SessionHalted => {
            // Unrecoverable breakdown: Cancel playback, trip token, transition to Error.
            if let Ok(guard) = state.engine.try_lock() {
                if let Some(ref engine) = *guard {
                    engine.playback_engine.cancel();
                }
            }
            state.pipeline.turn_token().cancel();
            state
                .pipeline
                .pending_synthesis_jobs
                .store(0, Ordering::Relaxed);
            state.pipeline.reset_turn_guards();
            transition(InteractionState::Error, ctx, app, state);
        }
    }

    // Classify error and delegate alerting strictly to the Notification Service
    let msg_lower = err.message.to_lowercase();
    let is_hw_error = err.source.contains("Audio")
        || err.source.contains("Microphone")
        || msg_lower.contains("microphone")
        || msg_lower.contains("audio device");

    let is_model_error = err.source.contains("Model")
        || msg_lower.contains("model")
        || msg_lower.contains("404")
        || msg_lower.contains("not found")
        || msg_lower.contains("nosuchmodel");

    let category = if is_hw_error {
        NotificationCategory::Hardware
    } else if is_model_error {
        NotificationCategory::Models
    } else {
        NotificationCategory::Pipeline
    };

    let severity = match err.impact {
        PipelineImpact::None | PipelineImpact::Degraded => Severity::Warning,
        PipelineImpact::TurnAborted => Severity::Warning,
        PipelineImpact::SessionHalted => Severity::Critical,
    };

    let action = match err.impact {
        PipelineImpact::None | PipelineImpact::Degraded => Action::Transient,
        PipelineImpact::TurnAborted => {
            if msg_lower.contains("context")
                || msg_lower.contains("prompt too long")
                || msg_lower.contains("nokvcache")
                || msg_lower.contains("rate limit")
                || msg_lower.contains("rate_limit")
                || msg_lower.contains("429")
                || msg_lower.contains("quota")
            {
                Action::Interactive(ActionPayload::Navigate {
                    target: "settings/ai".to_string(),
                })
            } else {
                Action::Transient
            }
        }
        PipelineImpact::SessionHalted => {
            if is_hw_error {
                Action::Interactive(ActionPayload::Navigate {
                    target: "settings/audio".to_string(),
                })
            } else if is_model_error {
                Action::Interactive(ActionPayload::Navigate {
                    target: "settings/models".to_string(),
                })
            } else {
                Action::Interactive(ActionPayload::Navigate {
                    target: "settings/ai".to_string(),
                })
            }
        }
    };

    let title = if is_hw_error {
        "Voice Notice: Audio".to_string()
    } else if is_model_error {
        "Voice Notice: Models".to_string()
    } else if msg_lower.contains("context") || msg_lower.contains("prompt too long") {
        "Voice Notice: Context".to_string()
    } else if msg_lower.contains("rate limit") || msg_lower.contains("429") {
        "Voice Notice: Rate Limit".to_string()
    } else if msg_lower.contains("auth")
        || msg_lower.contains("401")
        || msg_lower.contains("api key")
    {
        "Voice Notice: Auth".to_string()
    } else {
        format!("Voice Notice: {}", err.source)
    };

    let app_handle = app.clone();
    let db = std::sync::Arc::clone(&state.db);
    let message = err.message.clone();
    let group_key = format!("pipeline_error:{}", err.source);
    let impact = err.impact;

    tauri::async_runtime::spawn(async move {
        if let Err(e) = notify(
            &app_handle,
            &db,
            NotificationParams {
                group_key: Some(&group_key),
                category,
                severity,
                impact: Some(impact),
                action,
                title: &title,
                message: &message,
                session_id: None,
                metadata: None,
                duration_ms: None,
            },
        )
        .await
        {
            log::warn!(
                "[Pipeline::Error] Failed to dispatch error notification: {}",
                e
            );
        }
    });
}

/// Handles turn cancellation by clearing accumulator state, resetting synthesis jobs, and returning to Ready.
pub fn on_cancelled<R: tauri::Runtime>(
    turn_id: u32,
    app: &AppHandle<R>,
    state: &AppState,
    ctx: &RoutingContext,
) {
    log::info!(
        "[Pipeline::Cancelled] Interaction cancelled on turn {}",
        turn_id
    );

    state.pipeline_accumulator.lock().clear();
    state
        .pipeline
        .pending_synthesis_jobs
        .store(0, Ordering::Relaxed);
    state.pipeline.reset_turn_guards();

    if let Ok(guard) = state.engine.try_lock() {
        if let Some(ref engine) = *guard {
            engine.playback_engine.cancel();
        }
    }

    transition(InteractionState::Ready, ctx, app, state);
}
