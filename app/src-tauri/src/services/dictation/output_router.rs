use std::sync::Arc;

use tauri::{AppHandle, Manager};

use crate::{
    core::{
        error::DictationError, events::Severity, settings::DictationOutputMode, state::AppState,
    },
    services::{
        dictation::{clipboard, input::create_input_adapter},
        notifications::{notify, Action, NotificationCategory, NotificationParams},
    },
};

/// Routes a completed transcript to the configured OS output destination (Clipboard or OS Paste).
pub async fn route_transcript<R: tauri::Runtime>(
    app: &AppHandle<R>,
    text: &str,
    output_mode: DictationOutputMode,
) -> Result<(), DictationError> {
    log::info!(
        "[Dictation::Router] Routing transcript ({} chars) to mode: {:?}",
        text.len(),
        output_mode
    );

    match output_mode {
        DictationOutputMode::Tray => {
            log::debug!("[Dictation::Router] Tray mode active; OS text injection bypassed");
            Ok(())
        }
        DictationOutputMode::Clipboard => dispatch_to_clipboard(text),
        DictationOutputMode::Paste => dispatch_to_paste(app, text).await,
    }
}

/// Sets transcript directly on system clipboard.
/// Per spec §8.1, manual clipboard copies produce no toast overlay.
fn dispatch_to_clipboard(text: &str) -> Result<(), DictationError> {
    clipboard::set_text(text)?;
    log::info!("[Dictation::Router] Transcript written to system clipboard.");
    Ok(())
}

/// Simulates OS paste into active window with fallback clipboard preservation on failure.
async fn dispatch_to_paste<R: tauri::Runtime>(
    app: &AppHandle<R>,
    text: &str,
) -> Result<(), DictationError> {
    let input_adapter = create_input_adapter();
    let paste_result =
        clipboard::with_clipboard_safe(text, || async { input_adapter.simulate_paste() }).await;

    let app_state = app.try_state::<Arc<AppState>>();

    match paste_result {
        Ok(()) => {
            log::info!(
                "[Dictation::Router] Transcript successfully pasted into focused application."
            );
            let snippet = format_paste_snippet(text);
            if let Some(state) = app_state {
                if let Err(e) = notify(
                    app,
                    &state.db,
                    NotificationParams {
                        group_key: Some("dictation:paste_success"),
                        category: NotificationCategory::Dictation,
                        severity: Severity::Info,
                        impact: None,
                        action: Action::Transient,
                        title: "Dictation Pasted",
                        message: &snippet,
                        session_id: None,
                        metadata: None,
                        duration_ms: None,
                    },
                )
                .await
                {
                    log::warn!("[Dictation::Router] Failed to emit paste toast: {}", e);
                }
            }
            Ok(())
        }
        Err(e) => {
            log::warn!(
                "[Dictation::Router] Paste simulation failed ({:?}). Transcript is preserved on clipboard.",
                e
            );
            if let Some(state) = app_state {
                if let Err(e) = notify(
                    app,
                    &state.db,
                    NotificationParams {
                        group_key: Some("dictation:paste_blocked"),
                        category: NotificationCategory::Dictation,
                        severity: Severity::Warning,
                        impact: None,
                        action: Action::Transient,
                        title: "Paste Blocked by OS",
                        message: "Transcript saved to clipboard — paste manually with Ctrl+V.",
                        session_id: None,
                        metadata: None,
                        duration_ms: None,
                    },
                )
                .await
                {
                    log::warn!(
                        "[Dictation::Router] Failed to emit paste-blocked toast: {}",
                        e
                    );
                }
            }
            Ok(())
        }
    }
}

/// Formats text paste snippet: “<start> ... <end>” (up to 40 characters).
fn format_paste_snippet(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    if chars.len() <= 40 {
        format!("“{}”", text)
    } else {
        let prefix: String = chars[..18].iter().collect();
        let suffix: String = chars[chars.len() - 18..].iter().collect();
        format!("“{} ... {}”", prefix, suffix)
    }
}
