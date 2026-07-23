use anyhow::{anyhow, Result};
use turso::Connection;
use std::collections::HashMap;
use crate::core::constants::{
    PM_RELATION_USER_SUPERSEDES, PM_SOURCE_USER, PM_QUEUE_STATUS_STAGED, PM_QUEUE_STATUS_PENDING,
    PM_QUEUE_STATUS_COMPLETED, PM_RELATION_MERGED, collection_type, PM_TYPE_SEMANTIC, MemoryCollection,
};
use crate::services::memory::MemoryFact;

pub fn encode_f32_blob(floats: &[f32]) -> Vec<u8> {
    floats.iter().flat_map(|f| f.to_le_bytes()).collect()
}

pub fn decode_f32_blob(bytes: &[u8]) -> Vec<f32> {
    bytes
        .chunks_exact(4)
        .map(|chunk| f32::from_le_bytes(chunk.try_into().unwrap_or_default()))
        .collect()
}

/// Enqueues extracted personal memory facts into `personal_memory_queue`.
/// Ephemerality contract (v5.1):
/// - 'Context' and 'Tasks' are staged (status = 'staged')
/// - 'Goals' and all semantic/foundational collections are enqueued as 'pending'
pub async fn enqueue_personal_facts(
    conn: &Connection,
    facts: HashMap<String, Vec<String>>,
    session_id: &str,
) -> Result<()> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;

    for (collection, fact_list) in facts {
        let status = if MemoryCollection::parse(&collection).map_or(false, |mc| mc.is_staged_during_session()) {
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

/// Marks a queue item as failed in `personal_memory_queue`.
pub async fn mark_job_failed(conn: &Connection, job_id: i64, err_msg: &str) {
    let _ = conn.execute(
        "UPDATE personal_memory_queue SET status = 'failed', error_msg = ?, attempts = attempts + 1 WHERE id = ?",
        (err_msg.to_string(), job_id),
    ).await;
}

/// Fetches active Identity + Constraints facts (Tier 1 Foundational).
pub async fn fetch_foundational_facts(conn: &Connection) -> Result<Vec<MemoryFact>> {
    let mut rows = conn
        .query(
            "SELECT id, type, collection, fact, source, status, created_at FROM memory_facts
             WHERE type = 'foundational' AND status = 'active'
             ORDER BY created_at ASC",
            (),
        )
        .await?;

    let mut list = Vec::new();
    while let Some(row) = rows.next().await? {
        list.push(MemoryFact {
            id: row.get(0)?,
            fact_type: row.get(1)?,
            collection: row.get(2)?,
            fact: row.get(3)?,
            source: row.get(4)?,
            status: row.get(5)?,
            created_at: row.get(6)?,
        });
    }
    Ok(list)
}

/// Fetches active Tasks + Goals facts (Tier 1 Operational).
pub async fn fetch_operational_facts(conn: &Connection) -> Result<Vec<MemoryFact>> {
    let mut rows = conn
        .query(
            "SELECT id, type, collection, fact, source, status, created_at FROM memory_facts
             WHERE type = 'operational' AND collection IN ('Tasks', 'Goals') AND status = 'active'
             ORDER BY created_at DESC",
            (),
        )
        .await?;

    let mut list = Vec::new();
    while let Some(row) = rows.next().await? {
        list.push(MemoryFact {
            id: row.get(0)?,
            fact_type: row.get(1)?,
            collection: row.get(2)?,
            fact: row.get(3)?,
            source: row.get(4)?,
            status: row.get(5)?,
            created_at: row.get(6)?,
        });
    }
    Ok(list)
}

/// Fetches all currently active facts from SQLite grouped by collection.
/// This serves as the authoritative database source of truth for LLM context compaction.
pub async fn fetch_active_facts_grouped(conn: &Connection) -> Result<HashMap<String, Vec<String>>> {
    let mut rows = conn
        .query(
            "SELECT collection, fact FROM memory_facts WHERE status = 'active' ORDER BY created_at ASC",
            (),
        )
        .await?;

    let mut map: HashMap<String, Vec<String>> = HashMap::new();
    while let Some(row) = rows.next().await? {
        let col: String = row.get(0)?;
        let fact: String = row.get(1)?;
        map.entry(col).or_default().push(fact);
    }
    Ok(map)
}

/// Fetches candidate active facts with vector embeddings for a given collection.
pub async fn fetch_active_candidate_vectors(
    conn: &Connection,
    collection: &str,
) -> Result<Vec<(String, String, Vec<f32>)>> {
    let mut cand_rows = conn.query(
        "SELECT mf.id, mf.fact, mfv.embedding FROM memory_facts mf
         JOIN memory_facts_vectors mfv ON mfv.fact_id = mf.id
         WHERE mf.collection = ? AND mf.status = 'active'",
         (collection.to_string(),),
    ).await?;

    let mut candidates = Vec::new();
    while let Some(row) = cand_rows.next().await? {
        let id: String = row.get(0)?;
        let f_text: String = row.get(1)?;
        let emb_blob: Vec<u8> = row.get(2)?;
        let emb_vector = decode_f32_blob(&emb_blob);
        candidates.push((id, f_text, emb_vector));
    }
    Ok(candidates)
}

/// Executes an exact merge transaction for $O(1)$ duplicate matches.
pub async fn insert_exact_merged_fact(
    conn: &Connection,
    job_id: i64,
    fact_id: &str,
    fact: &str,
    collection: &str,
    source: &str,
    session_id: &str,
    matched_cand_id: &str,
    embedding: &[f32],
    now: i64,
) -> Result<()> {
    let coll_type = collection_type(collection);
    let blob_bytes = encode_f32_blob(embedding);

    conn.execute("BEGIN TRANSACTION;", ()).await?;
    match (|| async {
        conn.execute(
            "INSERT INTO memory_facts (id, type, collection, fact, source, status, created_at, session_id) 
             VALUES (?, ?, ?, ?, ?, 'superseded', ?, ?)",
            (fact_id, coll_type, collection, fact, source, now, session_id),
        ).await?;

        conn.execute(
            "INSERT INTO memory_facts_vectors (fact_id, collection, embedding) VALUES (?, ?, ?)",
            (fact_id, collection, blob_bytes),
        ).await?;

        conn.execute(
            "INSERT OR IGNORE INTO memory_relations (from_id, to_id, relation, created_at) VALUES (?, ?, ?, ?)",
            (fact_id, matched_cand_id, PM_RELATION_MERGED, now),
        ).await?;

        conn.execute(
            "UPDATE memory_facts SET created_at = ? WHERE id = ?",
            (now, matched_cand_id),
        ).await?;

        conn.execute(
            "UPDATE personal_memory_queue SET status = ?, processed_at = ? WHERE id = ?",
            (PM_QUEUE_STATUS_COMPLETED, now, job_id),
        ).await?;

        anyhow::Ok(())
    })().await {
        Ok(_) => {
            conn.execute("COMMIT;", ()).await?;
            anyhow::Ok(())
        }
        Err(e) => {
            let _ = conn.execute("ROLLBACK;", ()).await;
            Err(anyhow!("Exact merge transaction failed: {}", e))
        }
    }
}

/// Atomic database write transaction for persisting an ingested fact, its vector, relations, and updating queue status.
pub async fn insert_fact_with_vector_and_relations(
    conn: &Connection,
    job_id: i64,
    fact_id: &str,
    fact: &str,
    collection: &str,
    source: &str,
    session_id: &str,
    embedding: &[f32],
    relations: Vec<(String, String, &'static str)>,
    now: i64,
) -> Result<()> {
    let coll_type = collection_type(collection);
    let blob_bytes = encode_f32_blob(embedding);

    conn.execute("BEGIN TRANSACTION;", ()).await?;
    match (|| async {
        conn.execute(
            "INSERT INTO memory_facts (id, type, collection, fact, source, status, created_at, session_id) 
             VALUES (?, ?, ?, ?, ?, 'active', ?, ?)",
            (fact_id, coll_type, collection, fact, source, now, session_id),
        ).await?;

        conn.execute(
            "INSERT INTO memory_facts_vectors (fact_id, collection, embedding) VALUES (?, ?, ?)",
            (fact_id, collection, blob_bytes),
        ).await?;

        for (from, to, rel) in relations {
            conn.execute(
                "INSERT OR IGNORE INTO memory_relations (from_id, to_id, relation, created_at) VALUES (?, ?, ?, ?)",
                (from, to, rel, now),
            ).await?;
        }

        conn.execute(
            "UPDATE personal_memory_queue SET status = ?, processed_at = ? WHERE id = ?",
            (PM_QUEUE_STATUS_COMPLETED, now, job_id),
        ).await?;

        anyhow::Ok(())
    })().await {
        Ok(_) => {
            conn.execute("COMMIT;", ()).await?;
            anyhow::Ok(())
        }
        Err(e) => {
            let _ = conn.execute("ROLLBACK;", ()).await;
            Err(anyhow!("Fact persistence transaction failed: {}", e))
        }
    }
}

/// Executes Session End Consolidation Sweep in a single atomic transaction.
/// Promotes staged Tasks to pending, purges other staged queue items (e.g. Context),
/// and writes final session Context paragraph directly to memory_facts.
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
        // 1. Bulk-update staged Tasks to pending in personal_memory_queue
        conn.execute(
            "UPDATE personal_memory_queue 
             SET status = 'pending' 
             WHERE session_id = ? AND status = 'staged' AND collection = 'Tasks'",
            (session_id.to_string(),),
        ).await?;

        // 2. Delete any other staged queue items for this session (e.g. intermediate Context)
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

/// Inserts a manually edited user fact and writes a USER_SUPERSEDES edge old → new.
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
        "INSERT INTO memory_relations (from_id, to_id, relation, created_at) VALUES (?, ?, ?, ?)",
        (
            new_id.clone(),
            old_id.to_string(),
            PM_RELATION_USER_SUPERSEDES.to_string(),
            now,
        ),
    ).await?;

    conn.execute(
        "UPDATE memory_facts SET status = 'superseded' WHERE id = ?",
        (old_id.to_string(),),
    ).await?;

    Ok(new_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_enqueue_personal_facts_staging_ephemerality() -> Result<()> {
        let db = turso::Builder::new_local(":memory:").build().await?;
        let conn = db.connect()?;
        crate::persistence::schema::run_migrations(&conn).await?;

        let mut facts = HashMap::new();
        facts.insert("Context".to_string(), vec!["User discussed Rust.".to_string()]);
        facts.insert("Tasks".to_string(), vec!["Write unit test".to_string()]);
        facts.insert("Goals".to_string(), vec!["Run marathon".to_string()]);
        facts.insert("Identity".to_string(), vec!["Name is Alex".to_string()]);

        enqueue_personal_facts(&conn, facts, "session_123").await?;

        // Verify Context and Tasks are 'staged'
        let mut rows = conn.query("SELECT collection, status FROM personal_memory_queue WHERE session_id = 'session_123'", ()).await?;
        let mut statuses = HashMap::new();
        while let Some(row) = rows.next().await? {
            let col: String = row.get(0)?;
            let st: String = row.get(1)?;
            statuses.insert(col, st);
        }

        assert_eq!(statuses.get("Context").unwrap(), "staged");
        assert_eq!(statuses.get("Tasks").unwrap(), "staged");
        // Goals and Identity must be pending
        assert_eq!(statuses.get("Goals").unwrap(), "pending");
        assert_eq!(statuses.get("Identity").unwrap(), "pending");

        Ok(())
    }

    #[tokio::test]
    async fn test_session_end_consolidation() -> Result<()> {
        let db = turso::Builder::new_local(":memory:").build().await?;
        let conn = db.connect()?;
        crate::persistence::schema::run_migrations(&conn).await?;

        let mut facts = HashMap::new();
        facts.insert("Context".to_string(), vec!["Intermediate context".to_string()]);
        facts.insert("Tasks".to_string(), vec!["Final task".to_string()]);
        enqueue_personal_facts(&conn, facts, "session_456").await?;

        session_end_consolidation(&conn, "session_456", "Final session summary narrative.").await?;

        // Tasks should be promoted from 'staged' -> 'pending'
        let mut task_rows = conn.query("SELECT status FROM personal_memory_queue WHERE collection = 'Tasks'", ()).await?;
        let task_status: String = task_rows.next().await?.unwrap().get(0)?;
        assert_eq!(task_status, "pending");

        // Intermediate Context in queue should be deleted
        let mut ctx_queue = conn.query("SELECT count(*) FROM personal_memory_queue WHERE collection = 'Context'", ()).await?;
        let count: i64 = ctx_queue.next().await?.unwrap().get(0)?;
        assert_eq!(count, 0);

        // Final Context should be saved in memory_facts
        let mut mf_rows = conn.query("SELECT fact FROM memory_facts WHERE collection = 'Context' AND session_id = 'session_456'", ()).await?;
        let saved_fact: String = mf_rows.next().await?.unwrap().get(0)?;
        assert_eq!(saved_fact, "Final session summary narrative.");

        Ok(())
    }
}
