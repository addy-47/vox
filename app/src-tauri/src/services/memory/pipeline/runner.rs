use super::stage1_dedup::run_stage1_dedup_with_metrics;
use super::stage2_embed::run_stage2_embed_with_metrics;
use super::stage3_eval::run_stage3_eval_with_metrics_seq;
use super::stage4_commit::run_stage4_commit_with_metrics;
use anyhow::Result;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use turso::Connection;

/// Drives the 4-Stage Ingestion Pipeline sequentially:
/// Stage 1 (Dedup) -> Stage 2 (Embed) -> Stage 3 (Eval) -> Stage 4 (Commit & Prune)
pub async fn run_pipeline_cycle(conn: &Connection, cancel_flag: &Arc<AtomicBool>) -> Result<usize> {
    run_pipeline_cycle_with_id_seq(conn, cancel_flag, &uuid::Uuid::new_v4().to_string(), 0).await
}

pub async fn run_pipeline_cycle_with_id_seq(
    conn: &Connection,
    cancel_flag: &Arc<AtomicBool>,
    run_id: &str,
    stage3_batch_seq: usize,
) -> Result<usize> {
    if cancel_flag.load(Ordering::Relaxed) {
        return Ok(0);
    }

    // Quick exit if queue is empty to avoid logging and queries
    let count = match conn
        .query("SELECT COUNT(*) FROM personal_memory_queue", ())
        .await
    {
        Ok(mut rows) => match rows.next().await {
            Ok(Some(row)) => row.get::<i64>(0).unwrap_or(0),
            _ => 0,
        },
        Err(_) => 0,
    };
    if count == 0 {
        return Ok(0);
    }

    let mut total_processed = 0;

    tracing::info!(
        "[MemoryPipeline] === Starting consolidation cycle (Run ID: {}) ===",
        run_id
    );

    // Stage 1: Dedup
    let n1 = run_stage1_dedup_with_metrics(conn, run_id).await?;
    total_processed += n1;
    if n1 > 0 {
        tracing::info!("[MemoryPipeline] Stage 1 (Dedup): Processed {} items.", n1);
    }
    if cancel_flag.load(Ordering::Relaxed) {
        tracing::info!("[MemoryPipeline] Consolidation canceled after Stage 1.");
        return Ok(total_processed);
    }

    // Stage 2: Embed
    let n2 = run_stage2_embed_with_metrics(conn, run_id).await?;
    total_processed += n2;
    if n2 > 0 {
        tracing::info!("[MemoryPipeline] Stage 2 (Embed): Processed {} items.", n2);
    }
    if cancel_flag.load(Ordering::Relaxed) {
        tracing::info!("[MemoryPipeline] Consolidation canceled after Stage 2.");
        return Ok(total_processed);
    }

    // Stage 3: Eval
    let n3 = run_stage3_eval_with_metrics_seq(conn, run_id, stage3_batch_seq).await?;
    total_processed += n3;
    if n3 > 0 {
        tracing::info!(
            "[MemoryPipeline] Stage 3 (NLI Eval): Processed {} items.",
            n3
        );
    }
    if cancel_flag.load(Ordering::Relaxed) {
        tracing::info!("[MemoryPipeline] Consolidation canceled after Stage 3.");
        return Ok(total_processed);
    }

    // Stage 4: Commit & Prune
    let n4 = run_stage4_commit_with_metrics(conn, run_id).await?;
    total_processed += n4;
    if n4 > 0 {
        tracing::info!(
            "[MemoryPipeline] Stage 4 (Commit & Prune): Committed {} facts.",
            n4
        );
    }

    tracing::info!(
        "[MemoryPipeline] === Completed consolidation cycle: {} total items processed ===",
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
