use std::time::{Duration, Instant};

use anyhow::Result;
use serde::{Deserialize, Serialize};
use turso::Connection;

pub mod stage1_dedup;
pub mod stage2_embed;
pub mod sweep;

pub use stage1_dedup::{jaccard_similarity, run_stage1_exact_dedup, Stage1Summary};
pub use stage2_embed::{
    run_stage2_cosine_dedup, run_stage2_cosine_dedup_with_embedder, Stage2Summary,
};
pub use sweep::spawn_ingestion_sweep;

use crate::persistence::queue::{
    reconcile_crashed_queue_on_boot as persistence_reconcile, reset_failed_queue_items,
};

pub const INGESTION_THROTTLE_DURATION: Duration = Duration::from_millis(1000);
pub const JACCARD_EXACT_MATCH_THRESHOLD: f32 = 1.0;
pub const SOFT_VECTOR_DEDUP_THRESHOLD: f32 = 0.95;

/// Lower bound of the band reported as a near miss: a pair that scored close to
/// the merge threshold but was left unmerged.
///
/// Telemetry only. Nothing branches on this value, so reporting a near miss
/// cannot change what ingestion decides. It exists so the evaluation harness can
/// show the borderline pairs to a judge instead of asking the judge to guess
/// which ones mattered.
pub const NEAR_MISS_REPORT_FLOOR: f32 = 0.70;

pub const STAGE1_BATCH_CEILING: usize = 128;
pub const STAGE2_BATCH_SIZE: usize = 16;

/// Which deduplication stage produced a decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DedupStage {
    Stage1Exact,
    Stage2Cosine,
}

/// One deduplication that actually happened: the incoming fact won and the older
/// stored fact was deactivated. `similarity` is Jaccard for Stage 1 and cosine
/// for Stage 2.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DedupDecision {
    pub stage: DedupStage,
    pub incoming_queue_id: i64,
    pub incoming_text: String,
    pub incoming_type: String,
    pub deactivated_obs_id: String,
    pub deactivated_text: String,
    pub similarity: f32,
}

/// One comparison that scored at or above [`NEAR_MISS_REPORT_FLOOR`] but below
/// the stage's merge threshold, so nothing happened.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DedupNearMiss {
    pub stage: DedupStage,
    pub incoming_queue_id: i64,
    pub incoming_text: String,
    pub active_obs_id: String,
    pub active_text: String,
    pub similarity: f32,
}

/// True when a similarity falls in the reported near-miss band.
pub fn is_near_miss(similarity: f32, threshold: f32) -> bool {
    similarity >= NEAR_MISS_REPORT_FLOOR && similarity < threshold
}

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
#[derive(Debug, Clone, Default)]
pub struct IngestionCycleSummary {
    pub stage1: Stage1Summary,
    pub stage2: Stage2Summary,
}

/// Drains all items from the ingestion queue, looping Stage 1 and Stage 2 batches until empty or cancelled.
pub async fn drain_ingestion_queue(
    conn: &Connection,
    cancel: Option<&tokio_util::sync::CancellationToken>,
) -> Result<IngestionCycleSummary> {
    drain_ingestion_queue_with_progress(conn, cancel, None::<fn()>).await
}

/// Drains all items from the ingestion queue with an optional progress callback invoked after each batch.
pub async fn drain_ingestion_queue_with_progress<F>(
    conn: &Connection,
    cancel: Option<&tokio_util::sync::CancellationToken>,
    mut on_progress: Option<F>,
) -> Result<IngestionCycleSummary>
where
    F: FnMut() + Send,
{
    let started = Instant::now();
    let mut total_summary = IngestionCycleSummary::default();

    // Automatically reset failed queue items to pending so they are retried on this sweep
    if let Err(e) = reset_failed_queue_items(conn).await {
        log::warn!(
            "[Memory::Ingestion] Failed to reset failed items for retry: {}",
            e
        );
    }

    loop {
        if let Some(c) = cancel {
            if c.is_cancelled() {
                log::info!("[Memory::Ingestion] Ingestion drain cancelled.");
                break;
            }
        }

        let stage1 = run_stage1_exact_dedup(conn).await?;
        if stage1.processed > 0 {
            if let Some(ref mut cb) = on_progress {
                cb();
            }
        }
        total_summary.stage1.processed += stage1.processed;
        total_summary.stage1.errors += stage1.errors;
        total_summary.stage1.decisions.extend(stage1.decisions);
        total_summary.stage1.near_misses.extend(stage1.near_misses);

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
            if let Some(ref mut cb) = on_progress {
                cb();
            }
            stage2_drained += s2.processed;
            total_summary.stage2.processed += s2.processed;
            total_summary.stage2.inserted += s2.inserted;
            total_summary.stage2.duplicates_deactivated += s2.duplicates_deactivated;
            total_summary.stage2.errors += s2.errors;
            total_summary.stage2.decisions.extend(s2.decisions);
            total_summary.stage2.near_misses.extend(s2.near_misses);
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
