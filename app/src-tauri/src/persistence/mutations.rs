use anyhow::{anyhow, Result};
use turso::Connection;
use std::collections::HashMap;
use crate::core::constants::{
    PM_RELATION_SUPERSEDES, PM_SOURCE_USER, PM_QUEUE_STATUS_PAUSED, PM_QUEUE_STATUS_PENDING,
    PM_QUEUE_STATUS_COMPLETED, collection_type, PM_TYPE_SEMANTIC,
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
            PM_QUEUE_STATUS_PENDING
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
        assert_eq!(queue_items[0], ("Buy groceries".to_string(), "Tasks".to_string(), "pending".to_string(), "session_123".to_string()));
        assert_eq!(queue_items[1], ("Rust programming".to_string(), "Skills".to_string(), "pending".to_string(), "session_123".to_string()));

        Ok(())
    }

    #[tokio::test]
    async fn test_mark_job_failed() -> Result<()> {
        let conn = setup_test_db().await?;
        conn.execute(
            "INSERT INTO personal_memory_queue (fact, collection, session_id, status, attempts, created_at) VALUES ('fact1', 'Skills', 'sess1', 'pending', 0, 1000)",
            (),
        ).await?;

        mark_job_failed(&conn, 1, "Connection timeout").await;

        let mut rows = conn.query("SELECT status, error_msg, attempts FROM personal_memory_queue WHERE id = 1", ()).await?;
        let row = rows.next().await?.expect("job should exist");
        let status: String = row.get(0)?;
        let err_msg: String = row.get(1)?;
        let attempts: i64 = row.get(2)?;

        assert_eq!(status, "failed");
        assert_eq!(err_msg, "Connection timeout");
        assert_eq!(attempts, 1);

        Ok(())
    }

    #[tokio::test]
    async fn test_insert_exact_merged_fact() -> Result<()> {
        let conn = setup_test_db().await?;
        conn.execute(
            "INSERT INTO personal_memory_queue (id, fact, collection, session_id, status, created_at) VALUES (10, 'Rust dev', 'Skills', 'sess1', 'pending', 1000)",
            (),
        ).await?;

        let dummy_embedding = vec![0.1f32; 384];
        insert_exact_merged_fact(
            &conn,
            10,
            "mem_new_1",
            "Rust dev",
            "Skills",
            "LLM",
            "sess1",
            "mem_old_1",
            &dummy_embedding,
            2000,
        ).await?;

        let mut rows = conn.query("SELECT id, collection, status FROM memory_facts WHERE id = 'mem_new_1'", ()).await?;
        let row = rows.next().await?.expect("fact should exist");
        assert_eq!(row.get::<String>(0)?, "mem_new_1");
        assert_eq!(row.get::<String>(1)?, "Skills");
        assert_eq!(row.get::<String>(2)?, "merged");

        let mut v_rows = conn.query("SELECT fact_id FROM memory_facts_vectors WHERE fact_id = 'mem_new_1'", ()).await?;
        assert!(v_rows.next().await?.is_some());

        let mut r_rows = conn.query("SELECT from_id, to_id, relation, source FROM memory_relations WHERE from_id = 'mem_new_1'", ()).await?;
        let r_row = r_rows.next().await?.expect("relation edge should exist");
        assert_eq!(r_row.get::<String>(0)?, "mem_new_1");
        assert_eq!(r_row.get::<String>(1)?, "mem_old_1");
        assert_eq!(r_row.get::<String>(2)?, "SUPERSEDES");
        assert_eq!(r_row.get::<String>(3)?, "DEDUP");

        let mut q_rows = conn.query("SELECT status FROM personal_memory_queue WHERE id = 10", ()).await?;
        assert_eq!(q_rows.next().await?.unwrap().get::<String>(0)?, "completed");

        Ok(())
    }

    #[tokio::test]
    async fn test_insert_fact_with_vector_and_relations() -> Result<()> {
        let conn = setup_test_db().await?;
        conn.execute(
            "INSERT INTO personal_memory_queue (id, fact, collection, session_id, status, created_at) VALUES (5, 'Knows Rust', 'Skills', 'sess1', 'pending', 1000)",
            (),
        ).await?;

        conn.execute(
            "INSERT INTO memory_facts (id, type, collection, fact, source, status, created_at) VALUES ('mem_target_1', 'semantic', 'Projects', 'Building Vox', 'LLM', 'active', 900)",
            (),
        ).await?;

        let dummy_embedding = vec![0.2f32; 384];
        let relations = vec![
            ("mem_fact_1".to_string(), "mem_target_1".to_string(), "requires_skill", "LLM"),
            ("mem_target_1".to_string(), "mem_fact_1".to_string(), "used_in_project", "LLM"),
        ];

        insert_fact_with_vector_and_relations(
            &conn,
            5,
            "mem_fact_1",
            "Knows Rust",
            "Skills",
            "LLM",
            "sess1",
            &dummy_embedding,
            relations,
            2000,
        ).await?;

        let mut rows = conn.query("SELECT status FROM memory_facts WHERE id = 'mem_fact_1'", ()).await?;
        assert_eq!(rows.next().await?.unwrap().get::<String>(0)?, "active");

        let mut r_rows = conn.query("SELECT from_id, to_id, relation FROM memory_relations ORDER BY id ASC", ()).await?;
        let mut edges = Vec::new();
        while let Some(row) = r_rows.next().await? {
            edges.push((row.get::<String>(0)?, row.get::<String>(1)?, row.get::<String>(2)?));
        }
        assert_eq!(edges.len(), 2);
        assert_eq!(edges[0], ("mem_fact_1".to_string(), "mem_target_1".to_string(), "requires_skill".to_string()));
        assert_eq!(edges[1], ("mem_target_1".to_string(), "mem_fact_1".to_string(), "used_in_project".to_string()));

        let mut q_rows = conn.query("SELECT status FROM personal_memory_queue WHERE id = 5", ()).await?;
        assert_eq!(q_rows.next().await?.unwrap().get::<String>(0)?, "completed");

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
