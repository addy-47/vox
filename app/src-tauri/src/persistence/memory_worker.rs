use crossbeam_channel::{bounded, Sender};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use crate::services::memory::ProfileUpdate;

/// Events consumed exclusively by the background memory worker.
#[derive(Debug, Clone)]
pub enum MemoryWorkerEvent {
    /// A session has ended and its final compaction summary is ready for ingestion.
    SessionReadyForIngestion { session_id: u64, summary: String },
    /// User profile updates are ready for ingestion.
    ProfileUpdatesReady { updates: Vec<ProfileUpdate> },
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

    crate::services::memory::ensure_embedder_loaded(true)?;

    // Extract chunks: full summary + individual bullet/section chunks for crisp vector retrieval
    let mut chunks = Vec::new();
    chunks.push(summary.trim().to_string());

    for line in summary.lines() {
        let trimmed = line.trim().trim_start_matches('-').trim_start_matches('*').trim();
        if trimmed.len() > 15 && !trimmed.starts_with('#') {
            chunks.push(trimmed.to_string());
        }
    }

    let mut ingested_count = 0;
    let created_at = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;

    for chunk in chunks {
        if let Some(embedding) = crate::services::memory::generate_embedding(&chunk)? {
            let token_count = crate::services::memory::estimate_tokens(&chunk) as i64;
            let blob_bytes = encode_f32_blob(&embedding);

            conn.execute(
                "INSERT INTO episodes (session_id, summary, embedding, created_at, token_count)
                 VALUES (?, ?, ?, ?, ?)",
                (
                    session_id as i64,
                    chunk,
                    blob_bytes,
                    created_at,
                    token_count,
                ),
            )
            .await?;
            ingested_count += 1;
        }
    }

