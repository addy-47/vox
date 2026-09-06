use std::sync::atomic::Ordering;

use tauri::AppHandle;

use crate::{
    core::{
        settings::{InteractionMode, PipelineMode},
        state::{AppState, InteractionState},
    },
    persistence::events::PersistenceEvent,
    pipeline::{transition, RoutingContext},
};

/// Executes the 6-step canonical barge-in sequence when an interruption occurs during Thinking or Speaking.
pub fn on_interrupt<R: tauri::Runtime>(
    app: &AppHandle<R>,
    state: &AppState,
    ctx: &RoutingContext,
) -> u32 {
    if let Ok(guard) = state.engine.try_lock() {
        if let Some(ref engine) = *guard {
            engine.playback_engine.cancel();
        }
    }

    state.pipeline.cancel_flag.store(true, Ordering::SeqCst);
    state.pipeline.turn_token().cancel();
    state
        .pipeline
        .pending_synthesis_jobs
        .store(0, Ordering::Relaxed);

    let signal_provider_interrupt = matches!(
        (&ctx.pipeline_mode, &ctx.interaction_mode),
        (PipelineMode::Realtime, InteractionMode::PTT)
    );

    if signal_provider_interrupt {
        if let Ok(rt_guard) = state.realtime_engine.try_lock() {
            if let Some(ref rt_actor) = *rt_guard {
                if let Err(e) = rt_actor.signal_interrupt() {
                    log::warn!(
                        "[Pipeline::Interrupt] Error signaling interrupt to realtime actor: {}",
                        e
                    );
                }
            }
        }
    }

    let interrupted_turn_id = state.pipeline.peek_turn_id();
    let (partial_assistant, user_text) = {
        let mut acc = state.pipeline_accumulator.lock();
        (acc.take_assistant_response(), acc.user_transcript())
    };

    if !partial_assistant.trim().is_empty() {
        state
            .conversation_manager
            .lock()
            .push_assistant_turn(partial_assistant.clone());
    }

    let conv_id = state.conversation_id.load(Ordering::Relaxed);
    let persist_lock = state.persist_tx.lock();
    if let Some(ref tx) = *persist_lock {
        if let Err(e) = tx.try_send(PersistenceEvent::TurnCompleted {
            conversation_id: conv_id,
            turn_id: interrupted_turn_id,
            user_text,
            assistant_text: partial_assistant,
            stt_latency_ms: 0,
            ttft_ms: 0,
        }) {
            log::warn!(
                "[Pipeline::Interrupt] Failed to send TurnCompleted on interrupt: {}",
                e
            );
        }
    }

    state.pipeline_accumulator.lock().clear();

    let (new_turn_id, _) = state.pipeline.next_turn();
    state.pipeline.cancel_flag.store(false, Ordering::Relaxed);
    transition(InteractionState::Listening, ctx, app, state);

    log::info!(
        "[Pipeline::Interrupt] Interruption handled (interrupted turn: {}, new turn: {})",
        interrupted_turn_id,
        new_turn_id
    );

    new_turn_id
}
