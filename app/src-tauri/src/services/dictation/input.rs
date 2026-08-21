//! ============================================================================
//! src/services/dictation/input.rs — Platform Input Simulation Adapters
//! ============================================================================

use crate::core::error::DictationError;
use enigo::{Direction, Enigo, Key, Keyboard, Settings};

/// Abstract interface for simulated OS input actions.
pub trait SystemInputAdapter: Send + Sync {
    /// Simulates pasting the current clipboard into the focused OS application.
    fn simulate_paste(&self) -> Result<(), DictationError>;
}

// ─── Linux X11 Adapter ───────────────────────────────────────────────────────

/// Linux X11 implementation using Enigo + x11rb backend.
#[derive(Default)]
pub struct X11InputAdapter;

impl SystemInputAdapter for X11InputAdapter {
    fn simulate_paste(&self) -> Result<(), DictationError> {
        let mut enigo = Enigo::new(&Settings::default()).map_err(|e| {
            log::error!(
                "[Dictation::Input] Failed to initialize Enigo on X11: {:?}",
                e
            );
            DictationError::InputSimulationFailed {
                message: format!("Enigo initialization failed: {:?}", e),
            }
        })?;

        // Simulate Ctrl+V key combination
        enigo.key(Key::Control, Direction::Press).map_err(|e| {
            log::error!("[Dictation::Input] Failed to press Control key: {:?}", e);
            DictationError::InputSimulationFailed {
                message: format!("Failed to press Control: {:?}", e),
            }
        })?;

        enigo
            .key(Key::Unicode('v'), Direction::Click)
            .map_err(|e| {
                log::error!("[Dictation::Input] Failed to click 'v' key: {:?}", e);
                DictationError::InputSimulationFailed {
                    message: format!("Failed to click 'v': {:?}", e),
                }
            })?;

        enigo.key(Key::Control, Direction::Release).map_err(|e| {
            log::error!("[Dictation::Input] Failed to release Control key: {:?}", e);
            DictationError::InputSimulationFailed {
                message: format!("Failed to release Control: {:?}", e),
            }
        })?;

        log::debug!("[Dictation::Input] X11 simulated paste (Ctrl+V) executed successfully.");
        Ok(())
    }
}

// ─── Linux Wayland Adapter ───────────────────────────────────────────────────

/// Linux Wayland implementation. Attempts Enigo; falls back gracefully on compositor
/// security restrictions (most Wayland compositors block background input injection).
#[derive(Default)]
pub struct WaylandInputAdapter;

impl SystemInputAdapter for WaylandInputAdapter {
    fn simulate_paste(&self) -> Result<(), DictationError> {
        // Attempt Enigo with Wayland settings if available
        match Enigo::new(&Settings::default()) {
            Ok(mut enigo) => {
                let press_res = enigo.key(Key::Control, Direction::Press);
                let click_res = enigo.key(Key::Unicode('v'), Direction::Click);
                let release_res = enigo.key(Key::Control, Direction::Release);

                if press_res.is_ok() && click_res.is_ok() && release_res.is_ok() {
                    log::debug!(
                        "[Dictation::Input] Wayland simulated paste executed successfully."
                    );
                    return Ok(());
                }
            }
            Err(e) => {
                log::warn!(
                    "[Dictation::Input] Wayland compositor blocked synthetic keystroke injection: {:?}",
                    e
                );
            }
        }

        // Under Wayland security model, background input simulation is restricted.
        // Return clear error so OutputRouter preserves text on clipboard for recovery.
        Err(DictationError::InputSimulationFailed {
            message: "Direct simulated paste is restricted by the Wayland compositor. Transcript remains available on clipboard.".into(),
        })
    }
}

// ─── macOS Adapter ───────────────────────────────────────────────────────────

/// macOS implementation using Cmd+V (Meta+V).
///
/// macOS paste shortcut is always Cmd+V, not Ctrl+V. Using Ctrl+V on macOS is a
/// silent no-op — this was a functional bug on non-Linux builds before this fix.
#[cfg(target_os = "macos")]
#[derive(Default)]
pub struct MacOsInputAdapter;

