use std::{future::Future, time::Duration};

use arboard::Clipboard;

use crate::{
    core::error::DictationError,
    services::dictation::{CLIPBOARD_KEEPER_ALIVE_MS, CLIPBOARD_RESTORE_DELAY_MS},
};

/// Retrieve the current text from the system clipboard.
pub fn get_text() -> Result<String, DictationError> {
    let mut clipboard = open_clipboard("reader")?;

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

/// Set the system clipboard text and hold it alive for later manual pasting.
pub fn set_text(text: &str) -> Result<(), DictationError> {
    let mut clipboard = open_clipboard("writer")?;
    write_text(&mut clipboard, text)?;
    keep_alive(clipboard);
    Ok(())
}

/// Helper to execute an action while temporarily replacing clipboard text and restoring it on success.
pub async fn with_clipboard_safe<F, Fut, R>(new_text: &str, action: F) -> Result<R, DictationError>
where
    F: FnOnce() -> Fut,
    Fut: Future<Output = Result<R, DictationError>>,
{
    let mut clipboard = open_clipboard("paste window")?;
    let previous_text = clipboard.get_text().ok();
    write_text(&mut clipboard, new_text)?;

    let result = action().await;

    match result {
        Ok(val) => {
            if let Some(prev) = previous_text {
                tokio::time::sleep(Duration::from_millis(CLIPBOARD_RESTORE_DELAY_MS)).await;
                if let Err(e) = write_text(&mut clipboard, &prev) {
                    log::warn!(
                        "[Dictation::Clipboard] Failed to restore previous clipboard content: {}",
                        e
                    );
                }
            }
            Ok(val)
        }
        Err(e) => {
            log::warn!(
                "[Dictation::Clipboard] Action failed. Preserving transcribed text on clipboard for recovery: {}",
                e
            );
            keep_alive(clipboard);
            Err(e)
        }
    }
}

/// Opens a system clipboard handle tagged with its usage context.
fn open_clipboard(context: &str) -> Result<Clipboard, DictationError> {
    Clipboard::new().map_err(|e| {
        log::error!(
            "[Dictation::Clipboard] Failed to initialize clipboard {}: {}",
            context,
            e
        );
        DictationError::ClipboardFailed {
            message: format!("Failed to open clipboard: {}", e),
        }
    })
}

/// Writes text through an already-open clipboard handle.
fn write_text(clipboard: &mut Clipboard, text: &str) -> Result<(), DictationError> {
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

/// Hands a live clipboard handle to a detached keeper thread so fallback transcripts stay pastable.
fn keep_alive(clipboard: Clipboard) {
    if let Err(e) = std::thread::Builder::new()
        .name("vox-clipboard-keeper".to_string())
        .spawn(move || {
            std::thread::sleep(Duration::from_millis(CLIPBOARD_KEEPER_ALIVE_MS));
            drop(clipboard);
        })
    {
        log::warn!(
            "[Dictation::Clipboard] Failed to spawn clipboard keeper thread: {}",
            e
        );
    }
}
