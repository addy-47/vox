use std::sync::atomic::Ordering;

use tauri::AppHandle;

use crate::{
    core::state::{AppState, InteractionState},
    pipeline::{transition, RoutingContext},
};

/// Handles onset of audio playback, transitioning pipeline state from Thinking to Speaking.
pub fn on_playback_started<R: tauri::Runtime>(
    turn_id: u32,
    app: &AppHandle<R>,
    state: &AppState,
    ctx: &RoutingContext,
) {
    let current_state = state.pipeline.state();
    if current_state != InteractionState::Thinking {
        log::debug!(
            "[Pipeline::Playback] PlaybackStarted dropped (turn {}): state is {:?}, expected Thinking",
            turn_id,
            current_state
        );
        return;
    }

    transition(InteractionState::Speaking, ctx, app, state);
    log::info!(
        "[Pipeline::Playback] Playback started -> Speaking (turn: {})",
        turn_id
    );
}

/// Handles completion of audio playback, guarding against premature completion if synthesis jobs remain.
pub fn on_playback_finished<R: tauri::Runtime>(
    turn_id: u32,
    app: &AppHandle<R>,
    state: &AppState,
    ctx: &RoutingContext,
) {
    let current_state = state.pipeline.state();
    if current_state != InteractionState::Speaking {
        log::debug!(
            "[Pipeline::Playback] PlaybackFinished dropped (turn {}): state is {:?}, expected Speaking",
            turn_id,
            current_state
        );
        return;
    }

    let pending_jobs = state
        .pipeline
        .pending_synthesis_jobs
        .load(Ordering::Relaxed);
    if pending_jobs > 0 {
        log::debug!(
            "[Pipeline::Playback] PlaybackFinished deferred (turn {}): {} synthesis jobs still pending",
            turn_id,
            pending_jobs
        );
        return;
    }

    transition(InteractionState::Ready, ctx, app, state);
    log::info!(
        "[Pipeline::Playback] Playback finished -> Ready (turn: {})",
        turn_id
    );
}
