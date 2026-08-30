use super::{
    EVENT_DICTATION_STATE_CHANGED, EVENT_PIPELINE_ERROR, EVENT_TRANSCRIPT_FINAL,
    EVENT_TRANSCRIPT_PARTIAL, WINDOW_TRAY,
};
use crate::core::events::VoxEvent;
use crate::core::state::{AppState, DictationState};
use std::sync::atomic::Ordering;
use tauri::{AppHandle, Emitter, Manager};

fn emit_dictation_state<R: tauri::Runtime>(
    app: &AppHandle<R>,
    state: DictationState,
    turn_id: u32,
) {
    let payload = match state {
        DictationState::Recording => "RECORDING",
        DictationState::Transcribing => "PROCESSING",
        DictationState::Idle => "IDLE",
        DictationState::Error => "ERROR",
    };
    if let Err(e) = app.emit_to(
        WINDOW_TRAY,
        EVENT_DICTATION_STATE_CHANGED,
        serde_json::json!({
            "state": payload,
            "turn_id": turn_id,
            "owner": "Dictation",
        }),
    ) {
        log::warn!("[Dictation] Failed to emit dictation_state_changed: {}", e);
    }
}

/// Starts Push-To-Talk dictation recording on hotkey press.
pub async fn handle_hotkey_press<R: tauri::Runtime>(app: &AppHandle<R>, state: &AppState) -> Result<(), String> {
    if state.pipeline.dictation_state() == DictationState::Recording {
        return Ok(());
    }

    state.pipeline.cancel_flag.store(false, Ordering::Relaxed);

    if let Some(ref engine) = *state.engine.lock().await {
        if let Err(e) = engine.vad_tx.send(crate::core::state::VadCommand::StartWindowValidation) {
            log::warn!("[Dictation] Failed to send StartWindowValidation to VAD: {}", e);
        }
    }

    let turn_id = state.pipeline.next_turn_id();
    state.pipeline.set_dictation_state(DictationState::Recording);
    emit_dictation_state(app, DictationState::Recording, turn_id);

    log::info!("[Dictation] Hotkey recording started (Turn: {})", turn_id);
    Ok(())
}

