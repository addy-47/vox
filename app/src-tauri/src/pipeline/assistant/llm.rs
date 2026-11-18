use std::{sync::atomic::Ordering, time::Duration};

use tauri::AppHandle;

use crate::{
    core::{
        events::{emit_ipc, IpcEvent, PipelineMode},
        state::{AppState, InteractionState},
    },
    persistence::PersistenceEvent,
    pipeline::{transition, RoutingContext},
};

const PERSIST_SEND_TIMEOUT: Duration = Duration::from_secs(2);

/// Commits finalized assistant turn to persistence event queue.
fn persist_assistant_turn(turn_id: u32, full_text: String, user_text: String, state: &AppState) {
    let conv_id = state.conversation_id.load(Ordering::Relaxed);

    let persist_lock = state.persist_tx.lock();
    if let Some(ref tx) = *persist_lock {
        if let Err(e) = tx.send_timeout(
            PersistenceEvent::TurnCompleted {
                session_id: conv_id as i64,
                turn_id,
                user_text,
                assistant_text: full_text,
            },
            PERSIST_SEND_TIMEOUT,
        ) {
            log::error!(
                "[Pipeline::Llm] Failed to send TurnCompleted to persistence: {}",
                e
            );
        }
    }
}

/// Flushes audio pre-roll buffer in the playback engine based on pipeline mode.
fn flush_pre_roll(state: &AppState, mode: &PipelineMode) {
    if *mode == PipelineMode::Realtime {
        state.pipeline.pending_tts_jobs.store(0, Ordering::Relaxed);
    }
    if let Ok(guard) = state.engine.try_lock() {
        if let Some(ref engine) = *guard {
            engine.playback_engine.flush_pre_roll();
        }
    }
}

/// Finalizes LLM output generation, flushes audio pre-roll, and evaluates the synthesis latch.
pub fn on_llm_finished<R: tauri::Runtime>(
    turn_id: u32,
    app: Option<&AppHandle<R>>,
    state: &AppState,
    ctx: &RoutingContext,
) {
    let active_turn_id = state.pipeline.peek_turn_id();
    if turn_id != active_turn_id {
        log::debug!(
            "[Pipeline::Llm] Dropping stale LlmFinished for turn {} (active: {})",
            turn_id,
            active_turn_id
        );
        return;
    }

    let current_state = state.pipeline.state();
    if current_state != InteractionState::Thinking
        && current_state != InteractionState::Speaking
        && current_state != InteractionState::Working
    {
        log::debug!(
            "[Pipeline::Llm] LlmFinished dropped: state is {:?}, expected Thinking, Speaking, or Working",
            current_state
        );
        return;
    }

    state.turn_metrics.record_llm_finish();
    flush_pre_roll(state, &ctx.pipeline_mode);

    let (full_text, user_text) = {
        let acc = state.pipeline_accumulator.lock();
        (acc.assistant_response.clone(), acc.user_transcript())
    };
    if !full_text.trim().is_empty() {
        log::info!(
            "[Pipeline::Llm] LlmFinished processed (turn {}, chars {}): '{}'",
            turn_id,
            full_text.chars().count(),
            full_text
        );
        persist_assistant_turn(turn_id, full_text, user_text, state);
    } else {
        // No assistant artifact was produced. The harness discards its staged
        // query at the terminal commit; history stays untouched. No rollback.
        log::warn!(
            "[Pipeline::Llm] LlmFinished with empty response (turn {}); staged query discarded",
            turn_id
        );
    }

    evaluate_synthesis_latch(turn_id, app, state, ctx);
}

/// Marks the turn complete and resolves quiescence once its audio has drained.
fn evaluate_synthesis_latch<R: tauri::Runtime>(
    turn_id: u32,
    app: Option<&AppHandle<R>>,
    state: &AppState,
    ctx: &RoutingContext,
) {
    state.pipeline.mark_llm_finished(turn_id);

    let pending_jobs = state.pipeline.pending_tts_jobs.load(Ordering::Relaxed);
    let sink_active = state.pipeline.sink_occupied.load(Ordering::Relaxed);
    if pending_jobs > 0 || sink_active {
        // RULE-EVT-09 §3: keep the current state until the sound card drains.
        log::info!(
            "[Pipeline::Llm] LlmFinished latch marked; audio active (pending_jobs={}, sink_active={}, turn {})",
            pending_jobs,
            sink_active,
            turn_id
        );
        return;
    }

    if state.turn_metrics.playback_start_ms.load(Ordering::Relaxed) == 0 {
        let payload = state.turn_metrics.build_payload(turn_id);
        if let Some(app_handle) = app {
            let _ = emit_ipc(app_handle, IpcEvent::TurnMetrics(payload));
        }
    }

    let noop = crate::core::events::MockIpcEmitter;
    let emitter: &dyn crate::core::events::IpcEmitter = match app {
        Some(app_handle) => app_handle,
        None => &noop,
    };
    transition(InteractionState::Ready, None, ctx, emitter, state);
    log::info!(
        "[Pipeline::Llm] LlmFinished latch resolved (audio drained) -> Ready (turn {})",
        turn_id
    );
}
