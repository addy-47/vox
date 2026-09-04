//! Canonical speech boundary event handlers for passive interaction domains.

use tauri::AppHandle;

use crate::core::settings::{InteractionMode, PipelineMode};
use crate::core::state::{AppState, InteractionState};
use crate::pipeline::assistant::interrupt::on_interrupt;
use crate::pipeline::{transition, RoutingContext};
use crate::services::stt::actor::SttCommand;

/// Handles user speech onset for passive domains, evaluating barge-in vs direct onset and resetting STT stream.
pub fn on_speech_start<R: tauri::Runtime>(
    app: &AppHandle<R>,
    state: &AppState,
    ctx: &RoutingContext,
) {
    if ctx.interaction_mode == InteractionMode::PTT {
        log::debug!("[Pipeline::Speech] SpeechStart ignored in PTT mode");
        return;
    }

    let current_state = state.pipeline.state();
    if current_state == InteractionState::Idle || current_state == InteractionState::Paused {
        log::debug!(
            "[Pipeline::Speech] SpeechStart dropped in {:?} state",
            current_state
        );
        return;
    }

    let active_turn_id = if current_state == InteractionState::Thinking
        || current_state == InteractionState::Speaking
    {
        on_interrupt(app, state, ctx)
    } else if current_state == InteractionState::Ready {
        let (new_turn_id, _) = state.pipeline.next_turn();
        state.pipeline_accumulator.lock().clear();

        if let Ok(guard) = state.engine.try_lock() {
            if let Some(ref engine) = *guard {
                engine.playback_engine.cancel();
            }
        }

        transition(InteractionState::Listening, ctx, app, state);
        new_turn_id
    } else {
        return;
    };

    if ctx.pipeline_mode == PipelineMode::Modular {
        if let Ok(guard) = state.engine.try_lock() {
            if let Some(ref engine) = *guard {
                if let Err(e) = engine.stt_tx.send(SttCommand::ResetStream) {
                    log::warn!(
                        "[Pipeline::Speech] Failed to send ResetStream to STT: {}",
                        e
                    );
                }
            }
        }
    }

    log::info!(
        "[Pipeline::Speech] Speech start processed (turn: {})",
        active_turn_id
    );
}

/// Handles user speech completion boundary for passive domains, transitioning to Thinking.
pub fn on_speech_end<R: tauri::Runtime>(
    app: &AppHandle<R>,
    state: &AppState,
    ctx: &RoutingContext,
) {
    if ctx.interaction_mode == InteractionMode::PTT {
        log::debug!("[Pipeline::Speech] SpeechEnd ignored in PTT mode");
        return;
    }

    if state.pipeline.state() != InteractionState::Listening {
        log::debug!(
            "[Pipeline::Speech] SpeechEnd dropped: state is {:?}, expected Listening",
            state.pipeline.state()
        );
        return;
    }

    transition(InteractionState::Thinking, ctx, app, state);

    log::info!(
        "[Pipeline::Speech] Speech end processed (turn: {})",
        state.pipeline.peek_turn_id()
    );
}