/// Finalizes Push-To-Talk dictation recording with optional direct STT command sender override for testing.
pub async fn handle_hotkey_release_with_sender<R: tauri::Runtime>(
    app: &AppHandle<R>,
    state: &AppState,
    stt_tx: Option<&std::sync::mpsc::Sender<crate::services::stt::SttCommand>>,
) -> Result<(), String> {
    if state.pipeline.dictation_state() != DictationState::Recording {
        return Ok(());
    }

    let turn_id = state.pipeline.peek_turn_id();

    let engine_guard = state.engine.lock().await;
    let validation_result = if let Some(ref engine) = *engine_guard {
        let (tx, rx) = std::sync::mpsc::channel();
        if engine.vad_tx.send(crate::core::state::VadCommand::StopWindowValidation { response_tx: tx }).is_ok() {
            rx.recv_timeout(std::time::Duration::from_millis(500)).ok()
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
        log::info!("[Dictation] Non-speech hotkey hold discarded (Turn: {})", turn_id);
        state.pipeline.set_dictation_state(DictationState::Idle);
        emit_dictation_state(app, DictationState::Idle, turn_id);
        return Ok(());
    }

    state.pipeline.set_dictation_state(DictationState::Transcribing);
    emit_dictation_state(app, DictationState::Transcribing, turn_id);

    if let Some(tx) = stt_tx {
        if let Err(e) = tx.send(crate::services::stt::SttCommand::Final(turn_id, audio)) {
            log::warn!("[Dictation] Failed to dispatch Final audio to direct STT sender: {}", e);
        }
    } else if let Some(ref engine) = *engine_guard {
        if let Err(e) = engine.stt_tx.send(crate::services::stt::SttCommand::Final(turn_id, audio)) {
            log::warn!("[Dictation] Failed to dispatch Final audio to STT: {}", e);
        }
    }

    log::info!("[Dictation] Hotkey recording finalized (Turn: {})", turn_id);
    Ok(())
}

/// Finalizes Push-To-Talk dictation recording on hotkey release and dispatches to STT.
pub async fn handle_hotkey_release<R: tauri::Runtime>(app: &AppHandle<R>, state: &AppState) -> Result<(), String> {
    handle_hotkey_release_with_sender(app, state, None).await
}

/// Handles user speech onset for background passive dictation.
fn on_speech_start<R: tauri::Runtime>(turn_id: u32, app: &AppHandle<R>, state: &AppState) {
    state.pipeline.set_dictation_state(DictationState::Recording);
    emit_dictation_state(app, DictationState::Recording, turn_id);
}

/// Handles user speech completion for background passive dictation.
fn on_speech_end<R: tauri::Runtime>(turn_id: u32, app: &AppHandle<R>, state: &AppState) {
    state.pipeline.set_dictation_state(DictationState::Transcribing);
    emit_dictation_state(app, DictationState::Transcribing, turn_id);
}

/// Handles interim partial speech recognition results for dictation.
fn on_transcript_partial<R: tauri::Runtime>(turn_id: u32, text: String, app: &AppHandle<R>, state: &AppState) {
    let transliterate_enabled = state.settings.read().unwrap_or_else(|p| p.into_inner()).stt.transliterate_enabled;
    let processed_text = crate::services::translit::transliterate_if_hi(&text, false, transliterate_enabled);

    if let Err(e) = app.emit_to(
        WINDOW_TRAY,
        EVENT_TRANSCRIPT_PARTIAL,
        serde_json::json!({
            "turn_id": turn_id,
            "text": processed_text,
            "owner": "Dictation",
        }),
    ) {
        log::warn!("[Dictation] Failed to emit transcript_partial: {}", e);
    }
}

/// Routes finalized transcript directly to OS input simulation without invoking LLM or TTS.
fn on_transcript_final<R: tauri::Runtime>(turn_id: u32, text: String, app: &AppHandle<R>, state: &AppState) {
    let transliterate_enabled = state.settings.read().unwrap_or_else(|p| p.into_inner()).stt.transliterate_enabled;
    let processed_text = crate::services::translit::transliterate_if_hi(&text, true, transliterate_enabled);

    let output_mode = state
        .settings
        .read()
        .map(|s| s.dictation.output_mode.clone())
        .unwrap_or(crate::core::settings::DictationOutputMode::Paste);

    *state.dictation_last_transcript.lock() = Some(processed_text.clone());

    let app_handle = app.clone();
    let text_clone = processed_text.clone();

    tauri::async_runtime::spawn(async move {
        if let Err(e) =
            crate::services::dictation::output_router::route_transcript(&app_handle, &text_clone, output_mode)
                .await
        {
            log::warn!("[Dictation] Output routing failed: {}", e);
        }
    });

    state.pipeline.set_dictation_state(DictationState::Idle);
    emit_dictation_state(app, DictationState::Idle, turn_id);

    if let Err(e) = app.emit_to(
        WINDOW_TRAY,
        EVENT_TRANSCRIPT_FINAL,
        serde_json::json!({
            "turn_id": turn_id,
            "text": processed_text,
            "owner": "Dictation",
        }),
    ) {
        log::warn!("[Dictation] Failed to emit transcript_final: {}", e);
    }
}

/// Logs dictation errors and updates tray state.
fn on_error<R: tauri::Runtime>(turn_id: u32, message: String, app: &AppHandle<R>, state: &AppState) {
    log::error!("[Dictation] Error on turn {}: {}", turn_id, message);
    state.pipeline.set_dictation_state(DictationState::Error);
    emit_dictation_state(app, DictationState::Error, turn_id);

    if let Err(e) = app.emit_to(
        WINDOW_TRAY,
        EVENT_PIPELINE_ERROR,
        serde_json::json!({
            "turn_id": turn_id,
            "message": message,
            "owner": "Dictation",
        }),
    ) {
        log::warn!("[Dictation] Failed to emit pipeline_error: {}", e);
    }
}

/// Handles cancellation event and resets state machine to Ready.
fn on_cancelled<R: tauri::Runtime>(turn_id: u32, app: &AppHandle<R>, state: &AppState) {
    log::info!("[Dictation] Interaction cancelled on turn {}", turn_id);
    state.pipeline.set_dictation_state(DictationState::Idle);
    emit_dictation_state(app, DictationState::Idle, turn_id);
}

/// Main event dispatcher for the unified dictation domain.
pub fn handle_event<R: tauri::Runtime>(app: &AppHandle<R>, state: &AppState, event: VoxEvent) {
    match event {
        VoxEvent::SpeechStart { turn_id } => on_speech_start(turn_id, app, state),
        VoxEvent::SpeechEnd { turn_id, .. } => on_speech_end(turn_id, app, state),
        VoxEvent::TranscriptPartial { turn_id, text } => {
            on_transcript_partial(turn_id, text, app, state)
        }
        VoxEvent::TranscriptFinal { turn_id, text } => {
            on_transcript_final(turn_id, text, app, state)
        }
        VoxEvent::Cancelled { turn_id } => on_cancelled(turn_id, app, state),
        VoxEvent::Error { turn_id, message } => on_error(turn_id, message, app, state),
        _ => {}
    }
}

/// Spawns the async dictation hotkey listener loop and registers global shortcut with OS.
pub fn init_dictation_hotkey_listener(
    app: &AppHandle,
    shortcut_str: &str,
) -> Result<(), crate::core::error::DictationError> {
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<crate::services::dictation::hotkey::HotkeyAction>();
    let app_handle = app.clone();

    tauri::async_runtime::spawn(async move {
        while let Some(action) = rx.recv().await {
            let state: tauri::State<'_, std::sync::Arc<AppState>> = app_handle.state();
            match action {
                crate::services::dictation::hotkey::HotkeyAction::Press => {
                    if let Err(e) = handle_hotkey_press(&app_handle, &state).await {
                        log::error!("[Dictation::Pipeline] Error in handle_press: {}", e);
                    }
                }
                crate::services::dictation::hotkey::HotkeyAction::Release => {
                    if let Err(e) = handle_hotkey_release(&app_handle, &state).await {
                        log::error!("[Dictation::Pipeline] Error in handle_release: {}", e);
                    }
                }
            }
        }
    });

    crate::services::dictation::hotkey::register_global_hotkey(app, shortcut_str, tx)
}

