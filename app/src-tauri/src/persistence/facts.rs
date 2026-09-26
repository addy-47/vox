use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::Result;
use serde::{Deserialize, Serialize};
use turso::Connection;

use super::{decode_f32_blob, encode_f32_blob};

/// Strongly-typed row representation of a fact in `memory_facts`.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct FactRecord {
    pub id: String,
    pub session_id: Option<i64>,
    pub compaction_id: i64,
    pub fact_type: String,
    pub text: String,
    pub status: String,
    pub created_at: i64,
    pub updated_at: i64,
}

/// Inserts a newly deduplicated fact into `memory_facts`.
pub async fn insert_fact(conn: &Connection, fact: &FactRecord) -> Result<()> {
    conn.execute(
        "INSERT INTO memory_facts (id, session_id, compaction_id, type, text, status, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        (
            fact.id.clone(),
            fact.session_id,
            fact.compaction_id,
            fact.fact_type.clone(),
            fact.text.clone(),
            fact.status.clone(),
            fact.created_at,
            fact.updated_at,
        ),
    )
    .await?;

    Ok(())
}

/// Fetches all active facts matching a specific category/type.
pub async fn fetch_active_facts_by_type(
    conn: &Connection,
    fact_type: &str,
) -> Result<Vec<FactRecord>> {
    let mut rows = conn
        .query(
            "SELECT id, session_id, compaction_id, type, text, status, created_at, updated_at
             FROM memory_facts
             WHERE status = 'active' AND type = ?
             ORDER BY created_at DESC",
            (fact_type.to_string(),),
        )
        .await?;

    let mut facts = Vec::new();
    while let Some(row) = rows.next().await? {
        facts.push(FactRecord {
            id: row.get(0)?,
            session_id: row.get(1).ok(),
            compaction_id: row.get(2)?,
            fact_type: row.get(3)?,
            text: row.get(4)?,
            status: row.get(5)?,
            created_at: row.get(6)?,
            updated_at: row.get(7)?,
        });
    }

    Ok(facts)
}

/// Fetches all active facts across every type, optionally scoped to one project via the session join. Facts orphaned by hard session deletes (NULL session_id) appear only in the unscoped global view.
pub async fn fetch_all_active_facts(
    conn: &Connection,
    project_id: Option<&str>,
) -> Result<Vec<FactRecord>> {
    let mut rows = if let Some(pid) = project_id {
        conn.query(
            "SELECT f.id, f.session_id, f.compaction_id, f.type, f.text, f.status, f.created_at, f.updated_at
             FROM memory_facts f
             JOIN sessions s ON s.id = f.session_id
             WHERE f.status = 'active' AND s.project_id = ?
             ORDER BY f.created_at DESC",
            (pid.to_string(),),
        )
        .await?
    } else {
        conn.query(
            "SELECT id, session_id, compaction_id, type, text, status, created_at, updated_at
             FROM memory_facts
             WHERE status = 'active'
             ORDER BY created_at DESC",
            (),
        )
        .await?
    };

    let mut facts = Vec::new();
    while let Some(row) = rows.next().await? {
        facts.push(FactRecord {
            id: row.get(0)?,
            session_id: row.get(1).ok(),
            compaction_id: row.get(2)?,
            fact_type: row.get(3)?,
            text: row.get(4)?,
            status: row.get(5)?,
            created_at: row.get(6)?,
            updated_at: row.get(7)?,
        });
    }

    Ok(facts)
}

/// Deactivates a fact (superseded by newer duplicate in dedup) across facts and vectors tables.
pub async fn deactivate_fact(conn: &Connection, fact_id: &str) -> Result<()> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;

    conn.execute("BEGIN IMMEDIATE;", ()).await?;

    let tx_res: Result<()> = async {
        conn.execute(
            "UPDATE memory_facts SET status = 'inactive', updated_at = ? WHERE id = ?",
            (now, fact_id.to_string()),
        )
        .await?;

        conn.execute(
            "UPDATE memory_facts_vectors SET status = 'inactive' WHERE fact_id = ?",
            (fact_id.to_string(),),
        )
        .await?;

        Ok(())
    }
    .await;

    match tx_res {
        Ok(_) => {
            conn.execute("COMMIT;", ()).await?;
            Ok(())
        }
        Err(e) => {
            if let Err(rb_err) = conn.execute("ROLLBACK;", ()).await {
                log::warn!("[Persistence::Facts] Rollback failed: {}", rb_err);
            }
            Err(e)
        }
    }
}

