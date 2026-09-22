use std::sync::atomic::Ordering;

use tauri::AppHandle;

use crate::{
    core::{
        events::{emit_ipc, AudioIntent, IpcEvent},
        state::{AppState, InteractionState},
    },
    pipeline::{transition, RoutingContext},
};

/// Handles onset of audio playback, transitioning pipeline state from Thinking/Working to Speaking for TurnResponse.
pub fn on_playback_started<R: tauri::Runtime>(
    turn_id: u32,
    intent: AudioIntent,
    app: &AppHandle<R>,
    state: &AppState,
    ctx: &RoutingContext,
) {
    if intent == AudioIntent::InterimFiller {
        log::debug!(
            "[Pipeline::Playback] Interim filler playback started (turn {}) - remaining in Working state",
            turn_id
        );
        return;
    }

    state.pipeline.clear_drained_while_open();

    let current_state = state.pipeline.state();
    if current_state != InteractionState::Thinking && current_state != InteractionState::Working {
        log::debug!(
            "[Pipeline::Playback] PlaybackStarted dropped (turn {}): state is {:?}, expected Thinking or Working",
            turn_id,
            current_state
        );
        return;
    }

    transition(InteractionState::Speaking, ctx, app, state);
    let metrics_payload = state
        .turn_metrics
        .record_playback_started(turn_id, &state.telemetry);
    if let Err(e) = emit_ipc(app, IpcEvent::TurnMetrics(metrics_payload)) {
        log::warn!("[Pipeline::Playback] Failed to emit TurnMetrics IPC: {:?}", e);
    }
    log::info!(
        "[Pipeline::Playback] Playback started -> Speaking (turn: {})",
        turn_id
    );
}

/// Handles completion of audio playback, guarding against premature completion if synthesis jobs remain.
pub fn on_playback_finished<R: tauri::Runtime>(
    turn_id: u32,
    intent: AudioIntent,
    app: &AppHandle<R>,
    state: &AppState,
    ctx: &RoutingContext,
) {
    if intent == AudioIntent::InterimFiller {
        log::debug!(
            "[Pipeline::Playback] Interim filler playback finished (turn {}) - remaining in Working state",
            turn_id
        );
        return;
    }

    let current_state = state.pipeline.state();
    if current_state != InteractionState::Speaking {
        log::debug!(
            "[Pipeline::Playback] PlaybackFinished dropped (turn {}): state is {:?}, expected Speaking",
            turn_id,
            current_state
        );
        return;
    }

    if state.pipeline.is_turn_open() {
        state.pipeline.set_drained_while_open(true);
        log::debug!(
            "[Pipeline::Playback] PlaybackFinished deferred (turn {}): turn_open=true, latching drained_while_open",
            turn_id
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

    state.pipeline.clear_drained_while_open();
    transition(InteractionState::Ready, ctx, app, state);
    state.turn_metrics.record_playback_finished(turn_id);
    log::info!(
        "[Pipeline::Playback] Playback finished -> Ready (turn: {})",
        turn_id
    );
}
