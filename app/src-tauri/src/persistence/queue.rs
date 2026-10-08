use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::Result;
use serde::{Deserialize, Serialize};
use turso::Connection;

/// Strongly-typed row representation of a staging item in `memory_ingestion_queue`.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct QueueItem {
    pub id: i64,
    pub session_id: Option<i64>,
    pub compaction_id: i64,
    pub observation_type: String,
    pub text: String,
    pub status: String,
    pub retry_count: i64,
    pub error_msg: Option<String>,
    pub created_at: i64,
    pub processed_at: Option<i64>,
}

/// Aggregate counters for memory ingestion and active facts.
#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq, Eq)]
pub struct IngestionStatsRecord {
    pub total: i64,
    pub pending: i64,
    pub processing: i64,
    pub completed: i64,
    pub failed: i64,
}

/// Enqueues a newly extracted observation into `memory_ingestion_queue` with status 'pending'.
pub async fn enqueue_observation(
    conn: &Connection,
    session_id: Option<i64>,
    compaction_id: i64,
    observation_type: &str,
    text: &str,
) -> Result<i64> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;

    let mut rows = conn
        .query(
            "INSERT INTO memory_ingestion_queue (session_id, compaction_id, type, text, status, retry_count, created_at)
             VALUES (?, ?, ?, ?, 'pending', 0, ?) RETURNING id;",
            (session_id, compaction_id, observation_type.to_string(), text.to_string(), now),
        )
        .await?;

    if let Some(row) = rows.next().await? {
        Ok(row.get(0)?)
    } else {
        Err(anyhow::anyhow!(
            "Failed to retrieve generated queue item id"
        ))
    }
}

/// Atomically claims a batch of queue items matching `target_status` by updating them to `next_status`.
pub async fn claim_pending_queue_batch(
    conn: &Connection,
    target_status: &str,
    next_status: &str,
    limit: usize,
) -> Result<Vec<QueueItem>> {
    conn.execute("BEGIN IMMEDIATE;", ()).await?;

    let tx_res: Result<Vec<QueueItem>> = async {
        let mut rows = conn
            .query(
                "SELECT id, session_id, compaction_id, type, text, status, retry_count, error_msg, created_at, processed_at
                 FROM memory_ingestion_queue
                 WHERE status = ?
                 ORDER BY id ASC
                 LIMIT ?",
                (target_status.to_string(), limit as i64),
            )
            .await?;

        let mut items = Vec::new();
        while let Some(row) = rows.next().await? {
            items.push(QueueItem {
                id: row.get(0)?,
                session_id: row.get(1).ok(),
                compaction_id: row.get(2)?,
                observation_type: row.get(3)?,
                text: row.get(4)?,
                status: row.get(5)?,
                retry_count: row.get(6)?,
                error_msg: row.get(7).ok(),
                created_at: row.get(8)?,
                processed_at: row.get(9).ok(),
            });
        }

        if !items.is_empty() {
            let ids: Vec<String> = items.iter().map(|it| it.id.to_string()).collect();
            let sql = format!(
                "UPDATE memory_ingestion_queue SET status = ? WHERE id IN ({})",
                ids.join(",")
            );
            conn.execute(&sql, (next_status.to_string(),)).await?;
            for it in &mut items {
                it.status = next_status.to_string();
            }
        }

        Ok(items)
    }
    .await;

    match tx_res {
        Ok(items) => {
            conn.execute("COMMIT;", ()).await?;
            Ok(items)
        }
        Err(e) => {
            if let Err(rb_err) = conn.execute("ROLLBACK;", ()).await {
                log::warn!("[Persistence::Queue] Rollback failed: {}", rb_err);
            }
            Err(e)
        }
    }
}

/// Updates the status and error message of a specific queue item.
pub async fn update_queue_item_status(
    conn: &Connection,
    id: i64,
    new_status: &str,
    error_msg: Option<&str>,
) -> Result<()> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;

    if new_status == "failed" {
        conn.execute(
            "UPDATE memory_ingestion_queue
             SET status = ?, retry_count = retry_count + 1, error_msg = ?, processed_at = ?
             WHERE id = ?",
            (
                new_status.to_string(),
                error_msg.map(|s| s.to_string()),
                now,
                id,
            ),
        )
        .await?;
    } else {
        conn.execute(
            "UPDATE memory_ingestion_queue
             SET status = ?, error_msg = ?, processed_at = ?
             WHERE id = ?",
            (
                new_status.to_string(),
                error_msg.map(|s| s.to_string()),
                now,
                id,
            ),
        )
        .await?;
    }

    Ok(())
}

