use std::collections::HashSet;

use anyhow::Result;
use turso::Connection;

use super::{
    is_near_miss, DedupDecision, DedupNearMiss, DedupStage, JACCARD_EXACT_MATCH_THRESHOLD,
    STAGE1_BATCH_CEILING,
};
use crate::persistence::{
    deactivate_observations_batch, fetch_active_observations_by_type,
    queue::claim_pending_queue_batch, record_queue_item_failure, update_queue_item_status,
    QueueItem,
};

/// Strongly-typed lifecycle state for a single Stage 1 item's telemetry.
#[derive(Debug, Clone, Default)]
pub struct Stage1ItemTelemetry {
    pub decisions: Vec<DedupDecision>,
    pub near_misses: Vec<DedupNearMiss>,
}

/// Summary metrics returned after running a Stage 1 exact Jaccard deduplication pass.
#[derive(Debug, Clone, Default)]
pub struct Stage1Summary {
    pub processed: usize,
    pub duplicates_deactivated: usize,
    pub errors: usize,
    /// Telemetry only. Records what each merge matched and at what Jaccard, so an
    /// evaluator does not have to infer the decision from a before/after diff.
    pub decisions: Vec<DedupDecision>,
    /// Telemetry only. Records borderline pairs that were deliberately left alone.
    pub near_misses: Vec<DedupNearMiss>,
}

/// Computes the word-level Jaccard similarity between two text strings.
pub fn jaccard_similarity(s1: &str, s2: &str) -> f32 {
    let set1: HashSet<String> = s1
        .split_whitespace()
        .map(|w| {
            w.chars()
                .filter(|c| c.is_alphanumeric())
                .collect::<String>()
                .to_lowercase()
        })
        .filter(|w| !w.is_empty())
        .collect();
    let set2: HashSet<String> = s2
        .split_whitespace()
        .map(|w| {
            w.chars()
                .filter(|c| c.is_alphanumeric())
                .collect::<String>()
                .to_lowercase()
        })
        .filter(|w| !w.is_empty())
        .collect();

    if set1.is_empty() && set2.is_empty() {
        return 1.0;
    }
    if set1.is_empty() || set2.is_empty() {
        return 0.0;
    }
    let intersection = set1.intersection(&set2).count();
    let union = set1.union(&set2).count();
    intersection as f32 / union as f32
}

/// Executes Stage 1 exact match deduplication for up to `STAGE1_BATCH_CEILING` pending items.
pub async fn run_stage1_exact_dedup(conn: &Connection) -> Result<Stage1Summary> {
    let items =
        claim_pending_queue_batch(conn, "pending", "stage1_processing", STAGE1_BATCH_CEILING)
            .await?;

    if items.is_empty() {
        return Ok(Stage1Summary::default());
    }

    let mut summary = Stage1Summary::default();

    for item in items {
        match process_stage1_item(conn, &item).await {
            Ok(telemetry) => {
                let deactivated = telemetry.decisions.len();
                if let Err(e) = update_queue_item_status(conn, item.id, "stage1_done", None).await {
                    log::warn!(
                        "[Memory::Ingestion::Stage1] Failed to update item {} to stage1_done: {}",
                        item.id,
                        e
                    );
                    summary.errors += 1;
                } else {
                    summary.processed += 1;
                    summary.duplicates_deactivated += deactivated;
                    summary.decisions.extend(telemetry.decisions);
                    summary.near_misses.extend(telemetry.near_misses);
                }
            }
            Err(e) => {
                log::warn!(
                    "[Memory::Ingestion::Stage1] Error processing item {}: {}",
                    item.id,
                    e
                );
                if let Err(rec_err) = record_queue_item_failure(
                    conn,
                    item.id,
                    item.retry_count,
                    "pending",
                    &e.to_string(),
                )
                .await
                {
                    log::warn!(
                        "[Memory::Ingestion::Stage1] Failed to record failure for item {}: {}",
                        item.id,
                        rec_err
                    );
                }
                summary.errors += 1;
            }
        }
    }

    log::info!(
        "[Memory::Ingestion::Stage1] Cycle completed: {} processed, {} deactivated, {} errors",
        summary.processed,
        summary.duplicates_deactivated,
        summary.errors
    );

    Ok(summary)
}

/// Compares a single queue item against active observations of the same type and batch-deactivates
/// exact matches. On an exact match the incoming item wins: the older stored observation is deactivated.
///
/// Returns the telemetry for this item so callers can report which pair matched
/// and at what Jaccard, instead of only a count.
async fn process_stage1_item(conn: &Connection, item: &QueueItem) -> Result<Stage1ItemTelemetry> {
    let active_observations =
        fetch_active_observations_by_type(conn, &item.observation_type).await?;
    let mut telemetry = Stage1ItemTelemetry::default();
    let mut duplicate_ids: Vec<String> = Vec::new();

    for existing in &active_observations {
        let similarity = jaccard_similarity(&item.text, &existing.text);
        if similarity >= JACCARD_EXACT_MATCH_THRESHOLD {
            log::info!(
                "[Memory::Ingestion::Stage1] Exact match found (sim={:.2}). Queuing older observation {} for batch deactivation (incoming item {})",
                similarity,
                existing.id,
                item.id
            );
            duplicate_ids.push(existing.id.clone());
            telemetry.decisions.push(DedupDecision {
                stage: DedupStage::Stage1Exact,
                incoming_queue_id: item.id,
                incoming_text: item.text.clone(),
                incoming_type: item.observation_type.clone(),
                deactivated_obs_id: existing.id.clone(),
                deactivated_text: existing.text.clone(),
                similarity,
            });
        } else if is_near_miss(similarity, JACCARD_EXACT_MATCH_THRESHOLD) {
            telemetry.near_misses.push(DedupNearMiss {
                stage: DedupStage::Stage1Exact,
                incoming_queue_id: item.id,
                incoming_text: item.text.clone(),
                active_obs_id: existing.id.clone(),
                active_text: existing.text.clone(),
                similarity,
            });
        }
    }

    deactivate_observations_batch(conn, &duplicate_ids).await?;
    Ok(telemetry)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_jaccard_similarity_calculation() {
        assert_eq!(jaccard_similarity("", ""), 1.0);
        assert_eq!(jaccard_similarity("hello world", "HELLO WORLD!"), 1.0);
        assert_eq!(
            jaccard_similarity("User likes Rust", "user likes rust."),
            1.0
        );
        assert_eq!(jaccard_similarity("apples", "oranges"), 0.0);
        let sim = jaccard_similarity("apple orange banana", "apple orange pear");
        assert!((sim - 0.5).abs() < 0.001);
    }
}
