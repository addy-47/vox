#[cfg(target_os = "linux")]
use std::{backtrace::Backtrace, sync::Once};
use std::{
    fs::{create_dir_all, write},
    time::{SystemTime, UNIX_EPOCH},
};

use crate::utils::paths;

/// Kind recorded in the crash filename, so a native abort is distinguishable
/// from a Rust panic when triaging.
pub const CRASH_KIND_PANIC: &str = "panic";
pub const CRASH_KIND_NATIVE: &str = "native";

/// Writes a crash report to the crashes directory and returns its path.
///
/// This is deliberately a direct `std::fs::write` rather than a `tracing` emit:
/// the non-blocking tracing worker may itself be the thing that died, and on a
/// native abort there is no unwind to drain it.
pub fn write_crash_report(kind: &str, summary: &str, backtrace: &str) -> Option<String> {
    let crash_dir = paths::try_get()
        .map(|p| p.crashes)
        .unwrap_or_else(paths::crashes_dir);

    if create_dir_all(&crash_dir).is_err() {
        return None;
    }

    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let path = crash_dir.join(format!("crash_{}_{}.log", kind, timestamp));
    let report = format!(
        "Kind: {}\nSummary: {}\n\nBacktrace:\n{}\n",
        kind, summary, backtrace
    );

    match write(&path, report) {
        Ok(()) => Some(path.to_string_lossy().into_owned()),
        Err(_) => None,
    }
}

/// A SIGABRT from glibc's heap checker — `malloc(): invalid size (unsorted)`,
/// `free(): invalid pointer`, `double free or corruption` — is raised by the C
/// allocator, not by Rust. It therefore never reaches the panic hook, and
/// without this handler the only evidence of an FFI memory-safety violation is
/// one line on stderr followed by `[ELIFECYCLE] Command failed`.
///
/// Safe Rust cannot produce this abort; it requires an out-of-bounds write,
/// double free, or invalid free from a native library. The live suspects in this
/// process are the sherpa-onnx TTS/VAD bindings (which bundle espeak-ng and
/// ONNX Runtime), the `ort` embedder, and libasound via cpal.
///
/// Installing a handler is what turns "the app vanished" into a frame list.
#[cfg(target_os = "linux")]
static INSTALL: Once = Once::new();

/// Installs handlers for the native abort signals that indicate memory
/// corruption or an invalid access.
///
/// Note on async-signal-safety: `Backtrace::force_capture` and `fs::write`
/// allocate, which is not strictly async-signal-safe. That is accepted
/// deliberately: the process is already aborting, the alternative is losing the
/// only diagnostic, and nothing here can prevent the abort from completing.
#[cfg(target_os = "linux")]
extern "C" fn native_crash_handler(signal: libc::c_int) {
    let backtrace = Backtrace::force_capture().to_string();
    let name = match signal {
        libc::SIGABRT => "SIGABRT",
        libc::SIGSEGV => "SIGSEGV",
        libc::SIGBUS => "SIGBUS",
        libc::SIGILL => "SIGILL",
        libc::SIGFPE => "SIGFPE",
        _ => "SIGNAL",
    };
    let path = write_crash_report(CRASH_KIND_NATIVE, name, &backtrace);
    log::error!(
        target: "crash",
        "[FATAL NATIVE] {} captured; report: {}",
        name,
        path.as_deref().unwrap_or("<write failed>")
    );

    // Restore the default disposition and re-raise so the process still dies
    // with the correct signal rather than looping back into this handler.
    unsafe {
        libc::signal(signal, libc::SIG_DFL);
        libc::raise(signal);
    }
}

pub fn install_native_crash_handler() {
    #[cfg(target_os = "linux")]
    INSTALL.call_once(|| unsafe {
        for sig in [
            libc::SIGABRT,
            libc::SIGSEGV,
            libc::SIGBUS,
            libc::SIGILL,
            libc::SIGFPE,
        ] {
            libc::signal(sig, native_crash_handler as *const () as libc::sighandler_t);
        }
        log::debug!("[Crash] Native crash handlers installed (SIGABRT/SEGV/BUS/ILL/FPE)");
    });
}
