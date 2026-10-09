#[cfg(desktop)]
pub mod clipboard;
#[cfg(desktop)]
pub mod hotkey;
#[cfg(desktop)]
pub mod input;
#[cfg(desktop)]
pub mod output_router;

#[cfg(desktop)]
pub use hotkey::init_dictation_hotkey_listener;

#[cfg(not(desktop))]
pub fn init_dictation_hotkey_listener<R: tauri::Runtime>(
    _app: &tauri::AppHandle<R>,
    _hotkey: &str,
) -> Result<(), String> {
    Ok(())
}

/// Post-speech silence that auto-finalizes a PTT dictation turn (feature spec §10.3).
pub const DICTATION_SILENCE_AUTOSTOP_MS: u64 = 1200;
/// Keeper-thread lifetime holding a fallback transcript on the clipboard (feature spec §4.1 step 6).
pub const CLIPBOARD_KEEPER_ALIVE_MS: u64 = 30_000;
/// Grace window letting the target app consume a simulated paste before clipboard restore (feature spec §4.1).
pub const CLIPBOARD_RESTORE_DELAY_MS: u64 = 350;
/// Throttle for live partial-text notification updates (feature spec §6.2 row 1b).
pub const DICTATION_PARTIAL_UPDATE_THROTTLE_MS: u64 = 250;
