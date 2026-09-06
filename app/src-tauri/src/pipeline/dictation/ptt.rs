use std::sync::{atomic::Ordering, mpsc};

use tauri::AppHandle;

use crate::{
    core::{
        events::{Actionability, PipelineError, PipelineImpact},
        state::{AppState, InteractionState},
    },
    pipeline::dictation::transition_dictation,
};

/// Starts Push-To-Talk dictation recording on hotkey press.
pub fn on_ptt_start<R: tauri::Runtime>(app: &AppHandle<R>, state: &AppState) {
    let current = state.pipeline.dictation_state();
    match current {
        InteractionState::Idle => {
            crate::pipeline::dictation::error::on_error(
                PipelineError {
                    turn_id: 0,
                    message: "Dictation is disabled in Settings.".to_string(),
                    source: "DictationPtt".to_string(),
                    impact: PipelineImpact::TurnAborted,
                    actionability: Actionability::Actionable {
                        category: "dictation_disabled".to_string(),
                        hint: "Enable dictation in Settings to use the Push-To-Talk hotkey."
                            .to_string(),
                    },
                },
                app,
                state,
            );
            return;
        }
        InteractionState::Listening => return,
        InteractionState::Thinking => {
            // Pipelined overlap: start turn N+1 non-destructively while turn N is still transcribing.
        }
        InteractionState::Ready => {}
        _ => return,
    }

    let (turn_id, _token) = state.pipeline.next_turn();
    state.pipeline.cancel_flag.store(false, Ordering::Relaxed);

    if let Ok(guard) = state.engine.try_lock() {
        if let Some(ref engine) = *guard {
            if let Err(e) = engine
                .vad_tx
                .send(crate::services::vad::VadCommand::StartWindowValidation)
            {
                log::warn!("[Dictation::PTT] Failed to start window validation: {}", e);
            }
        }
    }

    transition_dictation(InteractionState::Listening, app, state);
    log::info!("[Dictation::PTT] PTT recording started (turn: {})", turn_id);
}

/// Finalizes Push-To-Talk dictation recording on hotkey release and dispatches to STT.
pub fn on_ptt_stop<R: tauri::Runtime>(app: &AppHandle<R>, state: &AppState) {
    on_ptt_stop_with_sender(app, state, None);
}

/// Finalizes Push-To-Talk dictation recording with optional direct STT command sender override for testing.
pub fn on_ptt_stop_with_sender<R: tauri::Runtime>(
    app: &AppHandle<R>,
    state: &AppState,
    stt_tx: Option<&mpsc::Sender<crate::services::stt::SttCommand>>,
) {
    if state.pipeline.dictation_state() != InteractionState::Listening {
        log::debug!("[Dictation::PTT] PttStop dropped: state is not Listening");
        return;
    }

    let turn_id = state.pipeline.peek_turn_id();

    let (vad_tx_opt, engine_stt_tx_opt) = match state.engine.try_lock() {
        Ok(guard) => (
            guard.as_ref().map(|e| e.vad_tx.clone()),
            guard.as_ref().map(|e| e.stt_tx.clone()),
        ),
        Err(_) => {
            log::warn!("[Dictation::PTT] Engine lock contended; could not access vad_tx / stt_tx");
            (None, None)
        }
    };

    let validation_result = if let Some(vad_tx) = vad_tx_opt {
        let (tx, rx) = mpsc::channel();
        if vad_tx
            .send(crate::services::vad::VadCommand::StopWindowValidation { response_tx: tx })
            .is_ok()
        {
            rx.recv_timeout(std::time::Duration::from_millis(
                crate::services::vad::VAD_VALIDATION_TIMEOUT_MS,
            ))
            .ok()
        } else {
            None
        }
    } else {
        None
    };

    let (is_speech, audio) = match validation_result {
        Some(val) => (val.is_speech_detected, val.audio),
        None => (false, Vec::new()),
    };

    if !is_speech || audio.is_empty() {
        log::info!(
            "[Dictation::PTT] Non-speech hotkey hold discarded (turn: {})",
            turn_id
        );
        transition_dictation(InteractionState::Ready, app, state);
        return;
    }

    transition_dictation(InteractionState::Thinking, app, state);

    if let Some(tx) = stt_tx {
        if let Err(e) = tx.send(crate::services::stt::SttCommand::Final(turn_id, audio)) {
            log::warn!(
                "[Dictation::PTT] Failed to dispatch Final audio to direct STT sender: {}",
                e
            );
        }
    } else if let Some(stt_tx) = engine_stt_tx_opt {
        if let Err(e) = stt_tx.send(crate::services::stt::SttCommand::Final(turn_id, audio)) {
            log::warn!(
                "[Dictation::PTT] Failed to dispatch Final audio to STT: {}",
                e
            );
        }
    }

    log::info!(
        "[Dictation::PTT] Hotkey recording finalized (turn: {})",
        turn_id
    );
}

/// Cancels in-flight PTT dictation recording and discards audio.
pub fn on_ptt_cancel<R: tauri::Runtime>(app: &AppHandle<R>, state: &AppState) {
    if state.pipeline.dictation_state() != InteractionState::Listening {
        return;
    }

    if let Ok(guard) = state.engine.try_lock() {
        if let Some(ref engine) = *guard {
            let (resp_tx, _) = mpsc::channel();
            let _ = engine
                .vad_tx
                .send(crate::services::vad::VadCommand::StopWindowValidation {
                    response_tx: resp_tx,
                });
        }
    }

    transition_dictation(InteractionState::Ready, app, state);
    log::info!("[Dictation::PTT] PTT cancelled");
}
