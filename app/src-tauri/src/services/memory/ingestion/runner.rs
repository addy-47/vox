use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

use anyhow::Result;
use turso::Connection;

use super::{
    stage1_dedup::run_stage1_dedup_with_metrics, stage2_embed::run_stage2_embed_with_metrics,
    stage3_eval::run_stage3_eval_with_metrics_seq, stage4_commit::run_stage4_commit_with_metrics,
};

/// Drives the 4-Stage Ingestion Pipeline sequentially:
/// Stage 1 (Dedup) -> Stage 2 (Embed) -> Stage 3 (Eval) -> Stage 4 (Commit & Prune)
pub async fn run_pipeline_cycle(conn: &Connection, cancel_flag: &Arc<AtomicBool>) -> Result<usize> {
    run_pipeline_cycle_with_id_seq(conn, cancel_flag, &uuid::Uuid::new_v4().to_string(), 0).await
}

/// Recovers orphaned items stuck in transient `processing_%` states from prior app crashes or abnormal restarts.
pub async fn recover_stuck_pipeline_jobs(conn: &Connection) -> Result<usize> {
    let res = conn
        .execute(
            "UPDATE personal_memory_queue SET status = 'staged_pending' WHERE status LIKE 'processing_%'",
            (),
        )
        .await?;
    if res > 0 {
        log::warn!(
            "[MemoryPipeline] Recovered {} orphaned personal_memory_queue items from transient processing state to 'staged_pending'.",
            res
        );
    }
    Ok(res as usize)
}

/// Executes a single consolidation pipeline cycle with a specific run ID and batch sequence counter.
pub async fn run_pipeline_cycle_with_id_seq(
    conn: &Connection,
    cancel_flag: &Arc<AtomicBool>,
    run_id: &str,
    stage3_batch_seq: usize,
) -> Result<usize> {
    if cancel_flag.load(Ordering::Relaxed) {
        return Ok(0);
    }

    if let Err(e) = recover_stuck_pipeline_jobs(conn).await {
        log::warn!(
            "[MemoryPipeline] Failed to recover stuck pipeline jobs: {}",
            e
        );
    }

    let count = match conn
        .query("SELECT COUNT(*) FROM personal_memory_queue", ())
        .await
    {
        Ok(mut rows) => match rows.next().await {
            Ok(Some(row)) => row.get::<i64>(0).unwrap_or(0),
            Ok(None) => 0,
            Err(e) => {
                log::warn!(
                    "[MemoryPipeline] Failed to read row from queue count query: {}",
                    e
                );
                return Err(e.into());
            }
        },
        Err(e) => {
            log::warn!(
                "[MemoryPipeline] Failed to query personal_memory_queue count: {}",
                e
            );
            return Err(e.into());
        }
    };
    if count == 0 {
        return Ok(0);
    }

    let mut total_processed = 0;

    log::info!(
        "[MemoryPipeline] Starting consolidation cycle run_id={}",
        run_id
    );

    let n1 = run_stage1_dedup_with_metrics(conn, run_id).await?;
    total_processed += n1;
    if n1 > 0 {
        log::info!("[MemoryPipeline] Stage 1 (Dedup) processed_items={}", n1);
    }
    if cancel_flag.load(Ordering::Relaxed) {
        log::info!("[MemoryPipeline] Consolidation canceled after Stage 1.");
        return Ok(total_processed);
    }

    let n2 = run_stage2_embed_with_metrics(conn, run_id).await?;
    total_processed += n2;
    if n2 > 0 {
        log::info!("[MemoryPipeline] Stage 2 (Embed) processed_items={}", n2);
    }
    if cancel_flag.load(Ordering::Relaxed) {
        log::info!("[MemoryPipeline] Consolidation canceled after Stage 2.");
        return Ok(total_processed);
    }

    let n3 = run_stage3_eval_with_metrics_seq(conn, run_id, stage3_batch_seq).await?;
    total_processed += n3;
    if n3 > 0 {
        log::info!("[MemoryPipeline] Stage 3 (NLI Eval) processed_items={}", n3);
    }
    if cancel_flag.load(Ordering::Relaxed) {
        log::info!("[MemoryPipeline] Consolidation canceled after Stage 3.");
        return Ok(total_processed);
    }

    let n4 = run_stage4_commit_with_metrics(conn, run_id).await?;
    total_processed += n4;
    if n4 > 0 {
        log::info!(
            "[MemoryPipeline] Stage 4 (Commit & Prune) committed_facts={}",
            n4
        );
    }

    log::info!(
        "[MemoryPipeline] Completed consolidation cycle total_processed={}",
        total_processed
    );

    Ok(total_processed)
}

/// Continuously executes pipeline cycles until the personal_memory_queue is completely drained.
pub async fn drain_pipeline_queue(
    conn: &Connection,
    cancel_flag: &Arc<AtomicBool>,
) -> Result<usize> {
    drain_pipeline_queue_with_run_id(conn, cancel_flag, &uuid::Uuid::new_v4().to_string()).await
}

/// Drains the memory queue in a loop with a fixed run identifier.
pub async fn drain_pipeline_queue_with_run_id(
    conn: &Connection,
    cancel_flag: &Arc<AtomicBool>,
    run_id: &str,
) -> Result<usize> {
    let mut total_drained = 0;
    let mut stage3_seq = 0;
    loop {
        if cancel_flag.load(Ordering::Relaxed) {
            break;
        }
        let cycle_processed =
            run_pipeline_cycle_with_id_seq(conn, cancel_flag, run_id, stage3_seq).await?;
        if cycle_processed == 0 {
            break;
        }
        total_drained += cycle_processed;
        stage3_seq += 1;
    }
    Ok(total_drained)
}
