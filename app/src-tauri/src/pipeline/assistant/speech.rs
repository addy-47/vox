use tauri::AppHandle;

use crate::{
    core::{
        events::InteractionMode,
        state::{AppState, InteractionState},
    },
    pipeline::{assistant::interrupt::on_interrupt, transition, RoutingContext},
};

/// Handles user speech onset for passive domains, evaluating barge-in vs continuation vs direct onset.
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

    // Every physical speech onset allocates a monotonic utterance sequence ID,
    // even if conversational turn_id remains held in Scenario C (Working state).
    let active_utterance_id = state.pipeline.next_utterance_id();

    let active_turn_id = if current_state == InteractionState::Thinking
        || current_state == InteractionState::Speaking
        || current_state == InteractionState::Working
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

        transition(InteractionState::Listening, None, ctx, app, state);
        state.turn_metrics.start_turn(new_turn_id);
        new_turn_id
    } else {
        return;
    };

    log::info!(
        "[Pipeline::Speech] Speech start processed (turn: {}, utterance: {})",
        active_turn_id,
        active_utterance_id
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

    state.turn_metrics.record_speech_end();
    transition(InteractionState::Thinking, None, ctx, app, state);

    log::info!(
        "[Pipeline::Speech] Speech end processed (turn: {})",
        state.pipeline.peek_turn_id()
    );
}
