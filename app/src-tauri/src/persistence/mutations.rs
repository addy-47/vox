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

    for (collection, fact_list) in facts {
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

        if fact_type == CollectionType::SemanticGraph {
            crate::services::memory::ensure_embedder_loaded(true)?;
            let embedding = match crate::services::memory::generate_embedding(new_fact_text)? {
                Some(v) => v,
                None => return Err(anyhow!("Failed to generate embedding for edited fact.")),
            };

            let blob_bytes = encode_f32_blob(&embedding);
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
