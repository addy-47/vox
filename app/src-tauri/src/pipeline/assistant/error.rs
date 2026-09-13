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
            transition(InteractionState::Error, ctx, app, state);
        }
    }

    // Classify error and delegate alerting strictly to the Notification Service
    let category = if err.source.contains("Audio") || err.source.contains("Microphone") {
        NotificationCategory::Hardware
    } else if err.source.contains("Model") {
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
            if err.message.contains("context") || err.message.contains("prompt too long") {
                Action::Interactive(ActionPayload::Navigate {
                    target: "settings/ai".to_string(),
                })
            } else {
                Action::Transient
            }
        }
        PipelineImpact::SessionHalted => {
            if err.source.contains("Audio") {
                Action::Interactive(ActionPayload::Navigate {
                    target: "settings/audio".to_string(),
                })
            } else if err.source.contains("Model") {
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

    let app_handle = app.clone();
    let db = std::sync::Arc::clone(&state.db);
    let title = format!("Voice Notice: {}", err.source);
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

    if let Ok(guard) = state.engine.try_lock() {
        if let Some(ref engine) = *guard {
            engine.playback_engine.cancel();
        }
    }

    transition(InteractionState::Ready, ctx, app, state);
}
