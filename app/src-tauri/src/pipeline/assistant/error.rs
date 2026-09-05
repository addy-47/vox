use std::sync::atomic::Ordering;
use tauri::AppHandle;

use crate::core::events::ToastLevel;
use crate::core::state::{AppState, InteractionState};
use crate::pipeline::{transition, RoutingContext};
use crate::toast::{should_show_error_toast, show_toast};

/// Handles pipeline subsystem errors by canceling playback, transitioning to Error, and emitting a toast.
pub fn on_error<R: tauri::Runtime>(
    turn_id: u32,
    message: String,
    source: String,
    app: &AppHandle<R>,
    state: &AppState,
    ctx: &RoutingContext,
) {
    log::error!(
        "[Pipeline::Error] Error on turn {} (source: {}): {}",
        turn_id,
        source,
        message
    );

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

    if should_show_error_toast(app) {
        if let Err(e) = show_toast(app, "Voice Error", &message, ToastLevel::Error) {
            log::warn!("[Pipeline::Error] Failed to show error toast: {}", e);
        }
    }
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
