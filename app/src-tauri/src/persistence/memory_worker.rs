use crossbeam_channel::{bounded, Sender};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};
use std::collections::HashMap;
use crate::core::settings::{VoxSettings, MemorySettings};
use crate::core::constants::{
    PM_RELATION_CONFLICTS, PM_RELATION_SUPPORTS, PM_QUEUE_STATUS_PENDING,
    PM_QUEUE_STATUS_DONE
};

/// Events consumed exclusively by the background memory worker.
#[derive(Debug, Clone)]
pub enum MemoryWorkerEvent {
    /// A session has ended and its final compaction summary is ready for ingestion.
    SessionReadyForIngestion { session_id: u64, summary: String },
    /// v2: Extracted facts from compaction — enqueued to personal_memory_queue
    PersonalFactsReady {
        facts: HashMap<String, Vec<String>>,  // collection → facts
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

async fn mark_job_failed(conn: &turso::Connection, job_id: i64, err_msg: &str) {
    let _ = conn.execute(
        "UPDATE personal_memory_queue SET status = 'failed', error_msg = ?, attempts = attempts + 1 WHERE id = ?",
        (err_msg.to_string(), job_id),
    ).await;
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

/// Enqueues new personal memory facts to the personal_memory_queue SQLite table.
pub async fn enqueue_personal_facts(
    conn: &turso::Connection,
    facts: HashMap<String, Vec<String>>,
    session_id: &str,
) -> anyhow::Result<()> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;

    for (collection, fact_list) in facts {
        for fact in fact_list {
            let trimmed = fact.trim();
            if trimmed.is_empty() {
                continue;
            }
            conn.execute(
                "INSERT INTO personal_memory_queue (fact, collection, source, session_id, status, created_at)
                 VALUES (?, ?, 'LLM', ?, ?, ?)",
                (
                    trimmed.to_string(),
                    collection.clone(),
                    session_id.to_string(),
                    PM_QUEUE_STATUS_PENDING.to_string(),
                    now,
                ),
            )
            .await?;
        }
    }
    Ok(())
}

/// Processes one pending job from the personal_memory_queue.
pub async fn process_one_queue_item(
    conn: &turso::Connection,
    settings: &MemorySettings,
) -> anyhow::Result<bool> {
    let mut rows = conn
        .query(
            "SELECT id, fact, collection, source, session_id FROM personal_memory_queue 
             WHERE status = 'pending' ORDER BY created_at ASC LIMIT 1",
            (),
        )
        .await?;

    let item = if let Some(row) = rows.next().await? {
        Some((
            row.get::<i64>(0)?,
            row.get::<String>(1)?,
            row.get::<String>(2)?,
            row.get::<String>(3)?,
            row.get::<String>(4)?,
        ))
    } else {
        None
    };

    let (job_id, fact, collection, source, session_id) = match item {
        Some(x) => x,
        None => return Ok(false),
    };

    conn.execute(
        "UPDATE personal_memory_queue SET status = 'processing' WHERE id = ?",
        (job_id,),
    )
    .await?;

    crate::services::memory::ensure_embedder_loaded(true)?;
    let embedding = match crate::services::memory::generate_embedding(&fact) {
        Ok(Some(v)) => v,
        Ok(None) => {
            mark_job_failed(conn, job_id, "Embedding generator returned None (not loaded)").await;
            return Ok(true);
        }
        Err(e) => {
            mark_job_failed(conn, job_id, &format!("Embedding generation failed: {}", e)).await;
            return Ok(true);
        }
    };

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;
    let fact_id = format!("mem_{}_{}", now, uuid::Uuid::new_v4().simple());

    if let Err(e) = conn.execute(
        "INSERT INTO memory_facts (id, collection, fact, source, created_at, session_id) VALUES (?, ?, ?, ?, ?, ?)",
        (fact_id.clone(), collection.clone(), fact.clone(), source.clone(), now, session_id.clone()),
    ).await {
        mark_job_failed(conn, job_id, &format!("Failed to insert memory_fact: {}", e)).await;
        return Ok(true);
    }

    let blob_bytes = encode_f32_blob(&embedding);
    let vector_rowid = match conn.execute(
        "INSERT INTO memory_facts_vectors (fact_id, collection, embedding) VALUES (?, ?, ?)",
        (fact_id.clone(), collection.clone(), blob_bytes),
    ).await {
        Ok(id) => id,
        Err(e) => {
            mark_job_failed(conn, job_id, &format!("Failed to insert memory_fact_vector: {}", e)).await;
            return Ok(true);
        }
    };

    let _ = conn.execute(
        "UPDATE memory_facts SET embedding_id = ? WHERE id = ?",
        (vector_rowid as i64, fact_id.clone()),
    ).await;

    // Fetch candidate facts in same collection to compare with NLI
    let mut cand_rows = match conn.query(
        "SELECT mf.id, mf.fact, mfv.embedding FROM memory_facts mf
         JOIN memory_facts_vectors mfv ON mfv.fact_id = mf.id
         WHERE mf.collection = ? AND mf.id != ?",
        (collection.clone(), fact_id.clone()),
    ).await {
        Ok(r) => r,
        Err(e) => {
            mark_job_failed(conn, job_id, &format!("Failed to fetch vector candidates: {}", e)).await;
            return Ok(true);
        }
    };

    let mut scored_candidates = Vec::new();
    while let Some(row) = cand_rows.next().await? {
        let id: String = row.get(0)?;
        let f_text: String = row.get(1)?;
        let emb_blob: Vec<u8> = row.get(2)?;
        let emb_vector = decode_f32_blob(&emb_blob);
        let sim = crate::services::memory::cosine_similarity(&embedding, &emb_vector);
        scored_candidates.push((sim, (id, f_text)));
    }

    scored_candidates.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    let candidates: Vec<(String, String)> = scored_candidates
        .into_iter()
        .take(settings.nli_candidate_limit as usize)
        .map(|(_, item)| item)
        .collect();

    let mut relations = Vec::new();
    if !candidates.is_empty() {
        if let Err(e) = crate::services::memory::nli::ensure_nli_loaded(&settings.nli_model_name) {
            log::warn!("[MemoryWorker] Failed to load NLI model: {}. Skipping NLI validation.", e);
        } else {
            for (cand_id, cand_fact) in candidates {
                let start_time = Instant::now();
                match crate::services::memory::nli::classify_pair(&fact, &cand_fact) {
                    Ok(nli_res) => {
                        let duration = start_time.elapsed().as_millis();
                        let use_degraded = duration > 50;

                        let relation = if use_degraded {
                            let cand_embedding = match fetch_embedding_by_fact_id(conn, &cand_id).await {
                                Ok(Some(v)) => v,
                                _ => continue,
                            };
                            let sim = crate::services::memory::embedder::cosine_similarity(&embedding, &cand_embedding);
                            if sim >= settings.cosine_auto_support_threshold {
                                crate::services::memory::nli::NliRelation::Supports
                            } else {
                                crate::services::memory::nli::NliRelation::Neutral
                            }
                        } else {
                            crate::services::memory::nli::relation_from_result(&nli_res, settings)
                        };

                        match relation {
                            crate::services::memory::nli::NliRelation::Conflicts => {
                                relations.push((fact_id.clone(), cand_id.clone(), PM_RELATION_CONFLICTS));
                            }
                            crate::services::memory::nli::NliRelation::Supports => {
                                relations.push((fact_id.clone(), cand_id.clone(), PM_RELATION_SUPPORTS));
                            }
                            crate::services::memory::nli::NliRelation::Neutral => {}
                        }
                    }
                    Err(e) => {
                        log::error!("[MemoryWorker] NLI classification failed: {}", e);
                    }
                }
            }
        }
    }

    for (from, to, rel) in relations {
        let _ = conn.execute(
            "INSERT OR IGNORE INTO memory_relations (from_id, to_id, relation, created_at) VALUES (?, ?, ?, ?)",
            (from, to, rel.to_string(), now),
        ).await;
    }

    conn.execute(
        "UPDATE personal_memory_queue SET status = ?, processed_at = ? WHERE id = ?",
        (PM_QUEUE_STATUS_DONE.to_string(), now, job_id),
    )
    .await?;

    Ok(true)
}

async fn fetch_embedding_by_fact_id(conn: &turso::Connection, id: &str) -> anyhow::Result<Option<Vec<f32>>> {
    let mut rows = conn
        .query(
            "SELECT embedding FROM memory_facts_vectors WHERE fact_id = ?",
            (id.to_string(),),
        )
        .await?;

    if let Some(row) = rows.next().await? {
        let bytes: Vec<u8> = row.get(0)?;
        Ok(Some(decode_f32_blob(&bytes)))
    } else {
        Ok(None)
    }
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

/// Spawns the dedicated background memory worker thread.
pub fn spawn_memory_worker(
    db_path: PathBuf,
    is_private_mode: Arc<AtomicBool>,
    settings: Arc<RwLock<VoxSettings>>,
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
                        }
                        MemoryWorkerEvent::PipelineIdle => {
                            state.is_idle = true;
                            state.current_session_id = 0;
                        }
                        MemoryWorkerEvent::PipelineActive => {
                            state.is_idle = false;
                        }
                        MemoryWorkerEvent::SessionReadyForIngestion { session_id, summary } => {
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
                        MemoryWorkerEvent::PersonalFactsReady { facts, session_id } => {
                            if let Some(ref db_conn) = conn {
                                if let Err(e) = handle.block_on(async {
                                    enqueue_personal_facts(db_conn, facts, &session_id).await
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
                    if let Some(ref db_conn) = conn {
                        let memory_settings = match settings.read() {
                            Ok(s) => s.memory.clone(),
                            Err(_) => {
                                tracing::error!("[MemoryWorker] Settings lock poisoned! Skipping loop iteration.");
                                std::thread::sleep(std::time::Duration::from_millis(100));
                                continue;
                            }
                        };

                        let mut processed_any = false;
                        loop {
                            let processed_queue = handle.block_on(async {
                                process_one_queue_item(db_conn, &memory_settings).await
                            });

                            match processed_queue {
                                Ok(true) => {
                                    processed_any = true;
                                    // Process next item immediately
                                    continue;
                                }
                                Ok(false) => {
                                    break;
                                }
                                Err(e) => {
                                    tracing::error!("[MemoryWorker] Failed processing queue item: {}", e);
                                    break;
                                }
                            }
                        }

                        if !processed_any {
                            let _ = handle.block_on(async {
                                sweep_next_pending_session(db_conn, state.current_session_id).await
                            });
                        }
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
}
