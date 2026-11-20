use std::{
    sync::{
        atomic::{AtomicBool, AtomicU32, Ordering},
        Arc,
    },
    thread::Builder,
    time::{Instant, SystemTime, UNIX_EPOCH},
};

use crossbeam_channel::{bounded, Receiver, Sender};
use turso::Connection;

use super::{
    queue::reconcile_crashed_queue_on_boot,
    sessions::{cleanup_zero_turn_sessions, ensure_session_exists},
    tool_calls::persist_tool_call,
    PersistenceEvent, PERSISTENCE_CHANNEL_CAPACITY, PERSISTENCE_RATE_INTERVAL,
    WORKER_EVENT_POLL_TIMEOUT,
};
use crate::{
    core::error::PersistenceError,
    persistence::{get_tokio_handle, VoxDb},
};

pub const MAX_TOOL_OUTPUTS_CACHE_BYTES: u64 = 100 * 1024 * 1024;
pub const MAX_TOOL_OUTPUT_FILE_AGE_SECS: u64 = 7 * 24 * 3600;
pub const MAX_TASKS_CACHE_BYTES: u64 = 100 * 1024 * 1024;

/// Spawns the persistence worker on a dedicated OS thread holding an isolated database connection.
pub fn spawn_persistence_worker(
    vox_db: Arc<VoxDb>,
    is_db_healthy: Arc<AtomicBool>,
    persistence_rate: Arc<AtomicU32>,
    is_private_mode: Arc<AtomicBool>,
) -> Sender<PersistenceEvent> {
    let (tx, rx) = bounded::<PersistenceEvent>(PERSISTENCE_CHANNEL_CAPACITY);

    Builder::new()
        .name("vox-persistence".to_string())
        .spawn(move || {
            let rt_handle = get_tokio_handle();
            let conn = match vox_db.connect() {
                Ok(c) => c,
                Err(e) => {
                    log::error!(
                        "[Persistence::Worker] Failed to vend worker connection: {}",
                        e
                    );
                    is_db_healthy.store(false, Ordering::Relaxed);
                    return;
                }
            };

            run_startup_sweeps(&conn, &rt_handle);
            log::info!("[Persistence::Worker] Worker started on dedicated connection");

            run_event_loop(
                rx,
                &conn,
                &rt_handle,
                &is_db_healthy,
                &persistence_rate,
                &is_private_mode,
            );
        })
        .expect("[Persistence::Worker] Failed to spawn worker thread");

    tx
}

fn run_startup_sweeps(db: &Connection, rt_handle: &tokio::runtime::Handle) {
    if let Err(e) = rt_handle.block_on(cleanup_zero_turn_sessions(db)) {
        log::warn!(
            "[Persistence::Worker] Zero-turn startup cleanup failed (non-fatal): {}",
            e
        );
    }

    if let Err(e) = rt_handle.block_on(reconcile_crashed_queue_on_boot(db)) {
        log::warn!(
            "[Persistence::Worker] Queue reconciliation startup sweep failed (non-fatal): {}",
            e
        );
    }

    if let Err(e) = rt_handle.block_on(sweep_tool_outputs_cache(db)) {
        log::warn!(
            "[Persistence::Worker] Tool outputs cache startup sweep failed (non-fatal): {}",
            e
        );
    }

    if let Err(e) = rt_handle.block_on(sweep_tasks_cache(db)) {
        log::warn!(
            "[Persistence::Worker] Tasks cache startup sweep failed (non-fatal): {}",
            e
        );
    }
}

