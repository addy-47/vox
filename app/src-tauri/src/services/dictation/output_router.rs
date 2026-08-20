//! ============================================================================
//! src/services/dictation/output_router.rs — Output Destination Dispatcher
//! ============================================================================

use crate::core::error::DictationError;
use crate::core::settings::DictationOutputMode;
use crate::core::state::{AppState, InteractionOwner};
use crate::services::dictation::clipboard;
use crate::services::dictation::input::create_input_adapter;
use tauri::{AppHandle, Emitter, Manager, State};

/// Routes a completed, transliterated transcript to the configured output destination.
pub async fn route_transcript(app: &AppHandle, text: &str) -> Result<(), DictationError> {
    let state: State<'_, std::sync::Arc<AppState>> = app.state();

    // 1. Always cache the latest transcript for recovery queries (FR-08)
    *state.dictation_last_transcript.lock() = Some(text.to_string());

    // 2. Resolve configured output mode
    let output_mode = {
        let s = state.settings.read().unwrap();
        s.dictation.output_mode.clone()
    };

    log::info!(
        "[Dictation::Router] Routing transcript ({} chars) to mode: {:?}",
        text.len(),
        output_mode
    );

    match output_mode {
        DictationOutputMode::Tray => {
            // Deliver to Tray HUD overlay window (ensure webview exists)
            let _ = crate::tray::ensure_tray_window(app);
            let _ = app.emit_to(
                "tray",
                "transcript_final",
                serde_json::json!({
                    "text": text,
                    "turn_id": state.pipeline.turn_id.load(std::sync::atomic::Ordering::Relaxed),
                    "owner": InteractionOwner::Dictation,
                }),
            );
            log::debug!("[Dictation::Router] Emitted final transcript to Tray HUD window.");
            Ok(())
        }

        DictationOutputMode::Clipboard => {
            // Write directly to OS clipboard
            clipboard::set_text(text)?;
            let _ = app.emit(
                "dictation_success",
                serde_json::json!({
                    "mode": "clipboard",
                    "length": text.len()
                }),
            );
            log::info!("[Dictation::Router] Transcript written to system clipboard.");
            Ok(())
        }

        DictationOutputMode::Paste => {
            let input_adapter = create_input_adapter();

            // Safely write to clipboard, simulate OS paste, and restore prior clipboard if successful
            let paste_result = clipboard::with_clipboard_safe(text, || async {
                input_adapter.simulate_paste()
            })
            .await;

            match paste_result {
                Ok(()) => {
                    let _ = app.emit(
                        "dictation_success",
                        serde_json::json!({
                            "mode": "paste",
                            "length": text.len()
                        }),
                    );
                    log::info!("[Dictation::Router] Transcript successfully pasted into focused application.");
                    Ok(())
                }
                Err(e) => {
                    log::warn!(
                        "[Dictation::Router] Paste simulation failed ({:?}). Transcript is preserved on clipboard.",
                        e
                    );
                    let _ = app.emit(
                        "dictation_recovery_available",
                        serde_json::json!({
                            "text": text,
                            "error": format!("{}", e)
                        }),
                    );
                    // Do not bubble up as an uncaught crash — the transcript is safely preserved on clipboard
                    Ok(())
                }
            }
        }
    }
}
