use std::env::var;

use enigo::{Direction, Enigo, Key, Keyboard, Settings};

use crate::core::error::DictationError;

pub trait SystemInputAdapter: Send + Sync {
    fn simulate_paste(&self) -> Result<(), DictationError>;
}

#[derive(Default)]
pub struct X11InputAdapter;

impl SystemInputAdapter for X11InputAdapter {
    /// Simulates Ctrl+V keystroke injection on X11 display servers.
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

#[cfg(target_os = "linux")]
use once_cell::sync::Lazy;

#[cfg(target_os = "linux")]
const ATSPI_KEY_PRESS: libc::c_int = 0;
#[cfg(target_os = "linux")]
const ATSPI_KEY_RELEASE: libc::c_int = 1;
#[cfg(target_os = "linux")]
const ATSPI_KEY_PRESSRELEASE: libc::c_int = 2;
#[cfg(target_os = "linux")]
const KEYSYM_CONTROL_L: libc::c_long = 65507;
#[cfg(target_os = "linux")]
const KEYSYM_V_LOWER: libc::c_long = 118;

#[cfg(target_os = "linux")]
type AtspiInitFn = unsafe extern "C" fn() -> libc::c_int;
#[cfg(target_os = "linux")]
type AtspiGenerateKeyboardEventFn = unsafe extern "C" fn(
    libc::c_long,
    *const libc::c_char,
    libc::c_int,
    *mut *mut libc::c_void,
) -> libc::c_int;

#[cfg(target_os = "linux")]
struct AtspiLibrary {
    init: AtspiInitFn,
    generate_keyboard_event: AtspiGenerateKeyboardEventFn,
}

#[cfg(target_os = "linux")]
unsafe impl Send for AtspiLibrary {}
#[cfg(target_os = "linux")]
unsafe impl Sync for AtspiLibrary {}

#[cfg(target_os = "linux")]
impl AtspiLibrary {
    /// Dynamically loads libatspi and resolves the required keyboard synthesis functions.
    fn load() -> Option<&'static Self> {
        static INSTANCE: Lazy<Option<AtspiLibrary>> = Lazy::new(|| {
            let candidate_names = [
                c"libatspi.so.0".as_ptr(),
                c"libatspi.so".as_ptr(),
            ];

            let mut handle = std::ptr::null_mut();
            for &name in &candidate_names {
                // SAFETY: dlopen is passed a valid null-terminated C-string pointer and standard POSIX flags.
                handle = unsafe { libc::dlopen(name, libc::RTLD_LAZY | libc::RTLD_LOCAL) };
                if !handle.is_null() {
                    break;
                }
            }

            if handle.is_null() {
                log::warn!(
                    "[Dictation::Input] [Wayland] libatspi could not be opened dynamically"
                );
                return None;
            }

            // SAFETY: dlsym calls look up known AT-SPI C ABI function names in the successfully loaded libatspi library.
            unsafe {
                let init_ptr = libc::dlsym(handle, c"atspi_init".as_ptr());
                let gen_ptr =
                    libc::dlsym(handle, c"atspi_generate_keyboard_event".as_ptr());

                if init_ptr.is_null() || gen_ptr.is_null() {
                    log::warn!(
                        "[Dictation::Input] [Wayland] Required AT-SPI symbols missing in libatspi"
                    );
                    libc::dlclose(handle);
                    return None;
                }

                Some(AtspiLibrary {
                    init: std::mem::transmute::<*mut libc::c_void, AtspiInitFn>(init_ptr),
                    generate_keyboard_event: std::mem::transmute::<
                        *mut libc::c_void,
                        AtspiGenerateKeyboardEventFn,
                    >(gen_ptr),
                })
            }
        });

        INSTANCE.as_ref()
    }

    /// Dispatches a simulated Ctrl+V key combination via the AT-SPI accessibility bus.
    fn dispatch_ctrl_v(&self) -> bool {
        // SAFETY: Function pointers were validated during initialization and null-pointers are passed for unused parameters.
        unsafe {
            let init_status = (self.init)();
            log::debug!(
                "[Dictation::Input] [Wayland] atspi_init returned status={}",
                init_status
            );
            if init_status != 0 {
                log::warn!(
                    "[Dictation::Input] [Wayland] atspi_init returned non-zero status ({}); AT-SPI bus unavailable",
                    init_status
                );
                return false;
            }

            let press_ctrl = (self.generate_keyboard_event)(
                KEYSYM_CONTROL_L,
                std::ptr::null(),
                ATSPI_KEY_PRESS,
                std::ptr::null_mut(),
            );
            let click_v = (self.generate_keyboard_event)(
                KEYSYM_V_LOWER,
                std::ptr::null(),
                ATSPI_KEY_PRESSRELEASE,
                std::ptr::null_mut(),
            );
            let release_ctrl = (self.generate_keyboard_event)(
                KEYSYM_CONTROL_L,
                std::ptr::null(),
                ATSPI_KEY_RELEASE,
                std::ptr::null_mut(),
            );

            log::info!(
                "[Dictation::Input] [Wayland] AT-SPI events dispatched: ctrl_press={} v_click={} ctrl_release={}",
                press_ctrl, click_v, release_ctrl
            );

            press_ctrl != 0 && click_v != 0 && release_ctrl != 0
        }
    }
}

