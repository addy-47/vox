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
    async fn test_fetch_foundational_facts() -> Result<()> {
        let conn = setup_test_db().await?;

        conn.execute(
            "INSERT INTO memory_facts (id, type, collection, fact, source, status, created_at, session_id)
             VALUES ('id_1', 'foundational', 'Identity', 'Name is Bob', 'User', 'active', 100, 'sess_1')",
            (),
        ).await?;
        conn.execute(
            "INSERT INTO memory_facts (id, type, collection, fact, source, status, created_at, session_id)
             VALUES ('id_2', 'foundational', 'Constraints', 'No nuts', 'LLM', 'active', 200, '')",
            (),
        ).await?;
        conn.execute(
            "INSERT INTO memory_facts (id, type, collection, fact, source, status, created_at, session_id)
             VALUES ('id_3', 'foundational', 'Constraints', 'No sugar', 'LLM', 'superseded', 300, '')",
            (),
        ).await?;

        let facts = fetch_foundational_facts(&conn, "sess_1").await?;
        assert_eq!(facts.len(), 1);
        assert_eq!(facts[0].id, "id_2");

        let facts2 = fetch_foundational_facts(&conn, "sess_2").await?;
        assert_eq!(facts2.len(), 2);
        assert_eq!(facts2[0].id, "id_1");
        assert_eq!(facts2[1].id, "id_2");

        Ok(())
    }

    #[tokio::test]
    async fn test_fetch_operational_facts() -> Result<()> {
        let conn = setup_test_db().await?;

        conn.execute(
            "INSERT INTO memory_facts (id, type, collection, fact, source, status, created_at, session_id)
             VALUES ('op_1', 'operational', 'Tasks', 'Task A', 'LLM', 'active', 100, '')",
            (),
        ).await?;
        conn.execute(
            "INSERT INTO memory_facts (id, type, collection, fact, source, status, created_at, session_id)
             VALUES ('op_2', 'operational', 'Goals', 'Goal B', 'LLM', 'active', 200, '')",
            (),
        ).await?;
        conn.execute(
            "INSERT INTO memory_facts (id, type, collection, fact, source, status, created_at, session_id)
             VALUES ('op_3', 'operational', 'Context', 'Ctx C', 'LLM', 'active', 300, '')",
            (),
        ).await?;

        let facts = fetch_operational_facts(&conn, "sess_none").await?;
        assert_eq!(facts.len(), 2);
        assert_eq!(facts[0].id, "op_2");
        assert_eq!(facts[1].id, "op_1");

        Ok(())
    }

    #[tokio::test]
    async fn test_fetch_facts_by_ids() -> Result<()> {
        let conn = setup_test_db().await?;

        conn.execute(
            "INSERT INTO memory_facts (id, type, collection, fact, source, status, created_at)
             VALUES ('f_1', 'semantic', 'Skills', 'Rust', 'LLM', 'active', 100)",
            (),
        ).await?;
        conn.execute(
            "INSERT INTO memory_facts (id, type, collection, fact, source, status, created_at)
             VALUES ('f_2', 'semantic', 'Skills', 'C++', 'LLM', 'superseded', 100)",
            (),
        ).await?;

        let ids = vec!["f_1".to_string(), "f_2".to_string(), "f_3".to_string()];
        let map = fetch_facts_by_ids(&conn, &ids).await?;

        assert_eq!(map.len(), 1);
        assert!(map.contains_key("f_1"));
        assert_eq!(map.get("f_1").unwrap().fact, "Rust");

        Ok(())
    }

    #[tokio::test]
    async fn test_fetch_active_collection_counts_and_grouped() -> Result<()> {
        let conn = setup_test_db().await?;

        conn.execute(
            "INSERT INTO memory_facts (id, type, collection, fact, source, status, created_at)
             VALUES ('c_1', 'foundational', 'Identity', 'Alex', 'LLM', 'active', 100)",
            (),
        ).await?;
        conn.execute(
            "INSERT INTO memory_facts (id, type, collection, fact, source, status, created_at)
             VALUES ('c_2', 'semantic', 'Skills', 'Rust', 'LLM', 'active', 100)",
            (),
        ).await?;
        conn.execute(
            "INSERT INTO memory_facts (id, type, collection, fact, source, status, created_at)
             VALUES ('c_3', 'semantic', 'Skills', 'Go', 'LLM', 'active', 200)",
            (),
        ).await?;

        let counts = fetch_active_collection_counts(&conn, "sess_1").await?;
        assert_eq!(counts.get("Identity"), Some(&1));
        assert_eq!(counts.get("Skills"), Some(&2));

        let grouped = fetch_active_facts_grouped(&conn).await?;
        assert_eq!(grouped.get("Skills").unwrap().len(), 2);
        assert_eq!(grouped.get("Skills").unwrap(), &vec!["Rust".to_string(), "Go".to_string()]);

        Ok(())
    }

    #[tokio::test]
    async fn test_fetch_graph_neighbors() -> Result<()> {
        let conn = setup_test_db().await?;

        conn.execute(
            "INSERT INTO memory_facts (id, type, collection, fact, source, status, created_at)
             VALUES ('node_a', 'semantic', 'Projects', 'Project Vox', 'LLM', 'active', 100)",
            (),
        ).await?;
        conn.execute(
            "INSERT INTO memory_facts (id, type, collection, fact, source, status, created_at)
             VALUES ('node_b', 'semantic', 'Skills', 'Rust', 'LLM', 'active', 100)",
            (),
        ).await?;
        conn.execute(
            "INSERT INTO memory_facts (id, type, collection, fact, source, status, created_at)
             VALUES ('node_c', 'operational', 'Tasks', 'Write tests', 'LLM', 'active', 100)",
            (),
        ).await?;

        conn.execute(
            "INSERT INTO memory_relations (from_id, to_id, relation, source, created_at)
             VALUES ('node_a', 'node_b', 'requires_skill', 'LLM', 1000)",
            (),
        ).await?;
        conn.execute(
            "INSERT INTO memory_relations (from_id, to_id, relation, source, created_at)
             VALUES ('node_b', 'node_a', 'used_in_project', 'LLM', 1000)",
            (),
        ).await?;
        conn.execute(
            "INSERT INTO memory_relations (from_id, to_id, relation, source, created_at)
             VALUES ('node_a', 'node_c', 'contains_task', 'LLM', 1000)",
            (),
        ).await?;

        let neighbors_b = fetch_graph_neighbors(&conn, &["node_b".to_string()]).await?;
        assert_eq!(neighbors_b.len(), 2);

        let neighbors_a = fetch_graph_neighbors(&conn, &["node_a".to_string()]).await?;
        assert_eq!(neighbors_a.len(), 3);

        Ok(())
    }

    #[tokio::test]
    async fn test_fetch_intra_and_inter_collection_candidates() -> Result<()> {
        let conn = setup_test_db().await?;

        conn.execute(
            "INSERT INTO memory_facts (id, type, collection, fact, source, status, created_at)
             VALUES ('v_1', 'semantic', 'Skills', 'Rust programming', 'LLM', 'active', 100)",
            (),
        ).await?;
        conn.execute(
            "INSERT INTO memory_facts (id, type, collection, fact, source, status, created_at)
             VALUES ('v_2', 'semantic', 'Projects', 'Vox desktop app', 'LLM', 'active', 100)",
            (),
        ).await?;

        let emb1 = vec![1.0f32; 384];
        let emb2 = vec![0.5f32; 384];
        let blob1 = encode_f32_blob(&emb1);
        let blob2 = encode_f32_blob(&emb2);

        conn.execute(
            "INSERT INTO memory_facts_vectors (fact_id, collection, embedding) VALUES ('v_1', 'Skills', ?)",
            (blob1,),
        ).await?;
        conn.execute(
            "INSERT INTO memory_facts_vectors (fact_id, collection, embedding) VALUES ('v_2', 'Projects', ?)",
            (blob2,),
        ).await?;

        let query_emb = vec![1.0f32; 384];
        let intra = fetch_intra_collection_candidates(&conn, "Skills", &query_emb, 0.5, 10).await?;
        assert_eq!(intra.len(), 1);
        assert_eq!(intra[0].0, "v_1");

        let target_colls = vec!["Skills", "Projects"];
        let inter = fetch_inter_collection_candidates(&conn, &target_colls, &query_emb, 0.5, 10).await?;
        assert_eq!(inter.len(), 2);

        Ok(())
    }

    #[tokio::test]
    async fn test_fetch_context_records_and_session_context() -> Result<()> {
        let conn = setup_test_db().await?;

        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64;

        conn.execute(
            "INSERT INTO memory_facts (id, type, collection, fact, source, status, session_id, created_at)
             VALUES ('ctx_1', 'operational', 'Context', 'Session 1 context summary', 'LLM', 'active', 's1', ?)",
            (now_ms - 1000,),
        ).await?;

        conn.execute(
            "INSERT INTO memory_facts (id, type, collection, fact, source, status, session_id, created_at)
             VALUES ('ctx_old', 'operational', 'Context', 'Old context summary', 'LLM', 'active', 's1', ?)",
            (now_ms - (100 * 3600 * 1000),),
        ).await?;

        let records = fetch_context_records(&conn, 24).await?;
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].0, "Session 1 context summary");

        let ctx = fetch_session_context(&conn, "s1", now_ms).await?;
        assert_eq!(ctx, Some("Session 1 context summary".to_string()));

        let fact_ctx = fetch_fact_session_context(&conn, "ctx_1").await?;
        assert_eq!(fact_ctx, Some("Session 1 context summary".to_string()));

        Ok(())
    }
}
