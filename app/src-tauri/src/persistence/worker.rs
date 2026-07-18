use crossbeam_channel::{bounded, Sender};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

use crate::persistence::db::VoxDb;
use crate::persistence::events::PersistenceEvent;
use crate::persistence::schema;

/// Spawn the persistence worker on a dedicated OS thread.
///
/// Returns a bounded SyncSender. The pipeline uses try_send() exclusively —
/// it never blocks. If the channel is full (128 events), the event is dropped
/// with a warning rather than stalling the real-time pipeline.
///
/// The worker thread is the ONLY thread that writes to the database.
/// All reads happen on separate connections in IPC handlers.
pub fn spawn_persistence_worker(
    db_path: PathBuf,
    is_db_healthy: Arc<AtomicBool>,
    persistence_rate: Arc<AtomicU32>,
    is_private_mode: Arc<AtomicBool>,
) -> Sender<PersistenceEvent> {
    let (tx, rx) = bounded::<PersistenceEvent>(128);

    std::thread::Builder::new()
        .name("vox-persistence".to_string())
        .spawn(move || {
            // Get the global/fallback Tokio runtime handle
            let rt_handle = crate::persistence::db::get_tokio_handle();

            let db = match rt_handle.block_on(VoxDb::open(&db_path)) {
                Ok(d) => {
                    is_db_healthy.store(true, Ordering::Relaxed);
                    d
                }
                Err(e) => {
                    is_db_healthy.store(false, Ordering::Relaxed);
                    tracing::error!("[Persistence] Failed to open DB at {:?}: {}", db_path, e);
                    return;
                }
            };

            if let Err(e) = rt_handle.block_on(schema::run_migrations(&db)) {
                tracing::error!("[Persistence] Migration failed: {}", e);
                return;
            }

            // Startup sweep: clean up zero-activity sessions from previous runs
            if let Err(e) = rt_handle.block_on(cleanup_zero_turn_sessions(&db)) {
                tracing::warn!(
                    "[Persistence] Zero-turn startup cleanup failed (non-fatal): {}",
                    e
                );
            }

            if let Err(e) = rt_handle.block_on(cleanup_stuck_queue_items(&db)) {
                tracing::warn!(
                    "[Persistence] Stuck queue items startup cleanup failed (non-fatal): {}",
                    e
                );
            }

            tracing::info!("[Persistence] Worker started. DB at {:?}", db_path);

            let mut writes_last_second = 0u32;
            let mut last_tick = std::time::Instant::now();

            // Main event loop — blocking recv, never panic
            loop {
                let event = match rx.recv_timeout(std::time::Duration::from_millis(100)) {
                    Ok(e) => e,
                    Err(crossbeam_channel::RecvTimeoutError::Timeout) => {
                        // Update rate at 1Hz
                        if last_tick.elapsed() >= std::time::Duration::from_secs(1) {
                            persistence_rate.store(
                                (writes_last_second as f32).to_bits(),
                                Ordering::Relaxed,
                            );
                            writes_last_second = 0;
                            last_tick = std::time::Instant::now();
                        }
                        continue;
                    }
                    Err(crossbeam_channel::RecvTimeoutError::Disconnected) => {
                        tracing::info!("[Persistence] Channel disconnected. Worker exiting.");
                        break;
                    }
                };

                // Respect Private Mode — skip all writes
                if is_private_mode.load(Ordering::Relaxed) {
                    match &event {
                        PersistenceEvent::SessionStarted { id, .. } => {
                            tracing::info!(
                                "[Persistence] Private Mode active: skipping session start (id={})",
                                id
                            );
                        }
                        PersistenceEvent::TurnCompleted {
                            conversation_id,
                            turn_id,
                            ..
                        } => {
                            tracing::info!(
                                "[Persistence] Private Mode active: skipping turn record (session={}, turn={})",
                                conversation_id,
                                turn_id
                            );
                        }
                        _ => {}
                    }
                    continue;
                }

                if let Err(e) = rt_handle.block_on(process_event(&db, event)) {
                    if e.to_string() == "SHUTDOWN" {
                        break;
                    }
                    is_db_healthy.store(false, Ordering::Relaxed);
                    // Log and continue — persistence errors must never crash the app
                    tracing::error!("[Persistence] Event processing error: {}", e);
                } else {
                    is_db_healthy.store(true, Ordering::Relaxed);
                    writes_last_second += 1;
                }

                // Periodic rate update if we're busy
                if last_tick.elapsed() >= std::time::Duration::from_secs(1) {
                    persistence_rate
                        .store((writes_last_second as f32).to_bits(), Ordering::Relaxed);
                    writes_last_second = 0;
                    last_tick = std::time::Instant::now();
                }
            }
        })
        .expect("[Persistence] Failed to spawn worker thread");

    tx
}

