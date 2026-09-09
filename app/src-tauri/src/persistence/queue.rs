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
    pub fact_type: String,
    pub text: String,
    pub status: String,
    pub retry_count: i64,
    pub error_msg: Option<String>,
    pub created_at: i64,
    pub processed_at: Option<i64>,
}

/// Enqueues a newly extracted fact into `memory_ingestion_queue` with status 'pending'.
pub async fn enqueue_fact(
    conn: &Connection,
    session_id: Option<i64>,
    compaction_id: i64,
    fact_type: &str,
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
            (session_id, compaction_id, fact_type.to_string(), text.to_string(), now),
        )
        .await?;

    if let Some(row) = rows.next().await? {
        Ok(row.get(0)?)
    } else {
        Err(anyhow::anyhow!("Failed to retrieve generated queue item id"))
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
                fact_type: row.get(3)?,
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
            (new_status.to_string(), error_msg.map(|s| s.to_string()), now, id),
        )
        .await?;
    } else {
        conn.execute(
            "UPDATE memory_ingestion_queue
             SET status = ?, error_msg = ?, processed_at = ?
             WHERE id = ?",
            (new_status.to_string(), error_msg.map(|s| s.to_string()), now, id),
        )
        .await?;
    }

    Ok(())
}

/// Records a failure on a queue item, incrementing its retry count and setting status to 'failed' if retries reach 3.
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
    let next_retry = retry_count + 1;
    let new_status = if next_retry >= 3 { "failed" } else { "stage1_done" };
    conn.execute(
        "UPDATE memory_ingestion_queue
         SET status = ?, retry_count = ?, error_msg = ?, processed_at = ?
         WHERE id = ?",
        (new_status.to_string(), next_retry, Some(error_msg.to_string()), now, id),
    )
    .await?;
    Ok(())
}

/// Reconciles items left in indeterminate processing states on boot.
/// Stage1Processing items are reset to 'pending', Stage2Processing items to 'stage1_done'.
/// Items exceeding 3 retries are marked 'failed'.
pub async fn reconcile_crashed_queue_on_boot(conn: &Connection) -> Result<usize> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;

    let reset_failed = conn
        .execute(
            "UPDATE memory_ingestion_queue
             SET status = 'failed', processed_at = ?, error_msg = 'Process crashed while processing batch (max retries exceeded)'
             WHERE status IN ('stage1_processing', 'stage2_processing') AND retry_count >= 3",
            (now,),
        )
        .await?;

    let reset_s1 = conn
        .execute(
            "UPDATE memory_ingestion_queue
             SET status = 'pending', retry_count = retry_count + 1
             WHERE status = 'stage1_processing' AND retry_count < 3",
            (),
        )
        .await?;

    let reset_s2 = conn
        .execute(
            "UPDATE memory_ingestion_queue
             SET status = 'stage1_done', retry_count = retry_count + 1
             WHERE status = 'stage2_processing' AND retry_count < 3",
            (),
        )
        .await?;

    let total = reset_failed + reset_s1 + reset_s2;
    if total > 0 {
        log::info!(
            "[Persistence::Queue] Reconciled {} crashed queue items ({} failed, {} reset to pending, {} reset to stage1_done)",
            total,
            reset_failed,
            reset_s1,
            reset_s2
        );
    }

    Ok(total as usize)
}
