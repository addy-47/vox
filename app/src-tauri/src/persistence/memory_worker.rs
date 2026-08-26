use crate::core::settings::VoxSettings;
use crate::persistence::{
    MEMORY_WORKER_CHANNEL_CAPACITY, MEMORY_WORKER_POLL_TIMEOUT, MIN_IDLE_DEBOUNCE_SECS,
};
pub use crate::persistence::mutations::{enqueue_personal_facts, session_end_consolidation};
pub use crate::persistence::{decode_f32_blob, encode_f32_blob};
pub use crate::services::memory::pipeline::run_pipeline_cycle;
use crossbeam_channel::{bounded, Receiver, Sender};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};
use turso::Connection;

/// Events consumed exclusively by the background memory worker.
#[derive(Debug, Clone)]
pub enum MemoryWorkerEvent {
    /// A session has ended. Trigger the consolidation sweep.
    SessionEnd { session_id: String, summary: String },
    /// Extracted facts from compaction — enqueued to personal_memory_queue
    PersonalFactsReady {
        facts: HashMap<String, Vec<String>>,
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
    graph_version: Arc<AtomicU64>,
) -> Sender<MemoryWorkerEvent> {
    let (tx, rx) = bounded::<MemoryWorkerEvent>(MEMORY_WORKER_CHANNEL_CAPACITY);

    std::thread::Builder::new()
        .name("vox-memory-worker".to_string())
        .spawn(move || {
            log::info!("[Persistence::MemoryWorker] Worker started. DB at {:?}", db_path);

            let handle = crate::persistence::db::get_tokio_handle();
            let conn = match handle.block_on(async { crate::persistence::db::VoxDb::open(&db_path).await }) {
                Ok(c) => Some(c),
                Err(e) => {
                    log::error!("[Persistence::MemoryWorker] Failed to open DB connection: {}", e);
                    None
                }
            };

            let mut state = WorkerState {
                current_session_id: 0,
                is_idle: true,
                idle_since: Some(Instant::now()),
            };
            let cancel_flag = Arc::new(AtomicBool::new(false));
            let ctx = MemoryWorkerContext {
                is_private_mode,
                settings,
                graph_version,
                cancel_flag,
                handle: &handle,
            };

            run_worker_loop(
                rx,
                conn,
                &mut state,
                ctx,
            );
        })
        .expect("[Persistence::MemoryWorker] Failed to spawn worker thread");

    tx
}

struct MemoryWorkerContext<'a> {
    is_private_mode: Arc<AtomicBool>,
    settings: Arc<RwLock<VoxSettings>>,
    graph_version: Arc<AtomicU64>,
    cancel_flag: Arc<AtomicBool>,
    handle: &'a tokio::runtime::Handle,
}

fn run_worker_loop(
    rx: Receiver<MemoryWorkerEvent>,
    conn: Option<Connection>,
    state: &mut WorkerState,
    ctx: MemoryWorkerContext<'_>,
) {
    loop {
        let event = match rx.recv_timeout(MEMORY_WORKER_POLL_TIMEOUT) {
            Ok(e) => Some(e),
            Err(crossbeam_channel::RecvTimeoutError::Timeout) => None,
            Err(crossbeam_channel::RecvTimeoutError::Disconnected) => {
                log::info!("[Persistence::MemoryWorker] Channel disconnected. Worker exiting");
                break;
            }
        };

        if let Some(event) = event {
            if ctx.is_private_mode.load(Ordering::Relaxed) {
                log::debug!("[Persistence::MemoryWorker] Private mode active: skipping memory event");
                continue;
            }

            if handle_single_event(
                event,
                conn.as_ref(),
                state,
                &ctx,
            ) {
                break;
            }
        } else if state.is_idle && !ctx.is_private_mode.load(Ordering::Relaxed) {
            process_idle_queue(
                conn.as_ref(),
                state,
                &rx,
                &ctx.settings,
                &ctx.graph_version,
                &ctx.cancel_flag,
                ctx.handle,
            );
        }
    }
}

