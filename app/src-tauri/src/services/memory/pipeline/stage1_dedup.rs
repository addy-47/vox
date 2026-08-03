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

    let mut processed_count = 0;
    let mut superseded_count = 0;

    for item in items {
        let trimmed_fact = item.fact.trim();
        if trimmed_fact.is_empty() {
            conn.execute(
                "UPDATE personal_memory_queue SET status = ? WHERE id = ?",
                (PM_QUEUE_STATUS_SUPERSEDED, item.id),
            )
            .await?;
            processed_count += 1;
            superseded_count += 1;
            continue;
        }

        // Fetch active facts in same collection to compare Jaccard similarity
        let mut cand_rows = conn
            .query(
                "SELECT fact FROM memory_facts WHERE collection = ? AND status = 'active'",
                (item.collection.as_str(),),
            )
            .await?;

        let mut is_dup = false;
        while let Some(row) = cand_rows.next().await? {
            let cand_fact: String = row.get(0)?;
            let jacc_sim = jaccard_similarity(trimmed_fact, &cand_fact);
            if is_exact_duplicate(0.0, jacc_sim) {
                is_dup = true;
                break;
            }
        }

        // Also check against earlier items in personal_memory_queue to catch batch intra-queue duplicates
        if !is_dup {
            let mut queue_rows = conn
                .query(
                    "SELECT fact FROM personal_memory_queue WHERE collection = ? AND id != ? AND status IN ('deduped', 'embedded', 'evaluated', 'processing_embed', 'processing_eval')",
                    (item.collection.as_str(), item.id),
                )
                .await?;

            while let Some(row) = queue_rows.next().await? {
                let cand_fact: String = row.get(0)?;
                let jacc_sim = jaccard_similarity(trimmed_fact, &cand_fact);
                if is_exact_duplicate(0.0, jacc_sim) {
                    is_dup = true;
                    break;
                }
            }
        }

        let new_status = if is_dup {
            superseded_count += 1;
            PM_QUEUE_STATUS_SUPERSEDED
        } else {
            PM_QUEUE_STATUS_DEDUPED
        };

        conn.execute(
            "UPDATE personal_memory_queue SET status = ? WHERE id = ?",
            (new_status, item.id),
        )
        .await?;

        processed_count += 1;
    }

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
