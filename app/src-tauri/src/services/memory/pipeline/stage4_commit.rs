use anyhow::Result;
use turso::Connection;
use crate::core::constants::{
    collection_type, PM_QUEUE_STATUS_EVALUATED, PM_QUEUE_STATUS_PROCESSING_COMMIT,
    PM_QUEUE_STATUS_SUPERSEDED,
};
use super::batch_result::RelationEdge;

pub const STAGE4_BATCH_SIZE: usize = 32;

#[derive(Debug, Clone)]
pub struct Stage4Item {
    pub id: i64,
    pub fact: String,
    pub collection: String,
    pub source: String,
    pub session_id: String,
    pub status: String,
    pub created_at: i64,
    pub vector: Option<Vec<u8>>,
    pub relations_json: Option<String>,
}

/// Stage 4: Commit & Prune Worker (Batch Size 32)
/// Atomically claims `evaluated` and `superseded` items, writes active facts to `memory_facts`,
/// vectors to `memory_facts_vectors`, graph edges to `memory_relations`, and deletes completed rows from `personal_memory_queue`.
pub async fn run_stage4_commit(conn: &Connection) -> Result<usize> {
    run_stage4_commit_with_metrics(conn, "").await
}

pub async fn run_stage4_commit_with_metrics(conn: &Connection, run_id: &str) -> Result<usize> {
    let start_time = std::time::Instant::now();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;

    // 1. Select candidate evaluated and superseded items
    let mut rows = conn
        .query(
            "SELECT id, fact, collection, source, session_id, status, created_at, vector, relations_json
             FROM personal_memory_queue
             WHERE status IN ('evaluated', 'superseded') ORDER BY created_at ASC LIMIT ?",
            (STAGE4_BATCH_SIZE as i64,),
        )
        .await?;

    let mut candidate_items = Vec::new();
    while let Some(row) = rows.next().await? {
        candidate_items.push(Stage4Item {
            id: row.get::<i64>(0)?,
            fact: row.get::<String>(1)?,
            collection: row.get::<String>(2)?,
            source: row.get::<String>(3)?,
            session_id: row.get::<String>(4)?,
            status: row.get::<String>(5)?,
            created_at: row.get::<i64>(6)?,
            vector: row.get::<Option<Vec<u8>>>(7)?,
            relations_json: row.get::<Option<String>>(8)?,
        });
    }

    if candidate_items.is_empty() {
        return Ok(0);
    }

    // 2. Atomically claim candidate items in DB
    let mut items = Vec::new();
    for item in candidate_items {
        let updated = conn.execute(
            "UPDATE personal_memory_queue SET status = ?, claimed_at = ? WHERE id = ? AND status IN ('evaluated', 'superseded')",
            (PM_QUEUE_STATUS_PROCESSING_COMMIT, now, item.id),
        )
        .await?;

        if updated > 0 {
            items.push(item);
        }
    }

    if items.is_empty() {
        return Ok(0);
    }

    let items_claimed = items.len();
    let session_id = items.first().map(|i| i.session_id.clone()).unwrap_or_default();

    // Pre-allocate UUID fact_ids for all items in the batch so intra-batch relations resolve cleanly
    let mut id_map: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    for item in &items {
        let fact_id = format!("mem_{}_{}", now, uuid::Uuid::new_v4().simple());
        id_map.insert(format!("item_{}", item.id), fact_id);
    }

    conn.execute("BEGIN TRANSACTION", ()).await?;

    let mut committed_ids = Vec::new();
    let mut total_relations_committed = 0;

    let commit_res: Result<()> = async {
        for item in items {
            let item_placeholder = format!("item_{}", item.id);
            let fact_id = id_map
                .get(&item_placeholder)
                .cloned()
                .unwrap_or_else(|| format!("mem_{}_{}", now, uuid::Uuid::new_v4().simple()));
            let coll_type = collection_type(&item.collection);

            let item_status = item.status.as_str();
            if item_status == PM_QUEUE_STATUS_EVALUATED || item_status == PM_QUEUE_STATUS_SUPERSEDED {
                let fact_status = if item_status == PM_QUEUE_STATUS_SUPERSEDED { "superseded" } else { "active" };

                // 1. Insert into memory_facts
                conn.execute(
                    "INSERT INTO memory_facts (id, type, collection, fact, source, status, session_id, created_at)
                     VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
                    (
                        fact_id.clone(),
                        coll_type.to_string(),
                        item.collection.clone(),
                        item.fact.clone(),
                        item.source.clone(),
                        fact_status,
                        item.session_id.clone(),
                        now,
                    ),
                )
                .await?;

                // 2. Insert into memory_facts_vectors if present
                if let Some(ref vec_blob) = item.vector {
                    conn.execute(
                        "INSERT INTO memory_facts_vectors (fact_id, collection, embedding) VALUES (?, ?, ?)",
                        (fact_id.clone(), item.collection.clone(), vec_blob.clone()),
                    )
                    .await?;
                }

                // 3. Insert relations if present
                if let Some(ref rels_json) = item.relations_json {
                    if let Ok(relations) = serde_json::from_str::<Vec<RelationEdge>>(rels_json) {
                        for rel in relations {
                            let from_id = id_map.get(&rel.from_id).cloned().unwrap_or(rel.from_id);
                            let to_id = id_map.get(&rel.to_id).cloned().unwrap_or(rel.to_id);

                            // Guard against self-referential graph loops
                            if from_id != to_id {
                                conn.execute(
                                    "INSERT OR IGNORE INTO memory_relations (from_id, to_id, relation, source, created_at)
                                     VALUES (?, ?, ?, ?, ?)",
                                    (from_id, to_id.clone(), rel.relation.clone(), rel.source, now),
                                )
                                .await?;

                                total_relations_committed += 1;

                                if rel.relation == crate::core::constants::PM_RELATION_SUPERSEDES {
                                    conn.execute(
                                        "UPDATE memory_facts SET status = 'inactive' WHERE id = ?",
                                        (to_id,),
                                    )
                                    .await?;
                                }
                            }
                        }
                    }
                }
            }

            committed_ids.push(item.id);
        }

        // 4. Prune completed queue rows
        for id in &committed_ids {
            conn.execute("DELETE FROM personal_memory_queue WHERE id = ?", (*id,))
                .await?;
        }

        Ok(())
    }.await;

    if let Err(e) = commit_res {
        let _ = conn.execute("ROLLBACK", ()).await;
        return Err(e);
    }

    conn.execute("COMMIT", ()).await?;

    let processed_count = committed_ids.len();
    let duration_ms = start_time.elapsed().as_millis();

    if !run_id.is_empty() {
        let metrics = super::metrics::PipelineStageMetrics {
            run_id: run_id.to_string(),
            stage_name: "stage4_commit".to_string(),
            session_id,
            batch_seq: 0,
            items_claimed,
            error_count: 0,
            duration_ms,
        };
        let _ = crate::persistence::mutations::record_stage_metrics(conn, &metrics).await;
    }

    Ok(processed_count)
}
