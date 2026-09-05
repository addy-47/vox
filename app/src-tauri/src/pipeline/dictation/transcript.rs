use tauri::AppHandle;

use crate::core::events::ToastLevel;
use crate::core::events::{emit_ipc_to, IpcEvent, TranscriptPayload};
use crate::core::state::{AppState, InteractionOwner, InteractionState};
use crate::pipeline::dictation::transition_dictation;
use crate::pipeline::WINDOW_TRAY;

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
        if let Err(e) =
            crate::toast::show_toast(app, "Dictation", "No speech recognized", ToastLevel::Info)
        {
            log::warn!("[Dictation::Transcript] Failed to show empty toast: {}", e);
        }
        return;
    }

    let transliterate_enabled = state
        .settings
        .read()
        .unwrap_or_else(|p| p.into_inner())
        .stt
        .transliterate_enabled;
    let processed_text =
        crate::services::translit::transliterate_if_hi(&text, true, transliterate_enabled);

    let output_mode = state
        .settings
        .read()
        .map(|s| s.dictation.output_mode.clone())
        .unwrap_or(crate::core::settings::DictationOutputMode::Paste);

    *state.dictation_last_transcript.lock() = Some(processed_text.clone());

    let app_handle = app.clone();
    let text_clone = processed_text.clone();

    tauri::async_runtime::spawn(async move {
        if let Err(e) = crate::services::dictation::output_router::route_transcript(
            &app_handle,
            &text_clone,
            output_mode,
        )
        .await
        {
            log::warn!("[Dictation::Transcript] Output routing failed: {}", e);
        }
    });

    if state.pipeline.dictation_state() != InteractionState::Listening {
        transition_dictation(InteractionState::Ready, app, state);
    }

    if let Err(e) = emit_ipc_to(
        app,
        WINDOW_TRAY,
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