async fn sweep_tool_outputs_cache(db: &Connection) -> Result<(), PersistenceError> {
    let outputs_dir = crate::utils::paths::tool_outputs_dir();
    if !outputs_dir.exists() {
        return Ok(());
    }

    let entries = match std::fs::read_dir(&outputs_dir) {
        Ok(e) => e,
        Err(e) => {
            log::warn!(
                "[Persistence::Worker] Failed to read tool_outputs_dir: {}",
                e
            );
            return Ok(());
        }
    };

    let now_system = SystemTime::now();

    struct SpilloverFile {
        path: std::path::PathBuf,
        size: u64,
        mtime: SystemTime,
    }

    let mut remaining_files: Vec<SpilloverFile> = Vec::new();
    let mut total_bytes: u64 = 0;

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }

        let dir_name = match path.file_name().and_then(|n| n.to_str()) {
            Some(name) => name,
            None => continue,
        };

        let session_id: i64 = match dir_name.parse() {
            Ok(sid) => sid,
            Err(_) => {
                let _ = std::fs::remove_dir_all(&path);
                continue;
            }
        };

        let is_active_session: bool = {
            let mut rows = match db
                .query(
                    "SELECT 1 FROM sessions WHERE id = ? AND deleted_at IS NULL;",
                    (session_id,),
                )
                .await
            {
                Ok(r) => r,
                Err(e) => {
                    log::warn!(
                        "[Persistence::Worker] Error checking session {}: {}",
                        session_id,
                        e
                    );
                    continue;
                }
            };
            match rows.next().await {
                Ok(opt) => opt.is_some(),
                Err(_) => false,
            }
        };

        if !is_active_session {
            log::info!(
                "[Persistence::Worker] Purging tool outputs for inactive/orphaned session {}",
                session_id
            );
            let _ = std::fs::remove_dir_all(&path);
            continue;
        }

        if let Ok(file_entries) = std::fs::read_dir(&path) {
            let mut child_count = 0;
            for file_entry in file_entries.flatten() {
                let file_path = file_entry.path();
                if !file_path.is_file() {
                    continue;
                }

                let metadata = match file_entry.metadata() {
                    Ok(m) => m,
                    Err(_) => continue,
                };

                let mtime = metadata.modified().unwrap_or(now_system);
                let age = now_system.duration_since(mtime).unwrap_or_default();
                if age.as_secs() > MAX_TOOL_OUTPUT_FILE_AGE_SECS {
                    log::info!(
                        "[Persistence::Worker] Purging expired tool output file (>7d): {:?}",
                        file_path
                    );
                    let _ = std::fs::remove_file(&file_path);
                } else {
                    let size = metadata.len();
                    total_bytes += size;
                    remaining_files.push(SpilloverFile {
                        path: file_path,
                        size,
                        mtime,
                    });
                    child_count += 1;
                }
            }
            if child_count == 0 {
                let _ = std::fs::remove_dir(&path);
            }
        }
    }

    if total_bytes > MAX_TOOL_OUTPUTS_CACHE_BYTES {
        remaining_files.sort_by_key(|f| f.mtime);
        for file in remaining_files {
            if total_bytes <= MAX_TOOL_OUTPUTS_CACHE_BYTES {
                break;
            }
            if std::fs::remove_file(&file.path).is_ok() {
                total_bytes = total_bytes.saturating_sub(file.size);
                log::info!(
                    "[Persistence::Worker] LRU evicted tool output: {:?} ({} bytes)",
                    file.path,
                    file.size
                );
            }
        }
    }

    Ok(())
}

