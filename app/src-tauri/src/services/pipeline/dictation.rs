use super::{
    transition, RoutingContext, EVENT_PIPELINE_ERROR, EVENT_PTT_STATUS, EVENT_SPEECH_END,
    EVENT_SPEECH_START, EVENT_TRANSCRIPT_FINAL, EVENT_TRANSCRIPT_PARTIAL, OWNER_DICTATION,
    STATUS_IDLE, STATUS_PROCESSING, STATUS_RECORDING, WINDOW_TRAY,
};
use crate::core::events::VoxEvent;
use crate::core::state::{AppState, InteractionState};
use parking_lot::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{AppHandle, Emitter, Manager};

static IS_RECORDING: AtomicBool = AtomicBool::new(false);
static DICTATION_BUFFER: Mutex<Vec<f32>> = Mutex::new(Vec::new());

/// Ingests audio samples into the dictation Push-To-Talk buffer when recording is active.
pub fn ingest_audio(chunk: &[f32]) {
    if IS_RECORDING.load(Ordering::Relaxed) {
        DICTATION_BUFFER.lock().extend_from_slice(chunk);
    }
}

/// Returns true if dictation Push-To-Talk audio recording is currently active.
pub fn is_recording() -> bool {
    IS_RECORDING.load(Ordering::Relaxed)
}

/// Returns the current sample count in the dictation Push-To-Talk buffer.
pub fn get_buffer_len() -> usize {
    DICTATION_BUFFER.lock().len()
}

/// Starts Push-To-Talk dictation recording on hotkey press.
pub async fn handle_hotkey_press<R: tauri::Runtime>(app: &AppHandle<R>, state: &AppState) -> Result<(), String> {
    if IS_RECORDING.swap(true, Ordering::SeqCst) {
        return Ok(());
    }

    DICTATION_BUFFER.lock().clear();
    state.pipeline.cancel_flag.store(false, Ordering::Relaxed);

    if let Some(ref engine) = *state.engine.lock().await {
        let _ = engine.vad_tx.send(crate::core::state::VadCommand::StartWindowValidation);
    }

    let turn_id = state.pipeline.turn_id.fetch_add(1, Ordering::Relaxed) + 1;
    let ctx = RoutingContext::from_app_state(state);
    transition(InteractionState::Listening, &ctx, app, state);

    if let Err(e) = app.emit_to(
        WINDOW_TRAY,
        EVENT_PTT_STATUS,
        serde_json::json!({
            "state": STATUS_RECORDING,
            "turn_id": turn_id,
            "owner": OWNER_DICTATION,
        }),
    ) {
        log::warn!("[Dictation] Failed to emit ptt_status RECORDING: {}", e);
    }

    log::info!("[Dictation] Hotkey recording started (Turn: {})", turn_id);
    Ok(())
}

