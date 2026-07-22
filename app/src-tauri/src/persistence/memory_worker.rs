use crossbeam_channel::{bounded, Sender};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock};
use std::time::Duration;
use std::collections::HashMap;
use crate::core::settings::{VoxSettings, MemorySettings};
use crate::core::constants::{
    PM_RELATION_CONFLICTS, PM_RELATION_SUPPORTS, PM_RELATION_SIMILAR, PM_RELATION_MERGED,
    PM_QUEUE_STATUS_PENDING, PM_QUEUE_STATUS_STAGED, PM_QUEUE_STATUS_COMPLETED, collection_type
};

/// Events consumed exclusively by the background memory worker.
#[derive(Debug, Clone)]
pub enum MemoryWorkerEvent {
    /// A session has ended. Trigger the consolidation sweep.
    SessionEnd { session_id: String, summary: String },
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

pub fn jaccard_similarity(s1: &str, s2: &str) -> f32 {
    let w1: std::collections::HashSet<String> = s1
        .to_lowercase()
        .split_whitespace()
        .map(|s| s.replace(|c: char| !c.is_alphanumeric(), ""))
        .filter(|s| !s.is_empty())
        .collect();
    let w2: std::collections::HashSet<String> = s2
        .to_lowercase()
        .split_whitespace()
        .map(|s| s.replace(|c: char| !c.is_alphanumeric(), ""))
        .filter(|s| !s.is_empty())
        .collect();

    if w1.is_empty() && w2.is_empty() {
        return 1.0;
    }
    if w1.is_empty() || w2.is_empty() {
        return 0.0;
    }

    let intersection = w1.intersection(&w2).count() as f32;
    let union = w1.union(&w2).count() as f32;
    intersection / union
}

async fn mark_job_failed(conn: &turso::Connection, job_id: i64, err_msg: &str) {
    let _ = conn.execute(
        "UPDATE personal_memory_queue SET status = 'failed', error_msg = ?, attempts = attempts + 1 WHERE id = ?",
        (err_msg.to_string(), job_id),
    ).await;
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
        let status = if collection == "Context" || collection == "Tasks" || collection == "Goals" {
            PM_QUEUE_STATUS_STAGED
        } else {
            PM_QUEUE_STATUS_PENDING
        };

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
                    status.to_string(),
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

    let coll_type = collection_type(&collection);
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;
    let fact_id = format!("mem_{}_{}", now, uuid::Uuid::new_v4().simple());



    // Semantic collection: requires embedding and multi-tier routing
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

    // Fetch candidate facts in same collection to compare (only active ones)
    let mut cand_rows = match conn.query(
        "SELECT mf.id, mf.fact, mfv.embedding FROM memory_facts mf
         JOIN memory_facts_vectors mfv ON mfv.fact_id = mf.id
         WHERE mf.collection = ? AND mf.status = 'active'",
         (collection.clone(),),
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

    // Sort candidates descending by similarity score
    scored_candidates.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));

    // Check for $O(1)$ Semantic Identity Match among ALL candidates (not just candidate limit)
    let mut exact_match = None;
    for (sim, (cand_id, cand_fact)) in &scored_candidates {
        let jacc_sim = jaccard_similarity(&fact, cand_fact);
        if *sim >= 0.9999 || jacc_sim >= 1.0 {
            log::info!("[MemoryWorker] O(1) Semantic Identity Match found. Cosine: {}, Jaccard: {}. Candidate: {:?}", sim, jacc_sim, cand_id);
            exact_match = Some(cand_id.clone());
            break;
        }
    }

