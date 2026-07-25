use anyhow::Result;
use turso::Connection;
use std::collections::HashMap;
use crate::persistence::encode_f32_blob;
use crate::services::memory::MemoryFact;

/// Fetches active Identity + Constraints facts (Tier 1 Foundational), excluding the active session.
pub async fn fetch_foundational_facts(
    conn: &Connection,
    current_session_id: &str,
) -> Result<Vec<MemoryFact>> {
    let mut rows = conn
        .query(
            "SELECT id, type, collection, fact, source, status, created_at FROM memory_facts
             WHERE type = 'foundational' AND status = 'active'
               AND (session_id = '' OR session_id != ?)
             ORDER BY created_at ASC",
            (current_session_id.to_string(),),
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

/// Fetches active Tasks + Goals facts (Tier 1 Operational), excluding the active session.
pub async fn fetch_operational_facts(
    conn: &Connection,
    current_session_id: &str,
) -> Result<Vec<MemoryFact>> {
    let mut rows = conn
        .query(
            "SELECT id, type, collection, fact, source, status, created_at FROM memory_facts
             WHERE type = 'operational' AND collection IN ('Tasks', 'Goals') AND status = 'active'
               AND (session_id = '' OR session_id != ?)
             ORDER BY created_at DESC",
            (current_session_id.to_string(),),
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

/// Fetches active Class C semantic seed facts via Turso vector_distance_cos pushdown SQL query.
pub async fn fetch_semantic_seeds(
    conn: &Connection,
    query_embedding: &[f32],
    threshold: f32,
    limit_per_collection: i64,
    current_session_id: &str,
) -> Result<Vec<MemoryFact>> {
    let query_blob = encode_f32_blob(query_embedding);
    let mut rows = conn.query(
        "WITH Ranked AS (
             SELECT mf.id, mf.type, mf.collection, mf.fact, mf.source, mf.status, mf.created_at,
                    (1.0 - vector_distance_cos(mfv.embedding, ?)) as similarity,
                    ROW_NUMBER() OVER (
                        PARTITION BY mf.collection
                        ORDER BY vector_distance_cos(mfv.embedding, ?) ASC
                    ) as rank
             FROM memory_facts mf
             JOIN memory_facts_vectors mfv ON mfv.fact_id = mf.id
             WHERE mfv.collection IN ('Skills', 'Preferences', 'Projects', 'Experiences', 'Relationships')
               AND mf.status = 'active'
               AND (mf.session_id = '' OR mf.session_id != ?)
               AND (1.0 - vector_distance_cos(mfv.embedding, ?)) >= ?
         )
         SELECT id, type, collection, fact, source, status, created_at
         FROM Ranked
         WHERE rank <= ?",
        (query_blob.clone(), query_blob.clone(), current_session_id.to_string(), query_blob, threshold as f64, limit_per_collection),
    ).await?;

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

/// Fetches graph neighbor edges from `memory_relations` for a batch of fact IDs.
/// Returns tuples of (from_id, to_id, relation, source).
pub async fn fetch_graph_neighbors(
    conn: &Connection,
    fact_ids: &[String],
) -> Result<Vec<(String, String, String, String)>> {
    if fact_ids.is_empty() {
        return Ok(Vec::new());
    }

    let placeholders = vec!["?"; fact_ids.len()].join(",");
    let query_str = format!(
        "SELECT from_id, to_id, relation, source FROM memory_relations
         WHERE from_id IN ({0}) OR to_id IN ({0})",
        placeholders
    );

    let mut params: Vec<turso::Value> = Vec::with_capacity(fact_ids.len() * 2);
    for id in fact_ids {
        params.push(turso::Value::Text(id.clone()));
    }
    for id in fact_ids {
        params.push(turso::Value::Text(id.clone()));
    }

    let mut rows = conn.query(&query_str, params).await?;
    let mut neighbors = Vec::new();
    while let Some(row) = rows.next().await? {
        neighbors.push((
            row.get::<String>(0)?,
            row.get::<String>(1)?,
            row.get::<String>(2)?,
            row.get::<String>(3)?,
        ));
    }
    Ok(neighbors)
}

/// Batch fetches memory facts by their IDs, returning a HashMap of id -> MemoryFact.
pub async fn fetch_facts_by_ids(
    conn: &Connection,
    fact_ids: &[String],
) -> Result<HashMap<String, MemoryFact>> {
    if fact_ids.is_empty() {
        return Ok(HashMap::new());
    }

    let placeholders = vec!["?"; fact_ids.len()].join(",");
    let query_str = format!(
        "SELECT id, type, collection, fact, source, status, created_at FROM memory_facts
         WHERE id IN ({}) AND status = 'active'",
        placeholders
    );

    let params: Vec<turso::Value> = fact_ids.iter().map(|id| turso::Value::Text(id.clone())).collect();
    let mut rows = conn.query(&query_str, params).await?;
    let mut map = HashMap::new();
    while let Some(row) = rows.next().await? {
        let mf = MemoryFact {
            id: row.get(0)?,
            fact_type: row.get(1)?,
            collection: row.get(2)?,
            fact: row.get(3)?,
            source: row.get(4)?,
            status: row.get(5)?,
            created_at: row.get(6)?,
        };
        map.insert(mf.id.clone(), mf);
    }
    Ok(map)
}

/// Fetches active record counts for all memory collections, excluding the current session.
pub async fn fetch_active_collection_counts(
    conn: &Connection,
    current_session_id: &str,
) -> Result<HashMap<String, usize>> {
    let mut rows = conn
        .query(
            "SELECT collection, COUNT(*) FROM memory_facts
             WHERE status = 'active' AND (session_id = '' OR session_id != ?)
             GROUP BY collection",
            (current_session_id.to_string(),),
        )
        .await?;

    let mut map: HashMap<String, usize> = HashMap::new();
    while let Some(row) = rows.next().await? {
        let col: String = row.get(0)?;
        let count: i64 = row.get(1)?;
        map.insert(col, count as usize);
    }
    Ok(map)
}

/// Fetches all currently active facts from SQLite grouped by collection.
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

/// Fetches intra-collection NLI candidates using Turso vector_distance_cos pushdown SQL search.
/// Returns (id, fact_text) tuples pre-filtered by cosine similarity threshold.
pub async fn fetch_intra_collection_candidates(
    conn: &Connection,
    collection: &str,
    query_embedding: &[f32],
    threshold: f32,
    limit: i64,
) -> Result<Vec<(String, String)>> {
    let query_blob = encode_f32_blob(query_embedding);
    let mut cand_rows = conn.query(
        "SELECT mf.id, mf.fact
         FROM memory_facts_vectors mfv
         JOIN memory_facts mf ON mf.id = mfv.fact_id
         WHERE mf.collection = ? AND mf.status = 'active'
           AND (1.0 - vector_distance_cos(mfv.embedding, ?)) >= ?
         ORDER BY vector_distance_cos(mfv.embedding, ?) ASC
         LIMIT ?",
        (collection.to_string(), query_blob.clone(), threshold as f64, query_blob, limit),
    ).await?;

    let mut candidates = Vec::new();
    while let Some(row) = cand_rows.next().await? {
        let id: String = row.get(0)?;
        let f_text: String = row.get(1)?;
        candidates.push((id, f_text));
    }
    Ok(candidates)
}

/// Fetches inter-collection LLM edge candidates using Turso vector_distance_cos pushdown SQL search.
/// Returns (id, fact_text, collection) tuples pre-filtered by cosine similarity threshold.
pub async fn fetch_inter_collection_candidates(
    conn: &Connection,
    target_collections: &[&str],
    query_embedding: &[f32],
    threshold: f32,
    limit: i64,
) -> Result<Vec<(String, String, String)>> {
    if target_collections.is_empty() {
        return Ok(Vec::new());
    }

    let query_blob = encode_f32_blob(query_embedding);
    let placeholders = vec!["?"; target_collections.len()].join(",");
    let query_str = format!(
        "SELECT mf.id, mf.fact, mf.collection
         FROM memory_facts_vectors mfv
         JOIN memory_facts mf ON mf.id = mfv.fact_id
         WHERE mf.collection IN ({}) AND mf.status = 'active'
           AND (1.0 - vector_distance_cos(mfv.embedding, ?)) >= ?
         ORDER BY vector_distance_cos(mfv.embedding, ?) ASC
         LIMIT ?",
        placeholders
    );

    let mut params: Vec<turso::Value> = Vec::new();
    for col in target_collections {
        params.push(turso::Value::Text(col.to_string()));
    }
    params.push(turso::Value::Blob(query_blob.clone()));
    params.push(turso::Value::Real(threshold as f64));
    params.push(turso::Value::Blob(query_blob));
    params.push(turso::Value::Integer(limit));

    let mut cand_rows = conn.query(&query_str, params).await?;

    let mut candidates = Vec::new();
    while let Some(row) = cand_rows.next().await? {
        let id: String = row.get(0)?;
        let f_text: String = row.get(1)?;
        let col: String = row.get(2)?;
        candidates.push((id, f_text, col));
    }
    Ok(candidates)
}

/// Fetches active Context facts within the last window_hours, or falls back to the single most recent Context fact.
/// Returns (fact_text, created_at_ms) tuples.
pub async fn fetch_context_records(
    conn: &Connection,
    window_hours: u32,
) -> Result<Vec<(String, i64)>> {
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;
    let window_start = now_ms - (window_hours as i64 * 3600 * 1000);

    let mut rows = conn
        .query(
            "SELECT fact, created_at FROM memory_facts
             WHERE type = 'operational' AND collection = 'Context' AND status = 'active'
               AND created_at >= ?
             ORDER BY created_at DESC",
            (window_start,),
        )
        .await?;

    let mut contexts: Vec<(String, i64)> = Vec::new();
    while let Some(row) = rows.next().await? {
        let fact: String = row.get(0)?;
        let created_at: i64 = row.get(1)?;
        contexts.push((fact, created_at));
    }

    if contexts.is_empty() {
        let mut fallback_rows = conn
            .query(
                "SELECT fact, created_at FROM memory_facts
                 WHERE type = 'operational' AND collection = 'Context' AND status = 'active'
                   AND created_at < ?
                 ORDER BY created_at DESC LIMIT 1",
                (window_start,),
            )
            .await?;

        if let Some(row) = fallback_rows.next().await? {
            let fact: String = row.get(0)?;
            let created_at: i64 = row.get(1)?;
            contexts.push((fact, created_at));
        }
    }

    Ok(contexts)
}

/// Fetches the point-in-time session Context fact created on or before a fact's timestamp.
/// If none created on or before, falls back to the earliest Context fact for that session.
pub async fn fetch_fact_session_context(
    conn: &Connection,
    fact_id: &str,
) -> Result<Option<String>> {
    let mut fact_rows = conn
        .query(
            "SELECT session_id, created_at FROM memory_facts WHERE id = ?",
            (fact_id.to_string(),),
        )
        .await?;

    let (session_id, fact_created_at): (String, i64) = match fact_rows.next().await? {
        Some(row) => (row.get(0)?, row.get(1)?),
        None => return Ok(None),
    };

    if session_id.trim().is_empty() {
        return Ok(None);
    }

    let mut ctx_rows = conn
        .query(
            "SELECT fact FROM memory_facts
             WHERE type = 'operational' AND collection = 'Context' AND status = 'active'
               AND session_id = ? AND created_at <= ?
             ORDER BY created_at DESC LIMIT 1",
            (session_id.clone(), fact_created_at),
        )
        .await?;

    if let Some(row) = ctx_rows.next().await? {
        let text: String = row.get(0)?;
        return Ok(Some(text));
    }

    // Fallback: earliest Context fact for this session
    let mut fallback_rows = conn
        .query(
            "SELECT fact FROM memory_facts
             WHERE type = 'operational' AND collection = 'Context' AND status = 'active'
               AND session_id = ?
             ORDER BY created_at ASC LIMIT 1",
            (session_id,),
        )
        .await?;

    if let Some(row) = fallback_rows.next().await? {
        let text: String = row.get(0)?;
        return Ok(Some(text));
    }

    Ok(None)
}

/// Fetch session context for a parent (source) fact that hasn't been inserted yet.
/// Uses session_id + created_at directly instead of looking up the fact first.
pub async fn fetch_session_context(
    conn: &Connection,
    session_id: &str,
    at_or_before: i64,
) -> Result<Option<String>> {
    if session_id.trim().is_empty() {
        return Ok(None);
    }

    // Point-in-time: the most recent Context fact from this session created at or before `at_or_before`
    let mut ctx_rows = conn
        .query(
            "SELECT fact FROM memory_facts
             WHERE type = 'operational' AND collection = 'Context' AND status = 'active'
               AND session_id = ? AND created_at <= ?
             ORDER BY created_at DESC LIMIT 1",
            (session_id.to_string(), at_or_before),
        )
        .await?;

    if let Some(row) = ctx_rows.next().await? {
        let text: String = row.get(0)?;
        return Ok(Some(text));
    }

    // Fallback: earliest Context fact for this session
    let mut fallback_rows = conn
        .query(
            "SELECT fact FROM memory_facts
             WHERE type = 'operational' AND collection = 'Context' AND status = 'active'
               AND session_id = ?
             ORDER BY created_at ASC LIMIT 1",
            (session_id.to_string(),),
        )
        .await?;

    if let Some(row) = fallback_rows.next().await? {
        let text: String = row.get(0)?;
        return Ok(Some(text));
    }

    Ok(None)
}