    if ingested_count > 0 {
        conn.execute(
            "UPDATE sessions SET embedding_status = 'embedded' WHERE id = ?",
            (session_id as i64,),
        )
        .await?;
        tracing::info!(
            "[MemoryWorker] Successfully ingested {} episodic memory chunks for session_id={}",
            ingested_count,
            session_id
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

/// Applies user profile updates to the personal_memory and personal_memory_history tables.
pub async fn apply_profile_updates(
    conn: &turso::Connection,
    updates: Vec<ProfileUpdate>,
) -> anyhow::Result<()> {
    if updates.is_empty() {
        return Ok(());
    }

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;

    let mut applied_count = 0;
    for update in updates {
        let mut key = update.key.trim().to_lowercase();
        let val = update.value.trim().to_string();
        let mut cat = update.category.trim().to_string();

        if key.is_empty() || val.is_empty() || cat.is_empty() {
            continue;
        }

        // Normalize common keys and categories to eliminate duplicate/redundant facts
        match key.as_str() {
            "name" | "username" | "user_name" => {
                key = "name".to_string();
                cat = "Identity".to_string();
            }
            "favorite_language" | "preferred_language" | "programming_language" | "fav_language" | "languages_used" => {
                key = "favorite_language".to_string();
                cat = "Preferences".to_string();
            }
            "disliked_language" | "hated_language" | "disliked_programming_language" => {
                key = "disliked_language".to_string();
                cat = "Preferences".to_string();
            }
            "current_project" | "project_name" | "project" => {
                key = "current_project".to_string();
                cat = "Projects".to_string();
            }
            "target_latency" | "latency_target" | "latency" | "latency_limit" | "project_goals" => {
                key = "target_latency".to_string();
                cat = "Goals".to_string();
            }
            "favorite_color" | "favourite_color" | "color" | "colour" => {
                key = "favorite_color".to_string();
                cat = "Preferences".to_string();
            }
            "role" | "technical_role" | "occupation" | "job" => {
                key = "role".to_string();
                cat = "Identity".to_string();
            }
            _ => {}
        }

        conn.execute(
            "INSERT OR REPLACE INTO personal_memory (key, category, value, updated_at)
             VALUES (?, ?, ?, ?)",
            (key.clone(), cat.clone(), val.clone(), now),
        )
        .await?;

        conn.execute(
            "INSERT INTO personal_memory_history (key, category, value, recorded_at)
             VALUES (?, ?, ?, ?)",
            (key, cat, val, now),
        )
        .await?;
        
        applied_count += 1;
    }

    tracing::info!(
        "[MemoryWorker] Successfully applied {} personal memory profile updates to Turso DB",
        applied_count
    );

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
                            state.current_session_id = 0;
                            tracing::debug!("[MemoryWorker] Pipeline transitioned to IDLE (active session cleared)");
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
                        MemoryWorkerEvent::ProfileUpdatesReady { updates } => {
                            if let Some(ref db_conn) = conn {
                                if let Err(e) = handle.block_on(async {
                                    apply_profile_updates(db_conn, updates).await
                                }) {
                                    tracing::error!(
                                        "[MemoryWorker] Failed to apply profile updates: {}", e
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

    #[test]
    fn test_profile_updates_ingestion() {
        let temp_dir = std::env::temp_dir().join("vox_profile_updates_test");
        let _ = std::fs::remove_dir_all(&temp_dir); // clean up
        std::fs::create_dir_all(&temp_dir).unwrap();
        let db_path = temp_dir.join("test_profile.db");
        let is_private = Arc::new(AtomicBool::new(false));

        // Run migrations first to create tables
        let rt = crate::persistence::db::get_tokio_handle();
        rt.block_on(async {
            let db = turso::Builder::new_local(db_path.to_str().unwrap()).build().await.unwrap();
            let conn = db.connect().unwrap();
            crate::persistence::schema::run_migrations(&conn).await.unwrap();
        });

        let tx = spawn_memory_worker(db_path.clone(), is_private);

        let updates = vec![
            crate::services::memory::ProfileUpdate {
                category: "Identity".to_string(),
                key: "name".to_string(),
                value: "Alex".to_string(),
            },
            crate::services::memory::ProfileUpdate {
                category: "Preferences".to_string(),
                key: "favorite_language".to_string(),
                value: "Rust".to_string(),
            },
        ];

        // Send profile updates
        assert!(tx.try_send(MemoryWorkerEvent::ProfileUpdatesReady { updates }).is_ok());

        // Shutdown to force sync
        assert!(tx.try_send(MemoryWorkerEvent::Shutdown).is_ok());

        // Wait a small moment for worker to complete writing and shut down
        std::thread::sleep(std::time::Duration::from_millis(500));

        // Verify database content
        let rt = crate::persistence::db::get_tokio_handle();
        rt.block_on(async {
            let db = turso::Builder::new_local(db_path.to_str().unwrap()).build().await.unwrap();
            let conn = db.connect().unwrap();
            
            // Query personal_memory
            let mut rows = conn.query("SELECT category, key, value FROM personal_memory ORDER BY key ASC", ()).await.unwrap();
            
            let row1 = rows.next().await.unwrap().unwrap();
            let cat1: String = row1.get(0).unwrap();
            let key1: String = row1.get(1).unwrap();
            let val1: String = row1.get(2).unwrap();
            assert_eq!(cat1, "Preferences");
            assert_eq!(key1, "favorite_language");
            assert_eq!(val1, "Rust");

            let row2 = rows.next().await.unwrap().unwrap();
            let cat2: String = row2.get(0).unwrap();
            let key2: String = row2.get(1).unwrap();
            let val2: String = row2.get(2).unwrap();
            assert_eq!(cat2, "Identity");
            assert_eq!(key2, "name");
            assert_eq!(val2, "Alex");

            // Query personal_memory_history
            let mut h_rows = conn.query("SELECT key, value FROM personal_memory_history ORDER BY key ASC", ()).await.unwrap();
            let h_row1 = h_rows.next().await.unwrap().unwrap();
            assert_eq!(h_row1.get::<String>(0).unwrap(), "favorite_language");
            assert_eq!(h_row1.get::<String>(1).unwrap(), "Rust");

            let h_row2 = h_rows.next().await.unwrap().unwrap();
            assert_eq!(h_row2.get::<String>(0).unwrap(), "name");
            assert_eq!(h_row2.get::<String>(1).unwrap(), "Alex");
        });
    }
}
