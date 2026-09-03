use crate::persistence::db::VoxDb;
use crate::persistence::{encode_f32_blob, MAX_QUEUE_RETRY_ATTEMPTS};
use crate::services::memory::{collection_type, CollectionType, FactSource, QueueStatus, Relation};
use anyhow::{anyhow, Result};
use std::collections::HashMap;
use turso::Connection;

/// Enqueues extracted personal memory facts into `personal_memory_queue`.
pub async fn enqueue_personal_facts(
    conn: &Connection,
    facts: HashMap<String, Vec<String>>,
    session_id: &str,
    pipeline_processing_enabled: bool,
) -> Result<()> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;

    if facts.is_empty() {
        return Ok(());
    }

    conn.execute("BEGIN IMMEDIATE;", ()).await?;
    let res: Result<()> = async {
        for (collection, fact_list) in &facts {
            let status = if pipeline_processing_enabled {
                QueueStatus::StagedPending.as_str()
            } else {
                QueueStatus::Paused.as_str()
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
    .await;

    if res.is_ok() {
        conn.execute("COMMIT;", ()).await?;
    } else {
        let _ = conn.execute("ROLLBACK;", ()).await;
    }
    res
}

/// Marks a queue item failure in `personal_memory_queue`, incrementing `retry_count`.
pub async fn mark_job_failed(conn: &Connection, job_id: i64, err_msg: &str) {
    let query = format!(
        "UPDATE personal_memory_queue
         SET retry_count = retry_count + 1,
             error_msg = ?,
             status = CASE WHEN retry_count + 1 >= {} THEN 'failed' ELSE 'staged_pending' END
         WHERE id = ?",
        MAX_QUEUE_RETRY_ATTEMPTS
    );
    if let Err(e) = conn.execute(&query, (err_msg.to_string(), job_id)).await {
        log::warn!(
            "[Persistence::Mutations] Failed to update failed job status for job_id={}: {}",
            job_id,
            e
        );
    }
}

/// Atomically transitions paused facts for session to pending, and saves session Context memory.
pub async fn session_end_consolidation(
    conn: &Connection,
    session_id: &str,
    session_context_raw: &str,
) -> Result<()> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;

    conn.execute("BEGIN TRANSACTION;", ()).await?;
    match async {
        conn.execute(
            "UPDATE personal_memory_queue 
             SET status = 'staged_pending' 
             WHERE session_id = ? AND (status = 'staged_pending' OR status = 'paused' OR status = 'staged')",
            (session_id.to_string(),),
        ).await?;

        if !session_context_raw.trim().is_empty() {
            let context_id = format!("mem_{}_{}", now, uuid::Uuid::new_v4().simple());
            conn.execute(
                "INSERT INTO memory_facts (id, type, collection, fact, source, status, created_at, session_id) 
                 VALUES (?, 'operational', 'Context', ?, 'LLM', 'active', ?, ?)",
                (context_id, session_context_raw.trim().to_string(), now, session_id.to_string()),
            ).await?;
            log::info!("[Persistence::Mutations] Saved session Context memory for session_id={}", session_id);
        }
        anyhow::Ok(())
    }.await {
        Ok(_) => {
            conn.execute("COMMIT;", ()).await?;
            Ok(())
        }
        Err(e) => {
            if let Err(rollback_err) = conn.execute("ROLLBACK;", ()).await {
                log::warn!("[Persistence::Mutations] Rollback failed during session consolidation: {}", rollback_err);
            }
            Err(anyhow!("Session consolidation transaction failed: {}", e))
        }
    }
}

/// Inserts a manually edited user fact and writes a SUPERSEDES edge old → new with source 'USER'.
pub async fn supersede_user_fact(
    conn: &Connection,
    old_id: &str,
    new_fact_text: &str,
    collection: &str,
) -> Result<String> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;

    let new_id = format!("mem_{}_{}", now, uuid::Uuid::new_v4().simple());
    let fact_type = collection_type(collection);

    let embedding_opt = if fact_type == CollectionType::SemanticGraph {
        crate::services::memory::ensure_embedder_loaded(true)?;
        match crate::services::memory::generate_embedding(new_fact_text)? {
            Some(v) => Some(v),
            None => return Err(anyhow!("Failed to generate embedding for edited fact.")),
        }
    } else {
        None
    };

    conn.execute("BEGIN TRANSACTION;", ()).await?;
    let res: Result<String> = async {
        conn.execute(
            "INSERT INTO memory_facts (id, type, collection, fact, source, status, created_at) VALUES (?, ?, ?, ?, ?, 'active', ?)",
            (
                new_id.clone(),
                fact_type.as_str().to_string(),
                collection.to_string(),
                new_fact_text.to_string(),
                FactSource::User.as_str().to_string(),
                now,
            ),
        ).await?;

        if let Some(ref embedding) = embedding_opt {
            let blob_bytes = encode_f32_blob(embedding);
            conn.execute(
                "INSERT INTO memory_facts_vectors (fact_id, collection, embedding) VALUES (?, ?, ?)",
                (new_id.clone(), collection.to_string(), blob_bytes),
            )
            .await?;
        }

        conn.execute(
            "INSERT INTO memory_relations (from_id, to_id, relation, source, created_at) VALUES (?, ?, ?, 'USER', ?)",
            (
                new_id.clone(),
                old_id.to_string(),
                Relation::Supersedes.as_str().to_string(),
                now,
            ),
        ).await?;

        conn.execute(
            "UPDATE memory_facts SET status = 'superseded' WHERE id = ?",
            (old_id.to_string(),),
        )
        .await?;

        Ok(new_id.clone())
    }.await;

    match res {
        Ok(id) => {
            conn.execute("COMMIT;", ()).await?;
            Ok(id)
        }
        Err(err) => {
            if let Err(e) = conn.execute("ROLLBACK;", ()).await {
                log::warn!(
                    "[Persistence::Mutations] Failed to rollback supersede_user_fact: {}",
                    e
                );
            }
            Err(err)
        }
    }
}

