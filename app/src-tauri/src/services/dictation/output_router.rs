use tauri::AppHandle;

use crate::{
    core::{error::DictationError, events::Severity, settings::DictationOutputMode},
    persistence::VoxDb,
    services::{
        dictation::{clipboard, input::create_input_adapter},
        notifications::lifecycle::{self, LifecycleCard},
    },
};

/// Routes a completed transcript to the configured OS output destination (Clipboard or OS Paste).
pub async fn route_transcript<R: tauri::Runtime>(
    app: &AppHandle<R>,
    text: &str,
    output_mode: DictationOutputMode,
    db: &VoxDb,
) -> Result<(), DictationError> {
    log::info!(
        "[Dictation::Trace] OutputRouter routing transcript ({} chars) to mode: {:?}",
        text.len(),
        output_mode
    );

    match output_mode {
        DictationOutputMode::Tray => {
            log::info!(
                "[Dictation::Trace] OutputRouter: Tray mode active; transcript rendered in HUD"
            );
            Ok(())
        }
        DictationOutputMode::Clipboard => dispatch_to_clipboard(app, db, text).await,
        DictationOutputMode::Paste => dispatch_to_paste(app, db, text).await,
    }
}

/// Sets transcript directly on system clipboard and notifies user.
async fn dispatch_to_clipboard<R: tauri::Runtime>(
    app: &AppHandle<R>,
    db: &VoxDb,
    text: &str,
) -> Result<(), DictationError> {
    log::info!(
        "[Dictation::Trace] OutputRouter: Writing {} chars to system clipboard...",
        text.len()
    );
    clipboard::set_text(text)?;
    log::info!(
        "[Dictation::Trace] OutputRouter: Successfully wrote transcript to system clipboard."
    );
    let snippet = format_paste_snippet(text);
    let msg = format!("<b>Saved to clipboard</b> (press Ctrl+V):\n\"{}\"", snippet);
    lifecycle::dictation_terminal(
        app,
        db,
        LifecycleCard {
            title: "📋 Dictation Copied",
            message: &msg,
            severity: Severity::Info,
            duration_ms: 3000,
        },
    )
    .await;
    Ok(())
}

/// Simulates OS paste into active window with fallback clipboard preservation on failure.
async fn dispatch_to_paste<R: tauri::Runtime>(
    app: &AppHandle<R>,
    db: &VoxDb,
    text: &str,
) -> Result<(), DictationError> {
    log::info!(
        "[Dictation::Trace] OutputRouter: Starting simulated paste for {} chars...",
        text.len()
    );
    let input_adapter = create_input_adapter();
    let paste_result =
        clipboard::with_clipboard_safe(text, || async { input_adapter.simulate_paste() }).await;

    match paste_result {
        Ok(()) => {
            let snippet = format_paste_snippet(text);
            log::info!(
                "[Dictation::Trace] OutputRouter: Transcript successfully pasted into focused application ('{}').",
                snippet
            );
            let msg = format!("\"{}\"", snippet);
            lifecycle::dictation_terminal(
                app,
                db,
                LifecycleCard {
                    title: "✓ Dictation Pasted",
                    message: &msg,
                    severity: Severity::Info,
                    duration_ms: 3000,
                },
            )
            .await;
            Ok(())
        }
        Err(e) => {
            let snippet = format_paste_snippet(text);
            log::warn!(
                "[Dictation::Trace] OutputRouter: Paste simulation failed ({:?}). Transcript is preserved on clipboard.",
                e
            );
            let msg = format!("<b>Saved to clipboard</b> (press Ctrl+V):\n\"{}\"", snippet);
            lifecycle::dictation_terminal(
                app,
                db,
                LifecycleCard {
                    title: "📋 Dictation Copied",
                    message: &msg,
                    severity: Severity::Warning,
                    duration_ms: 15_000,
                },
            )
            .await;
            Err(e)
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