/// Deactivates a batch of facts in a single transaction (eliminates N+1 write cycles).
/// Mirrors the batched pattern in `mark_facts_consolidated`.
pub async fn deactivate_facts_batch(conn: &Connection, fact_ids: &[String]) -> Result<usize> {
    if fact_ids.is_empty() {
        return Ok(0);
    }

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;

    conn.execute("BEGIN IMMEDIATE;", ()).await?;

    let tx_res: Result<()> = async {
        let quoted_ids: Vec<String> = fact_ids.iter().map(|id| format!("'{}'", id)).collect();
        let in_clause = quoted_ids.join(",");

        let sql_facts = format!(
            "UPDATE memory_facts SET status = 'inactive', updated_at = ? WHERE id IN ({})",
            in_clause
        );
        conn.execute(&sql_facts, (now,)).await?;

        let sql_vectors = format!(
            "UPDATE memory_facts_vectors SET status = 'inactive' WHERE fact_id IN ({})",
            in_clause
        );
        conn.execute(&sql_vectors, ()).await?;

        Ok(())
    }
    .await;

    match tx_res {
        Ok(_) => {
            conn.execute("COMMIT;", ()).await?;
            Ok(fact_ids.len())
        }
        Err(e) => {
            if let Err(rb_err) = conn.execute("ROLLBACK;", ()).await {
                log::warn!("[Persistence::Facts] Batch rollback failed: {}", rb_err);
            }
            Err(e)
        }
    }
}

/// Marks a list of personal facts as consolidated into the personal memory document.
pub async fn mark_facts_consolidated(conn: &Connection, fact_ids: &[String]) -> Result<()> {
    if fact_ids.is_empty() {
        return Ok(());
    }

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;

    conn.execute("BEGIN IMMEDIATE;", ()).await?;

    let tx_res: Result<()> = async {
        let quoted_ids: Vec<String> = fact_ids.iter().map(|id| format!("'{}'", id)).collect();
        let in_clause = quoted_ids.join(",");

        let sql_facts = format!(
            "UPDATE memory_facts SET status = 'consolidated', updated_at = ? WHERE id IN ({})",
            in_clause
        );
        conn.execute(&sql_facts, (now,)).await?;

        let sql_vectors = format!(
            "UPDATE memory_facts_vectors SET status = 'consolidated' WHERE fact_id IN ({})",
            in_clause
        );
        conn.execute(&sql_vectors, ()).await?;

        Ok(())
    }
    .await;

    match tx_res {
        Ok(_) => {
            conn.execute("COMMIT;", ()).await?;
            Ok(())
        }
        Err(e) => {
            if let Err(rb_err) = conn.execute("ROLLBACK;", ()).await {
                log::warn!("[Persistence::Facts] Rollback failed: {}", rb_err);
            }
            Err(e)
        }
    }
}

/// Marks a list of personal facts as staged for review in personal memory suggestions.
pub async fn mark_facts_staged(conn: &Connection, fact_ids: &[String]) -> Result<()> {
    if fact_ids.is_empty() {
        return Ok(());
    }

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;

    conn.execute("BEGIN IMMEDIATE;", ()).await?;

    let tx_res: Result<()> = async {
        let quoted_ids: Vec<String> = fact_ids.iter().map(|id| format!("'{}'", id)).collect();
        let in_clause = quoted_ids.join(",");

        let sql_facts = format!(
            "UPDATE memory_facts SET status = 'staged', updated_at = ? WHERE id IN ({})",
            in_clause
        );
        conn.execute(&sql_facts, (now,)).await?;

        let sql_vectors = format!(
            "UPDATE memory_facts_vectors SET status = 'staged' WHERE fact_id IN ({})",
            in_clause
        );
        conn.execute(&sql_vectors, ()).await?;

        Ok(())
    }
    .await;

    match tx_res {
        Ok(_) => {
            conn.execute("COMMIT;", ()).await?;
            Ok(())
        }
        Err(e) => {
            if let Err(rb_err) = conn.execute("ROLLBACK;", ()).await {
                log::warn!("[Persistence::Facts] Rollback failed: {}", rb_err);
            }
            Err(e)
        }
    }
}

