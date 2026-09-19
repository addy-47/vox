use std::sync::atomic::Ordering;

use crate::{
    core::{
        settings::PipelineMode,
        state::{AppState, InteractionState},
    },
    persistence::PersistenceEvent,
    pipeline::RoutingContext,
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

/// Finalizes LLM output generation, flushes audio pre-roll, and persists turn.
pub fn on_llm_finished(turn_id: u32, state: &AppState, ctx: &RoutingContext) {
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

    if ctx.pipeline_mode == PipelineMode::Modular {
        if let Ok(guard) = state.engine.try_lock() {
            if let Some(ref engine) = *guard {
                engine.playback_engine.flush_pre_roll();
            }
        }
    } else if ctx.pipeline_mode == PipelineMode::Realtime {
        state
            .pipeline
            .pending_synthesis_jobs
            .store(0, Ordering::Relaxed);

        if let Ok(guard) = state.engine.try_lock() {
            if let Some(ref engine) = *guard {
                engine.playback_engine.flush_pre_roll();
            }
        }
    }

    let (full_text, user_text) = {
        let mut acc = state.pipeline_accumulator.lock();
        (acc.take_assistant_response(), acc.user_transcript())
    };
    if !full_text.trim().is_empty() {
        log::info!(
            "[Pipeline::Llm] LlmFinished processed (turn {}, response_chars {}, response_words {}): '{}'",
            turn_id,
            full_text.chars().count(),
            full_text.split_whitespace().count(),
            full_text
        );
        persist_assistant_turn(turn_id, full_text, user_text, state);
    } else {
        log::info!(
            "[Pipeline::Llm] LlmFinished processed (turn {}): empty response",
            turn_id
        );
    }
}
