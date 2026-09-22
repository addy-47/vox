use std::sync::atomic::Ordering;

use tauri::AppHandle;

use crate::{
    core::{
        settings::PipelineMode,
        state::{AppState, InteractionState},
    },
    persistence::PersistenceEvent,
    pipeline::{transition, RoutingContext},
};

/// Commits finalized assistant turn to persistence event queue.
fn persist_assistant_turn(turn_id: u32, full_text: String, user_text: String, state: &AppState) {
    let conv_id = state.conversation_id.load(Ordering::Relaxed);

    let persist_lock = state.persist_tx.lock();
    if let Some(ref tx) = *persist_lock {
        if let Err(e) = tx.try_send(PersistenceEvent::TurnCompleted {
            session_id: conv_id as i64,
            turn_id,
            user_text,
            assistant_text: full_text,
        }) {
            log::warn!(
                "[Pipeline::Llm] Failed to send TurnCompleted to persistence: {}",
                e
            );
        }
    }
}

/// Flushes audio pre-roll buffer in the playback engine based on pipeline mode.
fn flush_pre_roll(state: &AppState, mode: &PipelineMode) {
    if *mode == PipelineMode::Realtime {
        state
            .pipeline
            .pending_synthesis_jobs
            .store(0, Ordering::Relaxed);
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

    state.pipeline.clear_turn_open();
    flush_pre_roll(state, &ctx.pipeline_mode);

    let (full_text, user_text) = {
        let mut acc = state.pipeline_accumulator.lock();
        (acc.take_assistant_response(), acc.user_transcript())
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
        log::warn!(
            "[Pipeline::Llm] LlmFinished with empty response (turn {}); rolling back staged user turn",
            turn_id
        );
        let mut guard = state.harness.lock();
        if let Some(ref mut harness) = *guard {
            harness.history.rollback_last_user_turn();
        }
    }

    evaluate_synthesis_latch(turn_id, current_state, app, state, ctx);
}

/// Evaluates whether synthesis has drained and pipeline can transition to Ready.
fn evaluate_synthesis_latch<R: tauri::Runtime>(
    turn_id: u32,
    current_state: InteractionState,
    app: Option<&AppHandle<R>>,
    state: &AppState,
    ctx: &RoutingContext,
) {
    let drained = state.pipeline.is_drained_while_open();
    let pending_jobs = state.pipeline.pending_synthesis_jobs.load(Ordering::Relaxed);
    let should_transition = pending_jobs == 0
        && (drained || current_state != InteractionState::Speaking);

    if should_transition {
        state.pipeline.clear_drained_while_open();
        if let Some(app_handle) = app {
            transition(InteractionState::Ready, ctx, app_handle, state);
        } else {
            state.pipeline.set_state(InteractionState::Ready);
        }
        log::info!(
            "[Pipeline::Llm] LlmFinished evaluated latch (drained={}, state={:?}) -> Ready (turn {})",
            drained,
            current_state,
            turn_id
        );
    }
}
