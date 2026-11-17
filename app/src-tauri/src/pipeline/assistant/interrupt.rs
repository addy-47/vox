use std::sync::atomic::Ordering;

use tauri::AppHandle;

use crate::{
    core::{
        events::{InteractionMode, PipelineMode},
        state::{AppState, InteractionState},
    },
    harness::TurnPhase,
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
        // Keep the active turn token and turn_id unchanged; speech hold absorbs this utterance.
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
                // Cancel turn token for Thinking
                state.pipeline.cancel_active_turn();
                state.pipeline.pending_tts_jobs.store(0, Ordering::Relaxed);

                let has_committed = {
                    let guard = state.harness.lock();
                    guard
                        .as_ref()
                        .map(|h| h.turn_phase() == TurnPhase::Committed)
                        .unwrap_or(false)
                };

                if !has_committed {
                    // Scenario A: Step 0 Thinking with no committed artifact.
                    // The query is staged, not in history; take it for continuation.
                    let staged_query = {
                        let mut guard = state.harness.lock();
                        guard
                            .as_mut()
                            .and_then(|h| h.take_staged_query(interrupted_turn_id))
                    };
                    {
                        let mut acc = state.pipeline_accumulator.lock();
                        if let Some(text) = staged_query {
                            if acc.user_transcript.is_empty() {
                                acc.set_user_transcript(text);
                            }
                        }
                        acc.stash_for_continuation();
                        log::info!(
                        "[Pipeline::Interrupt] Scenario A: Step 0 thinking interrupt; discarded staged turn {}; stashed prefix='{}'",
                        interrupted_turn_id,
                        acc.continuation_prefix
                    );
                    }
                } else {
                    // Scenario B: Step K >= 1 Thinking (turn phase Committed)
                    // Committed tool steps remain permanently in history.
                    // Call acc.clear() (do NOT call stash_for_continuation!).
                    state.pipeline_accumulator.lock().clear();
                    log::info!(
                    "[Pipeline::Interrupt] Scenario B: Mid-turn thinking interrupt (step K >= 1); committed tools preserved, accumulator cleared"
                );
                }

                let (new_turn_id, _) = state.pipeline.next_turn();
                transition(InteractionState::Listening, None, ctx, app, state);
                state.turn_metrics.start_turn(new_turn_id);
                new_turn_id
            }
            _ => {
                // Speaking or other states: genuine barge-in
                state.pipeline.cancel_active_turn();
                state.pipeline.pending_tts_jobs.store(0, Ordering::Relaxed);

                state.pipeline_accumulator.lock().clear();

                let (new_turn_id, _) = state.pipeline.next_turn();
                transition(InteractionState::Listening, None, ctx, app, state);
                state.turn_metrics.start_turn(new_turn_id);
                new_turn_id
            }
        }
    }
}
