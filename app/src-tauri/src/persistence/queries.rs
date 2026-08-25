use crate::persistence::encode_f32_blob;
use crate::services::memory::MemoryFact;
use anyhow::Result;
use std::collections::HashMap;
use turso::Connection;

/// Fetches all active Identity facts (deterministic baseline for non-ChitChat scopes).
pub async fn fetch_all_active_identity(conn: &Connection) -> Result<Vec<MemoryFact>> {
    let mut rows = conn
        .query(
            "SELECT id, type, collection, fact, source, status, created_at FROM memory_facts
             WHERE collection = 'Identity' AND status = 'active'
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

/// Fetches latest K active Directives facts (ordered by recency DESC).
pub async fn fetch_latest_directives(conn: &Connection, limit: u32) -> Result<Vec<MemoryFact>> {
    let mut rows = conn
        .query(
            "SELECT id, type, collection, fact, source, status, created_at FROM memory_facts
             WHERE collection = 'Directives' AND status = 'active'
             ORDER BY created_at DESC LIMIT ?",
            (limit as i64,),
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

/// Fetches active Narrative history facts (ordered by recency DESC).
pub async fn fetch_narrative_history(conn: &Connection, limit: u32) -> Result<Vec<MemoryFact>> {
    let mut rows = conn
        .query(
            "SELECT id, type, collection, fact, source, status, created_at FROM memory_facts
             WHERE collection = 'Narrative' AND status = 'active'
             ORDER BY created_at DESC LIMIT ?",
            (limit as i64,),
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

    let params: Vec<turso::Value> = fact_ids
        .iter()
        .map(|id| turso::Value::Text(id.clone()))
        .collect();
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

/// Fetches intra-collection NLI candidates using Turso vector_distance_cos pushdown SQL search.
/// Returns (id, fact_text, cosine_sim) tuples pre-filtered by cosine similarity threshold.
/// Combines active historical facts in `memory_facts` AND in-flight items in `personal_memory_queue`.
pub async fn fetch_intra_collection_candidates(
    conn: &Connection,
    collection: &str,
    query_embedding: &[f32],
    threshold: f32,
    limit: Option<i64>,
) -> Result<Vec<(String, String, f32)>> {
    let query_blob = encode_f32_blob(query_embedding);

    let (query_str, params) = match limit {
        Some(lim) if lim > 0 => (
            "SELECT id, fact, sim FROM (
                SELECT mf.id as id, mf.fact as fact, (1.0 - vector_distance_cos(mfv.embedding, ?)) as sim
                FROM memory_facts_vectors mfv
                JOIN memory_facts mf ON mf.id = mfv.fact_id
                WHERE mf.collection = ? AND mf.status = 'active'
                
                UNION ALL
                
                SELECT printf('item_%d', q.id) as id, q.fact as fact, (1.0 - vector_distance_cos(q.vector, ?)) as sim
                FROM personal_memory_queue q
                WHERE q.collection = ? AND q.status IN ('embedded', 'evaluated') AND q.vector IS NOT NULL
             ) WHERE sim >= ? ORDER BY sim DESC LIMIT ?".to_string(),
            vec![
                turso::Value::Blob(query_blob.clone()),
                turso::Value::Text(collection.to_string()),
                turso::Value::Blob(query_blob.clone()),
                turso::Value::Text(collection.to_string()),
                turso::Value::Real(threshold as f64),
                turso::Value::Integer(lim),
            ],
        ),
        _ => (
            "SELECT id, fact, sim FROM (
                SELECT mf.id as id, mf.fact as fact, (1.0 - vector_distance_cos(mfv.embedding, ?)) as sim
                FROM memory_facts_vectors mfv
                JOIN memory_facts mf ON mf.id = mfv.fact_id
                WHERE mf.collection = ? AND mf.status = 'active'
                
                UNION ALL
                
                SELECT printf('item_%d', q.id) as id, q.fact as fact, (1.0 - vector_distance_cos(q.vector, ?)) as sim
                FROM personal_memory_queue q
                WHERE q.collection = ? AND q.status IN ('embedded', 'evaluated') AND q.vector IS NOT NULL
             ) WHERE sim >= ? ORDER BY sim DESC".to_string(),
            vec![
                turso::Value::Blob(query_blob.clone()),
                turso::Value::Text(collection.to_string()),
                turso::Value::Blob(query_blob.clone()),
                turso::Value::Text(collection.to_string()),
                turso::Value::Real(threshold as f64),
            ],
        ),
    };

    let mut cand_rows = conn.query(&query_str, params).await?;

    let mut candidates = Vec::new();
    while let Some(row) = cand_rows.next().await? {
        let id: String = row.get(0)?;
        let f_text: String = row.get(1)?;
        let sim: f64 = row.get(2)?;
        candidates.push((id, f_text, sim as f32));
    }
    Ok(candidates)
}

/// Fetches active vector candidates across 5 factual collections (for Stage 2 soft deduplication & priority resolution).
/// Returns (id, fact_text, collection, cosine_sim) tuples pre-filtered by cosine similarity threshold.
/// Combines active historical facts in `memory_facts` AND in-flight items in `personal_memory_queue`.
pub async fn fetch_cross_collection_candidates(
    conn: &Connection,
    query_embedding: &[f32],
    threshold: f32,
    limit: Option<i64>,
) -> Result<Vec<(String, String, String, f32)>> {
    let query_blob = encode_f32_blob(query_embedding);

    let (query_str, params) = match limit {
        Some(lim) if lim > 0 => (
            "SELECT id, fact, collection, sim FROM (
                SELECT mf.id as id, mf.fact as fact, mf.collection as collection, (1.0 - vector_distance_cos(mfv.embedding, ?)) as sim
                FROM memory_facts_vectors mfv
                JOIN memory_facts mf ON mf.id = mfv.fact_id
                WHERE mf.collection IN ('Identity', 'Constraints', 'Directives', 'Profile', 'Entities') AND mf.status = 'active'
                
                UNION ALL
                
                SELECT printf('item_%d', q.id) as id, q.fact as fact, q.collection as collection, (1.0 - vector_distance_cos(q.vector, ?)) as sim
                FROM personal_memory_queue q
                WHERE q.collection IN ('Identity', 'Constraints', 'Directives', 'Profile', 'Entities') AND q.status IN ('embedded', 'evaluated') AND q.vector IS NOT NULL
             ) WHERE sim >= ? ORDER BY sim DESC LIMIT ?".to_string(),
            vec![
                turso::Value::Blob(query_blob.clone()),
                turso::Value::Blob(query_blob.clone()),
                turso::Value::Real(threshold as f64),
                turso::Value::Integer(lim),
            ],
        ),
        _ => (
            "SELECT id, fact, collection, sim FROM (
                SELECT mf.id as id, mf.fact as fact, mf.collection as collection, (1.0 - vector_distance_cos(mfv.embedding, ?)) as sim
                FROM memory_facts_vectors mfv
                JOIN memory_facts mf ON mf.id = mfv.fact_id
                WHERE mf.collection IN ('Identity', 'Constraints', 'Directives', 'Profile', 'Entities') AND mf.status = 'active'
                
                UNION ALL
                
                SELECT printf('item_%d', q.id) as id, q.fact as fact, q.collection as collection, (1.0 - vector_distance_cos(q.vector, ?)) as sim
                FROM personal_memory_queue q
                WHERE q.collection IN ('Identity', 'Constraints', 'Directives', 'Profile', 'Entities') AND q.status IN ('embedded', 'evaluated') AND q.vector IS NOT NULL
             ) WHERE sim >= ? ORDER BY sim DESC".to_string(),
            vec![
                turso::Value::Blob(query_blob.clone()),
                turso::Value::Blob(query_blob.clone()),
                turso::Value::Real(threshold as f64),
            ],
        ),
    };

    let mut cand_rows = conn.query(&query_str, params).await?;

    let mut candidates = Vec::new();
    while let Some(row) = cand_rows.next().await? {
        let id: String = row.get(0)?;
        let f_text: String = row.get(1)?;
        let col: String = row.get(2)?;
        let sim: f64 = row.get(3)?;
        candidates.push((id, f_text, col, sim as f32));
    }
    Ok(candidates)
}

/// Fetches inter-collection LLM edge candidates using Turso vector_distance_cos pushdown SQL search.
/// Returns (id, fact_text, collection, cosine_sim) tuples pre-filtered by cosine similarity threshold.
/// Combines active historical facts in `memory_facts` AND in-flight items in `personal_memory_queue`.
pub async fn fetch_inter_collection_candidates(
    conn: &Connection,
    target_collections: &[&str],
    query_embedding: &[f32],
    threshold: f32,
    limit: Option<i64>,
) -> Result<Vec<(String, String, String, f32)>> {
    if target_collections.is_empty() {
        return Ok(Vec::new());
    }

    let query_blob = encode_f32_blob(query_embedding);
    let placeholders = vec!["?"; target_collections.len()].join(",");

    let has_limit = matches!(limit, Some(lim) if lim > 0);

    let query_str = if has_limit {
        format!(
            "SELECT id, fact, collection, sim FROM (
                SELECT mf.id as id, mf.fact as fact, mf.collection as collection, (1.0 - vector_distance_cos(mfv.embedding, ?)) as sim
                FROM memory_facts_vectors mfv
                JOIN memory_facts mf ON mf.id = mfv.fact_id
                WHERE mf.collection IN ({}) AND mf.status = 'active'
                
                UNION ALL
                
                SELECT printf('item_%d', q.id) as id, q.fact as fact, q.collection as collection, (1.0 - vector_distance_cos(q.vector, ?)) as sim
                FROM personal_memory_queue q
                WHERE q.collection IN ({}) AND q.status IN ('embedded', 'evaluated') AND q.vector IS NOT NULL
             ) WHERE sim >= ? ORDER BY sim DESC LIMIT ?",
            placeholders, placeholders
        )
    } else {
        format!(
            "SELECT id, fact, collection, sim FROM (
                SELECT mf.id as id, mf.fact as fact, mf.collection as collection, (1.0 - vector_distance_cos(mfv.embedding, ?)) as sim
                FROM memory_facts_vectors mfv
                JOIN memory_facts mf ON mf.id = mfv.fact_id
                WHERE mf.collection IN ({}) AND mf.status = 'active'
                
                UNION ALL
                
                SELECT printf('item_%d', q.id) as id, q.fact as fact, q.collection as collection, (1.0 - vector_distance_cos(q.vector, ?)) as sim
                FROM personal_memory_queue q
                WHERE q.collection IN ({}) AND q.status IN ('embedded', 'evaluated') AND q.vector IS NOT NULL
             ) WHERE sim >= ? ORDER BY sim DESC",
            placeholders, placeholders
        )
    };

    let mut params: Vec<turso::Value> = Vec::new();
    params.push(turso::Value::Blob(query_blob.clone()));
    for col in target_collections {
        params.push(turso::Value::Text(col.to_string()));
    }
    params.push(turso::Value::Blob(query_blob.clone()));
    for col in target_collections {
        params.push(turso::Value::Text(col.to_string()));
    }
    params.push(turso::Value::Real(threshold as f64));

    if let Some(lim) = limit {
        if lim > 0 {
            params.push(turso::Value::Integer(lim));
        }
    }

    let mut cand_rows = conn.query(&query_str, params).await?;

    let mut candidates = Vec::new();
    while let Some(row) = cand_rows.next().await? {
        let id: String = row.get(0)?;
        let f_text: String = row.get(1)?;
        let col: String = row.get(2)?;
        let sim: f64 = row.get(3)?;
        candidates.push((id, f_text, col, sim as f32));
    }
    Ok(candidates)
}

