use anyhow::{anyhow, Result};
use turso::Connection;
use std::collections::HashMap;
use crate::core::constants::{
    PM_RELATION_SUPERSEDES, PM_SOURCE_USER, PM_QUEUE_STATUS_PAUSED, PM_QUEUE_STATUS_STAGED_PENDING,
    collection_type, PM_TYPE_SEMANTIC_GRAPH,
};
use crate::persistence::encode_f32_blob;

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
            PM_QUEUE_STATUS_STAGED_PENDING
        } else {
            PM_QUEUE_STATUS_PAUSED
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
/// Transitions to status `'failed'` when `retry_count >= 3`.
pub async fn mark_job_failed(conn: &Connection, job_id: i64, err_msg: &str) {
    let _ = conn
        .execute(
            "UPDATE personal_memory_queue
             SET retry_count = retry_count + 1,
                 error_msg = ?,
                 status = CASE WHEN retry_count + 1 >= 3 THEN 'failed' ELSE 'staged_pending' END
             WHERE id = ?",
            (err_msg.to_string(), job_id),
        )
        .await;
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
    match (|| async {
        conn.execute(
            "UPDATE personal_memory_queue 
             SET status = 'pending' 
             WHERE session_id = ? AND (status = 'staged' OR status = 'paused')",
            (session_id.to_string(),),
        ).await?;

        if !session_context_raw.trim().is_empty() {
            let context_id = format!("mem_{}_{}", now, uuid::Uuid::new_v4().simple());
            conn.execute(
                "INSERT INTO memory_facts (id, type, collection, fact, source, status, created_at, session_id) 
                 VALUES (?, 'operational', 'Context', ?, 'LLM', 'active', ?, ?)",
                (context_id, session_context_raw.trim().to_string(), now, session_id.to_string()),
            ).await?;
            tracing::info!("[Repository] Saved session Context memory for session_id={}", session_id);
        }
        anyhow::Ok(())
    })().await {
        Ok(_) => {
            conn.execute("COMMIT;", ()).await?;
            Ok(())
        }
        Err(e) => {
            let _ = conn.execute("ROLLBACK;", ()).await;
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

    conn.execute(
        "INSERT INTO memory_facts (id, type, collection, fact, source, status, created_at) VALUES (?, ?, ?, ?, ?, 'active', ?)",
        (
            new_id.clone(),
            fact_type.to_string(),
            collection.to_string(),
            new_fact_text.to_string(),
            PM_SOURCE_USER.to_string(),
            now,
        ),
    ).await?;

    if fact_type == PM_TYPE_SEMANTIC_GRAPH {
        crate::services::memory::ensure_embedder_loaded(true)?;
        let embedding = match crate::services::memory::generate_embedding(new_fact_text)? {
            Some(v) => v,
            None => return Err(anyhow!("Failed to generate embedding for edited fact.")),
        };

        let blob_bytes = encode_f32_blob(&embedding);
        conn.execute(
            "INSERT INTO memory_facts_vectors (fact_id, collection, embedding) VALUES (?, ?, ?)",
            (new_id.clone(), collection.to_string(), blob_bytes),
        ).await?;
    }

    conn.execute(
        "INSERT INTO memory_relations (from_id, to_id, relation, source, created_at) VALUES (?, ?, ?, 'USER', ?)",
        (
            new_id.clone(),
            old_id.to_string(),
            PM_RELATION_SUPERSEDES.to_string(),
            now,
        ),
    ).await?;

    conn.execute(
        "UPDATE memory_facts SET status = 'superseded' WHERE id = ?",
        (old_id.to_string(),),
    ).await?;

    Ok(new_id)
}

/// Records operational pipeline stage metrics into `memory_pipeline_metrics`.
pub async fn record_stage_metrics(
    conn: &Connection,
    metrics: &crate::services::memory::pipeline::PipelineStageMetrics,
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

pub async fn write_dedup_audit(
    conn: &Connection,
    item_id: i64,
    log: &crate::services::memory::pipeline::DedupAuditLog,
) -> Result<()> {
    let json_str = serde_json::to_string(log)?;
    conn.execute(
        "UPDATE personal_memory_queue SET dedup_match_json = ? WHERE id = ?",
        (json_str, item_id),
    )
    .await?;
    Ok(())
}

pub async fn write_candidate_audit(
    conn: &Connection,
    item_id: i64,
    logs: &[crate::services::memory::pipeline::CandidateAuditLog],
) -> Result<()> {
    let json_str = serde_json::to_string(logs)?;
    conn.execute(
        "UPDATE personal_memory_queue SET audit_json = ? WHERE id = ?",
        (json_str, item_id),
    )
    .await?;
    Ok(())
}


#[cfg(test)]
mod tests {
    use super::*;
    use turso::Connection;

    async fn setup_test_db() -> Result<Connection> {
        let db = turso::Builder::new_local(":memory:")
            .experimental_index_method(true)
            .build()
            .await?;
        let conn = db.connect()?;
        crate::persistence::schema::run_migrations(&conn).await?;
        Ok(conn)
    }

    #[tokio::test]
    async fn test_enqueue_personal_facts() -> Result<()> {
        let conn = setup_test_db().await?;
        let mut facts = HashMap::new();
        facts.insert("Tasks".to_string(), vec!["Buy groceries".to_string(), "  ".to_string()]);
        facts.insert("Skills".to_string(), vec!["Rust programming".to_string()]);

        enqueue_personal_facts(&conn, facts, "session_123", true).await?;

        let mut rows = conn
            .query("SELECT fact, collection, status, session_id FROM personal_memory_queue ORDER BY id ASC", ())
            .await?;

        let mut queue_items = Vec::new();
        while let Some(row) = rows.next().await? {
            queue_items.push((
                row.get::<String>(0)?,
                row.get::<String>(1)?,
                row.get::<String>(2)?,
                row.get::<String>(3)?,
            ));
        }

        assert_eq!(queue_items.len(), 2);
        queue_items.sort_by(|a, b| a.0.cmp(&b.0));
        assert_eq!(queue_items[0], ("Buy groceries".to_string(), "Tasks".to_string(), "staged_pending".to_string(), "session_123".to_string()));
        assert_eq!(queue_items[1], ("Rust programming".to_string(), "Skills".to_string(), "staged_pending".to_string(), "session_123".to_string()));

        Ok(())
    }

    #[tokio::test]
    async fn test_mark_job_failed() -> Result<()> {
        let conn = setup_test_db().await?;
        conn.execute(
            "INSERT INTO personal_memory_queue (fact, collection, session_id, status, retry_count, created_at) VALUES ('fact1', 'Skills', 'sess1', 'staged_pending', 0, 1000)",
            (),
        ).await?;

        // 1st attempt failure -> retry_count = 1, status reset to staged_pending
        mark_job_failed(&conn, 1, "Attempt 1 error").await;
        let mut rows = conn.query("SELECT status, error_msg, retry_count FROM personal_memory_queue WHERE id = 1", ()).await?;
        let row = rows.next().await?.expect("job should exist");
        assert_eq!(row.get::<String>(0)?, "staged_pending");
        assert_eq!(row.get::<String>(1)?, "Attempt 1 error");
        assert_eq!(row.get::<i64>(2)?, 1);

        // 2nd attempt failure -> retry_count = 2, status reset to staged_pending
        mark_job_failed(&conn, 1, "Attempt 2 error").await;

        // 3rd attempt failure -> retry_count = 3, status transitions to failed
        mark_job_failed(&conn, 1, "Attempt 3 error").await;
        let mut rows = conn.query("SELECT status, error_msg, retry_count FROM personal_memory_queue WHERE id = 1", ()).await?;
        let row = rows.next().await?.expect("job should exist");
        assert_eq!(row.get::<String>(0)?, "failed");
        assert_eq!(row.get::<String>(1)?, "Attempt 3 error");
        assert_eq!(row.get::<i64>(2)?, 3);

        Ok(())
    }



    #[tokio::test]
    async fn test_session_end_consolidation() -> Result<()> {
        let conn = setup_test_db().await?;

        conn.execute(
            "INSERT INTO personal_memory_queue (fact, collection, session_id, status, created_at) VALUES ('Task 1', 'Tasks', 'sess_alpha', 'paused', 1000)",
            (),
        ).await?;
        conn.execute(
            "INSERT INTO personal_memory_queue (fact, collection, session_id, status, created_at) VALUES ('Task 2', 'Tasks', 'sess_beta', 'paused', 1000)",
            (),
        ).await?;

        session_end_consolidation(&conn, "sess_alpha", "User discussed task priorities for session alpha").await?;

        let mut rows = conn.query("SELECT status FROM personal_memory_queue WHERE session_id = 'sess_alpha'", ()).await?;
        assert_eq!(rows.next().await?.unwrap().get::<String>(0)?, "pending");

        let mut rows_b = conn.query("SELECT status FROM personal_memory_queue WHERE session_id = 'sess_beta'", ()).await?;
        assert_eq!(rows_b.next().await?.unwrap().get::<String>(0)?, "paused");

        let mut ctx_rows = conn.query("SELECT collection, fact, type, status FROM memory_facts WHERE session_id = 'sess_alpha'", ()).await?;
        let ctx_row = ctx_rows.next().await?.expect("Context fact should be created");
        assert_eq!(ctx_row.get::<String>(0)?, "Context");
        assert_eq!(ctx_row.get::<String>(1)?, "User discussed task priorities for session alpha");
        assert_eq!(ctx_row.get::<String>(2)?, "operational");
        assert_eq!(ctx_row.get::<String>(3)?, "active");

        Ok(())
    }

    #[tokio::test]
    async fn test_supersede_user_fact_non_semantic() -> Result<()> {
        let conn = setup_test_db().await?;

        conn.execute(
            "INSERT INTO memory_facts (id, type, collection, fact, source, status, created_at) VALUES ('old_id_1', 'foundational', 'Identity', 'Name is Alex', 'LLM', 'active', 1000)",
            (),
        ).await?;

        let new_id = supersede_user_fact(&conn, "old_id_1", "Name is Alexander", "Identity").await?;

        let mut old_rows = conn.query("SELECT status FROM memory_facts WHERE id = 'old_id_1'", ()).await?;
        assert_eq!(old_rows.next().await?.unwrap().get::<String>(0)?, "superseded");

        let mut new_rows = conn.query("SELECT fact, source, status FROM memory_facts WHERE id = ?", (new_id.clone(),)).await?;
        let new_row = new_rows.next().await?.expect("new fact should exist");
        assert_eq!(new_row.get::<String>(0)?, "Name is Alexander");
        assert_eq!(new_row.get::<String>(1)?, "User");
        assert_eq!(new_row.get::<String>(2)?, "active");

        let mut rel_rows = conn.query("SELECT from_id, to_id, relation, source FROM memory_relations WHERE from_id = ?", (new_id,)).await?;
        let rel_row = rel_rows.next().await?.expect("supersedes relation should exist");
        assert_eq!(rel_row.get::<String>(1)?, "old_id_1");
        assert_eq!(rel_row.get::<String>(2)?, "SUPERSEDES");
        assert_eq!(rel_row.get::<String>(3)?, "USER");

        Ok(())
    }
}
