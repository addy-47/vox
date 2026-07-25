use crossbeam_channel::{bounded, Sender};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};
use std::collections::HashMap;
use crate::core::settings::VoxSettings;
pub use crate::persistence::{encode_f32_blob, decode_f32_blob};
pub use crate::persistence::mutations::{enqueue_personal_facts, session_end_consolidation};
pub use crate::services::memory::orchestrator::process_one_queue_item;

pub const MIN_IDLE_DEBOUNCE_SECS: u64 = 30;

/// Events consumed exclusively by the background memory worker.
#[derive(Debug, Clone)]
pub enum MemoryWorkerEvent {
    /// A session has ended. Trigger the consolidation sweep.
    SessionEnd { session_id: String, summary: String },
    /// Extracted facts from compaction — enqueued to personal_memory_queue
    PersonalFactsReady {
        facts: HashMap<String, Vec<String>>,  // collection → facts
        session_id: String,
    },
    /// The pipeline has entered Idle state. Trigger background memory sweep.
    PipelineIdle,
    /// The pipeline is active. Pause background memory sweep.
    PipelineActive,
    /// Track current active session ID to enforce current-session exclusion invariant.
    ActiveSessionChanged { session_id: u64 },
    /// Signals the memory worker to flush and exit cleanly.
    Shutdown,
}

struct WorkerState {
    current_session_id: u64,
    is_idle: bool,
    idle_since: Option<Instant>,
}

/// Spawns the dedicated background memory worker thread.
pub fn spawn_memory_worker(
    db_path: PathBuf,
    is_private_mode: Arc<AtomicBool>,
    settings: Arc<RwLock<VoxSettings>>,
) -> Sender<MemoryWorkerEvent> {
    let (tx, rx) = bounded::<MemoryWorkerEvent>(32);

    std::thread::Builder::new()
        .name("vox-memory-worker".to_string())
        .spawn(move || {
            tracing::info!("[MemoryWorker] Worker started. DB at {:?}", db_path);

            let handle = crate::persistence::db::get_tokio_handle();
            let conn_res = handle.block_on(async { crate::persistence::db::VoxDb::open(&db_path).await });
            let conn = match conn_res {
                Ok(c) => Some(c),
                Err(e) => {
                    tracing::error!("[MemoryWorker] Failed to open DB connection: {}", e);
                    None
                }
            };

            let mut state = WorkerState {
                current_session_id: 0,
                is_idle: true,
                idle_since: Some(Instant::now()),
            };
            let cancel_flag = Arc::new(AtomicBool::new(false));

            loop {
                let timeout = if state.is_idle {
                    Duration::from_millis(500)
                } else {
                    Duration::from_millis(500)
                };

                let event = match rx.recv_timeout(timeout) {
                    Ok(e) => Some(e),
                    Err(crossbeam_channel::RecvTimeoutError::Timeout) => None,
                    Err(crossbeam_channel::RecvTimeoutError::Disconnected) => {
                        tracing::info!("[MemoryWorker] Channel disconnected. Worker exiting.");
                        break;
                    }
                };

                if let Some(event) = event {
                    if is_private_mode.load(Ordering::Relaxed) {
                        tracing::debug!("[MemoryWorker] Private mode active: skipping memory event.");
                        continue;
                    }

                    match event {
                        MemoryWorkerEvent::ActiveSessionChanged { session_id } => {
                            state.current_session_id = session_id;
                        }
                        MemoryWorkerEvent::PipelineIdle => {
                            if !state.is_idle {
                                state.is_idle = true;
                                state.idle_since = Some(Instant::now());
                            }
                            cancel_flag.store(false, Ordering::Relaxed);
                        }
                        MemoryWorkerEvent::PipelineActive => {
                            state.is_idle = false;
                            state.idle_since = None;
                            cancel_flag.store(true, Ordering::Relaxed);
                        }
                        MemoryWorkerEvent::SessionEnd { session_id, summary } => {
                            if let Some(ref db_conn) = conn {
                                if let Err(e) = handle.block_on(async {
                                    session_end_consolidation(db_conn, &session_id, &summary).await
                                }) {
                                    tracing::error!(
                                        "[MemoryWorker] Failed consolidation sweep for session_id={}: {}",
                                        session_id, e
                                    );
                                }
                            }
                        }
                        MemoryWorkerEvent::PersonalFactsReady { facts, session_id } => {
                            if let Some(ref db_conn) = conn {
                                let pipeline_enabled = match settings.read() {
                                    Ok(s) => s.memory.pipeline_processing_enabled,
                                    Err(_) => true,
                                };
                                if let Err(e) = handle.block_on(async {
                                    enqueue_personal_facts(db_conn, facts, &session_id, pipeline_enabled).await
                                }) {
                                    tracing::error!(
                                        "[MemoryWorker] Failed to enqueue personal facts: {}", e
                                    );
                                }
                            }
                        }
                        MemoryWorkerEvent::Shutdown => {
                            tracing::info!("[MemoryWorker] Shutdown event received. Exiting thread.");
                            break;
                        }
                    }
                } else if state.is_idle && !is_private_mode.load(Ordering::Relaxed) {
                    // Enforce 30-second minimum continuous idle debounce before executing queue orchestration
                    let is_debounced = state.idle_since.map_or(false, |since| {
                        since.elapsed() >= Duration::from_secs(MIN_IDLE_DEBOUNCE_SECS)
                    });

                    if is_debounced {
                        if let Some(ref db_conn) = conn {
                            let memory_settings = match settings.read() {
                                Ok(s) => s.memory.clone(),
                                Err(_) => {
                                    tracing::error!("[MemoryWorker] Settings lock poisoned! Skipping loop iteration.");
                                    std::thread::sleep(std::time::Duration::from_millis(100));
                                    continue;
                                }
                            };

                            loop {
                                if !state.is_idle || !rx.is_empty() {
                                    break;
                                }

                                let processed_queue = handle.block_on(async {
                                    process_one_queue_item(db_conn, &memory_settings, &cancel_flag).await
                                });

                                match processed_queue {
                                    Ok(crate::services::memory::orchestrator::PipelineOutcome::Merged { .. })
                                  | Ok(crate::services::memory::orchestrator::PipelineOutcome::Ingested { .. }) => {
                                        continue;
                                    }
                                    _ => break,
                                }
                            }
                        }
                    }
                }
            }
        })
        .expect("[MemoryWorker] Failed to spawn worker thread");

    tx
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_f32_blob_encode_decode() {
        let floats = vec![0.1f32, -0.5, 0.99, 123.456];
        let encoded = encode_f32_blob(&floats);
        assert_eq!(encoded.len(), 16);
        let decoded = decode_f32_blob(&encoded);
        assert_eq!(floats, decoded);
    }
}