/// Fetches intra-collection candidates in the sub-floor window [floor_threshold, ceil_threshold).
/// Used exclusively by eval_pipeline.rs post-pipeline audit pass.
pub async fn fetch_intra_subfloor_candidates(
    conn: &Connection,
    collection: &str,
    query_embedding: &[f32],
    floor_threshold: f32,
    ceil_threshold: f32,
    limit: Option<i64>,
) -> Result<Vec<(String, String, f32)>> {
    let query_blob = encode_f32_blob(query_embedding);

    let (query_str, params) = match limit {
        Some(lim) if lim > 0 => (
            "SELECT id, fact, sim FROM (
                SELECT mf.id as id, mf.fact as fact, (1.0 - vector_distance_cos(mfv.embedding, ?)) as sim
                FROM memory_facts_vectors mfv
                JOIN memory_facts mf ON mf.id = mfv.fact_id
                WHERE mf.collection = ? AND mf.status = 'active'
                
                UNION ALL
                
                SELECT printf('item_%d', q.id) as id, q.fact as fact, (1.0 - vector_distance_cos(q.vector, ?)) as sim
                FROM personal_memory_queue q
                WHERE q.collection = ? AND q.status IN ('embedded', 'evaluated') AND q.vector IS NOT NULL
             ) WHERE sim >= ? AND sim < ? ORDER BY sim DESC LIMIT ?".to_string(),
            vec![
                turso::Value::Blob(query_blob.clone()),
                turso::Value::Text(collection.to_string()),
                turso::Value::Blob(query_blob.clone()),
                turso::Value::Text(collection.to_string()),
                turso::Value::Real(floor_threshold as f64),
                turso::Value::Real(ceil_threshold as f64),
                turso::Value::Integer(lim),
            ],
        ),
        _ => (
            "SELECT id, fact, sim FROM (
                SELECT mf.id as id, mf.fact as fact, (1.0 - vector_distance_cos(mfv.embedding, ?)) as sim
                FROM memory_facts_vectors mfv
                JOIN memory_facts mf ON mf.id = mfv.fact_id
                WHERE mf.collection = ? AND mf.status = 'active'
                
                UNION ALL
                
                SELECT printf('item_%d', q.id) as id, q.fact as fact, (1.0 - vector_distance_cos(q.vector, ?)) as sim
                FROM personal_memory_queue q
                WHERE q.collection = ? AND q.status IN ('embedded', 'evaluated') AND q.vector IS NOT NULL
             ) WHERE sim >= ? AND sim < ? ORDER BY sim DESC".to_string(),
            vec![
                turso::Value::Blob(query_blob.clone()),
                turso::Value::Text(collection.to_string()),
                turso::Value::Blob(query_blob.clone()),
                turso::Value::Text(collection.to_string()),
                turso::Value::Real(floor_threshold as f64),
                turso::Value::Real(ceil_threshold as f64),
            ],
        ),
    };

    let mut cand_rows = conn.query(&query_str, params).await?;

    let mut candidates = Vec::new();
    while let Some(row) = cand_rows.next().await? {
        let id: String = row.get(0)?;
        let f_text: String = row.get(1)?;
        let sim: f64 = row.get(2)?;
        candidates.push((id, f_text, sim as f32));
    }
    Ok(candidates)
}

