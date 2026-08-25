use super::{transition, RoutingContext};
use crate::core::events::VoxEvent;
use crate::core::state::{AppState, InteractionState};
use parking_lot::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{AppHandle, Emitter};

static IS_RECORDING: AtomicBool = AtomicBool::new(false);
static DICTATION_BUFFER: Mutex<Vec<f32>> = Mutex::new(Vec::new());

/// Starts Push-To-Talk dictation recording on hotkey press.
pub async fn handle_hotkey_press(app: &AppHandle, state: &AppState) -> Result<(), String> {
    if IS_RECORDING.swap(true, Ordering::SeqCst) {
        return Ok(());
    }

    DICTATION_BUFFER.lock().clear();
    state.pipeline.cancel_flag.store(false, Ordering::Relaxed);
    let turn_id = state.pipeline.turn_id.fetch_add(1, Ordering::Relaxed) + 1;

    let ctx = RoutingContext::from_app_state(state);
    transition(InteractionState::Listening, &ctx, app, state);

    if let Err(e) = app.emit_to(
        "tray",
        "ptt_status",
        serde_json::json!({
            "state": "RECORDING",
            "turn_id": turn_id,
            "owner": "dictation",
        }),
    ) {
        log::warn!("[Dictation] Failed to emit ptt_status RECORDING: {}", e);
    }

    log::info!("[Dictation] Hotkey recording started (Turn: {})", turn_id);
    Ok(())
}

/// Finalizes Push-To-Talk dictation recording on hotkey release and dispatches to STT.
pub async fn handle_hotkey_release(app: &AppHandle, state: &AppState) -> Result<(), String> {
    if !IS_RECORDING.swap(false, Ordering::SeqCst) {
        return Ok(());
    }

    let buffer = DICTATION_BUFFER.lock().split_off(0);
    let turn_id = state.pipeline.turn_id.load(Ordering::Relaxed);

    if buffer.is_empty() {
        let ctx = RoutingContext::from_app_state(state);
        transition(InteractionState::Idle, &ctx, app, state);
        let _ = app.emit_to(
            "tray",
            "ptt_status",
            serde_json::json!({ "state": "IDLE", "owner": "dictation" }),
        );
        return Ok(());
    }

    let ctx = RoutingContext::from_app_state(state);
    transition(InteractionState::Thinking, &ctx, app, state);

    if let Err(e) = app.emit_to(
        "tray",
        "ptt_status",
        serde_json::json!({
            "state": "PROCESSING",
            "turn_id": turn_id,
            "owner": "dictation",
        }),
    ) {
        log::warn!("[Dictation] Failed to emit ptt_status PROCESSING: {}", e);
    }

    if let Some(ref engine) = *state.engine.lock().await {
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

/// Handles user speech onset for background passive dictation.
fn on_speech_start(turn_id: u32, app: &AppHandle, state: &AppState) {
    let ctx = RoutingContext::from_app_state(state);
    transition(InteractionState::Listening, &ctx, app, state);

    if let Err(e) = app.emit_to(
        "tray",
        "speech_start",
        serde_json::json!({
            "turn_id": turn_id,
            "owner": "dictation",
        }),
    ) {
        log::warn!("[Dictation] Failed to emit speech_start: {}", e);
    }
}

/// Handles user speech completion for background passive dictation.
fn on_speech_end(turn_id: u32, app: &AppHandle, state: &AppState) {
    let ctx = RoutingContext::from_app_state(state);
    transition(InteractionState::Thinking, &ctx, app, state);

    if let Err(e) = app.emit_to(
        "tray",
        "speech_end",
        serde_json::json!({
            "turn_id": turn_id,
            "owner": "dictation",
        }),
    ) {
        log::warn!("[Dictation] Failed to emit speech_end: {}", e);
    }
}

/// Handles interim partial speech recognition results for dictation.
fn on_transcript_partial(turn_id: u32, text: String, app: &AppHandle) {
    if let Err(e) = app.emit_to(
        "tray",
        "transcript_partial",
        serde_json::json!({
            "turn_id": turn_id,
            "text": text,
            "owner": "dictation",
        }),
    ) {
        log::warn!("[Dictation] Failed to emit transcript_partial: {}", e);
    }
}

/// Routes finalized transcript directly to OS input simulation without invoking LLM or TTS.
fn on_transcript_final(turn_id: u32, text: String, app: &AppHandle, state: &AppState) {
    let ctx = RoutingContext::from_app_state(state);
    let app_handle = app.clone();
    let text_clone = text.clone();

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
        "tray",
        "transcript_final",
        serde_json::json!({
            "turn_id": turn_id,
            "text": text,
            "owner": "dictation",
        }),
    ) {
        log::warn!("[Dictation] Failed to emit transcript_final: {}", e);
    }
}

/// Logs dictation errors and updates tray state.
fn on_error(turn_id: u32, message: String, app: &AppHandle, state: &AppState) {
    log::error!("[Dictation] Error on turn {}: {}", turn_id, message);
    let ctx = RoutingContext::from_app_state(state);
    transition(InteractionState::Error, &ctx, app, state);

    if let Err(e) = app.emit_to(
        "tray",
        "pipeline_error",
        serde_json::json!({
            "turn_id": turn_id,
            "message": message,
            "owner": "dictation",
        }),
    ) {
        log::warn!("[Dictation] Failed to emit pipeline_error: {}", e);
    }
}

/// Main event dispatcher for the unified dictation domain.
pub fn handle_event(app: &AppHandle, state: &AppState, event: VoxEvent) {
    match event {
        VoxEvent::SpeechStart { turn_id } => on_speech_start(turn_id, app, state),
        VoxEvent::SpeechEnd { turn_id, .. } => on_speech_end(turn_id, app, state),
        VoxEvent::TranscriptPartial { turn_id, text } => on_transcript_partial(turn_id, text, app),
        VoxEvent::TranscriptFinal { turn_id, text } => {
            on_transcript_final(turn_id, text, app, state)
        }
        VoxEvent::Error { turn_id, message } => on_error(turn_id, message, app, state),
        _ => {}
    }
}
