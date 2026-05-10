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
pub fn spawn_persistence_worker(db_path: PathBuf) -> Sender<PersistenceEvent> {
    // Bounded channel — 128 slots provides plenty of headroom before backpressure
    let (tx, rx) = bounded::<PersistenceEvent>(128);

    std::thread::Builder::new()
        .name("vox-persistence".to_string())
        .spawn(move || {
            // Open DB and run migrations
            let db = match VoxDb::open(&db_path) {
                Ok(d) => d,
                Err(e) => {
                    tracing::error!("[Persistence] Failed to open DB at {:?}: {}", db_path, e);
                    return;
                }
            };

            if let Err(e) = schema::run_migrations(&db.0) {
                tracing::error!("[Persistence] Migration failed: {}", e);
                return;
            }

            tracing::info!("[Persistence] Worker started. DB at {:?}", db_path);

            // Main event loop — blocking recv, never panic
            loop {
                let event = match rx.recv() {
                    Ok(e) => e,
                    Err(_) => {
                        tracing::info!("[Persistence] Channel disconnected. Worker exiting.");
                        break;
                    }
                };

                if let Err(e) = process_event(&db.0, event) {
                    // Log and continue — persistence errors must never crash the app
                    tracing::error!("[Persistence] Event processing error: {}", e);
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
            tracing::debug!("[Persistence] SessionEnded: id={}", id);
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
            tracing::info!("[Persistence] Shutdown event received. Exiting worker.");
            // The recv loop will exit naturally when the sender is dropped.
            // This variant is here for explicit shutdown sequencing if needed.
        }
    }
    Ok(())
}
