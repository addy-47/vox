use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::Result;
use serde::{Deserialize, Serialize};
use turso::Connection;

use super::{decode_f32_blob, encode_f32_blob};

/// Strongly-typed row representation of an observation in `memory_facts`.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct ObservationRecord {
    pub id: String,
    pub session_id: Option<i64>,
    pub compaction_id: i64,
    pub observation_type: String,
    pub text: String,
    pub status: String,
    pub created_at: i64,
    pub updated_at: i64,
}

/// Inserts a newly deduplicated observation into `memory_facts`.
pub async fn insert_observation(conn: &Connection, observation: &ObservationRecord) -> Result<()> {
    conn.execute(
        "INSERT INTO memory_facts (id, session_id, compaction_id, type, text, status, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        (
            observation.id.clone(),
            observation.session_id,
            observation.compaction_id,
            observation.observation_type.clone(),
            observation.text.clone(),
            observation.status.clone(),
            observation.created_at,
            observation.updated_at,
        ),
    )
    .await?;

    Ok(())
}

/// Fetches all active observations matching a specific category/type.
pub async fn fetch_active_observations_by_type(
    conn: &Connection,
    observation_type: &str,
) -> Result<Vec<ObservationRecord>> {
    let mut rows = conn
        .query(
            "SELECT id, session_id, compaction_id, type, text, status, created_at, updated_at
             FROM memory_facts
             WHERE status = 'active' AND type = ?
             ORDER BY created_at DESC",
            (observation_type.to_string(),),
        )
        .await?;

    let mut observations = Vec::new();
    while let Some(row) = rows.next().await? {
        observations.push(ObservationRecord {
            id: row.get(0)?,
            session_id: row.get(1).ok(),
            compaction_id: row.get(2)?,
            observation_type: row.get(3)?,
            text: row.get(4)?,
            status: row.get(5)?,
            created_at: row.get(6)?,
            updated_at: row.get(7)?,
        });
    }

    Ok(observations)
}

/// Fetches all already integrated personal observations for whole-memory re-synthesis / regeneration.
pub async fn fetch_integrated_personal_facts(
    conn: &Connection,
    project_id: Option<&str>,
) -> Result<Vec<ObservationRecord>> {
    let mut rows = match project_id {
        Some(pid) => {
            conn.query(
                "SELECT f.id, f.session_id, f.compaction_id, f.type, f.text, f.status, f.created_at, f.updated_at
                 FROM memory_facts f
                 JOIN sessions s ON s.id = f.session_id
                 WHERE f.type = 'personal' AND f.status = 'integrated' AND s.project_id = ?
                 ORDER BY f.created_at ASC",
                (pid.to_string(),),
            )
            .await?
        }
        None => {
            conn.query(
                "SELECT id, session_id, compaction_id, type, text, status, created_at, updated_at
                 FROM memory_facts
                 WHERE type = 'personal' AND status = 'integrated'
                 ORDER BY created_at ASC",
                (),
            )
            .await?
        }
    };

    let mut observations = Vec::new();
    while let Some(row) = rows.next().await? {
        observations.push(ObservationRecord {
            id: row.get(0)?,
            session_id: row.get(1).ok(),
            compaction_id: row.get(2)?,
            observation_type: row.get(3)?,
            text: row.get(4)?,
            status: row.get(5)?,
            created_at: row.get(6)?,
            updated_at: row.get(7)?,
        });
    }

    Ok(observations)
}

