use tauri::AppHandle;

use crate::{
    core::{
        events::{InteractionMode, PipelineMode},
        state::{AppState, InteractionState},
    },
    pipeline::{transition, RoutingContext},
};

/// Executes Stage 1 of the two-stage interruption protocol (RULE-EVT-08 §1).
pub fn on_interrupt<R: tauri::Runtime>(
    app: &AppHandle<R>,
    state: &AppState,
    ctx: &RoutingContext,
) -> u32 {
    let prior_state = state.pipeline.state();
    let interrupted_turn_id = state.pipeline.peek_turn_id();

    // Stage 1a: mute the sound card. This does not latch the hardware cancel flag.
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

    // Stage 1b: pause token generation only. The turn token and hardware flag stay armed.
    state.pipeline.cancel_active_stream();

    // Stage 1c: bind the physical speech segment to a fresh utterance id.
    let utterance_id = state.pipeline.next_utterance_id();
    state.pipeline_accumulator.lock().bound_utterance_id = Some(utterance_id);
    if let Ok(guard) = state.engine.try_lock() {
        if let Some(ref engine) = *guard {
            if let Err(e) = engine
                .vad_tx
                .send(crate::services::vad::VadCommand::BindUtterance {
                    turn_id: interrupted_turn_id,
                    utterance_id,
                })
            {
                log::warn!("[Pipeline::Interrupt] Failed to bind utterance: {}", e);
            }
        }
    }

    // Stage 1d: park the cognitive loop at the speech hold barrier.
    {
        let guard = state.harness.lock();
        if let Some(ref harness) = *guard {
            harness.inbox().engage_speech_hold();
        }
    }

    transition(InteractionState::Listening, None, ctx, app, state);
    log::info!(
        "[Pipeline::Interrupt] Stage 1 acoustic pause from {:?}: turn {}, utterance {}",
        prior_state,
        interrupted_turn_id,
        utterance_id
    );
    interrupted_turn_id
}