/// Marks a list of personal facts as rejected so they are not re-suggested in future consolidations.
pub async fn mark_facts_rejected(conn: &Connection, fact_ids: &[String]) -> Result<()> {
    if fact_ids.is_empty() {
        return Ok(());
    }

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;

    conn.execute("BEGIN IMMEDIATE;", ()).await?;

    let tx_res: Result<()> = async {
        let quoted_ids: Vec<String> = fact_ids.iter().map(|id| format!("'{}'", id)).collect();
        let in_clause = quoted_ids.join(",");

        let sql_facts = format!(
            "UPDATE memory_facts SET status = 'rejected', updated_at = ? WHERE id IN ({})",
            in_clause
        );
        conn.execute(&sql_facts, (now,)).await?;

        let sql_vectors = format!(
            "UPDATE memory_facts_vectors SET status = 'rejected' WHERE fact_id IN ({})",
            in_clause
        );
        conn.execute(&sql_vectors, ()).await?;

        Ok(())
    }
    .await;

    match tx_res {
        Ok(_) => {
            conn.execute("COMMIT;", ()).await?;
            Ok(())
        }
        Err(e) => {
            if let Err(rb_err) = conn.execute("ROLLBACK;", ()).await {
                log::warn!("[Persistence::Facts] Rollback failed: {}", rb_err);
            }
            Err(e)
        }
    }
}

/// Inserts a dense float vector embedding for a fact into `memory_facts_vectors`.
pub async fn insert_vector(
    conn: &Connection,
    fact_id: &str,
    fact_type: &str,
    status: &str,
    project_id: Option<&str>,
    embedding: &[f32],
) -> Result<()> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;
    let blob = encode_f32_blob(embedding);

    conn.execute(
        "INSERT INTO memory_facts_vectors (fact_id, type, status, project_id, created_at, embedding)
         VALUES (?, ?, ?, ?, ?, ?)",
        (
            fact_id.to_string(),
            fact_type.to_string(),
            status.to_string(),
            project_id.map(|s| s.to_string()),
            now,
            blob,
        ),
    )
    .await?;

    Ok(())
}

/// Fetches all active vector embeddings for a given fact type.
pub async fn fetch_active_vectors_by_type(
    conn: &Connection,
    fact_type: &str,
) -> Result<Vec<(String, Vec<f32>)>> {
    let mut rows = conn
        .query(
            "SELECT fact_id, embedding FROM memory_facts_vectors WHERE status = 'active' AND type = ?",
            (fact_type.to_string(),),
        )
        .await?;

    let mut results = Vec::new();
    while let Some(row) = rows.next().await? {
        let fact_id: String = row.get(0)?;
        let blob: Vec<u8> = row.get(1)?;
        let floats = decode_f32_blob(&blob);
        results.push((fact_id, floats));
    }

    Ok(results)
}

/// Episodic fact candidate returned from persistence layer for hybrid retrieval.
#[derive(Debug, Clone)]
pub struct EpisodicFactCandidate {
    pub id: String,
    pub fact_type: String,
    pub text: String,
    pub embedding: Option<Vec<f32>>,
}

/// Fetches all active non-personal episodic facts with their vector embeddings.
pub async fn fetch_active_episodic_memory(conn: &Connection) -> Result<Vec<EpisodicFactCandidate>> {
    let mut rows = conn
        .query(
            "SELECT f.id, f.type, f.text, v.embedding
             FROM memory_facts f
             LEFT JOIN memory_facts_vectors v ON f.id = v.fact_id
             WHERE f.status = 'active' AND f.type != 'personal'
             ORDER BY f.created_at DESC",
            (),
        )
        .await?;

    let mut results = Vec::new();
    while let Some(row) = rows.next().await? {
        let id: String = row.get(0)?;
        let fact_type: String = row.get(1)?;
        let text: String = row.get(2)?;
        let embedding = row
            .get::<Vec<u8>>(3)
            .ok()
            .map(|blob| decode_f32_blob(&blob));

        results.push(EpisodicFactCandidate {
            id,
            fact_type,
            text,
            embedding,
        });
    }

    Ok(results)
}
