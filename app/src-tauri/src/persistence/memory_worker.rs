use crate::core::settings::VoxSettings;
pub use crate::persistence::mutations::{enqueue_personal_facts, session_end_consolidation};
pub use crate::persistence::{decode_f32_blob, encode_f32_blob};
pub use crate::services::memory::pipeline::run_pipeline_cycle;
use crossbeam_channel::{bounded, Sender};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

pub const MIN_IDLE_DEBOUNCE_SECS: u64 = 30;

/// Events consumed exclusively by the background memory worker.
#[derive(Debug, Clone)]
pub enum MemoryWorkerEvent {
    /// A session has ended. Trigger the consolidation sweep.
    SessionEnd { session_id: String, summary: String },
    /// Extracted facts from compaction — enqueued to personal_memory_queue
    PersonalFactsReady {
        facts: HashMap<String, Vec<String>>, // collection → facts
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
    graph_version: Arc<std::sync::atomic::AtomicU64>,
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
                let timeout = Duration::from_millis(500);

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
                            crate::services::memory::unload_memory_pipeline_onnx_models();
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
                                } else {
                                    graph_version.fetch_add(1, Ordering::SeqCst);
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
                    let pipeline_enabled = match settings.read() {
                        Ok(s) => s.memory.pipeline_processing_enabled,
                        Err(_) => true,
                    };

                    if pipeline_enabled {
                        // Enforce 30-second minimum continuous idle debounce before executing queue orchestration
                        let is_debounced = state.idle_since.is_some_and(|since| {
                            since.elapsed() >= Duration::from_secs(MIN_IDLE_DEBOUNCE_SECS)
                        });

                        if is_debounced {
                            if let Some(ref db_conn) = conn {
                                let has_pending = handle.block_on(async {
                                    if let Ok(mut rows) = db_conn
                                        .query(
                                            "SELECT COUNT(*) FROM personal_memory_queue WHERE status IN ('staged_pending', 'deduped', 'embedded')",
                                            (),
                                        )
                                        .await
                                    {
                                        if let Ok(Some(row)) = rows.next().await {
                                            return row.get::<i64>(0).unwrap_or(0) > 0;
                                        }
                                    }
                                    false
                                });

                                if has_pending {
                                    loop {
                                        if !state.is_idle || !rx.is_empty() {
                                            break;
                                        }

                                        let processed_count = handle.block_on(async {
                                            crate::services::memory::pipeline::run_pipeline_cycle(db_conn, &cancel_flag).await
                                        });

                                        match processed_count {
                                            Ok(n) if n > 0 => {
                                                graph_version.fetch_add(1, Ordering::SeqCst);
                                                continue;
                                            }
                                            _ => {
                                                // Check if any items have status = 'failed' and retry_count < 3 to auto-retry
                                                let auto_retried = handle.block_on(async {
                                                    db_conn
                                                        .execute(
                                                            "UPDATE personal_memory_queue 
                                                             SET status = 'staged_pending', attempts = attempts + 1, retry_count = retry_count + 1 
                                                             WHERE status = 'failed' AND retry_count < 3",
                                                            (),
                                                        )
                                                        .await
                                                        .unwrap_or(0)
                                                });

                                                if auto_retried > 0 {
                                                    tracing::info!("[MemoryWorker] Auto-retrying {} failed queue items.", auto_retried);
                                                    continue;
                                                }

                                                // Reset idle_since timer so empty queue doesn't re-trigger every 500ms
                                                state.idle_since = Some(Instant::now());
                                                crate::services::memory::unload_memory_pipeline_onnx_models();
                                                break;
                                            }
                                        }
                                    }
                                } else {
                                    state.idle_since = Some(Instant::now());
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
