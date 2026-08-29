use std::path::PathBuf;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

/// Initializes the tracing-based logging system.
pub fn init(log_dir: PathBuf) -> WorkerGuard {

    if let Err(e) = std::fs::create_dir_all(&log_dir) {
        eprintln!("[Logging] Failed to create log directory {:?}: {}", log_dir, e);
    }

    cleanup_old_logs(&log_dir, 5);

    let file_appender = tracing_appender::rolling::daily(log_dir, "vox.log");

    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| {
            EnvFilter::new("info,ort=warn,onnxruntime=warn,sherpa_onnx=warn,ort_sys=warn,onnx=warn,turso=warn,cpal=warn")
        }))
        .with(fmt::layer().with_ansi(true).with_target(false))
        .with(
            fmt::layer()
                .with_writer(non_blocking)
                .with_ansi(false)
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
        files.sort_by_key(|&(_, m)| m);

        let to_delete = files.len() - max_files;
        for (path, _) in files.iter().take(to_delete) {
            if let Err(e) = std::fs::remove_file(path) {
                log::debug!("[Logging] Old log deletion notice for {:?}: {}", path, e);
            }
        }
    }
}
