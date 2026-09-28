use std::{
    sync::{atomic::Ordering, mpsc},
    time::Duration,
};

use tauri::AppHandle;

use crate::{
    core::{
        error::{PipelineError, PipelineImpact},
        events::{InteractionOwner, Severity},
        state::{AppState, InteractionState},
    },
    pipeline::dictation::{error, transition_dictation},
    services::{
        dictation::DICTATION_SILENCE_AUTOSTOP_MS,
        notifications::lifecycle::{self, LifecycleCard},
        stt::SttCommand,
        vad::{VadCommand, VAD_VALIDATION_TIMEOUT_MS},
    },
};

/// Starts Push-To-Talk dictation recording on hotkey press.
pub fn on_ptt_start<R: tauri::Runtime>(app: &AppHandle<R>, state: &AppState) {
    let assistant_state = state.pipeline.state();
    if matches!(
        assistant_state,
        InteractionState::Listening
            | InteractionState::Thinking
            | InteractionState::Speaking
            | InteractionState::Working
    ) {
        log::info!(
            "[Dictation] Hotkey pressed while Assistant engaged ({:?}); dropping",
            assistant_state
        );
        let notify_app = app.clone();
        let notify_db = state.db.clone();
        tauri::async_runtime::spawn(async move {
            lifecycle::dictation_terminal(
                &notify_app,
                &notify_db,
                LifecycleCard {
                    title: "🎙️ Dictation",
                    message: "Assistant is currently active",
                    severity: Severity::Info,
                    duration_ms: 2000,
                },
            )
            .await;
        });
        return;
    }

    let current = state.pipeline.dictation_state();
    log::debug!(
        "[Dictation::Trace] on_ptt_start invoked (current state: {:?})",
        current
    );
    match current {
        InteractionState::Idle => {
            log::warn!("[Dictation::Trace] Dictation disabled in settings; aborting PTT");
            error::on_error(
                PipelineError {
                    turn_id: 0,
                    message: "Dictation is disabled in Settings.".to_string(),
                    source: "DictationPtt".to_string(),
                    impact: PipelineImpact::TurnAborted,
                },
                app,
                state,
            );
            return;
        }
        InteractionState::Listening => {
            log::debug!("[Dictation::Trace] Already Listening; ignoring duplicate PttStart");
            return;
        }
        InteractionState::Thinking => {
            log::debug!("[Dictation] Already Thinking (transcribing previous speech); ignoring overlapping PttStart");
            let notify_app = app.clone();
            let notify_db = state.db.clone();
            tauri::async_runtime::spawn(async move {
                lifecycle::dictation_terminal(
                    &notify_app,
                    &notify_db,
                    LifecycleCard {
                        title: "🎙️ Dictation",
                        message: "Previous turn transcribing",
                        severity: Severity::Info,
                        duration_ms: 1500,
                    },
                )
                .await;
            });
            return;
        }
        InteractionState::Ready => {}
        _ => {
            log::warn!(
                "[Dictation::Trace] State {:?} does not accept PttStart; aborting",
                current
            );
            return;
        }
    }

    let (turn_id, _token) = state.pipeline.next_turn();
    state.pipeline.cancel_flag.store(false, Ordering::Relaxed);

    let auto_stop_ms = state
        .settings
        .read()
        .map(|s| s.dictation.silence_auto_stop_ms)
        .unwrap_or(DICTATION_SILENCE_AUTOSTOP_MS);
    let auto_stop_silence_ms = if auto_stop_ms > 0 {
        Some(auto_stop_ms)
    } else {
        None
    };

    if let Ok(guard) = state.engine.try_lock() {
        if let Some(ref engine) = *guard {
            if let Err(e) = engine.vad_tx.send(VadCommand::StartWindowValidation {
                auto_stop_silence_ms,
                stream_partials: true,
                owner: InteractionOwner::Dictation,
            }) {
                log::warn!(
                    "[Dictation::Trace] Failed to start window validation: {}",
                    e
                );
            } else {
                log::info!(
                    "[Dictation::Trace] VAD window validation started for turn: {}",
                    turn_id
                );
            }
        }
    }

    transition_dictation(InteractionState::Listening, app, state);
    log::info!(
        "[Dictation::Trace] PTT recording started (turn: {}) -> dictation state: Listening",
        turn_id
    );
}

/// Finalizes Push-To-Talk dictation recording on hotkey release and dispatches to STT.
pub fn on_ptt_stop<R: tauri::Runtime>(app: &AppHandle<R>, state: &AppState) {
    on_ptt_stop_with_sender(app, state, None);
}

