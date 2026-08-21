//! ============================================================================
//! src/services/dictation/clipboard.rs — Cross-Platform Safe Clipboard Manager
//! ============================================================================

use crate::core::error::DictationError;
use arboard::Clipboard;

/// Retrieve the current text from the system clipboard.
pub fn get_text() -> Result<String, DictationError> {
    let mut clipboard = Clipboard::new().map_err(|e| {
        log::error!(
            "[Dictation::Clipboard] Failed to initialize clipboard reader: {}",
            e
        );
        DictationError::ClipboardFailed {
            message: format!("Failed to open clipboard: {}", e),
        }
    })?;

    clipboard.get_text().map_err(|e| {
        log::warn!(
            "[Dictation::Clipboard] No text on clipboard or failed to read: {}",
            e
        );
        DictationError::ClipboardFailed {
            message: format!("Failed to read clipboard text: {}", e),
        }
    })
}

/// Set the system clipboard text.
pub fn set_text(text: &str) -> Result<(), DictationError> {
    let mut clipboard = Clipboard::new().map_err(|e| {
        log::error!(
            "[Dictation::Clipboard] Failed to initialize clipboard writer: {}",
            e
        );
        DictationError::ClipboardFailed {
            message: format!("Failed to open clipboard: {}", e),
        }
    })?;

    clipboard.set_text(text.to_string()).map_err(|e| {
        log::error!(
            "[Dictation::Clipboard] Failed to write text to clipboard: {}",
            e
        );
        DictationError::ClipboardFailed {
            message: format!("Failed to write clipboard text: {}", e),
        }
    })
}

/// Helper to execute an action while temporarily replacing clipboard text,
/// restoring the original content afterwards if the action succeeds.
pub async fn with_clipboard_safe<F, Fut, R>(new_text: &str, action: F) -> Result<R, DictationError>
where
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = Result<R, DictationError>>,
{
    // 1. Capture current clipboard (best-effort)
    let previous_text = get_text().ok();

    // 2. Set new text on clipboard
    set_text(new_text)?;

    // 3. Execute the target action (e.g. simulated paste)
    let result = action().await;

    match result {
        Ok(val) => {
            // 4. On success: wait brief debounce and restore previous clipboard if captured
            if let Some(prev) = previous_text {
                tokio::time::sleep(std::time::Duration::from_millis(350)).await;
                if let Err(e) = set_text(&prev) {
                    log::warn!(
                        "[Dictation::Clipboard] Failed to restore previous clipboard content: {}",
                        e
                    );
                } else {
                    log::debug!(
                        "[Dictation::Clipboard] Restored prior clipboard content successfully."
                    );
                }
            }
            Ok(val)
        }
        Err(e) => {
            // 5. On failure: keep transcript on clipboard so user has recovery path
            log::warn!(
                "[Dictation::Clipboard] Action failed. Preserving transcribed text on clipboard for recovery: {}",
                e
            );
            Err(e)
        }
    }
}
