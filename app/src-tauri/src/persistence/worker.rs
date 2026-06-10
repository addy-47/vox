use std::path::PathBuf;
use crossbeam_channel::{bounded, Sender};

use crate::persistence::db::VoxDb;
use crate::persistence::events::PersistenceEvent;
use crate::persistence::schema;

/// Spawn the persistence worker on a dedicated OS thread.
///
/// Returns a bounded SyncSender. The pipeline uses try_send() exclusively —
/// it never blocks. If the channel is full (128 events), the event is dropped
/// with a warning rather than stalling the real-time pipeline.
///
/// The worker thread is the ONLY thread that writes to SQLite.
/// All reads happen on separate connections in IPC handlers.
pub fn spawn_persistence_worker(
    db_path: PathBuf,
    is_db_healthy: std::sync::Arc<std::sync::atomic::AtomicBool>,
    persistence_rate: std::sync::Arc<std::sync::atomic::AtomicU32>,
    is_private_mode: std::sync::Arc<std::sync::atomic::AtomicBool>,
) -> Sender<PersistenceEvent> {
    // Bounded channel — 128 slots provides plenty of headroom before backpressure
    let (tx, rx) = bounded::<PersistenceEvent>(128);

    std::thread::Builder::new()
        .name("vox-persistence".to_string())
        .spawn(move || {
            // Open DB and run migrations
            let db = match VoxDb::open(&db_path) {
                Ok(d) => {
                    is_db_healthy.store(true, std::sync::atomic::Ordering::Relaxed);
                    d
                },
                Err(e) => {
                    is_db_healthy.store(false, std::sync::atomic::Ordering::Relaxed);
                    tracing::error!("[Persistence] Failed to open DB at {:?}: {}", db_path, e);
                    return;
                }
            };

            if let Err(e) = schema::run_migrations(&db.0) {
                tracing::error!("[Persistence] Migration failed: {}", e);
                return;
            }

            // Startup sweep: clean up zero-activity sessions from previous runs
            if let Err(e) = cleanup_zero_turn_sessions(&db.0) {
                tracing::warn!("[Persistence] Zero-turn startup cleanup failed (non-fatal): {}", e);
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
                            persistence_rate.store((writes_last_second as f32).to_bits(), std::sync::atomic::Ordering::Relaxed);
                            writes_last_second = 0;
                            last_tick = std::time::Instant::now();
                        }
                        continue;
                    },
                    Err(crossbeam_channel::RecvTimeoutError::Disconnected) => {
                        tracing::info!("[Persistence] Channel disconnected. Worker exiting.");
                        break;
                    }
                };

                // Respect Private Mode — skip all writes
                if is_private_mode.load(std::sync::atomic::Ordering::Relaxed) {
                    match &event {
                        PersistenceEvent::SessionStarted { id, .. } => {
                            tracing::info!("[Persistence] Private Mode active: skipping session start (id={})", id);
                        }
                        PersistenceEvent::TurnCompleted { conversation_id, turn_id, .. } => {
                            tracing::info!("[Persistence] Private Mode active: skipping turn record (session={}, turn={})", conversation_id, turn_id);
                        }
                        _ => {}
                    }
                    continue;
                }

                if let Err(e) = process_event(&db.0, event) {
                    if e.to_string() == "SHUTDOWN" {
                        break;
                    }
                    is_db_healthy.store(false, std::sync::atomic::Ordering::Relaxed);
                    // Log and continue — persistence errors must never crash the app
                    tracing::error!("[Persistence] Event processing error: {}", e);
                } else {
                    is_db_healthy.store(true, std::sync::atomic::Ordering::Relaxed);
                    writes_last_second += 1;
                }

                // Periodic rate update if we're busy
                if last_tick.elapsed() >= std::time::Duration::from_secs(1) {
                    persistence_rate.store((writes_last_second as f32).to_bits(), std::sync::atomic::Ordering::Relaxed);
                    writes_last_second = 0;
                    last_tick = std::time::Instant::now();
                }
            }
        })
        .expect("[Persistence] Failed to spawn worker thread");

    tx
}

fn process_event(conn: &rusqlite::Connection, event: PersistenceEvent) -> anyhow::Result<()> {
    match event {
        PersistenceEvent::SessionStarted { id, timestamp_ms } => {
            conn.execute(
                "INSERT OR IGNORE INTO sessions (id, started_at) VALUES (?1, ?2)",
                rusqlite::params![id as i64, timestamp_ms as i64],
            )?;
            tracing::debug!("[Persistence] SessionStarted: id={}", id);
        }

        PersistenceEvent::SessionEnded { id, timestamp_ms } => {
            conn.execute(
                "UPDATE sessions SET ended_at = ?1 WHERE id = ?2",
                rusqlite::params![timestamp_ms as i64, id as i64],
            )?;
            // Immediately delete zero-activity sessions (user engaged but never spoke)
            let deleted = conn.execute(
                "DELETE FROM sessions WHERE id = ?1 AND turn_count = 0",
                rusqlite::params![id as i64],
            )?;
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
            // If Privacy Mode was toggled mid-session, the SessionStarted event might have been skipped.
            conn.execute(
                "INSERT OR IGNORE INTO sessions (id, started_at) VALUES (?1, ?2)",
                rusqlite::params![conversation_id as i64, conversation_id as i64],
            )?;

            conn.execute(
                "INSERT INTO turns (session_id, turn_id, user_text, assistant_text, stt_latency_ms, ttft_ms, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                rusqlite::params![
                    conversation_id as i64,
                    turn_id,
                    user_text,
                    assistant_text,
                    stt_latency_ms,
                    ttft_ms,
                    now,
                ],
            )?;

            // Increment turn_count on parent session
            conn.execute(
                "UPDATE sessions SET turn_count = turn_count + 1 WHERE id = ?1",
                rusqlite::params![conversation_id as i64],
            )?;

            tracing::info!(
                "[Persistence] TurnCompleted: session={}, turn={}, stt={}ms, ttft={}ms",
                conversation_id, turn_id, stt_latency_ms, ttft_ms
            );
        }

        PersistenceEvent::TurnCancelled { conversation_id, turn_id } => {
            if conversation_id == 0 {
                return Ok(());
            }
            tracing::debug!("[Persistence] TurnCancelled: session={}, turn={}", conversation_id, turn_id);
            // Cancelled turns are not stored — they simply don't increment turn_count.
            // This is acceptable: the session record itself persists.
        }

        PersistenceEvent::Shutdown => {
            tracing::info!("[Persistence] Shutdown event received. Flushing WAL and exiting.");
            let _ = conn.execute("PRAGMA wal_checkpoint(TRUNCATE)", []);
            return Err(anyhow::anyhow!("SHUTDOWN"));
        }
    }
    Ok(())
}

/// Deletes sessions where the user engaged but never completed a turn.
/// These are clutter sessions (accidental engage, empty background noise, etc.).
/// Safe to call at startup or any time — idempotent.
fn cleanup_zero_turn_sessions(conn: &rusqlite::Connection) -> anyhow::Result<()> {
    let deleted = conn.execute(
        "DELETE FROM sessions WHERE turn_count = 0 AND ended_at IS NOT NULL",
        [],
    )?;
    if deleted > 0 {
        tracing::info!("[Persistence] Startup cleanup: removed {} zero-activity session(s)", deleted);
    }
    Ok(())
}
