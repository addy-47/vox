use anyhow::Result;
use turso::Connection;

use super::{
    stage1_dedup::{run_stage1_exact_dedup, Stage1Summary},
    stage2_embed::{
        run_stage2_cosine_dedup, run_stage2_cosine_dedup_with_embedder, Stage2Summary,
    },
};
use crate::persistence::queue::reconcile_crashed_queue_on_boot as persistence_reconcile;

/// Composite metrics summarizing an entire 2-stage ingestion deduplication cycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct IngestionCycleSummary {
    pub stage1: Stage1Summary,
    pub stage2: Stage2Summary,
}

/// Executes a complete deduplication cycle: Stage 1 exact Jaccard dedup followed by Stage 2 semantic cosine dedup.
pub async fn run_ingestion_cycle(conn: &Connection) -> Result<IngestionCycleSummary> {
    let stage1 = run_stage1_exact_dedup(conn).await?;
    let stage2 = run_stage2_cosine_dedup(conn).await?;

    Ok(IngestionCycleSummary { stage1, stage2 })
}

/// Executes an ingestion cycle using an injected embedding function for deterministic testing.
pub async fn run_ingestion_cycle_with_embedder<F>(
    conn: &Connection,
    embed_fn: F,
) -> Result<IngestionCycleSummary>
where
    F: Fn(&str) -> Result<Option<Vec<f32>>> + Send + Sync + 'static,
{
    let stage1 = run_stage1_exact_dedup(conn).await?;
    let stage2 = run_stage2_cosine_dedup_with_embedder(conn, embed_fn).await?;

    Ok(IngestionCycleSummary { stage1, stage2 })
}

/// Reconciles crashed queue items on application boot.
pub async fn reconcile_crashed_queue_on_boot(conn: &Connection) -> Result<usize> {
    persistence_reconcile(conn).await
}
