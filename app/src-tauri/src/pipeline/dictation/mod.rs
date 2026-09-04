pub mod error;
pub mod ptt;
pub mod speech;
pub mod transcript;

use tauri::AppHandle;

use crate::core::events::{emit_ipc_to, IpcEvent, StateChangedPayload, VoxEvent};
use crate::core::state::{AppState, InteractionOwner, InteractionState};
use crate::pipeline::WINDOW_TRAY;

/// Helper function to atomically transition dictation state and emit StateChanged to WINDOW_TRAY.
pub fn transition_dictation<R: tauri::Runtime>(
    new_state: InteractionState,
    app: &AppHandle<R>,
    state: &AppState,
) {
    if state.pipeline.dictation_state() == new_state {
        return;
    }

    state.pipeline.set_dictation_state(new_state);
    let turn_id = state.pipeline.peek_turn_id();
    let state_str = match new_state {
        InteractionState::Idle => "Idle",
        InteractionState::Ready => "Ready",
        InteractionState::Listening => "Listening",
        InteractionState::Thinking => "Thinking",
        InteractionState::Error => "Error",
        _ => {
            log::warn!("[Dictation] Invalid transition target: {:?}", new_state);
            return;
        }
    };
    let payload = StateChangedPayload {
        owner: InteractionOwner::Dictation,
        state: state_str.to_string(),
        turn_id,
    };
    if let Err(e) = emit_ipc_to(app, WINDOW_TRAY, IpcEvent::StateChanged(payload)) {
        log::warn!("[Dictation] Failed to emit state_changed: {}", e);
    }
}

/// Main event dispatcher for the unified dictation domain.
pub fn handle_event<R: tauri::Runtime>(app: &AppHandle<R>, state: &AppState, event: VoxEvent) {
    match event {
        VoxEvent::SpeechStart => speech::on_speech_start(app, state),
        VoxEvent::SpeechEnd => speech::on_speech_end(app, state),
        VoxEvent::PttStart => ptt::on_ptt_start(app, state),
        VoxEvent::PttStop => ptt::on_ptt_stop(app, state),
        VoxEvent::PttCancel => ptt::on_ptt_cancel(app, state),
        VoxEvent::TranscriptFinal { turn_id, text } => {
            transcript::on_transcript_final(turn_id, text, app, state)
        }
        VoxEvent::Cancelled { turn_id } => error::on_cancelled(turn_id, app, state),
        VoxEvent::Error {
            turn_id, message, ..
        } => error::on_error(turn_id, message, app, state),
        _ => {}
    }
}