fn handle_single_event(
    event: MemoryWorkerEvent,
    conn: Option<&Connection>,
    state: &mut WorkerState,
    ctx: &MemoryWorkerContext<'_>,
) -> bool {
    match event {
        MemoryWorkerEvent::ActiveSessionChanged { session_id } => {
            state.current_session_id = session_id;
        }
        MemoryWorkerEvent::PipelineIdle => {
            if !state.is_idle {
                state.is_idle = true;
                state.idle_since = Some(Instant::now());
            }
            ctx.cancel_flag.store(false, Ordering::Relaxed);
        }
        MemoryWorkerEvent::PipelineActive => {
            state.is_idle = false;
            state.idle_since = None;
            ctx.cancel_flag.store(true, Ordering::Relaxed);
            crate::services::memory::unload_memory_pipeline_onnx_models();
        }
        MemoryWorkerEvent::SessionEnd { session_id, summary } => {
            if let Some(db_conn) = conn {
                if let Err(e) = ctx.handle.block_on(async {
                    session_end_consolidation(db_conn, &session_id, &summary).await
                }) {
                    log::error!(
                        "[Persistence::MemoryWorker] Failed consolidation sweep for session_id={}: {}",
                        session_id, e
                    );
                } else {
                    ctx.graph_version.fetch_add(1, Ordering::SeqCst);
                }
            }
        }
        MemoryWorkerEvent::PersonalFactsReady { facts, session_id } => {
            if let Some(db_conn) = conn {
                let pipeline_enabled = ctx.settings
                    .read()
                    .map(|s| s.memory.pipeline_processing_enabled)
                    .unwrap_or(true);
                if let Err(e) = ctx.handle.block_on(async {
                    enqueue_personal_facts(db_conn, facts, &session_id, pipeline_enabled).await
                }) {
                    log::error!(
                        "[Persistence::MemoryWorker] Failed to enqueue personal facts: {}", e
                    );
                }
            }
        }
        MemoryWorkerEvent::Shutdown => {
            log::info!("[Persistence::MemoryWorker] Shutdown event received. Exiting thread");
            return true;
        }
    }
    false
}

fn process_idle_queue(
    conn: Option<&Connection>,
    state: &mut WorkerState,
    rx: &Receiver<MemoryWorkerEvent>,
    settings: &Arc<RwLock<VoxSettings>>,
    graph_version: &Arc<AtomicU64>,
    cancel_flag: &Arc<AtomicBool>,
    handle: &tokio::runtime::Handle,
) {
    let pipeline_enabled = settings
        .read()
        .map(|s| s.memory.pipeline_processing_enabled)
        .unwrap_or(true);

    if !pipeline_enabled {
        return;
    }

    let is_debounced = state
        .idle_since
        .is_some_and(|since| since.elapsed() >= Duration::from_secs(MIN_IDLE_DEBOUNCE_SECS));

    if !is_debounced {
        return;
    }

    let Some(db_conn) = conn else {
        return;
    };

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
        run_drain_queue(db_conn, state, rx, graph_version, cancel_flag, handle);
    } else {
        state.idle_since = Some(Instant::now());
    }
}

fn run_drain_queue(
    db_conn: &Connection,
    state: &mut WorkerState,
    rx: &Receiver<MemoryWorkerEvent>,
    graph_version: &Arc<AtomicU64>,
    cancel_flag: &Arc<AtomicBool>,
    handle: &tokio::runtime::Handle,
) {
    loop {
        if !state.is_idle || !rx.is_empty() {
            break;
        }

        let processed_count = handle.block_on(async {
            crate::services::memory::pipeline::run_pipeline_cycle(db_conn, cancel_flag).await
        });

        match processed_count {
            Ok(n) if n > 0 => {
                graph_version.fetch_add(1, Ordering::SeqCst);
            }
            _ => {
                let auto_retried = handle.block_on(async {
                    let update_sql = format!(
                        "UPDATE personal_memory_queue 
                         SET status = 'staged_pending', attempts = attempts + 1, retry_count = retry_count + 1 
                         WHERE status = 'failed' AND retry_count < {}",
                        crate::persistence::MAX_QUEUE_RETRY_ATTEMPTS
                    );
                    db_conn
                        .execute(&update_sql, ())
                        .await
                        .unwrap_or(0)
                });

                if auto_retried > 0 {
                    log::info!("[Persistence::MemoryWorker] Auto-retrying {} failed queue items", auto_retried);
                    continue;
                }

                state.idle_since = Some(Instant::now());
                crate::services::memory::unload_memory_pipeline_onnx_models();
                break;
            }
        }
    }
}

