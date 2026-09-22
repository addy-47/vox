use std::collections::HashSet;

use anyhow::Result;
use turso::Connection;

use super::{JACCARD_EXACT_MATCH_THRESHOLD, STAGE1_BATCH_CEILING};
use crate::persistence::{
    deactivate_facts_batch, fetch_active_facts_by_type, queue::claim_pending_queue_batch,
    record_queue_item_failure, update_queue_item_status, QueueItem,
};

/// Summary metrics returned after running a Stage 1 exact Jaccard deduplication pass.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Stage1Summary {
    pub processed: usize,
    pub duplicates_deactivated: usize,
    pub errors: usize,
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
            Ok(deactivated_count) => {
                if let Err(e) = update_queue_item_status(conn, item.id, "stage1_done", None).await {
                    log::warn!(
                        "[Memory::Ingestion::Stage1] Failed to update item {} to stage1_done: {}",
                        item.id,
                        e
                    );
                    summary.errors += 1;
                } else {
                    summary.processed += 1;
                    summary.duplicates_deactivated += deactivated_count;
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

/// Compares a single queue item against active facts of the same type and batch-deactivates exact matches.
async fn process_stage1_item(conn: &Connection, item: &QueueItem) -> Result<usize> {
    let active_facts = fetch_active_facts_by_type(conn, &item.fact_type).await?;
    let mut duplicate_ids: Vec<String> = Vec::new();

    for fact in active_facts {
        let similarity = jaccard_similarity(&item.text, &fact.text);
        if similarity >= JACCARD_EXACT_MATCH_THRESHOLD {
            log::info!(
                "[Memory::Ingestion::Stage1] Exact match found (sim={:.2}). Queuing older fact {} for batch deactivation (incoming item {})",
                similarity,
                fact.id,
                item.id
            );
            duplicate_ids.push(fact.id);
        }
    }

    let deactivated = deactivate_facts_batch(conn, &duplicate_ids).await?;
    Ok(deactivated)
}

#[cfg(test)]
mod tests {
    use turso::Builder;

    use super::*;
    use crate::persistence::{
        compactions::record_compaction_start,
        facts::{fetch_active_facts_by_type, insert_fact, FactRecord},
        queue::enqueue_fact,
        schema::recreate_schema,
        sessions::create_session,
    };

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

    #[tokio::test]
    async fn test_stage1_exact_dedup_winner_takes_all() {
        let db = Builder::new_local(":memory:").build().await.unwrap();
        let conn = db.connect().unwrap();
        recreate_schema(&conn).await.unwrap();

        let session_id = create_session(&conn, Some("default")).await.unwrap();
        let compaction_id = record_compaction_start(&conn, session_id, "soft", 0, 5)
            .await
            .unwrap();

        let old_fact = FactRecord {
            id: "fact_old_1".to_string(),
            session_id: Some(session_id),
            compaction_id,
            fact_type: "objective".to_string(),
            text: "Build realtime audio transcription".to_string(),
            status: "active".to_string(),
            created_at: 1000,
            updated_at: 1000,
        };
        insert_fact(&conn, &old_fact).await.unwrap();

        let q_id = enqueue_fact(
            &conn,
            Some(session_id),
            compaction_id,
            "objective",
            "build realtime audio transcription!",
        )
        .await
        .unwrap();

        let summary = run_stage1_exact_dedup(&conn).await.unwrap();
        assert_eq!(summary.processed, 1);
        assert_eq!(summary.duplicates_deactivated, 1);
        assert_eq!(summary.errors, 0);

        let active_facts = fetch_active_facts_by_type(&conn, "objective")
            .await
            .unwrap();
        assert!(
            active_facts.is_empty(),
            "Older fact should have been deactivated"
        );

        let mut rows = conn
            .query(
                "SELECT status FROM memory_ingestion_queue WHERE id = ?",
                (q_id,),
            )
            .await
            .unwrap();
        let row = rows.next().await.unwrap().unwrap();
        let status: String = row.get(0).unwrap();
        assert_eq!(status, "stage1_done");
    }
}
