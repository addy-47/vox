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
    queue::reconcile_crashed_queue_on_boot, sessions::cleanup_zero_turn_sessions, PersistenceEvent,
    PERSISTENCE_CHANNEL_CAPACITY, PERSISTENCE_RATE_INTERVAL, WORKER_EVENT_POLL_TIMEOUT,
};
use crate::{
    core::error::PersistenceError,
    persistence::db::{get_tokio_handle, VoxDb},
};

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

        if is_private_mode.load(Ordering::Relaxed) {
            log_skipped_private_event(&event);
            continue;
        }

        if let Err(e) = rt_handle.block_on(process_event(db, event)) {
            if e.to_string() == "SHUTDOWN" {
                break;
            }
            is_db_healthy.store(false, Ordering::Relaxed);
            log::error!("[Persistence::Worker] Event processing error: {}", e);
        } else {
            is_db_healthy.store(true, Ordering::Relaxed);
            writes_last_second += 1;
        }

        maybe_flush_rate(&mut writes_last_second, &mut last_tick, persistence_rate);
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
        PersistenceEvent::SessionEnded {
            session_id,
            timestamp_ms,
        } => {
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
                conn.execute(
                    "UPDATE sessions SET updated_at = ? WHERE id = ?",
                    (timestamp_ms as i64, session_id),
                )
                .await?;
                log::debug!(
                    "[Persistence::Worker] SessionEnded: session_id={}",
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
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as i64;

            match key.as_str() {
                "title" => {
                    conn.execute(
                        "UPDATE sessions SET title = ?, updated_at = ? WHERE id = ?",
                        (value, now, session_id),
                    )
                    .await?;
                }
                "project_id" => {
                    conn.execute(
                        "UPDATE sessions SET project_id = ?, updated_at = ? WHERE id = ?",
                        (value, now, session_id),
                    )
                    .await?;
                }
                "is_pinned" => {
                    let pinned = value == "true" || value == "1";
                    conn.execute(
                        "UPDATE sessions SET is_pinned = ?, updated_at = ? WHERE id = ?",
                        (if pinned { 1i64 } else { 0i64 }, now, session_id),
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
        PersistenceEvent::Shutdown => {
            log::info!("[Persistence::Worker] Shutdown event received. Exiting");
            return Err(anyhow::anyhow!("SHUTDOWN"));
        }
    }
    Ok(())
}