/// Finalizes Push-To-Talk dictation recording with optional direct STT command sender override for testing.
pub async fn handle_hotkey_release_with_sender<R: tauri::Runtime>(
    app: &AppHandle<R>,
    state: &AppState,
    stt_tx: Option<&std::sync::mpsc::Sender<crate::services::stt::SttCommand>>,
) -> Result<(), String> {
    if !IS_RECORDING.swap(false, Ordering::SeqCst) {
        return Ok(());
    }

    let raw_buffer = DICTATION_BUFFER.lock().split_off(0);
    let turn_id = state.pipeline.turn_id.load(Ordering::Relaxed);

    if raw_buffer.is_empty() {
        let ctx = RoutingContext::from_app_state(state);
        transition(InteractionState::Idle, &ctx, app, state);
        if let Err(e) = app.emit_to(
            WINDOW_TRAY,
            EVENT_PTT_STATUS,
            serde_json::json!({
                "state": STATUS_IDLE,
                "turn_id": turn_id,
                "owner": OWNER_DICTATION,
            }),
        ) {
            log::warn!("[Dictation] Failed to emit ptt_status IDLE: {}", e);
        }
        return Ok(());
    }

    let engine_guard = state.engine.lock().await;
    let validation_result = if let Some(ref engine) = *engine_guard {
        let (tx, rx) = tokio::sync::oneshot::channel();
        if engine.vad_tx.send(crate::core::state::VadCommand::StopWindowValidation { response_tx: tx }).is_ok() {
            tokio::time::timeout(std::time::Duration::from_millis(100), rx)
                .await
                .ok()
                .and_then(|res| res.ok())
        } else {
            None
        }
    } else {
        None
    };

    let buffer_to_send = match validation_result {
        Some(ref val) if !val.is_speech_detected => {
            log::info!("[Dictation] Non-speech hotkey hold discarded (Turn: {})", turn_id);
            let ctx = RoutingContext::from_app_state(state);
            transition(InteractionState::Idle, &ctx, app, state);
            if let Err(e) = app.emit_to(
                WINDOW_TRAY,
                EVENT_PTT_STATUS,
                serde_json::json!({
                    "state": STATUS_IDLE,
                    "turn_id": turn_id,
                    "owner": OWNER_DICTATION,
                }),
            ) {
                log::warn!("[Dictation] Failed to emit ptt_status IDLE: {}", e);
            }
            return Ok(());
        }
        Some(ref val) => {
            let start = val.speech_start_sample.min(raw_buffer.len());
            let end = val.speech_end_sample.min(raw_buffer.len());
            if start < end && (end - start) >= 256 {
                raw_buffer[start..end].to_vec()
            } else {
                raw_buffer
            }
        }
        None => raw_buffer,
    };

    let ctx = RoutingContext::from_app_state(state);
    transition(InteractionState::Thinking, &ctx, app, state);

    if let Err(e) = app.emit_to(
        WINDOW_TRAY,
        EVENT_PTT_STATUS,
        serde_json::json!({
            "state": STATUS_PROCESSING,
            "turn_id": turn_id,
            "owner": OWNER_DICTATION,
        }),
    ) {
        log::warn!("[Dictation] Failed to emit ptt_status PROCESSING: {}", e);
    }

    if let Some(tx) = stt_tx {
        if let Err(e) = tx.send(crate::services::stt::SttCommand::Final(turn_id, buffer_to_send)) {
            log::warn!("[Dictation] Failed to dispatch Final audio to direct STT sender: {}", e);
        }
    } else if let Some(ref engine) = *engine_guard {
        if let Err(e) = engine
            .stt_tx
            .send(crate::services::stt::SttCommand::Final(turn_id, buffer_to_send))
        {
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
    let ctx = RoutingContext::from_app_state(state);
    transition(InteractionState::Listening, &ctx, app, state);

    if let Err(e) = app.emit_to(
        WINDOW_TRAY,
        EVENT_SPEECH_START,
        serde_json::json!({
            "turn_id": turn_id,
            "owner": OWNER_DICTATION,
        }),
    ) {
        log::warn!("[Dictation] Failed to emit speech_start: {}", e);
    }
}

/// Handles user speech completion for background passive dictation.
fn on_speech_end<R: tauri::Runtime>(turn_id: u32, app: &AppHandle<R>, state: &AppState) {
    let ctx = RoutingContext::from_app_state(state);
    transition(InteractionState::Thinking, &ctx, app, state);

    if let Err(e) = app.emit_to(
        WINDOW_TRAY,
        EVENT_SPEECH_END,
        serde_json::json!({
            "turn_id": turn_id,
            "owner": OWNER_DICTATION,
        }),
    ) {
        log::warn!("[Dictation] Failed to emit speech_end: {}", e);
    }
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
            "owner": OWNER_DICTATION,
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

    let ctx = RoutingContext::from_app_state(state);
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

    transition(InteractionState::Idle, &ctx, app, state);

    if let Err(e) = app.emit_to(
        WINDOW_TRAY,
        EVENT_TRANSCRIPT_FINAL,
        serde_json::json!({
            "turn_id": turn_id,
            "text": processed_text,
            "owner": OWNER_DICTATION,
        }),
    ) {
        log::warn!("[Dictation] Failed to emit transcript_final: {}", e);
    }
}

/// Logs dictation errors and updates tray state.
fn on_error<R: tauri::Runtime>(turn_id: u32, message: String, app: &AppHandle<R>, state: &AppState) {
    log::error!("[Dictation] Error on turn {}: {}", turn_id, message);
    let ctx = RoutingContext::from_app_state(state);
    transition(InteractionState::Error, &ctx, app, state);

    if let Err(e) = app.emit_to(
        WINDOW_TRAY,
        EVENT_PIPELINE_ERROR,
        serde_json::json!({
            "turn_id": turn_id,
            "message": message,
            "owner": OWNER_DICTATION,
        }),
    ) {
        log::warn!("[Dictation] Failed to emit pipeline_error: {}", e);
    }
}

/// Handles cancellation event and resets state machine to Ready.
fn on_cancelled<R: tauri::Runtime>(turn_id: u32, app: &AppHandle<R>, state: &AppState) {
    log::info!("[Dictation] Interaction cancelled on turn {}", turn_id);
    let ctx = RoutingContext::from_app_state(state);
    transition(InteractionState::Ready, &ctx, app, state);
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

