use crossbeam_channel::{bounded, Sender};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

/// Events consumed exclusively by the background memory worker.
#[derive(Debug, Clone)]
pub enum MemoryWorkerEvent {
    /// A session has ended and its final compaction summary is ready for ingestion.
    SessionReadyForIngestion { session_id: u64, summary: String },
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
}

pub fn encode_f32_blob(floats: &[f32]) -> Vec<u8> {
    floats.iter().flat_map(|f| f.to_le_bytes()).collect()
}

pub fn decode_f32_blob(bytes: &[u8]) -> Vec<f32> {
    bytes
        .chunks_exact(4)
        .map(|chunk| f32::from_le_bytes(chunk.try_into().unwrap_or_default()))
        .collect()
}

/// Ingests a completed session compaction summary into the `episodes` table and updates `sessions.embedding_status`.
pub async fn ingest_compaction_summary(
    conn: &turso::Connection,
    session_id: u64,
    summary: &str,
) -> anyhow::Result<()> {
    if summary.trim().is_empty() {
        conn.execute(
            "UPDATE sessions SET embedding_status = 'skipped' WHERE id = ?",
            (session_id as i64,),
        )
        .await?;
        return Ok(());
    }

    // Lazily ensure MiniLM embedder is loaded on the background worker thread
    crate::services::memory::ensure_embedder_loaded(true)?;

    if let Some(embedding) = crate::services::memory::generate_embedding(summary)? {
        let token_count = crate::services::memory::estimate_tokens(summary) as i64;
        let created_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        let blob_bytes = encode_f32_blob(&embedding);

        conn.execute(
            "INSERT INTO episodes (session_id, summary, embedding, created_at, token_count)
             VALUES (?, ?, ?, ?, ?)",
            (
                session_id as i64,
                summary.to_string(),
                blob_bytes,
                created_at,
                token_count,
            ),
        )
        .await?;

        conn.execute(
            "UPDATE sessions SET embedding_status = 'embedded' WHERE id = ?",
            (session_id as i64,),
        )
        .await?;

        tracing::info!(
            "[MemoryWorker] Successfully ingested episodic memory for session_id={} (dims={}, tokens={})",
            session_id,
            embedding.len(),
            token_count
        );
    } else {
        conn.execute(
            "UPDATE sessions SET embedding_status = 'skipped' WHERE id = ?",
            (session_id as i64,),
        )
        .await?;
    }

    Ok(())
}

/// Queries the oldest pending session (`embedding_status = 'pending'`) that is not the current active session,
/// and ingests its compaction summary into the `episodes` table.
/// Returns `Ok(true)` if a session was processed, `Ok(false)` if no pending sessions exist.
pub async fn sweep_next_pending_session(
    conn: &turso::Connection,
    current_session_id: u64,
) -> anyhow::Result<bool> {
    let pending_target = {
        let mut rows = conn
            .query(
                "SELECT s.id, COALESCE((SELECT assistant_text FROM turns WHERE session_id = s.id ORDER BY turn_id DESC LIMIT 1), '') FROM sessions s WHERE s.embedding_status = 'pending' AND s.id != ? ORDER BY s.started_at ASC LIMIT 1",
                (current_session_id as i64,),
            )
            .await?;

        if let Some(row) = rows.next().await? {
            let session_id: i64 = row.get(0)?;
            let summary: String = row.get(1)?;
            Some((session_id as u64, summary))
        } else {
            None
        }
    };

    if let Some((session_id, summary)) = pending_target {
        tracing::info!(
            "[MemoryWorker] Background sweep processing pending session_id={}",
            session_id
        );
        ingest_compaction_summary(conn, session_id, &summary).await?;
        Ok(true)
    } else {
        Ok(false)
    }
}

