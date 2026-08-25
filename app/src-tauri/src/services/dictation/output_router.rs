use crate::core::error::DictationError;
use crate::core::settings::DictationOutputMode;
use crate::core::state::{AppState, InteractionOwner};
use crate::services::dictation::clipboard;
use crate::services::dictation::input::create_input_adapter;
use tauri::{AppHandle, Emitter, Manager, State};

/// Routes a completed, transliterated transcript to the configured output destination.
pub async fn route_transcript(app: &AppHandle, text: &str) -> Result<(), DictationError> {
    let state: State<'_, std::sync::Arc<AppState>> = app.state();

    *state.dictation_last_transcript.lock() = Some(text.to_string());

    let output_mode = state
        .settings
        .read()
        .map(|s| s.dictation.output_mode.clone())
        .unwrap_or(DictationOutputMode::Paste);

    log::info!(
        "[Dictation::Router] Routing transcript ({} chars) to mode: {:?}",
        text.len(),
        output_mode
    );

    match output_mode {
        DictationOutputMode::Tray => dispatch_to_tray(app, text, &state),
        DictationOutputMode::Clipboard => dispatch_to_clipboard(app, text),
        DictationOutputMode::Paste => dispatch_to_paste(app, text).await,
    }
}

/// Emits final transcript to the Tray HUD overlay window.
fn dispatch_to_tray(app: &AppHandle, text: &str, state: &AppState) -> Result<(), DictationError> {
    if let Err(e) = crate::tray::ensure_tray_window(app) {
        log::warn!("[Dictation::Router] Failed to ensure tray window: {}", e);
    }

    if let Err(e) = app.emit_to(
        "tray",
        "transcript_final",
        serde_json::json!({
            "text": text,
            "turn_id": state.pipeline.turn_id.load(std::sync::atomic::Ordering::Relaxed),
            "owner": InteractionOwner::Dictation,
        }),
    ) {
        log::warn!(
            "[Dictation::Router] Failed to emit transcript_final to Tray HUD: {}",
            e
        );
    }

    log::debug!("[Dictation::Router] Emitted final transcript to Tray HUD window.");
    Ok(())
}

/// Sets transcript directly on system clipboard and broadcasts success event.
fn dispatch_to_clipboard(app: &AppHandle, text: &str) -> Result<(), DictationError> {
    clipboard::set_text(text)?;

    if let Err(e) = app.emit(
        "dictation_success",
        serde_json::json!({
            "mode": "clipboard",
            "length": text.len()
        }),
    ) {
        log::warn!(
            "[Dictation::Router] Failed to emit dictation_success: {}",
            e
        );
    }

    log::info!("[Dictation::Router] Transcript written to system clipboard.");
    Ok(())
}

/// Simulates OS paste into active window with fallback clipboard preservation on failure.
async fn dispatch_to_paste(app: &AppHandle, text: &str) -> Result<(), DictationError> {
    let input_adapter = create_input_adapter();
    let paste_result =
        clipboard::with_clipboard_safe(text, || async { input_adapter.simulate_paste() }).await;

    match paste_result {
        Ok(()) => {
            if let Err(e) = app.emit(
                "dictation_success",
                serde_json::json!({
                    "mode": "paste",
                    "length": text.len()
                }),
            ) {
                log::warn!(
                    "[Dictation::Router] Failed to emit dictation_success: {}",
                    e
                );
            }
            log::info!(
                "[Dictation::Router] Transcript successfully pasted into focused application."
            );
            Ok(())
        }
        Err(e) => {
            log::warn!(
                "[Dictation::Router] Paste simulation failed ({:?}). Transcript is preserved on clipboard.",
                e
            );
            if let Err(emit_err) = app.emit(
                "dictation_recovery_available",
                serde_json::json!({
                    "text": text,
                    "error": format!("{}", e)
                }),
            ) {
                log::warn!(
                    "[Dictation::Router] Failed to emit dictation_recovery_available: {}",
                    emit_err
                );
            }
            Ok(())
        }
    }
}
