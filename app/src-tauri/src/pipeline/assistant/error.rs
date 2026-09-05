use std::sync::atomic::Ordering;
use tauri::AppHandle;

use crate::core::events::{
    emit_ipc, Actionability, IpcEvent, PipelineError, PipelineImpact, ToastLevel,
};
use crate::core::state::{AppState, InteractionState};
use crate::pipeline::{transition, RoutingContext};
use crate::toast::{should_show_error_toast, show_toast};

/// Handles pipeline subsystem errors according to the 2D Error Classification Matrix.
pub fn on_error<R: tauri::Runtime + 'static>(
    err: PipelineError,
    app: &AppHandle<R>,
    state: &AppState,
    ctx: &RoutingContext,
) {
    log::error!(
        "[Pipeline::Error] Error on turn {} (source: {}, impact: {:?}, actionability: {:?}): {}",
        err.turn_id,
        err.source,
        err.impact,
        err.actionability,
        err.message
    );

    match err.impact {
        PipelineImpact::Degraded => {
            // Degraded fidelity: Turn continues without stopping. No state transition, no token cancellation.
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

    // Ephemeral Toast Surface
    let toast_level = match err.impact {
        PipelineImpact::Degraded => ToastLevel::Warning,
        PipelineImpact::TurnAborted | PipelineImpact::SessionHalted => ToastLevel::Error,
    };
    if should_show_error_toast(app) {
        if let Err(e) = show_toast(app, "Voice Notice", &err.message, toast_level) {
            log::warn!("[Pipeline::Error] Failed to show error toast: {}", e);
        }
    }

    // Persistent Notification Surface (only when actionability is Actionable)
    if let Actionability::Actionable { category, hint } = err.actionability {
        let app_handle = app.clone();
        let notif_id = format!("err_{}_{}", err.turn_id, current_timestamp_ms());
        let full_msg = format!("{}\nHint: {}", err.message, hint);
        tauri::async_runtime::spawn(async move {
            let db_path = crate::utils::paths::db_path();
            if let Ok(conn) = crate::persistence::db::VoxDb::open(&db_path).await {
                let new_notif = crate::persistence::notifications::NewNotification {
                    id: notif_id,
                    category: category.to_string(),
                    title: format!("Action Required: {}", err.source),
                    message: full_msg,
                    status: "active".to_string(),
                    session_id: None,
                    metadata: String::new(),
                };
                if let Ok(record) =
                    crate::persistence::notifications::create_notification(&conn, &new_notif).await
                {
                    let _ = emit_ipc(&app_handle, IpcEvent::NotificationCreated(record));
                }
            }
        });
    }
}

fn current_timestamp_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
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
