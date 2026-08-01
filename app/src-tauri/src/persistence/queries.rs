use anyhow::Result;
use turso::Connection;
use std::collections::HashMap;
use crate::persistence::encode_f32_blob;
use crate::services::memory::MemoryFact;

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



/// Fetches intra-collection NLI candidates using Turso vector_distance_cos pushdown SQL search.
/// Returns (id, fact_text) tuples pre-filtered by cosine similarity threshold.
/// If `limit` is None, candidate selection is purely threshold-driven without K-capping (Spec §System Behavioral Invariants #3).
pub async fn fetch_intra_collection_candidates(
    conn: &Connection,
    collection: &str,
    query_embedding: &[f32],
    threshold: f32,
    limit: Option<i64>,
) -> Result<Vec<(String, String)>> {
    let query_blob = encode_f32_blob(query_embedding);

    let (query_str, params) = match limit {
        Some(lim) if lim > 0 => (
            "SELECT mf.id, mf.fact
             FROM memory_facts_vectors mfv
             JOIN memory_facts mf ON mf.id = mfv.fact_id
             WHERE mf.collection = ? AND mf.status = 'active'
               AND (1.0 - vector_distance_cos(mfv.embedding, ?)) >= ?
             ORDER BY vector_distance_cos(mfv.embedding, ?) ASC
             LIMIT ?".to_string(),
            vec![
                turso::Value::Text(collection.to_string()),
                turso::Value::Blob(query_blob.clone()),
                turso::Value::Real(threshold as f64),
                turso::Value::Blob(query_blob),
                turso::Value::Integer(lim),
            ],
        ),
        _ => (
            "SELECT mf.id, mf.fact
             FROM memory_facts_vectors mfv
             JOIN memory_facts mf ON mf.id = mfv.fact_id
             WHERE mf.collection = ? AND mf.status = 'active'
               AND (1.0 - vector_distance_cos(mfv.embedding, ?)) >= ?
             ORDER BY vector_distance_cos(mfv.embedding, ?) ASC".to_string(),
            vec![
                turso::Value::Text(collection.to_string()),
                turso::Value::Blob(query_blob.clone()),
                turso::Value::Real(threshold as f64),
                turso::Value::Blob(query_blob),
            ],
        ),
    };

    let mut cand_rows = conn.query(&query_str, params).await?;

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
/// If `limit` is None, candidate selection is purely threshold-driven without K-capping (Spec §System Behavioral Invariants #3).
pub async fn fetch_inter_collection_candidates(
    conn: &Connection,
    target_collections: &[&str],
    query_embedding: &[f32],
    threshold: f32,
    limit: Option<i64>,
) -> Result<Vec<(String, String, String)>> {
    if target_collections.is_empty() {
        return Ok(Vec::new());
    }

    let query_blob = encode_f32_blob(query_embedding);
    let placeholders = vec!["?"; target_collections.len()].join(",");

    let has_limit = matches!(limit, Some(lim) if lim > 0);

    let query_str = if has_limit {
        format!(
            "SELECT mf.id, mf.fact, mf.collection
             FROM memory_facts_vectors mfv
             JOIN memory_facts mf ON mf.id = mfv.fact_id
             WHERE mf.collection IN ({}) AND mf.status = 'active'
               AND (1.0 - vector_distance_cos(mfv.embedding, ?)) >= ?
             ORDER BY vector_distance_cos(mfv.embedding, ?) ASC
             LIMIT ?",
            placeholders
        )
    } else {
        format!(
            "SELECT mf.id, mf.fact, mf.collection
             FROM memory_facts_vectors mfv
             JOIN memory_facts mf ON mf.id = mfv.fact_id
             WHERE mf.collection IN ({}) AND mf.status = 'active'
               AND (1.0 - vector_distance_cos(mfv.embedding, ?)) >= ?
             ORDER BY vector_distance_cos(mfv.embedding, ?) ASC",
            placeholders
        )
    };

    let mut params: Vec<turso::Value> = Vec::new();
    for col in target_collections {
        params.push(turso::Value::Text(col.to_string()));
    }
    params.push(turso::Value::Blob(query_blob.clone()));
    params.push(turso::Value::Real(threshold as f64));
    params.push(turso::Value::Blob(query_blob));

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
        candidates.push((id, f_text, col));
    }
    Ok(candidates)
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
    async fn test_fetch_all_active_identity() -> Result<()> {
        let conn = setup_test_db().await?;

        conn.execute(
            "INSERT INTO memory_facts (id, type, collection, fact, source, status, created_at, session_id)
             VALUES ('id_1', 'special_state', 'Identity', 'Name is Bob', 'User', 'active', 100, 'sess_1')",
            (),
        ).await?;
        conn.execute(
            "INSERT INTO memory_facts (id, type, collection, fact, source, status, created_at, session_id)
             VALUES ('id_2', 'special_state', 'Identity', 'User is Developer', 'LLM', 'active', 200, '')",
            (),
        ).await?;

        let facts = fetch_all_active_identity(&conn).await?;
        assert_eq!(facts.len(), 2);
        assert_eq!(facts[0].id, "id_1");
        assert_eq!(facts[1].id, "id_2");

        Ok(())
    }

    #[tokio::test]
    async fn test_fetch_latest_directives() -> Result<()> {
        let conn = setup_test_db().await?;

        conn.execute(
            "INSERT INTO memory_facts (id, type, collection, fact, source, status, created_at, session_id)
             VALUES ('dir_1', 'special_state', 'Directives', 'Format as markdown', 'LLM', 'active', 100, '')",
            (),
        ).await?;
        conn.execute(
            "INSERT INTO memory_facts (id, type, collection, fact, source, status, created_at, session_id)
             VALUES ('dir_2', 'special_state', 'Directives', 'Use dark theme', 'LLM', 'active', 200, '')",
            (),
        ).await?;

        let facts = fetch_latest_directives(&conn, 1).await?;
        assert_eq!(facts.len(), 1);
        assert_eq!(facts[0].id, "dir_2");

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
        let intra = fetch_intra_collection_candidates(&conn, "Skills", &query_emb, 0.5, Some(10)).await?;
        assert_eq!(intra.len(), 1);
        assert_eq!(intra[0].0, "v_1");

        let target_colls = vec!["Skills", "Projects"];
        let inter = fetch_inter_collection_candidates(&conn, &target_colls, &query_emb, 0.5, Some(10)).await?;
        assert_eq!(inter.len(), 2);

        Ok(())
    }

}
