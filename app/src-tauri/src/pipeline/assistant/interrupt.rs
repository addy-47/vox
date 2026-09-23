use std::sync::atomic::Ordering;

use tauri::AppHandle;

use crate::{
    core::{
        settings::{InteractionMode, PipelineMode},
        state::{AppState, InteractionState},
    },
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
    state.pipeline.reset_turn_guards();

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

    // 5. Accumulator Reset: Clears TurnAccumulator for the incoming user utterance (events-spec §5 item 5)
    state.pipeline_accumulator.lock().clear();

    let (new_turn_id, _) = state.pipeline.next_turn();
    state.pipeline.cancel_flag.store(false, Ordering::Relaxed);
    transition(InteractionState::Listening, ctx, app, state);
    state.turn_metrics.start_turn(new_turn_id);

    log::info!(
        "[Pipeline::Interrupt] Interruption handled (interrupted turn: {}, new turn: {})",
        interrupted_turn_id,
        new_turn_id
    );

    new_turn_id
}
