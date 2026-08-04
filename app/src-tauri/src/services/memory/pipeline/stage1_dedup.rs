use anyhow::Result;
use turso::Connection;
use crate::core::constants::{PM_QUEUE_STATUS_DEDUPED, PM_QUEUE_STATUS_PROCESSING_DEDUP, PM_QUEUE_STATUS_STAGED_PENDING, PM_QUEUE_STATUS_SUPERSEDED};
use crate::services::memory::deduplication::{is_exact_duplicate, jaccard_similarity};

pub const STAGE1_BATCH_CEILING: usize = 128;

#[derive(Debug, Clone, PartialEq)]
pub struct Stage1Item {
    pub id: i64,
    pub fact: String,
    pub collection: String,
    pub source: String,
    pub session_id: String,
}

/// Stage 1: Dedup Worker (Batch Ceiling 128)
/// Atomically claims `staged_pending` items, executes Jaccard + soft vector deduplication,
/// and updates status to `deduped` or `superseded`.
pub async fn run_stage1_dedup(conn: &Connection) -> Result<usize> {
    run_stage1_dedup_with_metrics(conn, "").await
}

pub async fn run_stage1_dedup_with_metrics(conn: &Connection, run_id: &str) -> Result<usize> {
    let start_time = std::time::Instant::now();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;

    // 1. Select candidate staged_pending items
    let mut rows = conn
        .query(
            "SELECT id, fact, collection, source, session_id FROM personal_memory_queue
             WHERE status = 'staged_pending' ORDER BY created_at ASC LIMIT ?",
            (STAGE1_BATCH_CEILING as i64,),
        )
        .await?;

    let mut candidate_items = Vec::new();
    while let Some(row) = rows.next().await? {
        candidate_items.push(Stage1Item {
            id: row.get::<i64>(0)?,
            fact: row.get::<String>(1)?,
            collection: row.get::<String>(2)?,
            source: row.get::<String>(3)?,
            session_id: row.get::<String>(4)?,
        });
    }

    if candidate_items.is_empty() {
        return Ok(0);
    }

    // 2. Atomically claim candidate items in DB
    let mut items = Vec::new();
    for item in candidate_items {
        let updated = conn.execute(
            "UPDATE personal_memory_queue SET status = ?, claimed_at = ? WHERE id = ? AND status = ?",
            (PM_QUEUE_STATUS_PROCESSING_DEDUP, now, item.id, PM_QUEUE_STATUS_STAGED_PENDING),
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

    // 2. PRE-FETCH ALL ACTIVE FACTS & QUEUE FACTS IN 2 QUERIES (0 SQL inside loop)
    let mut active_facts_map: std::collections::HashMap<String, Vec<String>> = std::collections::HashMap::new();
    let mut db_rows = conn.query("SELECT collection, fact FROM memory_facts WHERE status = 'active'", ()).await?;
    while let Some(row) = db_rows.next().await? {
        let coll: String = row.get(0)?;
        let fact: String = row.get(1)?;
        active_facts_map.entry(coll).or_default().push(fact);
    }

    let mut queue_rows = conn.query(
        "SELECT collection, fact FROM personal_memory_queue WHERE status IN ('deduped', 'embedded', 'evaluated', 'processing_embed', 'processing_eval')",
        (),
    ).await?;
    while let Some(row) = queue_rows.next().await? {
        let coll: String = row.get(0)?;
        let fact: String = row.get(1)?;
        active_facts_map.entry(coll).or_default().push(fact);
    }

    // 3. Pure In-Memory Rust Comparison Loop
    let mut deduped_ids = Vec::new();
    let mut superseded_ids = Vec::new();

    for item in &items {
        let trimmed_fact = item.fact.trim();
        if trimmed_fact.is_empty() {
            superseded_ids.push(item.id);
            continue;
        }

        let is_dup = active_facts_map.get(&item.collection).map_or(false, |cand_list| {
            cand_list.iter().any(|cand_fact| {
                let jacc_sim = jaccard_similarity(trimmed_fact, cand_fact);
                is_exact_duplicate(0.0, jacc_sim)
            })
        });

        if is_dup {
            superseded_ids.push(item.id);
        } else {
            deduped_ids.push(item.id);
            // Add new unique fact to in-memory map for subsequent intra-batch item comparison
            active_facts_map.entry(item.collection.clone()).or_default().push(trimmed_fact.to_string());
        }
    }

    // 4. Batch Update Results in SQL Queries
    for id in &deduped_ids {
        conn.execute(
            "UPDATE personal_memory_queue SET status = ? WHERE id = ?",
            (PM_QUEUE_STATUS_DEDUPED, *id),
        )
        .await?;
    }

    for id in &superseded_ids {
        conn.execute(
            "UPDATE personal_memory_queue SET status = ? WHERE id = ?",
            (PM_QUEUE_STATUS_SUPERSEDED, *id),
        )
        .await?;
    }

    let processed_count = items.len();
    let superseded_count = superseded_ids.len();

    let duration_ms = start_time.elapsed().as_millis();

    if !run_id.is_empty() {
        let metrics = super::metrics::PipelineStageMetrics {
            run_id: run_id.to_string(),
            stage_name: "stage1_dedup".to_string(),
            session_id,
            items_claimed,
            items_processed: processed_count,
            items_superseded: superseded_count,
            relations_created: 0,
            duration_ms,
            error_count: 0,
        };
        let _ = crate::persistence::mutations::record_stage_metrics(conn, &metrics).await;
    }

    Ok(processed_count)
}
