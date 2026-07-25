use anyhow::{anyhow, Result};
use turso::Connection;
use std::collections::HashMap;
use crate::core::constants::{
    PM_RELATION_SUPERSEDES, PM_SOURCE_USER, PM_QUEUE_STATUS_STAGED, PM_QUEUE_STATUS_PENDING,
    PM_QUEUE_STATUS_COMPLETED, collection_type, PM_TYPE_SEMANTIC, MemoryCollection,
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
        let status = if MemoryCollection::parse(&collection).map_or(false, |mc| mc.is_staged_during_session()) {
            PM_QUEUE_STATUS_STAGED
        } else if pipeline_processing_enabled {
            PM_QUEUE_STATUS_PENDING
        } else {
            PM_QUEUE_STATUS_STAGED
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

/// Marks a queue item as failed in `personal_memory_queue`.
pub async fn mark_job_failed(conn: &Connection, job_id: i64, err_msg: &str) {
    let _ = conn.execute(
        "UPDATE personal_memory_queue SET status = 'failed', error_msg = ?, attempts = attempts + 1 WHERE id = ?",
        (err_msg.to_string(), job_id),
    ).await;
}

/// Inserts an exact duplicate merge record in `memory_facts` and marks queue job completed.
pub async fn insert_exact_merged_fact(
    conn: &Connection,
    job_id: i64,
    fact_id: &str,
    fact: &str,
    collection: &str,
    source: &str,
    session_id: &str,
    matched_candidate_id: &str,
    embedding: &[f32],
    now: i64,
) -> Result<()> {
    let coll_type = collection_type(collection);
    let blob_bytes = encode_f32_blob(embedding);

    conn.execute("BEGIN TRANSACTION;", ()).await?;
    match (|| async {
        conn.execute(
            "INSERT INTO memory_facts (id, type, collection, fact, source, status, created_at, session_id) 
             VALUES (?, ?, ?, ?, ?, 'merged', ?, ?)",
            (
                fact_id.to_string(),
                coll_type.to_string(),
                collection.to_string(),
                fact.to_string(),
                source.to_string(),
                now,
                session_id.to_string(),
            ),
        ).await?;

        if coll_type == PM_TYPE_SEMANTIC {
            conn.execute(
                "INSERT INTO memory_facts_vectors (fact_id, collection, embedding) VALUES (?, ?, ?)",
                (fact_id.to_string(), collection.to_string(), blob_bytes),
            ).await?;
        }

        conn.execute(
            "INSERT INTO memory_relations (from_id, to_id, relation, source, created_at) VALUES (?, ?, ?, ?, ?)",
            (
                fact_id.to_string(),
                matched_candidate_id.to_string(),
                PM_RELATION_SUPERSEDES.to_string(),
                "DEDUP".to_string(),
                now,
            ),
        ).await?;

        conn.execute(
            "UPDATE personal_memory_queue SET status = ? WHERE id = ?",
            (PM_QUEUE_STATUS_COMPLETED.to_string(), job_id),
        ).await?;

        anyhow::Ok(())
    })().await {
        Ok(_) => {
            conn.execute("COMMIT;", ()).await?;
            Ok(())
        }
        Err(e) => {
            let _ = conn.execute("ROLLBACK;", ()).await;
            Err(e)
        }
    }
}

/// Atomic persistence transaction: Inserts `memory_facts`, `memory_facts_vectors`, and `memory_relations`.
pub async fn insert_fact_with_vector_and_relations(
    conn: &Connection,
    job_id: i64,
    fact_id: &str,
    fact: &str,
    collection: &str,
    source: &str,
    session_id: &str,
    embedding: &[f32],
    relations: Vec<(String, String, &'static str, &'static str)>,
    now: i64,
) -> Result<()> {
    let coll_type = collection_type(collection);
    let blob_bytes = encode_f32_blob(embedding);

    conn.execute("BEGIN TRANSACTION;", ()).await?;
    match (|| async {
        conn.execute(
            "INSERT INTO memory_facts (id, type, collection, fact, source, status, created_at, session_id) 
             VALUES (?, ?, ?, ?, ?, 'active', ?, ?)",
            (
                fact_id.to_string(),
                coll_type.to_string(),
                collection.to_string(),
                fact.to_string(),
                source.to_string(),
                now,
                session_id.to_string(),
            ),
        ).await?;

        if coll_type == PM_TYPE_SEMANTIC {
            conn.execute(
                "INSERT INTO memory_facts_vectors (fact_id, collection, embedding) VALUES (?, ?, ?)",
                (fact_id.to_string(), collection.to_string(), blob_bytes),
            ).await?;
        }

        for (from_id, to_id, rel, rel_src) in relations {
            conn.execute(
                "INSERT INTO memory_relations (from_id, to_id, relation, source, created_at) VALUES (?, ?, ?, ?, ?)",
                (from_id, to_id, rel.to_string(), rel_src.to_string(), now),
            ).await?;
        }

        conn.execute(
            "UPDATE personal_memory_queue SET status = ? WHERE id = ?",
            (PM_QUEUE_STATUS_COMPLETED.to_string(), job_id),
        ).await?;

        anyhow::Ok(())
    })().await {
        Ok(_) => {
            conn.execute("COMMIT;", ()).await?;
            Ok(())
        }
        Err(e) => {
            let _ = conn.execute("ROLLBACK;", ()).await;
            Err(e)
        }
    }
}

/// Atomically transitions staged facts for session to pending, and saves session Context memory.
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
             WHERE session_id = ? AND status = 'staged'",
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

    if fact_type == PM_TYPE_SEMANTIC {
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