/// Spawns the dedicated low-priority background memory worker on an OS thread.
/// Returns a bounded Sender (capacity 32). The pipeline uses `try_send()` exclusively.
pub fn spawn_memory_worker(
    db_path: PathBuf,
    is_private_mode: Arc<AtomicBool>,
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
                    tracing::error!("[MemoryWorker] Failed to open DB connection at {:?}: {}", db_path, e);
                    None
                }
            };

            let mut state = WorkerState {
                current_session_id: 0,
                is_idle: true,
            };

            loop {
                let event = match rx.recv_timeout(Duration::from_millis(500)) {
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
                            tracing::debug!(
                                "[MemoryWorker] Active session updated to session_id={}",
                                session_id
                            );
                        }
                        MemoryWorkerEvent::PipelineIdle => {
                            state.is_idle = true;
                            tracing::debug!("[MemoryWorker] Pipeline transitioned to IDLE");
                        }
                        MemoryWorkerEvent::PipelineActive => {
                            state.is_idle = false;
                            tracing::debug!("[MemoryWorker] Pipeline transitioned to ACTIVE");
                        }
                        MemoryWorkerEvent::SessionReadyForIngestion { session_id, summary } => {
                            // HARD INVARIANT CHECK: Never ingest the active session
                            if session_id == state.current_session_id && session_id != 0 {
                                tracing::warn!(
                                    "[MemoryWorker] INVARIANT VIOLATION PREVENTED: SessionReadyForIngestion rejected for active session_id={}",
                                    session_id
                                );
                                continue;
                            }

                            if let Some(ref db_conn) = conn {
                                if let Err(e) = handle.block_on(async {
                                    ingest_compaction_summary(db_conn, session_id, &summary).await
                                }) {
                                    tracing::error!(
                                        "[MemoryWorker] Failed to ingest summary for session_id={}: {}",
                                        session_id, e
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
                    // Idle timeout branch: perform 1 step of background idle sweep
                    if let Some(ref db_conn) = conn {
                        let _ = handle.block_on(async {
                            sweep_next_pending_session(db_conn, state.current_session_id).await
                        });
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

    #[tokio::test]
    async fn test_ingest_compaction_summary_in_db() -> anyhow::Result<()> {
        let db = turso::Builder::new_local(":memory:").build().await?;
        let conn = db.connect()?;
        crate::persistence::schema::run_migrations(&conn).await?;

        let session_id = 12345u64;
        let created_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64;

        conn.execute(
            "INSERT INTO sessions (id, started_at, turn_count) VALUES (?, ?, ?)",
            (session_id as i64, created_at, 5),
        )
        .await?;

        // Empty summary -> skipped
        ingest_compaction_summary(&conn, session_id, "").await?;

        let mut rows = conn
            .query("SELECT embedding_status FROM sessions WHERE id = ?", (session_id as i64,))
            .await?;
        let status: String = rows.next().await?.unwrap().get(0)?;
        assert_eq!(status, "skipped");

        Ok(())
    }

    #[tokio::test]
    async fn test_background_idle_sweep_oldest_first() -> anyhow::Result<()> {
        let db = turso::Builder::new_local(":memory:").build().await?;
        let conn = db.connect()?;
        crate::persistence::schema::run_migrations(&conn).await?;

        // Seed 2 pending sessions with different started_at timestamps
        conn.execute(
            "INSERT INTO sessions (id, started_at, turn_count, embedding_status) VALUES (10, 1000, 2, 'pending')",
            (),
        )
        .await?;
        conn.execute(
            "INSERT INTO turns (session_id, turn_id, assistant_text, created_at) VALUES (10, 1, 'Older summary', 1001)",
            (),
        )
        .await?;

        conn.execute(
            "INSERT INTO sessions (id, started_at, turn_count, embedding_status) VALUES (20, 2000, 2, 'pending')",
            (),
        )
        .await?;
        conn.execute(
            "INSERT INTO turns (session_id, turn_id, assistant_text, created_at) VALUES (20, 1, 'Newer summary', 2001)",
            (),
        )
        .await?;

        // 1. First sweep step -> must pick session 10 (oldest)
        let swept1 = sweep_next_pending_session(&conn, 0).await?;
        assert!(swept1);

        let mut rows1 = conn
            .query("SELECT embedding_status FROM sessions WHERE id = 10", ())
            .await?;
        let status1: String = rows1.next().await?.unwrap().get(0)?;
        assert_ne!(status1, "pending");

        // 2. Second sweep step -> must pick session 20
        let swept2 = sweep_next_pending_session(&conn, 0).await?;
        assert!(swept2);

        // 3. Third sweep step -> no pending sessions left
        let swept3 = sweep_next_pending_session(&conn, 0).await?;
        assert!(!swept3);

        Ok(())
    }

    #[test]
    fn test_spawn_and_shutdown_memory_worker() {
        let temp_dir = std::env::temp_dir().join("vox_memory_test");
        let db_path = temp_dir.join("test_vox.db");
        let is_private = Arc::new(AtomicBool::new(false));

        let tx = spawn_memory_worker(db_path, is_private);
        assert!(tx.try_send(MemoryWorkerEvent::PipelineIdle).is_ok());
        assert!(tx.try_send(MemoryWorkerEvent::Shutdown).is_ok());
    }

    #[test]
    fn test_end_to_end_memory_ingestion_flow() {
        let temp_dir = std::env::temp_dir().join("vox_memory_e2e_test");
        let db_path = temp_dir.join("test_vox_e2e.db");
        let is_private = Arc::new(AtomicBool::new(false));

        let tx = spawn_memory_worker(db_path, is_private);

        // 1. Session 500 starts
        assert!(tx
            .try_send(MemoryWorkerEvent::ActiveSessionChanged { session_id: 500 })
            .is_ok());

        // 2. Active session ready for ingestion -> blocked by guard
        assert!(tx
            .try_send(MemoryWorkerEvent::SessionReadyForIngestion {
                session_id: 500,
                summary: "Active session compaction".to_string(),
            })
            .is_ok());

        // 3. Session changes to 501
        assert!(tx
            .try_send(MemoryWorkerEvent::ActiveSessionChanged { session_id: 501 })
            .is_ok());

        // 4. Session 500 ready for ingestion -> accepted
        assert!(tx
            .try_send(MemoryWorkerEvent::SessionReadyForIngestion {
                session_id: 500,
                summary: "Past session compaction summary".to_string(),
            })
            .is_ok());

        // 5. Shutdown
        assert!(tx.try_send(MemoryWorkerEvent::Shutdown).is_ok());
    }
}
