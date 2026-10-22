use tauri::AppHandle;

use crate::{
    core::{error::DictationError, events::Severity},
    persistence::VoxDb,
    pipeline::dictation::DictationOutputMode,
    services::{
        dictation::{
            clipboard,
            input::{create_input_adapter, SystemInputAdapter},
        },
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
    let msg = format!("\"{}\" (press Ctrl+V to paste)", snippet);
    lifecycle::dictation_terminal(
        app,
        db,
        LifecycleCard {
            title: "Saved to Clipboard",
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
    let input_adapter = create_input_adapter();
    dispatch_to_paste_with_adapter(app, db, text, input_adapter.as_ref()).await
}

/// Simulates OS paste using the provided adapter.
pub async fn dispatch_to_paste_with_adapter<R: tauri::Runtime>(
    app: &AppHandle<R>,
    db: &VoxDb,
    text: &str,
    input_adapter: &dyn SystemInputAdapter,
) -> Result<(), DictationError> {
    log::info!(
        "[Dictation::Trace] OutputRouter: Starting simulated paste for {} chars...",
        text.len()
    );
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
                    title: "Pasted",
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
            let msg = format!("\"{}\" (press Ctrl+V to paste)", snippet);
            lifecycle::dictation_terminal(
                app,
                db,
                LifecycleCard {
                    title: "Saved to Clipboard",
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

/// Formats text paste snippet preserving full multi-line text up to 240 characters.
fn format_paste_snippet(text: &str) -> String {
    const MAX_PASTE_SNIPPET_CHARS: usize = 240;
    let chars: Vec<char> = text.chars().collect();
    if chars.len() <= MAX_PASTE_SNIPPET_CHARS {
        format!("“{}”", text)
    } else {
        let head: String = chars[..MAX_PASTE_SNIPPET_CHARS].iter().collect();
        format!("“{}…”", head)
    }
}

#[cfg(test)]
mod tests {
    use super::format_paste_snippet;

    #[test]
    fn paste_snippet_quotes_short_text() {
        assert_eq!(format_paste_snippet("hello"), "“hello”");
    }

    #[test]
    fn paste_snippet_truncates_long_text() {
        let text = "a".repeat(300);
        let snippet = format_paste_snippet(&text);
        assert!(
            snippet.starts_with(&format!("“{}…", "a".repeat(240))),
            "the snippet must keep the first 240 characters, got {} chars",
            snippet.chars().count()
        );
        assert_eq!(snippet.chars().count(), 243);
    }

    #[test]
    fn paste_snippet_keeps_text_at_the_limit() {
        let text = "b".repeat(240);
        let snippet = format_paste_snippet(&text);
        assert!(!snippet.contains('…'));
        assert_eq!(snippet.chars().count(), 242);
    }
}
