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

    let is_speech_held = state
        .harness
        .lock()
        .as_ref()
        .map(|h| h.inbox().is_speech_held())
        .unwrap_or(false);

    if is_speech_held || prior_state == InteractionState::Working {
        // Scenario C (Working / Speech-Held Turn):
        // Mute playback & clear AEC render queue (done above).
        // Do NOT cancel turn_token, keep turn_open = true, keep turn_id unchanged.
        // Engage speech hold on harness inbox.
        {
            let guard = state.harness.lock();
            if let Some(ref harness) = *guard {
                harness.inbox().engage_speech_hold();
            }
        }
        state.pipeline_accumulator.lock().clear();
        transition(InteractionState::Listening, None, ctx, app, state);
        log::info!(
            "[Pipeline::Interrupt] Scenario C: Working turn {} speech hold engaged -> Listening",
            interrupted_turn_id
        );
        interrupted_turn_id
    } else {
        match prior_state {
        InteractionState::Thinking => {
            // Cancel turn token & guards for Thinking
            state.pipeline.cancel_flag.store(true, Ordering::SeqCst);
            state.pipeline.turn_token().cancel();
            state
                .pipeline
                .pending_synthesis_jobs
                .store(0, Ordering::Relaxed);
            state.pipeline.reset_turn_guards();

            let has_committed = {
                let guard = state.harness.lock();
                guard
                    .as_ref()
                    .map(|h| h.has_committed_steps_this_turn())
                    .unwrap_or(false)
            };

            if !has_committed {
                // Scenario A: Step 0 Thinking (!has_committed_steps_this_turn)
                // Synchronously rollback user turn from history:
                let rolled_back_msg = {
                    let mut guard = state.harness.lock();
                    guard.as_mut().and_then(|h| h.rollback_user_turn(interrupted_turn_id))
                };
                {
                    let mut acc = state.pipeline_accumulator.lock();
                    if let Some(msg) = rolled_back_msg {
                        if acc.user_transcript.is_empty() {
                            acc.set_user_transcript(msg.content);
                        }
                    }
                    acc.stash_for_continuation();
                    log::info!(
                        "[Pipeline::Interrupt] Scenario A: Step 0 thinking interrupt; rolled back turn {}; stashed prefix='{}'",
                        interrupted_turn_id,
                        acc.continuation_prefix
                    );
                }
            } else {
                // Scenario B: Step K >= 1 Thinking (has_committed_steps_this_turn == true)
                // Committed tool steps remain permanently in history.
                // Call acc.clear() (do NOT call stash_for_continuation!).
                state.pipeline_accumulator.lock().clear();
                log::info!(
                    "[Pipeline::Interrupt] Scenario B: Mid-turn thinking interrupt (step K >= 1); committed tools preserved, accumulator cleared"
                );
            }

            let (new_turn_id, _) = state.pipeline.next_turn();
            state.pipeline.cancel_flag.store(false, Ordering::Relaxed);
            transition(InteractionState::Listening, None, ctx, app, state);
            state.turn_metrics.start_turn(new_turn_id);
            new_turn_id
        }
        _ => {
            // Speaking or other states: genuine barge-in
            state.pipeline.cancel_flag.store(true, Ordering::SeqCst);
            state.pipeline.turn_token().cancel();
            state
                .pipeline
                .pending_synthesis_jobs
                .store(0, Ordering::Relaxed);
            state.pipeline.reset_turn_guards();

            state.pipeline_accumulator.lock().clear();

            let (new_turn_id, _) = state.pipeline.next_turn();
            state.pipeline.cancel_flag.store(false, Ordering::Relaxed);
            transition(InteractionState::Listening, None, ctx, app, state);
            state.turn_metrics.start_turn(new_turn_id);
            new_turn_id
        }
    }
    }
}
