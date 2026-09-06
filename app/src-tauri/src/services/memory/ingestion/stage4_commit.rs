use std::collections::HashMap;

use anyhow::Result;
use turso::Connection;

use super::RelationEdge;
use crate::{
    persistence::mutations,
    services::memory::{collection_type, QueueStatus, Relation, STAGE4_BATCH_SIZE},
};

/// Claimed item pending stage 4 SQL database commit and queue deletion.
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

/// Atomically claims candidate evaluated and superseded items from the queue.
async fn claim_commit_candidates(conn: &Connection, now: i64) -> Result<Vec<Stage4Item>> {
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

    let mut items = Vec::new();
    for item in candidate_items {
        let updated = conn.execute(
            "UPDATE personal_memory_queue SET status = ?, claimed_at = ? WHERE id = ? AND status IN ('evaluated', 'superseded')",
            (QueueStatus::ProcessingCommit.as_str(), now, item.id),
        )
        .await?;

        if updated > 0 {
            items.push(item);
        }
    }

    Ok(items)
}

/// Commits fact, vector embedding, and graph relationships for an item into SQLite tables.
async fn commit_item_to_storage(
    conn: &Connection,
    item: &Stage4Item,
    fact_id: &str,
    id_map: &HashMap<String, String>,
    now: i64,
) -> Result<usize> {
    let coll_type = collection_type(&item.collection);
    let item_status = item.status.as_str();
    let mut relations_count = 0;

    if item_status == QueueStatus::Evaluated.as_str()
        || item_status == QueueStatus::Superseded.as_str()
    {
        let fact_status = if item_status == QueueStatus::Superseded.as_str() {
            "superseded"
        } else {
            "active"
        };

        conn.execute(
            "INSERT INTO memory_facts (id, type, collection, fact, source, status, session_id, created_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
            (
                fact_id,
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

        if let Some(ref vec_blob) = item.vector {
            conn.execute(
                "INSERT INTO memory_facts_vectors (fact_id, collection, embedding) VALUES (?, ?, ?)",
                (fact_id, item.collection.clone(), vec_blob.clone()),
            )
            .await?;
        }

        if let Some(ref rels_json) = item.relations_json {
            if let Ok(relations) = serde_json::from_str::<Vec<RelationEdge>>(rels_json) {
                for rel in relations {
                    let from_id = id_map.get(&rel.from_id).cloned().unwrap_or(rel.from_id);
                    let to_id = id_map.get(&rel.to_id).cloned().unwrap_or(rel.to_id);

                    if from_id != to_id {
                        conn.execute(
                            "INSERT OR IGNORE INTO memory_relations (from_id, to_id, relation, source, created_at)
                             VALUES (?, ?, ?, ?, ?)",
                            (from_id, to_id.clone(), rel.relation.clone(), rel.source, now),
                        )
                        .await?;

                        relations_count += 1;

                        if rel.relation == Relation::Supersedes.as_str() {
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

    Ok(relations_count)
}

/// Stage 4: Commit & Prune Worker (Batch Size 32)
pub async fn run_stage4_commit(conn: &Connection) -> Result<usize> {
    run_stage4_commit_with_metrics(conn, "").await
}

/// Executes Stage 4 fact commit and queue cleanup with metrics recording.
pub async fn run_stage4_commit_with_metrics(conn: &Connection, run_id: &str) -> Result<usize> {
    let start_time = std::time::Instant::now();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;

    let items = claim_commit_candidates(conn, now).await?;
    if items.is_empty() {
        return Ok(0);
    }

    let items_claimed = items.len();
    log::info!(
        "[MemoryPipeline::Stage4] Claimed {} evaluated/superseded items for SQLite commit",
        items_claimed
    );
    let session_id = items
        .first()
        .map(|i| i.session_id.clone())
        .unwrap_or_default();

    let mut id_map: HashMap<String, String> = HashMap::new();
    for item in &items {
        let fact_id = format!("mem_{}_{}", now, uuid::Uuid::new_v4().simple());
        id_map.insert(format!("item_{}", item.id), fact_id);
    }

    conn.execute("BEGIN TRANSACTION", ()).await?;

    let mut committed_ids = Vec::new();

    let commit_res: Result<()> = async {
        for item in &items {
            let item_placeholder = format!("item_{}", item.id);
            let fact_id = id_map
                .get(&item_placeholder)
                .cloned()
                .unwrap_or_else(|| format!("mem_{}_{}", now, uuid::Uuid::new_v4().simple()));

            commit_item_to_storage(conn, item, &fact_id, &id_map, now).await?;
            committed_ids.push(item.id);
        }

        for id in &committed_ids {
            conn.execute("DELETE FROM personal_memory_queue WHERE id = ?", (*id,))
                .await?;
        }

        Ok(())
    }
    .await;

    if let Err(e) = commit_res {
        if let Err(rb_err) = conn.execute("ROLLBACK", ()).await {
            log::warn!("[MemoryPipeline::Stage4] Rollback failed: {}", rb_err);
        }
        return Err(e);
    }

    conn.execute("COMMIT", ()).await?;

    let processed_count = committed_ids.len();
    let error_count = items_claimed.saturating_sub(processed_count);
    let duration_ms = start_time.elapsed().as_millis();

    if !run_id.is_empty() {
        let metrics = super::PipelineStageMetrics {
            run_id: run_id.to_string(),
            stage_name: "stage4_commit".to_string(),
            session_id,
            batch_seq: 0,
            items_claimed,
            error_count,
            duration_ms,
        };
        if let Err(e) = mutations::record_stage_metrics(conn, &metrics).await {
            log::warn!(
                "[MemoryPipeline::Stage4] Failed to record stage metrics: {}",
                e
            );
        }
    }

    Ok(processed_count)
}
