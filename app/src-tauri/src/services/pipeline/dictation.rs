use super::{
    transition, RoutingContext, EVENT_PIPELINE_ERROR, EVENT_PTT_STATUS, EVENT_SPEECH_END,
    EVENT_SPEECH_START, EVENT_TRANSCRIPT_FINAL, EVENT_TRANSCRIPT_PARTIAL, OWNER_DICTATION,
    STATUS_IDLE, STATUS_PROCESSING, STATUS_RECORDING, WINDOW_TRAY,
};
use crate::core::events::VoxEvent;
use crate::core::state::{AppState, InteractionState};
use parking_lot::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{AppHandle, Emitter};

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

    let buffer = DICTATION_BUFFER.lock().split_off(0);
    let turn_id = state.pipeline.turn_id.load(Ordering::Relaxed);

    if buffer.is_empty() {
        let ctx = RoutingContext::from_app_state(state);
        transition(InteractionState::Idle, &ctx, app, state);
        if let Err(e) = app.emit_to(
            WINDOW_TRAY,
            EVENT_PTT_STATUS,
            serde_json::json!({ "state": STATUS_IDLE, "owner": OWNER_DICTATION }),
        ) {
            log::warn!("[Dictation] Failed to emit ptt_status IDLE: {}", e);
        }
        return Ok(());
    }

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
        if let Err(e) = tx.send(crate::services::stt::SttCommand::Final(turn_id, buffer)) {
            log::warn!("[Dictation] Failed to dispatch Final audio to direct STT sender: {}", e);
        }
    } else if let Some(ref engine) = *state.engine.lock().await {
        if let Err(e) = engine
            .stt_tx
            .send(crate::services::stt::SttCommand::Final(turn_id, buffer))
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

    let ctx = RoutingContext::from_app_state(state);
    let app_handle = app.clone();
    let text_clone = processed_text.clone();

    tauri::async_runtime::spawn(async move {
        if let Err(e) =
            crate::services::dictation::output_router::route_transcript(&app_handle, &text_clone)
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
