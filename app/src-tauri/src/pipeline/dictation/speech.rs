use tauri::AppHandle;

use crate::core::state::{AppState, InteractionState};
use crate::pipeline::dictation::transition_dictation;

/// Handles user speech onset for background passive dictation.
pub fn on_speech_start<R: tauri::Runtime>(app: &AppHandle<R>, state: &AppState) {
    let current = state.pipeline.dictation_state();
    match current {
        InteractionState::Idle => return,
        InteractionState::Listening => return,
        InteractionState::Thinking => {
            // Pipelined overlap: allocate turn N+1 without aborting turn N STT
        }
        InteractionState::Ready => {}
        _ => return,
    }

    let (turn_id, _token) = state.pipeline.next_turn();
    transition_dictation(InteractionState::Listening, app, state);
    log::info!("[Dictation::Speech] Passive speech started (turn: {})", turn_id);
}

/// Handles user speech completion for background passive dictation.
pub fn on_speech_end<R: tauri::Runtime>(app: &AppHandle<R>, state: &AppState) {
    if state.pipeline.dictation_state() != InteractionState::Listening {
        return;
    }

    let turn_id = state.pipeline.peek_turn_id();
    transition_dictation(InteractionState::Thinking, app, state);
    log::info!("[Dictation::Speech] Passive speech ended -> Thinking (turn: {})", turn_id);
}
