use std::{
    fs::OpenOptions,
    io::Write,
    time::{SystemTime, UNIX_EPOCH},
};

use tauri::{AppHandle, Manager};

use crate::{
    core::{
        events::{emit_ipc_to, IpcEvent, Severity, TranscriptPayload},
        settings::DictationOutputMode,
        state::{AppState, AppWindow, InteractionOwner, InteractionState},
    },
    pipeline::dictation::transition_dictation,
    services::{
        dictation::output_router::route_transcript,
        notifications::lifecycle::{self, LifecycleCard},
        translit::transliterate_if_hi,
    },
    utils::paths,
};

const DICTATION_HISTORY_FILENAME: &str = "dictation_history.jsonl";

/// Routes finalized transcript directly to OS input simulation without invoking LLM or TTS.
pub fn on_transcript_final<R: tauri::Runtime>(
    turn_id: u32,
    text: String,
    app: &AppHandle<R>,
    state: &AppState,
) {
    if state.pipeline.dictation_state() == InteractionState::Idle {
        log::debug!(
            "[Dictation::Transcript] Dropping transcript — dictation disabled (turn: {})",
            turn_id
        );
        let notify_app = app.clone();
        let notify_db = state.db.clone();
        tauri::async_runtime::spawn(async move {
            lifecycle::dictation_terminal(
                &notify_app,
                &notify_db,
                LifecycleCard {
                    title: "🎙️ Dictation",
                    message: "Dictation disabled",
                    severity: Severity::Info,
                    duration_ms: 2000,
                },
            )
            .await;
        });
        return;
    }

    if text.trim().is_empty() {
        log::info!(
            "[Dictation] Empty speech transcript received (turn: {}) -> notifying user",
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
        if state.pipeline.dictation_state() != InteractionState::Listening {
            transition_dictation(InteractionState::Ready, app, state);
        }
        return;
    }

    log::info!("[Dictation] Transcribed: \"{}\" (turn: {})", text, turn_id);

    let transliterate_enabled = state
        .settings
        .read()
        .unwrap_or_else(|p| p.into_inner())
        .stt
        .transliterate_enabled;
    let processed_text = transliterate_if_hi(&text, true, transliterate_enabled);

    let output_mode = state
        .settings
        .read()
        .map(|s| s.dictation.output_mode.clone())
        .unwrap_or(DictationOutputMode::Paste);

    log::info!(
        "[Dictation::Trace] Final transcript prepared: '{}' (transliterated: '{}', output_mode: {:?})",
        text,
        processed_text,
        output_mode
    );

    *state.dictation_last_transcript.lock() = Some(processed_text.clone());
    state
        .pipeline
        .transcript_history
        .lock()
        .push_back(processed_text.clone());
    append_to_cache_history(&processed_text);

    let app_handle = app.clone();
    let db_handle = state.db.clone();
    let text_clone = processed_text.clone();

    tauri::async_runtime::spawn(async move {
        log::info!(
            "[Dictation::Trace] Spawning output router task for mode: {:?}",
            output_mode
        );
        if let Err(e) = route_transcript(&app_handle, &text_clone, output_mode, &db_handle).await {
            log::warn!("[Dictation::Trace] Output routing failed: {}", e);
        }
    });

    if state.pipeline.dictation_state() != InteractionState::Listening {
        transition_dictation(InteractionState::Ready, app, state);
    }

    log::info!(
        "[Dictation::Trace] Emitting transcript_final turn={} owner=Dictation target={} tray_exists={} main_exists={} chars={}",
        turn_id,
        AppWindow::Tray,
        app.get_webview_window(AppWindow::Tray.as_str()).is_some(),
        app.get_webview_window(AppWindow::Main.as_str()).is_some(),
        processed_text.chars().count()
    );
    if let Err(e) = emit_ipc_to(
        app,
        AppWindow::Tray,
        IpcEvent::TranscriptFinal(TranscriptPayload {
            turn_id,
            text: processed_text,
            owner: Some(InteractionOwner::Dictation),
        }),
    ) {
        log::warn!(
            "[Dictation::Trace] Failed to emit transcript_final to Tray HUD: {}",
            e
        );
    }
}

/// Appends a finalized dictation transcript record into the JSONL cache.
fn append_to_cache_history(text: &str) {
    let cache_dir = paths::cache_dir();
    let file_path = cache_dir.join(DICTATION_HISTORY_FILENAME);
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let entry = serde_json::json!({
        "timestamp": now,
        "text": text,
    });

    if let Ok(json_line) = serde_json::to_string(&entry) {
        if let Ok(mut file) = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&file_path)
        {
            let _ = writeln!(file, "{}", json_line);
        }
    }
}
