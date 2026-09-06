use tauri::AppHandle;

use crate::{
    core::{error::DictationError, events::ToastLevel, settings::DictationOutputMode},
    services::dictation::{clipboard, input::create_input_adapter},
    toast::show_toast,
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
        DictationOutputMode::Clipboard => dispatch_to_clipboard(app, text),
        DictationOutputMode::Paste => dispatch_to_paste(app, text).await,
    }
}

/// Sets transcript directly on system clipboard.
fn dispatch_to_clipboard<R: tauri::Runtime>(
    app: &AppHandle<R>,
    text: &str,
) -> Result<(), DictationError> {
    clipboard::set_text(text)?;
    log::info!("[Dictation::Router] Transcript written to system clipboard.");
    if let Err(e) = show_toast(app, "Dictation Copied", text, ToastLevel::Success) {
        log::warn!("[Dictation::Router] Failed to show toast: {}", e);
    }
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

    match paste_result {
        Ok(()) => {
            log::info!(
                "[Dictation::Router] Transcript successfully pasted into focused application."
            );
            if let Err(e) = show_toast(app, "Dictation Pasted", text, ToastLevel::Success) {
                log::warn!("[Dictation::Router] Failed to show toast: {}", e);
            }
            Ok(())
        }
        Err(e) => {
            log::warn!(
                "[Dictation::Router] Paste simulation failed ({:?}). Transcript is preserved on clipboard.",
                e
            );
            if let Err(toast_err) = show_toast(
                app,
                "Paste Blocked by OS",
                "Transcript saved to clipboard — paste manually with Ctrl+V.",
                ToastLevel::Warning,
            ) {
                log::warn!(
                    "[Dictation::Router] Failed to show paste-blocked toast: {}",
                    toast_err
                );
            }
            Ok(())
        }
    }
}