async fn sweep_tasks_cache(db: &Connection) -> Result<(), PersistenceError> {
    let tasks_dir = crate::utils::paths::tasks_dir();
    if !tasks_dir.exists() {
        return Ok(());
    }

    let entries = match std::fs::read_dir(&tasks_dir) {
        Ok(e) => e,
        Err(e) => {
            log::warn!("[Persistence::Worker] Failed to read tasks_dir: {}", e);
            return Ok(());
        }
    };

    let now_system = SystemTime::now();

    struct LogFile {
        path: std::path::PathBuf,
        size: u64,
        mtime: SystemTime,
    }

    let mut remaining_files: Vec<LogFile> = Vec::new();
    let mut total_bytes: u64 = 0;

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }

        let dir_name = match path.file_name().and_then(|n| n.to_str()) {
            Some(name) => name,
            None => continue,
        };

        let session_id: i64 = match dir_name.parse() {
            Ok(sid) => sid,
            Err(_) => {
                let _ = std::fs::remove_dir_all(&path);
                continue;
            }
        };

        let is_active_session: bool = {
            let mut rows = match db
                .query(
                    "SELECT 1 FROM sessions WHERE id = ? AND deleted_at IS NULL;",
                    (session_id,),
                )
                .await
            {
                Ok(r) => r,
                Err(e) => {
                    log::warn!(
                        "[Persistence::Worker] Error checking session {}: {}",
                        session_id,
                        e
                    );
                    continue;
                }
            };
            match rows.next().await {
                Ok(opt) => opt.is_some(),
                Err(_) => false,
            }
        };

        if !is_active_session {
            log::info!(
                "[Persistence::Worker] Purging task logs for inactive/orphaned session {}",
                session_id
            );
            let _ = std::fs::remove_dir_all(&path);
            continue;
        }

        if let Ok(file_entries) = std::fs::read_dir(&path) {
            let mut child_count = 0;
            for file_entry in file_entries.flatten() {
                let file_path = file_entry.path();
                if !file_path.is_file() {
                    continue;
                }
                let metadata = match file_entry.metadata() {
                    Ok(m) => m,
                    Err(_) => continue,
                };
                let size = metadata.len();
                let mtime = metadata.modified().unwrap_or(now_system);
                total_bytes += size;
                remaining_files.push(LogFile {
                    path: file_path,
                    size,
                    mtime,
                });
                child_count += 1;
            }
            if child_count == 0 {
                let _ = std::fs::remove_dir(&path);
            }
        }
    }

    if total_bytes > MAX_TASKS_CACHE_BYTES {
        remaining_files.sort_by_key(|f| f.mtime);
        for file in remaining_files {
            if total_bytes <= MAX_TASKS_CACHE_BYTES {
                break;
            }
            if std::fs::remove_file(&file.path).is_ok() {
                total_bytes = total_bytes.saturating_sub(file.size);
                log::info!(
                    "[Persistence::Worker] LRU evicted task log: {:?} ({} bytes)",
                    file.path,
                    file.size
                );
            }
        }
    }

    Ok(())
}

fn run_event_loop(
    rx: Receiver<PersistenceEvent>,
    db: &Connection,
    rt_handle: &tokio::runtime::Handle,
    is_db_healthy: &Arc<AtomicBool>,
    persistence_rate: &Arc<AtomicU32>,
    is_private_mode: &Arc<AtomicBool>,
) {
    let mut writes_last_second = 0u32;
    let mut last_tick = Instant::now();

    loop {
        let event = match rx.recv_timeout(WORKER_EVENT_POLL_TIMEOUT) {
            Ok(e) => e,
            Err(crossbeam_channel::RecvTimeoutError::Timeout) => {
                maybe_flush_rate(&mut writes_last_second, &mut last_tick, persistence_rate);
                continue;
            }
            Err(crossbeam_channel::RecvTimeoutError::Disconnected) => {
                log::info!("[Persistence::Worker] Channel disconnected. Worker exiting");
                break;
            }
        };

        if matches!(event, PersistenceEvent::Shutdown) {
            log::info!("[Persistence::Worker] Shutdown event received. Exiting");
            break;
        }

        if is_private_mode.load(Ordering::Relaxed) {
            log_skipped_private_event(&event);
            continue;
        }

        let label = persistence_event_label(&event);
        let mut exit_loop = false;
        let mut attempt = 0;
        loop {
            attempt += 1;
            match rt_handle.block_on(process_event(db, event.clone())) {
                Ok(()) => {
                    is_db_healthy.store(true, Ordering::Relaxed);
                    writes_last_second += 1;
                    break;
                }
                Err(e) if e.to_string() == "SHUTDOWN" => {
                    exit_loop = true;
                    break;
                }
                Err(e) if attempt < 4 && VoxDb::is_retryable_anyhow(&e) => {
                    log::warn!(
                        "[Persistence::Worker] {} processing failed (attempt {}): {}; retrying",
                        label,
                        attempt,
                        e
                    );
                    std::thread::sleep(std::time::Duration::from_millis(30 * attempt));
                }
                Err(e) => {
                    is_db_healthy.store(false, Ordering::Relaxed);
                    log::error!(
                        "[Persistence::Worker] Event processing error ({}): {}",
                        label,
                        e
                    );
                    break;
                }
            }
        }
        if exit_loop {
            break;
        }

        maybe_flush_rate(&mut writes_last_second, &mut last_tick, persistence_rate);
    }
}

