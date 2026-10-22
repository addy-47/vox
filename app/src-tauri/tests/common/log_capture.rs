//! ============================================================================
//! tests/common/log_capture.rs — In-Process Log Capture for Assertion
//! ============================================================================
//!
//! Captures `log` records so a test can assert on what the production code
//! actually wrote. Two production paths are only observable through the log:
//! the masked settings message and the engine-restart reason.
//!
//! The logger is installed once per test process. `log::set_logger` fails when
//! a logger is already installed; the failure is ignored, and a test that
//! expects a captured line then FAILS loudly rather than passing on an empty
//! buffer.

use std::sync::{Mutex, Once};

/// The captured log lines, oldest first.
static CAPTURED: Mutex<Vec<String>> = Mutex::new(Vec::new());

/// Guards the one-time logger installation.
static INSTALL: Once = Once::new();

/// The logger implementation handed to the `log` crate.
static LOGGER: CapturingLogger = CapturingLogger;

/// Captures every record the `log` facade emits.
struct CapturingLogger;

impl log::Log for CapturingLogger {
    fn enabled(&self, _metadata: &log::Metadata<'_>) -> bool {
        true
    }

    fn log(&self, record: &log::Record<'_>) {
        let mut captured = CAPTURED.lock().unwrap_or_else(|e| e.into_inner());
        captured.push(format!("[{}] {}", record.level(), record.args()));
    }

    fn flush(&self) {}
}

/// Installs the capturing logger and enables every level.
///
/// Safe to call from every test: the installation happens once per process.
pub fn install() {
    INSTALL.call_once(|| {
        let _ = log::set_logger(&LOGGER);
        log::set_max_level(log::LevelFilter::Trace);
    });
}

/// Clears the captured lines and returns them.
pub fn take() -> Vec<String> {
    let mut captured = CAPTURED.lock().unwrap_or_else(|e| e.into_inner());
    std::mem::take(&mut *captured)
}

/// Returns the captured lines joined with newlines.
pub fn take_text() -> String {
    take().join("\n")
}
