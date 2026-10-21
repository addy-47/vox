use std::sync::atomic::Ordering;

use tauri::AppHandle;

use crate::{
    core::{
        events::{InteractionMode, PipelineMode},
        state::{AppState, InteractionState},
    },
    pipeline::{transition, RoutingContext},
};

/// Executes the canonical barge-in or mid-thought continuation sequence when speech onset
/// occurs during Thinking, Working, or Speaking.
pub fn on_interrupt<R: tauri::Runtime>(
    app: &AppHandle<R>,
    state: &AppState,
    ctx: &RoutingContext,
) -> u32 {
    let prior_state = state.pipeline.state();

    if let Ok(guard) = state.engine.try_lock() {
        if let Some(ref engine) = *guard {
            engine.playback_engine.cancel();
            if let Some(ref aec_tx) = engine.aec_tx {
                let _ = aec_tx.send(crate::services::audio::aec::AecCommand::ClearRenderQueue);
            }
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

    // Distinguish between:
    // 1. Mid-thought continuation:
    //    - `Working`: tool execution or compaction is in flight before final response audio.
    //    - `Thinking`: LLM is actively thinking (`is_turn_open() == true`), OR STT decoding is
    //      still in flight before LLM dispatch (`!is_turn_open()` with 0 pending synthesis jobs).
    //      If the LLM has already finished (`!is_turn_open()` with pending synthesis jobs),
    //      the turn has already been answered and speech is a barge-in, NOT a continuation.
    // 2. Genuine barge-in (`Speaking` while assistant response is playing, or post-LLM synthesis):
    //    clear the accumulator for a brand-new user turn.
    let is_mid_thought = match prior_state {
        InteractionState::Working => true,
        InteractionState::Thinking => {
            let pending_jobs = state
                .pipeline
                .pending_synthesis_jobs
                .load(Ordering::Relaxed);
            state.pipeline.is_turn_open() || pending_jobs == 0
        }
        _ => false,
    };

    {
        let mut acc = state.pipeline_accumulator.lock();
        if is_mid_thought {
            acc.stash_for_continuation();
            log::info!(
                "[Pipeline::Interrupt] Mid-thought continuation detected in {:?} (turn {}); stashed prefix_len={} awaiting_stt={}",
                prior_state,
                interrupted_turn_id,
                acc.continuation_prefix.len(),
                acc.awaiting_continuation
            );
        } else {
            acc.clear();
        }
    }

    let (new_turn_id, _) = state.pipeline.next_turn();
    state.pipeline.cancel_flag.store(false, Ordering::Relaxed);
    transition(InteractionState::Listening, None, ctx, app, state);
    state.turn_metrics.start_turn(new_turn_id);

    log::info!(
        "[Pipeline::Interrupt] Interruption handled (prior_state: {:?}, interrupted turn: {}, new turn: {})",
        prior_state,
        interrupted_turn_id,
        new_turn_id
    );

    new_turn_id
}