/// Records a failure on a queue item, incrementing its retry count and setting status to 'failed'
/// for the current sweep with error details. The item will automatically be retried on the next sweep.
pub async fn record_queue_item_failure(
    conn: &Connection,
    id: i64,
    retry_count: i64,
    error_msg: &str,
) -> Result<()> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;
    conn.execute(
        "UPDATE memory_ingestion_queue
         SET status = 'failed', retry_count = ?, error_msg = ?, processed_at = ?
         WHERE id = ?",
        (
            retry_count + 1,
            Some(error_msg.to_string()),
            now,
            id,
        ),
    )
    .await?;
    Ok(())
}

/// Resets failed queue items back to 'pending' so that the next ingestion sweep retries them automatically.
pub async fn reset_failed_queue_items(conn: &Connection) -> Result<usize> {
    let rows_affected = conn
        .execute(
            "UPDATE memory_ingestion_queue SET status = 'pending' WHERE status = 'failed'",
            (),
        )
        .await?;
    if rows_affected > 0 {
        log::info!(
            "[Persistence::Queue] Reset {} failed ingestion queue items to pending for sweep retry",
            rows_affected
        );
    }
    Ok(rows_affected as usize)
}

/// Returns true when any ingestion queue item is not yet finished (`completed`).
pub async fn has_unfinished_items(conn: &Connection) -> Result<bool> {
    let mut rows = conn
        .query(
            "SELECT id FROM memory_ingestion_queue WHERE status != 'completed' LIMIT 1",
            (),
        )
        .await?;
    Ok(rows.next().await?.is_some())
}

/// Counts ingestion queue items that are not yet finished.
pub async fn count_unfinished_items(conn: &Connection) -> Result<i64> {
    let mut rows = conn
        .query(
            "SELECT COUNT(*) FROM memory_ingestion_queue WHERE status != 'completed'",
            (),
        )
        .await?;
    match rows.next().await? {
        Some(row) => Ok(row.get::<i64>(0)?),
        None => Ok(0),
    }
}

/// Reconciles items left in indeterminate processing states on boot.
pub async fn reconcile_crashed_queue_on_boot(conn: &Connection) -> Result<usize> {
    let reset_s1 = conn
        .execute(
            "UPDATE memory_ingestion_queue
             SET status = 'pending', retry_count = retry_count + 1
             WHERE status = 'stage1_processing'",
            (),
        )
        .await?;

    let reset_s2 = conn
        .execute(
            "UPDATE memory_ingestion_queue
             SET status = 'stage1_done', retry_count = retry_count + 1
             WHERE status = 'stage2_processing'",
            (),
        )
        .await?;

    let total = reset_s1 + reset_s2;
    if total > 0 {
        log::info!(
            "[Persistence::Queue] Reconciled {} crashed queue items ({} reset to pending, {} reset to stage1_done)",
            total,
            reset_s1,
            reset_s2
        );
    }

    Ok(total as usize)
}

/// Returns aggregate ingestion queue and active facts counts.
pub async fn fetch_ingestion_aggregate_stats(conn: &Connection) -> Result<IngestionStatsRecord> {
    let mut rows = conn
        .query(
            "SELECT 
                COUNT(*) as total,
                SUM(CASE WHEN status = 'pending' THEN 1 ELSE 0 END) as pending,
                SUM(CASE WHEN status IN ('stage1_processing', 'stage1_done', 'stage2_processing') THEN 1 ELSE 0 END) as processing,
                SUM(CASE WHEN status = 'completed' THEN 1 ELSE 0 END) as completed,
                SUM(CASE WHEN status = 'failed' THEN 1 ELSE 0 END) as failed
             FROM memory_ingestion_queue",
            (),
        )
        .await?;

    let (pending, processing, q_completed, failed) = if let Some(row) = rows.next().await? {
        (
            row.get::<Option<i64>>(1)?.unwrap_or(0),
            row.get::<Option<i64>>(2)?.unwrap_or(0),
            row.get::<Option<i64>>(3)?.unwrap_or(0),
            row.get::<Option<i64>>(4)?.unwrap_or(0),
        )
    } else {
        (0, 0, 0, 0)
    };

    let mut facts_rows = conn
        .query(
            "SELECT COUNT(*) FROM memory_facts WHERE status IN ('active', 'integrated')",
            (),
        )
        .await?;
    let facts_count = if let Some(r) = facts_rows.next().await? {
        r.get::<Option<i64>>(0)?.unwrap_or(0)
    } else {
        0
    };

    let completed = q_completed.max(facts_count);
    let total = pending + processing + completed + failed;

    Ok(IngestionStatsRecord {
        total,
        pending,
        processing,
        completed,
        failed,
    })
}