/// Returns the display label for an event so failures identify which event was dropped.
fn persistence_event_label(event: &PersistenceEvent) -> &'static str {
    match event {
        PersistenceEvent::SessionStarted { .. } => "SessionStarted",
        PersistenceEvent::SessionEnded { .. } => "SessionEnded",
        PersistenceEvent::TurnCompleted { .. } => "TurnCompleted",
        PersistenceEvent::StepCompleted { .. } => "StepCompleted",
        PersistenceEvent::ToolCallExecuted { .. } => "ToolCallExecuted",
        PersistenceEvent::UpdateSessionMetadata { .. } => "UpdateSessionMetadata",
        PersistenceEvent::Shutdown => "Shutdown",
    }
}

fn maybe_flush_rate(writes: &mut u32, last_tick: &mut Instant, rate_atomic: &Arc<AtomicU32>) {
    if last_tick.elapsed() >= PERSISTENCE_RATE_INTERVAL {
        rate_atomic.store((*writes as f32).to_bits(), Ordering::Relaxed);
        *writes = 0;
        *last_tick = Instant::now();
    }
}

fn log_skipped_private_event(event: &PersistenceEvent) {
    match event {
        PersistenceEvent::SessionStarted { session_id, .. } => {
            log::info!(
                "[Persistence::Worker] Private Mode active: skipping session start (session_id={})",
                session_id
            );
        }
        PersistenceEvent::TurnCompleted {
            session_id,
            turn_id,
            ..
        } => {
            log::info!(
                "[Persistence::Worker] Private Mode active: skipping turn record (session={}, turn={})",
                session_id,
                turn_id
            );
        }
        PersistenceEvent::StepCompleted {
            session_id,
            turn_id,
            step_index,
            ..
        } => {
            log::info!(
                "[Persistence::Worker] Private Mode active: skipping step record (session={}, turn={}, step={})",
                session_id,
                turn_id,
                step_index
            );
        }
        PersistenceEvent::ToolCallExecuted {
            session_id,
            turn_id,
            tool_name,
            ..
        } => {
            log::info!(
                "[Persistence::Worker] Private Mode active: skipping tool call record (session={}, turn={}, tool={})",
                session_id,
                turn_id,
                tool_name
            );
        }
        _ => {}
    }
}

