use std::sync::atomic::Ordering;

use tauri::AppHandle;

use crate::{
    core::{
        events::{emit_ipc, IpcEvent},
        state::{AppState, InteractionState},
    },
    pipeline::{transition, RoutingContext},
};

/// Handles onset of audio playback and transitions to Speaking (RULE-EVT-09 §1).
pub fn on_playback_started<R: tauri::Runtime>(
    turn_id: u32,
    app: &AppHandle<R>,
    state: &AppState,
    ctx: &RoutingContext,
) {
    let active_turn_id = state.pipeline.peek_turn_id();
    if turn_id != active_turn_id {
        log::debug!(
            "[Pipeline::Playback] PlaybackStarted dropped (turn {}): does not match active turn {}",
            turn_id,
            active_turn_id
        );
        return;
    }

    let current_state = state.pipeline.state();
    if !matches!(
        current_state,
        InteractionState::Thinking | InteractionState::Working | InteractionState::Ready
    ) {
        log::debug!(
            "[Pipeline::Playback] PlaybackStarted dropped (turn {}): state is {:?}",
            turn_id,
            current_state
        );
        return;
    }

    transition(InteractionState::Speaking, None, ctx, app, state);
    let metrics_payload = state
        .turn_metrics
        .record_playback_started(turn_id, &state.telemetry);
    if let Err(e) = emit_ipc(app, IpcEvent::TurnMetrics(metrics_payload)) {
        log::warn!(
            "[Pipeline::Playback] Failed to emit TurnMetrics IPC: {:?}",
            e
        );
    }
    log::info!(
        "[Pipeline::Playback] Playback started ({:?} -> Speaking, turn {})",
        current_state,
        turn_id
    );
}

/// Handles audio drain and resolves the duplex quiescence law (RULE-EVT-09 §2).
pub fn on_playback_finished<R: tauri::Runtime>(
    turn_id: u32,
    app: &AppHandle<R>,
    state: &AppState,
    ctx: &RoutingContext,
) {
    let active_turn_id = state.pipeline.peek_turn_id();
    if turn_id != active_turn_id {
        log::debug!(
            "[Pipeline::Playback] PlaybackFinished dropped (turn {}): does not match active turn {}",
            turn_id,
            active_turn_id
        );
        return;
    }

    let current_state = state.pipeline.state();
    let pending_jobs = state.pipeline.pending_tts_jobs.load(Ordering::Acquire);
    let sink_occupied = state.pipeline.sink_occupied.load(Ordering::Acquire);
    let zero_audio_fallback = matches!(
        current_state,
        InteractionState::Thinking | InteractionState::Working
    ) && state.pipeline.is_llm_done()
        && pending_jobs == 0
        && !sink_occupied;

    if current_state != InteractionState::Speaking && !zero_audio_fallback {
        log::debug!(
            "[Pipeline::Playback] PlaybackFinished dropped (turn {}): state is {:?}",
            turn_id,
            current_state
        );
        return;
    }

    if pending_jobs > 0 {
        log::debug!(
            "[Pipeline::Playback] PlaybackFinished deferred (turn {}): {} synthesis jobs still pending",
            turn_id,
            pending_jobs
        );
        return;
    }

    if let Ok(guard) = state.engine.try_lock() {
        if let Some(ref engine) = *guard {
            if let Some(ref aec_tx) = engine.aec_tx {
                let _ = aec_tx.send(crate::services::audio::aec::AecCommand::ClearRenderQueue);
            }
        }
    }

    // S-01 ordering assumption: clearing accumulator on zero-audio fallback is safe
    // because is_llm_done() is verified true, ensuring the harness turn artifact has
    // already been committed prior to quiescence latch resolution.
    state.pipeline_accumulator.lock().clear();
    let target = if state.pipeline.is_llm_done() {
        InteractionState::Ready
    } else if state.pipeline.current_activity().is_some() {
        InteractionState::Working
    } else {
        InteractionState::Thinking
    };
    transition(target, None, ctx, app, state);
    if target == InteractionState::Ready {
        super::tasks::check_and_dispatch_inbox_tasks(app, state, ctx);
    }
    state.turn_metrics.record_playback_finished(turn_id);
    log::info!(
        "[Pipeline::Playback] Playback finished ({:?} -> {:?}, turn {})",
        current_state,
        target,
        turn_id
    );
}