/// Linux Wayland implementation using the AT-SPI accessibility bus for reliable Ctrl+V keystroke injection.
#[derive(Default)]
pub struct WaylandInputAdapter;

impl SystemInputAdapter for WaylandInputAdapter {
    /// Attempts paste simulation on Wayland compositors via AT-SPI, falling back to Enigo.
    fn simulate_paste(&self) -> Result<(), DictationError> {
        log::info!("[Dictation::Input] [Wayland] Initiating simulated paste (Ctrl+V) via AT-SPI accessibility bus...");

        let atspi_result = AtspiLibrary::load().is_some_and(|lib| lib.dispatch_ctrl_v());

        if atspi_result {
            log::info!("[Dictation::Input] [Wayland] Successfully dispatched simulated Ctrl+V via AT-SPI.");
            return Ok(());
        }

        log::warn!("[Dictation::Input] [Wayland] AT-SPI injection failed or unavailable. Falling back to Enigo check...");
        if is_blocking_compositor() {
            log::warn!(
                "[Dictation::Input] [Wayland] Compositor is known to swallow Enigo synthetic keystrokes without error; treating injection as unverified (transcript preserved on clipboard)"
            );
            return Err(unverified_injection_error());
        }

        match Enigo::new(&Settings::default()) {
            Ok(mut enigo) => {
                let press_res = enigo.key(Key::Control, Direction::Press);
                let click_res = enigo.key(Key::Unicode('v'), Direction::Click);
                let release_res = enigo.key(Key::Control, Direction::Release);

                if press_res.is_ok() && click_res.is_ok() && release_res.is_ok() {
                    log::info!(
                        "[Dictation::Input] [Wayland] Simulated paste dispatched via Enigo fallback."
                    );
                    return Ok(());
                }
            }
            Err(e) => {
                log::warn!(
                    "[Dictation::Input] [Wayland] Compositor blocked Enigo synthetic keystroke injection: {:?}",
                    e
                );
            }
        }

        Err(unverified_injection_error())
    }
}

/// macOS implementation using Cmd+V (Meta+V).
#[cfg(target_os = "macos")]
#[derive(Default)]
pub struct MacOsInputAdapter;

#[cfg(target_os = "macos")]
impl SystemInputAdapter for MacOsInputAdapter {
    /// Simulates Cmd+V keystroke injection on macOS.
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

/// Windows implementation using Ctrl+V via enigo's Win32 SendInput backend.
#[cfg(target_os = "windows")]
#[derive(Default)]
pub struct WindowsInputAdapter;

#[cfg(target_os = "windows")]
impl SystemInputAdapter for WindowsInputAdapter {
    /// Simulates Ctrl+V keystroke injection on Windows.
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

/// Builds the shared unverified-injection error for blocked Wayland compositors.
fn unverified_injection_error() -> DictationError {
    DictationError::InputSimulationFailed {
        message: "Direct simulated paste is restricted by the Wayland compositor. Transcript remains available on clipboard.".into(),
    }
}

/// Reports whether the session compositor is known to swallow synthetic keystrokes without error.
fn is_blocking_compositor() -> bool {
    let desktop = var("XDG_CURRENT_DESKTOP")
        .unwrap_or_default()
        .to_lowercase();
    desktop.contains("gnome") || desktop.contains("ubuntu")
}

/// Factory function to return the appropriate SystemInputAdapter for the current platform/session.
pub fn create_input_adapter() -> Box<dyn SystemInputAdapter> {
    #[cfg(target_os = "linux")]
    {
        let is_wayland = var("WAYLAND_DISPLAY").is_ok()
            || var("XDG_SESSION_TYPE")
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
        log::warn!("[Dictation::Input] Unsupported platform — falling back to X11InputAdapter.");
        Box::new(X11InputAdapter)
    }
}