async fn process_event(conn: &Connection, event: PersistenceEvent) -> anyhow::Result<()> {
    match event {
        PersistenceEvent::SessionStarted {
            session_id,
            timestamp_ms,
        } => {
            conn.execute(
                "INSERT OR IGNORE INTO sessions (id, project_id, is_pinned, created_at, updated_at)
                 VALUES (?, 'default', 0, ?, ?)",
                (session_id, timestamp_ms as i64, timestamp_ms as i64),
            )
            .await?;
            log::debug!(
                "[Persistence::Worker] SessionStarted: session_id={}",
                session_id
            );
        }
        PersistenceEvent::SessionEnded { session_id, .. } => {
            // Delete zero-turn session if it produced no turns
            let deleted = conn
                .execute(
                    "DELETE FROM sessions
                     WHERE id = ? AND (SELECT COUNT(*) FROM turns WHERE session_id = ?) = 0",
                    (session_id, session_id),
                )
                .await?;
            if deleted > 0 {
                log::info!(
                    "[Persistence::Worker] Cleaned up zero-turn session_id={}",
                    session_id
                );
            } else {
                log::debug!(
                    "[Persistence::Worker] SessionEnded: session_id={} (updated_at untouched; last-turn owned)",
                    session_id
                );
            }
        }
        PersistenceEvent::TurnCompleted {
            session_id,
            turn_id,
            user_text,
            assistant_text,
        } => {
            if session_id == 0 {
                return Ok(());
            }
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as i64;

            let mut attempts = 0;
            loop {
                attempts += 1;
                match conn.execute("BEGIN CONCURRENT;", ()).await {
                    Ok(_) => {}
                    Err(ref e) if attempts < 3 && VoxDb::is_retryable(e) => {
                        tokio::task::yield_now().await;
                        continue;
                    }
                    Err(e) => return Err(e.into()),
                }

                let res: Result<(), PersistenceError> = async {
                    conn.execute(
                        "INSERT OR IGNORE INTO sessions (id, project_id, is_pinned, created_at, updated_at)
                         VALUES (?, 'default', 0, ?, ?)",
                        (session_id, now, now),
                    )
                    .await?;

                    conn.execute(
                        "INSERT INTO turns (session_id, turn_id, user_text, assistant_text, created_at)
                         VALUES (?, ?, ?, ?, ?)",
                        (session_id, turn_id as i64, user_text.as_str(), assistant_text.as_str(), now),
                    )
                    .await?;

                    conn.execute(
                        "UPDATE sessions SET updated_at = ? WHERE id = ?",
                        (now, session_id),
                    )
                    .await?;

                    Ok(())
                }
                .await;

                match res {
                    Ok(_) => match conn.execute("COMMIT;", ()).await {
                        Ok(_) => break,
                        Err(ref e) if attempts < 3 && VoxDb::is_retryable(e) => {
                            let _ = conn.execute("ROLLBACK;", ()).await;
                            tokio::task::yield_now().await;
                            continue;
                        }
                        Err(e) => {
                            let _ = conn.execute("ROLLBACK;", ()).await;
                            return Err(e.into());
                        }
                    },
                    Err(e) => {
                        let _ = conn.execute("ROLLBACK;", ()).await;
                        return Err(e.into());
                    }
                }
            }

            log::info!(
                "[Persistence::Worker] TurnCompleted: session={}, turn={}",
                session_id,
                turn_id
            );
        }
        PersistenceEvent::UpdateSessionMetadata {
            session_id,
            key,
            value,
        } => {
            ensure_session_exists(conn, session_id).await?;
            match key.as_str() {
                "title" => {
                    conn.execute(
                        "UPDATE sessions SET title = ? WHERE id = ?",
                        (value, session_id),
                    )
                    .await?;
                }
                "project_id" => {
                    conn.execute(
                        "UPDATE sessions SET project_id = ? WHERE id = ?",
                        (value, session_id),
                    )
                    .await?;
                }
                "is_pinned" => {
                    let pinned = value == "true" || value == "1";
                    conn.execute(
                        "UPDATE sessions SET is_pinned = ? WHERE id = ?",
                        ((if pinned { 1i64 } else { 0i64 }), session_id),
                    )
                    .await?;
                }
                other => {
                    log::warn!(
                        "[Persistence::Worker] Unrecognized session metadata key '{}' for session_id={}",
                        other,
                        session_id
                    );
                }
            }
        }
        PersistenceEvent::StepCompleted { .. } => {
            super::steps::persist_step(conn, &event).await?;
        }
        PersistenceEvent::ToolCallExecuted { .. } => {
            persist_tool_call(conn, &event).await?;
        }
        PersistenceEvent::Shutdown => {
            log::info!("[Persistence::Worker] Shutdown event received. Exiting");
            return Err(anyhow::anyhow!("SHUTDOWN"));
        }
    }
    Ok(())
}