    if let Some(matched_cand_id) = exact_match {
        // Run automatic merge bypass transaction:
        // Update existing fact's created_at, write MERGED relation, mark job completed.
        let blob_bytes = encode_f32_blob(&embedding);
        conn.execute("BEGIN TRANSACTION;", ()).await?;
        match (|| async {
            // Write new fact as superseded to preserve history and vector reference
            conn.execute(
                "INSERT INTO memory_facts (id, type, collection, fact, source, status, created_at, session_id) 
                 VALUES (?, ?, ?, ?, ?, 'superseded', ?, ?)",
                (fact_id.clone(), coll_type.to_string(), collection.clone(), fact.clone(), source.clone(), now, session_id.clone()),
            ).await?;

            conn.execute(
                "INSERT INTO memory_facts_vectors (fact_id, collection, embedding) VALUES (?, ?, ?)",
                (fact_id.clone(), collection.clone(), blob_bytes),
            ).await?;

            conn.execute(
                "INSERT OR IGNORE INTO memory_relations (from_id, to_id, relation, created_at) VALUES (?, ?, ?, ?)",
                (fact_id.clone(), matched_cand_id.clone(), PM_RELATION_MERGED.to_string(), now),
            ).await?;

            conn.execute(
                "UPDATE memory_facts SET created_at = ? WHERE id = ?",
                (now, matched_cand_id.clone()),
            ).await?;

            conn.execute(
                "UPDATE personal_memory_queue SET status = ?, processed_at = ? WHERE id = ?",
                (PM_QUEUE_STATUS_COMPLETED.to_string(), now, job_id),
            ).await?;

            anyhow::Ok(())
        })().await {
            Ok(_) => {
                conn.execute("COMMIT;", ()).await?;
                log::info!("[MemoryWorker] O(1) Consolidation completed successfully.");
            }
            Err(e) => {
                let _ = conn.execute("ROLLBACK;", ()).await;
                mark_job_failed(conn, job_id, &format!("O(1) Consolidation transaction failed: {}", e)).await;
            }
        }
        return Ok(true);
    }

    let candidates: Vec<(f32, String, String)> = scored_candidates
        .into_iter()
        .take(settings.nli_candidate_limit as usize)
        .map(|(sim, (id, f_text))| (sim, id, f_text))
        .collect();

    let mut relations = Vec::new();
    let mut nli_pairs_to_classify = Vec::new();

    for (sim, cand_id, cand_fact) in candidates {
        if sim > 0.95 {
            // Near-duplicate zone: Bypass NLI, write SIMILAR edge
            log::info!("[MemoryWorker] Multi-Tier Routing: Near-duplicate similarity detected ({:.4}). Writing SIMILAR edge.", sim);
            relations.push((fact_id.clone(), cand_id.clone(), PM_RELATION_SIMILAR));
        } else if sim >= 0.65 {
            // Candidate zone: Queue for DeBERTa-v3 cross-encoder classification
            nli_pairs_to_classify.push((cand_id, cand_fact));
        } else {
            // Neutral zone: sim < 0.65. Skip.
        }
    }

    if !nli_pairs_to_classify.is_empty() {
        if let Err(e) = crate::services::memory::nli::ensure_nli_loaded(&settings.nli_model_name) {
            mark_job_failed(conn, job_id, &format!("Failed to load NLI model: {}", e)).await;
            return Ok(true);
        }

        for (cand_id, cand_fact) in nli_pairs_to_classify {
            match crate::services::memory::nli::classify_pair(&fact, &cand_fact) {
                Ok(nli_res) => {
                    let relation = crate::services::memory::nli::relation_from_result(&nli_res, settings);
                    match relation {
                        crate::services::memory::nli::NliRelation::Conflicts => {
                            log::info!("[MemoryWorker] NLI: Conflict detected between enqueued fact and candidate fact.");
                            relations.push((fact_id.clone(), cand_id.clone(), PM_RELATION_CONFLICTS));
                        }
                        crate::services::memory::nli::NliRelation::Supports => {
                            log::info!("[MemoryWorker] NLI: Supports relationship detected between enqueued fact and candidate fact.");
                            relations.push((fact_id.clone(), cand_id.clone(), PM_RELATION_SUPPORTS));
                        }
                        crate::services::memory::nli::NliRelation::Neutral => {}
                    }
                }
                Err(e) => {
                    mark_job_failed(conn, job_id, &format!("NLI classification error: {}", e)).await;
                    return Ok(true);
                }
            }
        }
    }

    // Now write everything inside a single, safe transaction to prevent orphan rows/inconsistencies (Bug #1)
    let blob_bytes = encode_f32_blob(&embedding);
    conn.execute("BEGIN TRANSACTION;", ()).await?;
    match (|| async {
        conn.execute(
            "INSERT INTO memory_facts (id, type, collection, fact, source, status, created_at, session_id) 
             VALUES (?, ?, ?, ?, ?, 'active', ?, ?)",
            (fact_id.clone(), coll_type.to_string(), collection.clone(), fact.clone(), source.clone(), now, session_id.clone()),
        ).await?;

        conn.execute(
            "INSERT INTO memory_facts_vectors (fact_id, collection, embedding) VALUES (?, ?, ?)",
            (fact_id.clone(), collection.clone(), blob_bytes),
        ).await?;

        for (from, to, rel) in relations {
            conn.execute(
                "INSERT OR IGNORE INTO memory_relations (from_id, to_id, relation, created_at) VALUES (?, ?, ?, ?)",
                (from, to, rel.to_string(), now),
            ).await?;
        }

        conn.execute(
            "UPDATE personal_memory_queue SET status = ?, processed_at = ? WHERE id = ?",
            (PM_QUEUE_STATUS_COMPLETED.to_string(), now, job_id),
        ).await?;

        anyhow::Ok(())
    })().await {
        Ok(_) => {
            conn.execute("COMMIT;", ()).await?;
        }
        Err(e) => {
            let _ = conn.execute("ROLLBACK;", ()).await;
            mark_job_failed(conn, job_id, &format!("Database write transaction failed: {}", e)).await;
        }
    }