/// Records operational pipeline stage metrics into `memory_pipeline_metrics`.
pub async fn record_stage_metrics(
    conn: &Connection,
    metrics: &crate::services::memory::ingestion::PipelineStageMetrics,
) -> Result<()> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;

    conn.execute(
        "INSERT INTO memory_pipeline_metrics 
         (run_id, stage_name, session_id, batch_seq, items_claimed, error_count, duration_ms, created_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        (
            metrics.run_id.clone(),
            metrics.stage_name.clone(),
            metrics.session_id.clone(),
            metrics.batch_seq as i64,
            metrics.items_claimed as i64,
            metrics.error_count as i64,
            metrics.duration_ms as i64,
            now,
        ),
    )
    .await?;

    Ok(())
}

/// Writes deduplication audit log to personal_memory_queue.
pub async fn write_dedup_audit(
    conn: &Connection,
    item_id: i64,
    log: &crate::services::memory::ingestion::DedupAuditLog,
) -> Result<()> {
    let json_str = serde_json::to_string(log)?;
    conn.execute(
        "UPDATE personal_memory_queue SET dedup_match_json = ? WHERE id = ?",
        (json_str, item_id),
    )
    .await?;
    Ok(())
}

/// Writes candidate scoring audit log to personal_memory_queue.
pub async fn write_candidate_audit(
    conn: &Connection,
    item_id: i64,
    logs: &[crate::services::memory::ingestion::CandidateAuditLog],
) -> Result<()> {
    let json_str = serde_json::to_string(logs)?;
    conn.execute(
        "UPDATE personal_memory_queue SET audit_json = ? WHERE id = ?",
        (json_str, item_id),
    )
    .await?;
    Ok(())
}

/// Updates a memory fact's text content and its embedding vector.
pub async fn update_memory_fact(
    conn: &Connection,
    fact_id: &str,
    new_text: &str,
    embedding: &[f32],
) -> Result<()> {
    let mut rows = conn
        .query(
            "SELECT collection FROM memory_facts WHERE id = ?",
            (fact_id.to_string(),),
        )
        .await?;
    let row = rows
        .next()
        .await?
        .ok_or_else(|| anyhow!("Fact not found: {}", fact_id))?;
    let collection: String = row.get(0)?;

    let blob_bytes = encode_f32_blob(embedding);

    VoxDb::with_transaction(conn, async {
        conn.execute(
            "UPDATE memory_facts SET fact = ? WHERE id = ?",
            (new_text.to_string(), fact_id.to_string()),
        )
        .await
        .map_err(|e| e.to_string())?;

        conn.execute(
            "INSERT INTO memory_facts_vectors (fact_id, collection, embedding) VALUES (?, ?, ?)
             ON CONFLICT(fact_id) DO UPDATE SET embedding = excluded.embedding",
            (fact_id.to_string(), collection, blob_bytes),
        )
        .await
        .map_err(|e| e.to_string())?;

        Ok(())
    })
    .await
    .map_err(|e| anyhow!(e))?;

    Ok(())
}