async fn process_event(conn: &turso::Connection, event: PersistenceEvent) -> anyhow::Result<()> {
    match event {
        PersistenceEvent::SessionStarted { id, timestamp_ms } => {
            conn.execute(
                "INSERT OR IGNORE INTO sessions (id, started_at) VALUES (?, ?)",
                (id as i64, timestamp_ms as i64),
            )
            .await?;
            tracing::debug!("[Persistence] SessionStarted: id={}", id);
        }

        PersistenceEvent::SessionEnded { id, timestamp_ms } => {
            conn.execute(
                "UPDATE sessions SET ended_at = ? WHERE id = ?",
                (timestamp_ms as i64, id as i64),
            )
            .await?;
            // Immediately delete zero-activity sessions (user engaged but never spoke)
            let deleted = conn
                .execute(
                    "DELETE FROM sessions WHERE id = ? AND turn_count = 0",
                    (id as i64,),
                )
                .await?;
            if deleted > 0 {
                tracing::info!("[Persistence] Cleaned up zero-activity session id={}", id);
            } else {
                tracing::debug!("[Persistence] SessionEnded: id={}", id);
            }
        }

        PersistenceEvent::TurnCompleted {
            conversation_id,
            turn_id,
            user_text,
            assistant_text,
            stt_latency_ms,
            ttft_ms,
        } => {
            // Ignore events from Tray mode (conversation_id == 0)
            if conversation_id == 0 {
                return Ok(());
            }
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as i64;

            // RCA Fix: Ensure session exists before inserting turn.
            conn.execute(
                "INSERT OR IGNORE INTO sessions (id, started_at) VALUES (?, ?)",
                (conversation_id as i64, conversation_id as i64),
            )
            .await?;

            conn.execute(
                "INSERT INTO turns (session_id, turn_id, user_text, assistant_text, stt_latency_ms, ttft_ms, created_at)
                 VALUES (?, ?, ?, ?, ?, ?, ?)",
                (
                    conversation_id as i64,
                    turn_id,
                    user_text,
                    assistant_text,
                    stt_latency_ms as i64,
                    ttft_ms as i64,
                    now,
                ),
            )
            .await?;

            // Increment turn_count on parent session
            conn.execute(
                "UPDATE sessions SET turn_count = turn_count + 1 WHERE id = ?",
                (conversation_id as i64,),
            )
            .await?;

            tracing::info!(
                "[Persistence] TurnCompleted: session={}, turn={}, stt={:?}ms, ttft={:?}ms",
                conversation_id,
                turn_id,
                stt_latency_ms,
                ttft_ms
            );
        }

        PersistenceEvent::TurnCancelled {
            conversation_id,
            turn_id,
        } => {
            if conversation_id == 0 {
                return Ok(());
            }
            tracing::debug!(
                "[Persistence] TurnCancelled: session={}, turn={}",
                conversation_id,
                turn_id
            );
        }

        PersistenceEvent::Shutdown => {
            tracing::info!("[Persistence] Shutdown event received. Exiting.");
            return Err(anyhow::anyhow!("SHUTDOWN"));
        }
    }
    Ok(())
}

/// Deletes sessions where the user engaged but never completed a turn.
async fn cleanup_zero_turn_sessions(conn: &turso::Connection) -> anyhow::Result<()> {
    let deleted = conn.execute("DELETE FROM sessions WHERE turn_count = 0", ()).await?;
    if deleted > 0 {
        tracing::info!(
            "[Persistence] Startup cleanup: removed {} zero-activity session(s)",
            deleted
        );
    }
    Ok(())
}

/// Resets memory queue items stuck in 'processing' status to 'pending' (Bug #3).
async fn cleanup_stuck_queue_items(conn: &turso::Connection) -> anyhow::Result<()> {
    let reset = conn.execute("UPDATE personal_memory_queue SET status = 'pending' WHERE status = 'processing'", ()).await?;
    if reset > 0 {
        tracing::info!("[Persistence] Startup cleanup: reset {} stuck memory queue item(s) to pending", reset);
    }
    Ok(())
}
