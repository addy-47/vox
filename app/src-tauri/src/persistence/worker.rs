use crossbeam_channel::{bounded, Receiver, Sender};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Instant;

use crate::persistence::db::VoxDb;
use crate::persistence::events::PersistenceEvent;
use crate::persistence::schema;
use crate::persistence::{
    PERSISTENCE_CHANNEL_CAPACITY, PERSISTENCE_RATE_INTERVAL, WORKER_EVENT_POLL_TIMEOUT,
};

/// Spawn the persistence worker on a dedicated OS thread.
pub fn spawn_persistence_worker(
    db_path: PathBuf,
    is_db_healthy: Arc<AtomicBool>,
    persistence_rate: Arc<AtomicU32>,
    is_private_mode: Arc<AtomicBool>,
) -> Sender<PersistenceEvent> {
    let (tx, rx) = bounded::<PersistenceEvent>(PERSISTENCE_CHANNEL_CAPACITY);

    std::thread::Builder::new()
        .name("vox-persistence".to_string())
        .spawn(move || {
            let rt_handle = crate::persistence::db::get_tokio_handle();

            let db = match rt_handle.block_on(VoxDb::open(&db_path)) {
                Ok(d) => {
                    is_db_healthy.store(true, Ordering::Relaxed);
                    d
                }
                Err(e) => {
                    is_db_healthy.store(false, Ordering::Relaxed);
                    log::error!("[Persistence::Worker] Failed to open DB at {:?}: {}", db_path, e);
                    return;
                }
            };

            if let Err(e) = rt_handle.block_on(schema::run_migrations(&db)) {
                log::error!("[Persistence::Worker] Migration failed: {}", e);
                return;
            }

            run_startup_sweeps(&db, &rt_handle);
            log::info!("[Persistence::Worker] Worker started. DB at {:?}", db_path);

            run_event_loop(
                rx,
                &db,
                &rt_handle,
                &is_db_healthy,
                &persistence_rate,
                &is_private_mode,
            );
        })
        .expect("[Persistence::Worker] Failed to spawn worker thread");

    tx
}

fn run_startup_sweeps(db: &turso::Connection, rt_handle: &tokio::runtime::Handle) {
    if let Err(e) = rt_handle.block_on(cleanup_zero_turn_sessions(db)) {
        log::warn!(
            "[Persistence::Worker] Zero-turn startup cleanup failed (non-fatal): {}",
            e
        );
    }

    if let Err(e) = rt_handle.block_on(cleanup_stuck_queue_items(db)) {
        log::warn!(
            "[Persistence::Worker] Stuck queue items startup cleanup failed (non-fatal): {}",
            e
        );
    }
}

fn run_event_loop(
    rx: Receiver<PersistenceEvent>,
    db: &turso::Connection,
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

fn maybe_flush_rate(
    writes: &mut u32,
    last_tick: &mut Instant,
    rate_atomic: &Arc<AtomicU32>,
) {
    if last_tick.elapsed() >= PERSISTENCE_RATE_INTERVAL {
        rate_atomic.store((*writes as f32).to_bits(), Ordering::Relaxed);
        *writes = 0;
        *last_tick = Instant::now();
    }
}

fn log_skipped_private_event(event: &PersistenceEvent) {
    match event {
        PersistenceEvent::SessionStarted { id, .. } => {
            log::info!("[Persistence::Worker] Private Mode active: skipping session start (id={})", id);
        }
        PersistenceEvent::TurnCompleted {
            conversation_id,
            turn_id,
            ..
        } => {
            log::info!(
                "[Persistence::Worker] Private Mode active: skipping turn record (session={}, turn={})",
                conversation_id,
                turn_id
            );
        }
        _ => {}
    }
}

async fn process_event(conn: &turso::Connection, event: PersistenceEvent) -> anyhow::Result<()> {
    match event {
        PersistenceEvent::SessionStarted { id, timestamp_ms } => {
            conn.execute(
                "INSERT OR IGNORE INTO sessions (id, started_at) VALUES (?, ?)",
                (id as i64, timestamp_ms as i64),
            )
            .await?;
            log::debug!("[Persistence::Worker] SessionStarted: id={}", id);
        }
        PersistenceEvent::SessionEnded { id, timestamp_ms } => {
            let deleted = conn
                .execute(
                    "DELETE FROM sessions WHERE id = ? AND turn_count = 0",
                    (id as i64,),
                )
                .await?;
            if deleted > 0 {
                log::info!("[Persistence::Worker] Cleaned up zero-activity session id={}", id);
            } else {
                conn.execute(
                    "UPDATE sessions SET ended_at = ? WHERE id = ? AND turn_count > 0",
                    (timestamp_ms as i64, id as i64),
                )
                .await?;
                log::debug!("[Persistence::Worker] SessionEnded: id={}", id);
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
            if conversation_id == 0 {
                return Ok(());
            }
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as i64;

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

            conn.execute(
                "UPDATE sessions SET turn_count = turn_count + 1 WHERE id = ?",
                (conversation_id as i64,),
            )
            .await?;

            log::info!(
                "[Persistence::Worker] TurnCompleted: session={}, turn={}, stt={:?}ms, ttft={:?}ms",
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
            log::debug!(
                "[Persistence::Worker] TurnCancelled: session={}, turn={}",
                conversation_id,
                turn_id
            );
        }
        PersistenceEvent::Shutdown => {
            log::info!("[Persistence::Worker] Shutdown event received. Exiting");
            return Err(anyhow::anyhow!("SHUTDOWN"));
        }
    }
    Ok(())
}

async fn cleanup_zero_turn_sessions(conn: &turso::Connection) -> anyhow::Result<()> {
    let deleted = conn
        .execute("DELETE FROM sessions WHERE turn_count = 0", ())
        .await?;
    if deleted > 0 {
        log::info!(
            "[Persistence::Worker] Startup cleanup: removed {} zero-activity session(s)",
            deleted
        );
    }
    Ok(())
}

async fn cleanup_stuck_queue_items(conn: &turso::Connection) -> anyhow::Result<()> {
    let reset = conn
        .execute(
            "UPDATE personal_memory_queue SET status = 'staged_pending' WHERE status = 'processing'",
            (),
        )
        .await?;
    if reset > 0 {
        log::info!(
            "[Persistence::Worker] Startup cleanup: reset {} stuck memory queue item(s) to staged_pending",
            reset
        );
    }
    Ok(())
}