/// Finalizes Push-To-Talk dictation recording with optional direct STT command sender override for testing.
pub fn on_ptt_stop_with_sender<R: tauri::Runtime>(
    app: &AppHandle<R>,
    state: &AppState,
    stt_tx: Option<&mpsc::Sender<SttCommand>>,
) {
    if state.pipeline.dictation_state() != InteractionState::Listening {
        log::debug!(
            "[Dictation::Trace] PttStop dropped: state is not Listening ({:?})",
            state.pipeline.dictation_state()
        );
        return;
    }

    let turn_id = state.pipeline.peek_turn_id();
    log::info!("[Dictation::Trace] on_ptt_stop invoked (turn: {})", turn_id);

    let (vad_tx_opt, engine_stt_tx_opt) = match state.engine.try_lock() {
        Ok(guard) => (
            guard.as_ref().map(|e| e.vad_tx.clone()),
            guard.as_ref().map(|e| e.stt_tx.clone()),
        ),
        Err(_) => {
            log::warn!(
                "[Dictation::Trace] Engine lock contended; could not access vad_tx / stt_tx"
            );
            (None, None)
        }
    };

    let validation_result = if let Some(vad_tx) = vad_tx_opt {
        let (tx, rx) = mpsc::channel();
        if vad_tx
            .send(VadCommand::StopWindowValidation { response_tx: tx })
            .is_ok()
        {
            rx.recv_timeout(Duration::from_millis(VAD_VALIDATION_TIMEOUT_MS))
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

    log::debug!(
        "[Dictation] VAD validation complete (turn: {}, is_speech: {}, audio_samples: {})",
        turn_id,
        is_speech,
        audio.len()
    );

    if !is_speech || audio.len() < 1600 {
        log::info!(
            "[Dictation] Non-speech/short audio (<100ms) discarded (turn: {}) -> notifying user and returning to Ready",
            turn_id
        );
        let notify_app = app.clone();
        let notify_db = state.db.clone();
        tauri::async_runtime::spawn(async move {
            lifecycle::dictation_terminal(
                &notify_app,
                &notify_db,
                LifecycleCard {
                    title: "⚠️ Dictation: No Speech",
                    message: "<b>No speech recognized</b>\nSpeak clearly into the microphone",
                    severity: Severity::Warning,
                    duration_ms: 3000,
                },
            )
            .await;
        });
        transition_dictation(InteractionState::Ready, app, state);
        return;
    }

    log::info!(
        "[Dictation] Valid speech captured (turn: {}, {:.2}s) -> transcribing...",
        turn_id,
        audio.len() as f32 / 16000.0
    );

    let notify_app = app.clone();
    let notify_db = state.db.clone();
    tauri::async_runtime::spawn(async move {
        lifecycle::dictation_transcribing(&notify_app, &notify_db).await;
    });

    transition_dictation(InteractionState::Thinking, app, state);

    if let Some(tx) = stt_tx {
        if let Err(e) = tx.send(SttCommand::Final {
            turn_id,
            audio,
            owner: InteractionOwner::Dictation,
        }) {
            log::warn!(
                "[Dictation::Trace] Failed to dispatch Final audio to direct STT sender: {}",
                e
            );
        } else {
            log::info!(
                "[Dictation::Trace] Dispatched final audio to direct STT sender (turn: {})",
                turn_id
            );
        }
    } else if let Some(stt_tx) = engine_stt_tx_opt {
        if let Err(e) = stt_tx.send(SttCommand::Final {
            turn_id,
            audio,
            owner: InteractionOwner::Dictation,
        }) {
            log::warn!(
                "[Dictation::Trace] Failed to dispatch Final audio to STT: {}",
                e
            );
        } else {
            log::info!(
                "[Dictation::Trace] Dispatched final audio to STT engine (turn: {})",
                turn_id
            );
        }
    }

    log::info!(
        "[Dictation::Trace] Hotkey recording finalized (turn: {}) -> dictation state: Thinking",
        turn_id
    );
}

/// Cancels in-flight PTT dictation recording and discards audio.
pub fn on_ptt_cancel<R: tauri::Runtime>(app: &AppHandle<R>, state: &AppState) {
    if state.pipeline.dictation_state() != InteractionState::Listening {
        return;
    }

    log::info!("[Dictation::Trace] on_ptt_cancel invoked");
    if let Ok(guard) = state.engine.try_lock() {
        if let Some(ref engine) = *guard {
            let (resp_tx, _) = mpsc::channel();
            if let Err(e) = engine.vad_tx.send(VadCommand::StopWindowValidation {
                response_tx: resp_tx,
            }) {
                log::warn!(
                    "[Dictation::Trace] Failed to send StopWindowValidation: {}",
                    e
                );
            }
        }
    }

    transition_dictation(InteractionState::Ready, app, state);
    let notify_app = app.clone();
    let notify_db = state.db.clone();
    tauri::async_runtime::spawn(async move {
        lifecycle::dictation_terminal(
            &notify_app,
            &notify_db,
            LifecycleCard {
                title: "🎙️ Dictation",
                message: "Dictation cancelled",
                severity: Severity::Info,
                duration_ms: 1500,
            },
        )
        .await;
    });
    log::info!("[Dictation::Trace] PTT cancelled -> dictation state: Ready");
}