#[cfg(target_os = "macos")]
impl SystemInputAdapter for MacOsInputAdapter {
    fn simulate_paste(&self) -> Result<(), DictationError> {
        let mut enigo = Enigo::new(&Settings::default()).map_err(|e| {
            log::error!(
                "[Dictation::Input] Failed to initialize Enigo on macOS: {:?}",
                e
            );
            DictationError::InputSimulationFailed {
                message: format!("Enigo initialization failed on macOS: {:?}", e),
            }
        })?;

        // macOS paste = Cmd+V (Meta key), NOT Ctrl+V
        enigo.key(Key::Meta, Direction::Press).map_err(|e| {
            log::error!("[Dictation::Input] Failed to press Meta (Cmd) key: {:?}", e);
            DictationError::InputSimulationFailed {
                message: format!("Failed to press Meta: {:?}", e),
            }
        })?;

        enigo
            .key(Key::Unicode('v'), Direction::Click)
            .map_err(|e| {
                log::error!(
                    "[Dictation::Input] Failed to click 'v' key on macOS: {:?}",
                    e
                );
                DictationError::InputSimulationFailed {
                    message: format!("Failed to click 'v': {:?}", e),
                }
            })?;

        enigo.key(Key::Meta, Direction::Release).map_err(|e| {
            log::error!(
                "[Dictation::Input] Failed to release Meta (Cmd) key: {:?}",
                e
            );
            DictationError::InputSimulationFailed {
                message: format!("Failed to release Meta: {:?}", e),
            }
        })?;

        log::debug!("[Dictation::Input] macOS simulated paste (Cmd+V) executed successfully.");
        Ok(())
    }
}

// ─── Windows Adapter ─────────────────────────────────────────────────────────

/// Windows implementation using Ctrl+V via enigo's Win32 SendInput backend.
///
/// Enigo 0.2 on Windows sends Ctrl+V correctly via SendInput — same key sequence as X11.
/// A dedicated type is provided for clarity and future divergence (e.g. IME edge cases).
#[cfg(target_os = "windows")]
#[derive(Default)]
pub struct WindowsInputAdapter;

#[cfg(target_os = "windows")]
impl SystemInputAdapter for WindowsInputAdapter {
    fn simulate_paste(&self) -> Result<(), DictationError> {
        let mut enigo = Enigo::new(&Settings::default()).map_err(|e| {
            log::error!(
                "[Dictation::Input] Failed to initialize Enigo on Windows: {:?}",
                e
            );
            DictationError::InputSimulationFailed {
                message: format!("Enigo initialization failed on Windows: {:?}", e),
            }
        })?;

        enigo.key(Key::Control, Direction::Press).map_err(|e| {
            DictationError::InputSimulationFailed {
                message: format!("Failed to press Control on Windows: {:?}", e),
            }
        })?;

        enigo
            .key(Key::Unicode('v'), Direction::Click)
            .map_err(|e| DictationError::InputSimulationFailed {
                message: format!("Failed to click 'v' on Windows: {:?}", e),
            })?;

        enigo.key(Key::Control, Direction::Release).map_err(|e| {
            DictationError::InputSimulationFailed {
                message: format!("Failed to release Control on Windows: {:?}", e),
            }
        })?;

        log::debug!("[Dictation::Input] Windows simulated paste (Ctrl+V) executed successfully.");
        Ok(())
    }
}

// ─── Adapter Factory ─────────────────────────────────────────────────────────

/// Factory function to return the appropriate SystemInputAdapter for the current platform/session.
pub fn create_input_adapter() -> Box<dyn SystemInputAdapter> {
    #[cfg(target_os = "linux")]
    {
        let is_wayland = std::env::var("WAYLAND_DISPLAY").is_ok()
            || std::env::var("XDG_SESSION_TYPE")
                .map(|v| v.to_lowercase() == "wayland")
                .unwrap_or(false);

        if is_wayland {
            Box::new(WaylandInputAdapter)
        } else {
            Box::new(X11InputAdapter)
        }
    }

    #[cfg(target_os = "macos")]
    {
        Box::new(MacOsInputAdapter)
    }

    #[cfg(target_os = "windows")]
    {
        Box::new(WindowsInputAdapter)
    }

    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    {
        // Unsupported platform: return X11 adapter as a best-effort fallback.
        log::warn!("[Dictation::Input] Unsupported platform — falling back to X11InputAdapter.");
        Box::new(X11InputAdapter)
    }
}