/// Fetches inter-collection candidates in the sub-floor window [floor_threshold, ceil_threshold).
/// Used exclusively by eval_pipeline.rs post-pipeline audit pass.
pub async fn fetch_inter_subfloor_candidates(
    conn: &Connection,
    target_collections: &[&str],
    query_embedding: &[f32],
    floor_threshold: f32,
    ceil_threshold: f32,
    limit: Option<i64>,
) -> Result<Vec<(String, String, String, f32)>> {
    if target_collections.is_empty() {
        return Ok(Vec::new());
    }

    let query_blob = encode_f32_blob(query_embedding);
    let placeholders = vec!["?"; target_collections.len()].join(",");

    let has_limit = matches!(limit, Some(lim) if lim > 0);

    let query_str = if has_limit {
        format!(
            "SELECT id, fact, collection, sim FROM (
                SELECT mf.id as id, mf.fact as fact, mf.collection as collection, (1.0 - vector_distance_cos(mfv.embedding, ?)) as sim
                FROM memory_facts_vectors mfv
                JOIN memory_facts mf ON mf.id = mfv.fact_id
                WHERE mf.collection IN ({}) AND mf.status = 'active'
                
                UNION ALL
                
                SELECT printf('item_%d', q.id) as id, q.fact as fact, q.collection as collection, (1.0 - vector_distance_cos(q.vector, ?)) as sim
                FROM personal_memory_queue q
                WHERE q.collection IN ({}) AND q.status IN ('embedded', 'evaluated') AND q.vector IS NOT NULL
             ) WHERE sim >= ? AND sim < ? ORDER BY sim DESC LIMIT ?",
            placeholders, placeholders
        )
    } else {
        format!(
            "SELECT id, fact, collection, sim FROM (
                SELECT mf.id as id, mf.fact as fact, mf.collection as collection, (1.0 - vector_distance_cos(mfv.embedding, ?)) as sim
                FROM memory_facts_vectors mfv
                JOIN memory_facts mf ON mf.id = mfv.fact_id
                WHERE mf.collection IN ({}) AND mf.status = 'active'
                
                UNION ALL
                
                SELECT printf('item_%d', q.id) as id, q.fact as fact, q.collection as collection, (1.0 - vector_distance_cos(q.vector, ?)) as sim
                FROM personal_memory_queue q
                WHERE q.collection IN ({}) AND q.status IN ('embedded', 'evaluated') AND q.vector IS NOT NULL
             ) WHERE sim >= ? AND sim < ? ORDER BY sim DESC",
            placeholders, placeholders
        )
    };

    let mut params: Vec<turso::Value> = Vec::new();
    params.push(turso::Value::Blob(query_blob.clone()));
    for col in target_collections {
        params.push(turso::Value::Text(col.to_string()));
    }
    params.push(turso::Value::Blob(query_blob.clone()));
    for col in target_collections {
        params.push(turso::Value::Text(col.to_string()));
    }
    params.push(turso::Value::Real(floor_threshold as f64));
    params.push(turso::Value::Real(ceil_threshold as f64));

    if let Some(lim) = limit {
        if lim > 0 {
            params.push(turso::Value::Integer(lim));
        }
    }

    let mut cand_rows = conn.query(&query_str, params).await?;

    let mut candidates = Vec::new();
    while let Some(row) = cand_rows.next().await? {
        let id: String = row.get(0)?;
        let f_text: String = row.get(1)?;
        let col: String = row.get(2)?;
        let sim: f64 = row.get(3)?;
        candidates.push((id, f_text, col, sim as f32));
    }
    Ok(candidates)
}
