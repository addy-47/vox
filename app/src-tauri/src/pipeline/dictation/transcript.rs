use std::io::Write;

use tauri::AppHandle;

use crate::{
    core::{
        error::PipelineImpact,
        events::{emit_ipc_to, IpcEvent, Severity, TranscriptPayload},
        settings::DictationOutputMode,
        state::{AppState, AppWindow, InteractionOwner, InteractionState},
    },
    pipeline::dictation::transition_dictation,
    services::{
        self,
        dictation::output_router::route_transcript,
        notifications::{Action, NotificationCategory, NotificationParams},
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
        return;
    }

    if text.trim().is_empty() {
        if state.pipeline.dictation_state() != InteractionState::Listening {
            transition_dictation(InteractionState::Ready, app, state);
        }

        let app_handle = app.clone();
        let db = state.db.clone();
        tauri::async_runtime::spawn(async move {
            let params = NotificationParams {
                category: NotificationCategory::Dictation,
                severity: Severity::Info,
                impact: Some(PipelineImpact::None),
                action: Action::Transient,
                title: "Dictation",
                message: "Speech detected, but no words recognized.",
                group_key: Some("dictation:empty_speech"),
                session_id: None,
                metadata: None,
                duration_ms: None,
            };
            if let Err(e) = services::notifications::notify(&app_handle, &db, params).await {
                log::warn!("[Dictation::Transcript] Failed to dispatch notification: {}", e);
            }
        });
        return;
    }

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

    *state.dictation_last_transcript.lock() = Some(processed_text.clone());
    state
        .pipeline
        .transcript_history
        .lock()
        .push_back(processed_text.clone());
    append_to_cache_history(&processed_text);

    let app_handle = app.clone();
    let text_clone = processed_text.clone();

    tauri::async_runtime::spawn(async move {
        if let Err(e) = route_transcript(&app_handle, &text_clone, output_mode).await {
            log::warn!("[Dictation::Transcript] Output routing failed: {}", e);
        }
    });

    if state.pipeline.dictation_state() != InteractionState::Listening {
        transition_dictation(InteractionState::Ready, app, state);
    }

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
            "[Dictation::Transcript] Failed to emit transcript_final: {}",
            e
        );
    }
}

/// Appends a finalized dictation transcript record into the JSONL cache.
fn append_to_cache_history(text: &str) {
    let cache_dir = paths::cache_dir();
    let file_path = cache_dir.join(DICTATION_HISTORY_FILENAME);
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let entry = serde_json::json!({
        "timestamp": now,
        "text": text,
    });

    if let Ok(json_line) = serde_json::to_string(&entry) {
        if let Ok(mut file) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&file_path)
        {
            let _ = writeln!(file, "{}", json_line);
        }
    }
}