    Ok(true)
}



/// Session End Consolidation Sweep (spec §5.2).
/// 1. Reads 'staged' Tasks and Goals for this session.
/// 2. Inserts them into memory_facts as 'active' operational facts.
/// 3. Deletes 'staged' WAL items.
/// 4. Writes the raw session Context.
pub async fn session_end_consolidation(
    conn: &turso::Connection,
    session_id: &str,
    session_context_raw: &str,
) -> anyhow::Result<()> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;

    // Wrap all session consolidation writes in a single database transaction (Optimization #3)
    conn.execute("BEGIN TRANSACTION;", ()).await?;
    match (|| async {
        // 1. Bulk-update staged Tasks and Goals to pending in personal_memory_queue so they undergo embedding and NLI
        conn.execute(
            "UPDATE personal_memory_queue 
             SET status = 'pending' 
             WHERE session_id = ? AND status = 'staged' AND collection IN ('Tasks', 'Goals')",
            (session_id.to_string(),),
        ).await?;

        // 2. Delete any other staged queue items for this session (e.g. Context)
        conn.execute(
            "DELETE FROM personal_memory_queue WHERE session_id = ? AND status = 'staged'",
            (session_id.to_string(),),
        ).await?;

        // 3. Write final session Context paragraph directly (never embedded)
        if !session_context_raw.trim().is_empty() {
            let context_id = format!("mem_{}_{}", now, uuid::Uuid::new_v4().simple());
            conn.execute(
                "INSERT INTO memory_facts (id, type, collection, fact, source, status, created_at, session_id) 
                 VALUES (?, 'operational', 'Context', ?, 'LLM', 'active', ?, ?)",
                (context_id, session_context_raw.trim().to_string(), now, session_id.to_string()),
            ).await?;
            tracing::info!("[MemoryWorker] Saved session Context memory for session_id={}", session_id);
        }
        anyhow::Ok(())
    })().await {
        Ok(_) => {
            conn.execute("COMMIT;", ()).await?;
        }
        Err(e) => {
            let _ = conn.execute("ROLLBACK;", ()).await;
            return Err(anyhow::anyhow!("Session consolidation transaction failed: {}", e));
        }
    }

    Ok(())
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
                // If idle, wait with shorter timeout to check for cooperative yield
                let timeout = if state.is_idle {
                    Duration::from_millis(100)
                } else {
                    Duration::from_millis(500)
                };

                let event = match rx.recv_timeout(timeout) {
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
                        }
                        MemoryWorkerEvent::PipelineActive => {
                            state.is_idle = false;
                        }
                        MemoryWorkerEvent::SessionEnd { session_id, summary } => {
                            if let Some(ref db_conn) = conn {
                                if let Err(e) = handle.block_on(async {
                                    session_end_consolidation(db_conn, &session_id, &summary).await
                                }) {
                                    tracing::error!(
                                        "[MemoryWorker] Failed consolidation sweep for session_id={}: {}",
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
                    // Process queue items only when idle
                    if let Some(ref db_conn) = conn {
                        let memory_settings = match settings.read() {
                            Ok(s) => s.memory.clone(),
                            Err(_) => {
                                tracing::error!("[MemoryWorker] Settings lock poisoned! Skipping loop iteration.");
                                std::thread::sleep(std::time::Duration::from_millis(100));
                                continue;
                            }
                        };

                        loop {
                            // Cooperative Yield Check:
                            // Check if a new event is waiting in the channel (e.g. PipelineActive)
                            // or if is_idle was toggled externally.
                            if !state.is_idle || !rx.is_empty() {
                                break;
                            }

                            let processed_queue = handle.block_on(async {
                                process_one_queue_item(db_conn, &memory_settings).await
                            });

                            match processed_queue {
                                Ok(true) => {
                                    // Process next item immediately, yielding if channel has events
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
}