/// Fetches observations across every type, optionally filtered by status, project, and observation type, with limit and offset.
pub async fn fetch_all_observations(
    conn: &Connection,
    project_id: Option<&str>,
    status: Option<&str>,
    limit: Option<u32>,
    offset: Option<u32>,
    observation_type: Option<&str>,
) -> Result<Vec<ObservationRecord>> {
    let limit_clause = match (limit, offset) {
        (Some(l), Some(o)) => format!(" LIMIT {} OFFSET {}", l, o),
        (Some(l), None) => format!(" LIMIT {}", l),
        (None, Some(o)) => format!(" LIMIT -1 OFFSET {}", o),
        (None, None) => String::new(),
    };

    let mut rows = match (project_id, status, observation_type) {
        (Some(pid), Some(st), Some(ot)) => {
            let sql = format!(
                "SELECT f.id, f.session_id, f.compaction_id, f.type, f.text, f.status, f.created_at, f.updated_at
                 FROM memory_facts f
                 JOIN sessions s ON s.id = f.session_id
                 WHERE f.status = ? AND s.project_id = ? AND f.type = ?
                 ORDER BY f.created_at DESC{}",
                limit_clause
            );
            conn.query(&sql, (st.to_string(), pid.to_string(), ot.to_string())).await?
        }
        (Some(pid), Some(st), None) => {
            let sql = format!(
                "SELECT f.id, f.session_id, f.compaction_id, f.type, f.text, f.status, f.created_at, f.updated_at
                 FROM memory_facts f
                 JOIN sessions s ON s.id = f.session_id
                 WHERE f.status = ? AND s.project_id = ?
                 ORDER BY f.created_at DESC{}",
                limit_clause
            );
            conn.query(&sql, (st.to_string(), pid.to_string())).await?
        }
        (Some(pid), None, Some(ot)) => {
            let sql = format!(
                "SELECT f.id, f.session_id, f.compaction_id, f.type, f.text, f.status, f.created_at, f.updated_at
                 FROM memory_facts f
                 JOIN sessions s ON s.id = f.session_id
                 WHERE s.project_id = ? AND f.type = ?
                 ORDER BY f.created_at DESC{}",
                limit_clause
            );
            conn.query(&sql, (pid.to_string(), ot.to_string())).await?
        }
        (Some(pid), None, None) => {
            let sql = format!(
                "SELECT f.id, f.session_id, f.compaction_id, f.type, f.text, f.status, f.created_at, f.updated_at
                 FROM memory_facts f
                 JOIN sessions s ON s.id = f.session_id
                 WHERE s.project_id = ?
                 ORDER BY f.created_at DESC{}",
                limit_clause
            );
            conn.query(&sql, (pid.to_string(),)).await?
        }
        (None, Some(st), Some(ot)) => {
            let sql = format!(
                "SELECT id, session_id, compaction_id, type, text, status, created_at, updated_at
                 FROM memory_facts
                 WHERE status = ? AND type = ?
                 ORDER BY created_at DESC{}",
                limit_clause
            );
            conn.query(&sql, (st.to_string(), ot.to_string())).await?
        }
        (None, Some(st), None) => {
            let sql = format!(
                "SELECT id, session_id, compaction_id, type, text, status, created_at, updated_at
                 FROM memory_facts
                 WHERE status = ?
                 ORDER BY created_at DESC{}",
                limit_clause
            );
            conn.query(&sql, (st.to_string(),)).await?
        }
        (None, None, Some(ot)) => {
            let sql = format!(
                "SELECT id, session_id, compaction_id, type, text, status, created_at, updated_at
                 FROM memory_facts
                 WHERE type = ?
                 ORDER BY created_at DESC{}",
                limit_clause
            );
            conn.query(&sql, (ot.to_string(),)).await?
        }
        (None, None, None) => {
            let sql = format!(
                "SELECT id, session_id, compaction_id, type, text, status, created_at, updated_at
                 FROM memory_facts
                 ORDER BY created_at DESC{}",
                limit_clause
            );
            conn.query(&sql, ()).await?
        }
    };

    let mut observations = Vec::new();
    while let Some(row) = rows.next().await? {
        observations.push(ObservationRecord {
            id: row.get(0)?,
            session_id: row.get(1).ok(),
            compaction_id: row.get(2)?,
            observation_type: row.get(3)?,
            text: row.get(4)?,
            status: row.get(5)?,
            created_at: row.get(6)?,
            updated_at: row.get(7)?,
        });
    }

    Ok(observations)
}

/// Fetches raw items from `memory_ingestion_queue` with status = 'pending', shaped as ObservationRecords.
/// Used by the UI "Pending" filter to show facts queued for background LLM extraction.
pub async fn fetch_pending_queue_observations(
    conn: &Connection,
    limit: Option<u32>,
    offset: Option<u32>,
    observation_type: Option<&str>,
) -> Result<Vec<ObservationRecord>> {
    let limit_clause = match (limit, offset) {
        (Some(l), Some(o)) => format!(" LIMIT {} OFFSET {}", l, o),
        (Some(l), None) => format!(" LIMIT {}", l),
        (None, Some(o)) => format!(" LIMIT -1 OFFSET {}", o),
        (None, None) => String::new(),
    };

    let mut rows = match observation_type {
        Some(ot) => {
            let sql = format!(
                "SELECT id, session_id, compaction_id, type, text, status, created_at \
                 FROM memory_ingestion_queue \
                 WHERE status = 'pending' AND type = ? \
                 ORDER BY created_at DESC{}",
                limit_clause
            );
            conn.query(&sql, (ot.to_string(),)).await?
        }
        None => {
            let sql = format!(
                "SELECT id, session_id, compaction_id, type, text, status, created_at \
                 FROM memory_ingestion_queue \
                 WHERE status = 'pending' \
                 ORDER BY created_at DESC{}",
                limit_clause
            );
            conn.query(&sql, ()).await?
        }
    };

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;

    let mut observations = Vec::new();
    while let Some(row) = rows.next().await? {
        let id: i64 = row.get(0)?;
        observations.push(ObservationRecord {
            id: id.to_string(),
            session_id: row.get(1).ok(),
            compaction_id: row.get(2)?,
            observation_type: row.get(3)?,
            text: row.get(4)?,
            status: "pending".to_string(),
            created_at: row.get(6)?,
            updated_at: now,
        });
    }

    Ok(observations)
}

