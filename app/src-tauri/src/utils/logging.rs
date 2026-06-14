use std::path::PathBuf;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

/// Initializes the tracing-based logging system.
///
/// Sets up:
/// 1. A daily rolling file appender in `~/.vox/logs/`
/// 2. A non-blocking writer to avoid stalling real-time threads
/// 3. A console layer for development (stdout)
/// 4. An EnvFilter to control verbosity via `RUST_LOG`
///
/// Returns a `WorkerGuard` which MUST be held in `AppState` to ensure logs
/// are flushed to disk before the app exits.
pub fn init(log_dir: PathBuf) -> WorkerGuard {
    // Ensure log directory exists
    let _ = std::fs::create_dir_all(&log_dir);

    // Cleanup old logs (keep max 5)
    cleanup_old_logs(&log_dir, 5);

    // Daily rolling appender: vox.log, vox.log.2026-05-10, etc.
    // Retains max 5 log files by default.
    let file_appender = tracing_appender::rolling::daily(log_dir, "vox.log");

    // Wrap in non-blocking writer (dedicated background thread)
    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

    // Build the subscriber
    tracing_subscriber::registry()
        // Filter based on RUST_LOG env var or default to info, but suppress verbose ONNX/ORT crates
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| {
            EnvFilter::new("info,ort=warn,onnxruntime=warn,sherpa_onnx=warn,ort_sys=warn,onnx=warn")
        }))
        // Console layer (stdout)
        .with(fmt::layer().with_ansi(true).with_target(false))
        // File layer (non-blocking)
        .with(
            fmt::layer()
                .with_writer(non_blocking)
                .with_ansi(false) // No ANSI codes in files
                .with_target(true)
                .with_thread_ids(true),
        )
        .init();

    log::info!("[Logging] Tracing initialized (Non-blocking, daily rolling).");
    guard
}

fn cleanup_old_logs(log_dir: &std::path::Path, max_files: usize) {
    let mut files = match std::fs::read_dir(log_dir) {
        Ok(entries) => entries
            .filter_map(|e| e.ok())
            .filter(|e| e.path().is_file())
            .filter_map(|e| {
                let path = e.path();
                let meta = std::fs::metadata(&path).ok()?;
                let modified = meta.modified().ok()?;
                Some((path, modified))
            })
            .collect::<Vec<_>>(),
        Err(_) => return,
    };

    if files.len() > max_files {
        // Sort by modification time (oldest first)
        files.sort_by_key(|&(_, m)| m);

        let to_delete = files.len() - max_files;
        for i in 0..to_delete {
            let _ = std::fs::remove_file(&files[i].0);
        }
    }
}
