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

/// Linux X11 implementation using Enigo.
#[derive(Default)]
pub struct X11InputAdapter;

impl SystemInputAdapter for X11InputAdapter {
    fn simulate_paste(&self) -> Result<(), DictationError> {
        let mut enigo = Enigo::new(&Settings::default()).map_err(|e| {
            log::error!("[Dictation::Input] Failed to initialize Enigo on X11: {:?}", e);
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

        enigo.key(Key::Unicode('v'), Direction::Click).map_err(|e| {
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

/// Linux Wayland implementation.
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
                    log::debug!("[Dictation::Input] Wayland simulated paste executed successfully.");
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

/// Factory function to return the appropriate SystemInputAdapter for current platform/session.
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

    #[cfg(not(target_os = "linux"))]
    {
        Box::new(X11InputAdapter)
    }
}
