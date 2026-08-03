use anyhow::Result;
use turso::Connection;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use super::stage1_dedup::run_stage1_dedup_with_metrics;
use super::stage2_embed::run_stage2_embed_with_metrics;
use super::stage3_eval::run_stage3_eval_with_metrics;
use super::stage4_commit::run_stage4_commit_with_metrics;

/// Drives the 4-Stage Ingestion Pipeline sequentially:
/// Stage 1 (Dedup) -> Stage 2 (Embed) -> Stage 3 (Eval) -> Stage 4 (Commit & Prune)
pub async fn run_pipeline_cycle(conn: &Connection, cancel_flag: &Arc<AtomicBool>) -> Result<usize> {
    if cancel_flag.load(Ordering::Relaxed) {
        return Ok(0);
    }

    let run_id = uuid::Uuid::new_v4().to_string();
    let mut total_processed = 0;

    // Stage 1: Dedup
    let n1 = run_stage1_dedup_with_metrics(conn, &run_id).await?;
    total_processed += n1;
    if cancel_flag.load(Ordering::Relaxed) { return Ok(total_processed); }

    // Stage 2: Embed
    let n2 = run_stage2_embed_with_metrics(conn, &run_id).await?;
    total_processed += n2;
    if cancel_flag.load(Ordering::Relaxed) { return Ok(total_processed); }

    // Stage 3: Eval
    let n3 = run_stage3_eval_with_metrics(conn, &run_id).await?;
    total_processed += n3;
    if cancel_flag.load(Ordering::Relaxed) { return Ok(total_processed); }

    // Stage 4: Commit & Prune
    let n4 = run_stage4_commit_with_metrics(conn, &run_id).await?;
    total_processed += n4;

    Ok(total_processed)
}