/// Deactivates an observation (superseded by newer duplicate in dedup) across observations and vectors tables.
pub async fn deactivate_observation(conn: &Connection, observation_id: &str) -> Result<()> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;

    conn.execute("BEGIN IMMEDIATE;", ()).await?;

    let tx_res: Result<()> = async {
        conn.execute(
            "UPDATE memory_facts SET status = 'inactive', updated_at = ? WHERE id = ?",
            (now, observation_id.to_string()),
        )
        .await?;

        conn.execute(
            "UPDATE memory_facts_vectors SET status = 'inactive' WHERE fact_id = ?",
            (observation_id.to_string(),),
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
                log::warn!("[Persistence::Observations] Rollback failed: {}", rb_err);
            }
            Err(e)
        }
    }
}

/// Deactivates a batch of observations in a single transaction (eliminates N+1 write cycles).
/// Mirrors the batched pattern in `mark_observations_integrated`.
pub async fn deactivate_observations_batch(
    conn: &Connection,
    observation_ids: &[String],
) -> Result<usize> {
    if observation_ids.is_empty() {
        return Ok(0);
    }

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;

    conn.execute("BEGIN IMMEDIATE;", ()).await?;

    let tx_res: Result<()> = async {
        let quoted_ids: Vec<String> = observation_ids
            .iter()
            .map(|id| format!("'{}'", id))
            .collect();
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
            Ok(observation_ids.len())
        }
        Err(e) => {
            if let Err(rb_err) = conn.execute("ROLLBACK;", ()).await {
                log::warn!(
                    "[Persistence::Observations] Batch rollback failed: {}",
                    rb_err
                );
            }
            Err(e)
        }
    }
}

/// Marks a list of personal observations as integrated into the semantic Personal Memory model.
pub async fn mark_observations_integrated(
    conn: &Connection,
    observation_ids: &[String],
) -> Result<()> {
    if observation_ids.is_empty() {
        return Ok(());
    }

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;

    conn.execute("BEGIN IMMEDIATE;", ()).await?;

    let tx_res: Result<()> = async {
        let quoted_ids: Vec<String> = observation_ids
            .iter()
            .map(|id| format!("'{}'", id))
            .collect();
        let in_clause = quoted_ids.join(",");

        let sql_observations = format!(
            "UPDATE memory_facts SET status = 'integrated', updated_at = ? WHERE id IN ({})",
            in_clause
        );
        conn.execute(&sql_observations, (now,)).await?;

        let sql_vectors = format!(
            "UPDATE memory_facts_vectors SET status = 'integrated' WHERE fact_id IN ({})",
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
                log::warn!("[Persistence::Observations] Rollback failed: {}", rb_err);
            }
            Err(e)
        }
    }
}

/// Inserts a dense float vector embedding for an observation into `memory_facts_vectors`.
pub async fn insert_vector(
    conn: &Connection,
    observation_id: &str,
    observation_type: &str,
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
            observation_id.to_string(),
            observation_type.to_string(),
            status.to_string(),
            project_id.map(|s| s.to_string()),
            now,
            blob,
        ),
    )
    .await?;

    Ok(())
}

/// Fetches all active vector embeddings for a given observation type.
pub async fn fetch_active_vectors_by_type(
    conn: &Connection,
    observation_type: &str,
) -> Result<Vec<(String, Vec<f32>)>> {
    let mut rows = conn
        .query(
            "SELECT fact_id, embedding FROM memory_facts_vectors WHERE status = 'active' AND type = ?",
            (observation_type.to_string(),),
        )
        .await?;

    let mut results = Vec::new();
    while let Some(row) = rows.next().await? {
        let observation_id: String = row.get(0)?;
        let blob: Vec<u8> = row.get(1)?;
        let floats = decode_f32_blob(&blob);
        results.push((observation_id, floats));
    }

    Ok(results)
}

/// Episodic observation candidate returned from persistence layer for hybrid retrieval.
#[derive(Debug, Clone)]
pub struct EpisodicObservationCandidate {
    pub id: String,
    pub observation_type: String,
    pub text: String,
    pub embedding: Option<Vec<f32>>,
}

/// Fetches all active non-personal episodic observations with their vector embeddings.
pub async fn fetch_active_episodic_observations(
    conn: &Connection,
) -> Result<Vec<EpisodicObservationCandidate>> {
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
        let observation_type: String = row.get(1)?;
        let text: String = row.get(2)?;
        let embedding = row
            .get::<Vec<u8>>(3)
            .ok()
            .map(|blob| decode_f32_blob(&blob));

        results.push(EpisodicObservationCandidate {
            id,
            observation_type,
            text,
            embedding,
        });
    }

    Ok(results)
}
