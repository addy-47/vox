use std::time::Instant;

use anyhow::Result;
use serde::{Deserialize, Serialize};
use turso::Connection;

pub mod stage1_dedup;
pub mod stage2_embed;

pub use stage1_dedup::{jaccard_similarity, run_stage1_exact_dedup, Stage1Summary};
pub use stage2_embed::{
    run_stage2_cosine_dedup, run_stage2_cosine_dedup_with_embedder, Stage2Summary,
};

use crate::persistence::queue::reconcile_crashed_queue_on_boot as persistence_reconcile;

pub const JACCARD_EXACT_MATCH_THRESHOLD: f32 = 1.0;
pub const SOFT_VECTOR_DEDUP_THRESHOLD: f32 = 0.95;

pub const STAGE1_BATCH_CEILING: usize = 128;
pub const STAGE2_BATCH_SIZE: usize = 16;

/// Strongly-typed lifecycle state for items in `memory_ingestion_queue`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum QueueStatus {
    Pending,
    Stage1Processing,
    Stage1Done,
    Stage2Processing,
    Completed,
    Failed,
}

impl QueueStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Stage1Processing => "stage1_processing",
            Self::Stage1Done => "stage1_done",
            Self::Stage2Processing => "stage2_processing",
            Self::Completed => "completed",
            Self::Failed => "failed",
        }
    }
}

/// Composite metrics summarizing an entire 2-stage ingestion deduplication cycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct IngestionCycleSummary {
    pub stage1: Stage1Summary,
    pub stage2: Stage2Summary,
}

/// Executes a complete deduplication cycle: Stage 1 exact Jaccard dedup followed by Stage 2 semantic cosine dedup.
pub async fn run_ingestion_cycle(conn: &Connection) -> Result<IngestionCycleSummary> {
    let started = Instant::now();
    let stage1 = run_stage1_exact_dedup(conn).await?;
    let stage2 = run_stage2_cosine_dedup(conn).await?;
    let summary = IngestionCycleSummary { stage1, stage2 };
    log::info!(
        "[Memory::Ingestion] Cycle completed in {:?}; stage1_processed={} stage2_processed={} stage2_inserted={} stage1_errors={} stage2_errors={}",
        started.elapsed(),
        summary.stage1.processed,
        summary.stage2.processed,
        summary.stage2.inserted,
        summary.stage1.errors,
        summary.stage2.errors,
    );
    Ok(summary)
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