/// Marks a memory fact as superseded and creates a user tombstone linking it.
pub async fn delete_memory_fact(
    conn: &Connection,
    fact_id: &str,
) -> Result<()> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;
    let tombstone_id = format!("mem_{}_{}", now, uuid::Uuid::new_v4().simple());

    VoxDb::with_transaction(conn, async {
        conn.execute(
            "UPDATE memory_facts SET status = 'superseded' WHERE id = ?",
            (fact_id.to_string(),),
        )
        .await
        .map_err(|e| e.to_string())?;

        conn.execute(
            "INSERT INTO memory_facts (id, type, collection, fact, source, status, created_at) VALUES (?, 'foundational', 'Identity', '', ?, 'active', ?)",
            (tombstone_id.clone(), FactSource::User.as_str().to_string(), now),
        )
        .await
        .map_err(|e| e.to_string())?;

        conn.execute(
            "INSERT INTO memory_relations (from_id, to_id, relation, source, created_at) VALUES (?, ?, ?, 'USER', ?)",
            (tombstone_id, fact_id.to_string(), Relation::Supersedes.as_str().to_string(), now),
        )
        .await
        .map_err(|e| e.to_string())?;

        Ok(())
    })
    .await
    .map_err(|e| anyhow!(e))?;

    Ok(())
}

/// Resolves a conflict between two facts by marking loser as superseded and linking winner with SUPERSEDES.
pub async fn resolve_fact_conflict(
    conn: &Connection,
    winner_id: &str,
    loser_id: &str,
) -> Result<()> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;

    VoxDb::with_transaction(conn, async {
        conn.execute(
            "UPDATE memory_facts SET status = 'superseded' WHERE id = ?",
            (loser_id.to_string(),),
        )
        .await
        .map_err(|e| e.to_string())?;

        conn.execute(
            "INSERT INTO memory_relations (from_id, to_id, relation, source, created_at) VALUES (?, ?, ?, 'USER', ?)",
            (winner_id.to_string(), loser_id.to_string(), Relation::Supersedes.as_str().to_string(), now),
        )
        .await
        .map_err(|e| e.to_string())?;

        Ok(())
    })
    .await
    .map_err(|e| anyhow!(e))?;

    Ok(())
}

/// Resets failed queue items to staged_pending for re-processing.
pub async fn retry_failed_queue_items(
    conn: &Connection,
    item_ids: Option<Vec<i64>>,
) -> Result<u64> {
    let affected = match item_ids {
        Some(ids) if !ids.is_empty() => {
            if ids.len() > 1000 {
                return Err(anyhow!("Too many items in retry batch. Maximum allowed is 1000."));
            }
            let placeholders = ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
            let sql = format!(
                "UPDATE personal_memory_queue 
                 SET status = 'staged_pending', attempts = 0, retry_count = 0, error_msg = NULL 
                 WHERE status = 'failed' AND id IN ({})",
                placeholders
            );
            let params: Vec<turso::Value> = ids.into_iter().map(|id| id.into()).collect();
            conn.execute(&sql, params).await?
        }
        _ => {
            conn.execute(
                "UPDATE personal_memory_queue 
                 SET status = 'staged_pending', attempts = 0, retry_count = 0, error_msg = NULL 
                 WHERE status = 'failed'",
                (),
            )
            .await?
        }
    };
    Ok(affected)
}

/// Reassigns an existing memory fact to a new collection by staging it in personal_memory_queue.
pub async fn reassign_memory_fact(
    conn: &Connection,
    fact_id: &str,
    new_collection: &str,
) -> Result<()> {
    let mut rows = conn
        .query(
            "SELECT fact, source, session_id FROM memory_facts WHERE id = ?",
            (fact_id.to_string(),),
        )
        .await?;

    let row = rows
        .next()
        .await?
        .ok_or_else(|| anyhow!("Fact not found: {}", fact_id))?;

    let fact_text: String = row.get(0)?;
    let source_str: String = row.get(1)?;
    let session_id: String = row.get(2).unwrap_or_default();

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;

    conn.execute(
        "INSERT INTO personal_memory_queue (fact, collection, source, session_id, status, created_at)
         VALUES (?, ?, ?, ?, 'staged_pending', ?)",
        (fact_text, new_collection.to_string(), source_str, session_id, now),
    )
    .await?;

    Ok(())
}
