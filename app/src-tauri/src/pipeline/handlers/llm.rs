//! Canonical LLM generation completed event handler.

use std::sync::atomic::Ordering;

use crate::core::settings::PipelineMode;
use crate::core::state::{AppState, InteractionState};
use crate::pipeline::RoutingContext;
use crate::services::tts::actor::TtsCommand;

/// Commits finalized assistant turn to conversation memory and dispatches persistence event.
fn persist_assistant_turn(turn_id: u32, full_text: String, user_text: String, state: &AppState) {
    state
        .conversation_manager
        .lock()
        .push_assistant_turn(full_text.clone());

    let conv_id = state.conversation_id.load(Ordering::Relaxed);
    let stt_latency_ms = state.telemetry.latest_stt_ms.load(Ordering::Relaxed);
    let ttft_ms = state.telemetry.latest_ttft_ms.load(Ordering::Relaxed);

    let persist_lock = state.persist_tx.lock();
    if let Some(ref tx) = *persist_lock {
        if let Err(e) = tx.try_send(
            crate::persistence::events::PersistenceEvent::TurnCompleted {
                conversation_id: conv_id,
                turn_id,
                user_text,
                assistant_text: full_text,
                stt_latency_ms,
                ttft_ms,
            },
        ) {
            log::warn!(
                "[Pipeline::Llm] Failed to send TurnCompleted to persistence: {}",
                e
            );
        }
    }
}

/// Flushes remaining unpunctuated text from clause chunker to TTS worker.
fn flush_modular_tts_remainder(
    turn_id: u32,
    state: &AppState,
    tts_tx: Option<&std::sync::mpsc::Sender<TtsCommand>>,
) {
    let remainder = state.pipeline_accumulator.lock().flush_chunker();
    if let Some(remainder_text) = remainder {
        if let Some(tx) = tts_tx {
            state
                .pipeline
                .pending_synthesis_jobs
                .fetch_add(1, Ordering::Relaxed);
            if let Err(e) = tx.send(TtsCommand::Generate {
                turn_id,
                text: remainder_text,
            }) {
                state
                    .pipeline
                    .pending_synthesis_jobs
                    .fetch_sub(1, Ordering::Relaxed);
                log::warn!(
                    "[Pipeline::Llm] Failed to send remainder Generate to TTS: {}",
                    e
                );
            }
        }
    }
}

/// Finalizes LLM output generation, flushes remaining TTS clause, flushes audio pre-roll, and commits turn.
pub fn on_llm_finished(turn_id: u32, state: &AppState, ctx: &RoutingContext) {
    let current_state = state.pipeline.state();
    if current_state != InteractionState::Thinking && current_state != InteractionState::Speaking {
        log::debug!(
            "[Pipeline::Llm] LlmFinished dropped: state is {:?}, expected Thinking or Speaking",
            current_state
        );
        return;
    }

    if ctx.pipeline_mode == PipelineMode::Modular {
        if let Ok(guard) = state.engine.try_lock() {
            if let Some(ref engine) = *guard {
                flush_modular_tts_remainder(turn_id, state, engine.tts_tx.as_ref());
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
        persist_assistant_turn(turn_id, full_text, user_text, state);
    }

    log::info!("[Pipeline::Llm] LlmFinished processed (turn: {})", turn_id);
}
