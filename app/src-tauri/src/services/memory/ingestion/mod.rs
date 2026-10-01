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

/// Drains all items from the ingestion queue, looping Stage 1 and Stage 2 batches until empty or cancelled.
pub async fn drain_ingestion_queue(
    conn: &Connection,
    cancel: Option<&tokio_util::sync::CancellationToken>,
) -> Result<IngestionCycleSummary> {
    let started = Instant::now();
    let mut total_summary = IngestionCycleSummary::default();

    loop {
        if let Some(c) = cancel {
            if c.is_cancelled() {
                log::info!("[Memory::Ingestion] Ingestion drain cancelled.");
                break;
            }
        }

        let stage1 = run_stage1_exact_dedup(conn).await?;
        total_summary.stage1.processed += stage1.processed;
        total_summary.stage1.errors += stage1.errors;

        let mut stage2_drained = 0;
        loop {
            if let Some(c) = cancel {
                if c.is_cancelled() {
                    break;
                }
            }

            let s2 = run_stage2_cosine_dedup(conn).await?;
            if s2.processed == 0 {
                break;
            }
            stage2_drained += s2.processed;
            total_summary.stage2.processed += s2.processed;
            total_summary.stage2.inserted += s2.inserted;
            total_summary.stage2.duplicates_deactivated += s2.duplicates_deactivated;
            total_summary.stage2.errors += s2.errors;
        }

        // If neither Stage 1 nor Stage 2 made progress, the queue is drained
        if stage1.processed == 0 && stage2_drained == 0 {
            break;
        }
    }

    log::info!(
        "[Memory::Ingestion] Drain completed in {:?}; stage1_processed={} stage2_processed={} stage2_inserted={} stage2_dedup={}",
        started.elapsed(),
        total_summary.stage1.processed,
        total_summary.stage2.processed,
        total_summary.stage2.inserted,
        total_summary.stage2.duplicates_deactivated,
    );

    Ok(total_summary)
}

/// Executes a complete deduplication cycle: drains Stage 1 and Stage 2 batches to completion.
pub async fn run_ingestion_cycle(conn: &Connection) -> Result<IngestionCycleSummary> {
    drain_ingestion_queue(conn, None).await
}

/// Reconciles crashed queue items on application boot.
pub async fn reconcile_crashed_queue_on_boot(conn: &Connection) -> Result<usize> {
    persistence_reconcile(conn).await
}
