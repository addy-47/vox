use super::DedupAuditLog;
use crate::core::constants::{
    PM_QUEUE_STATUS_DEDUPED, PM_QUEUE_STATUS_PROCESSING_DEDUP, PM_QUEUE_STATUS_STAGED_PENDING,
    PM_QUEUE_STATUS_SUPERSEDED,
};
use crate::services::memory::STAGE1_BATCH_CEILING;
use anyhow::Result;
use std::collections::{HashMap, HashSet};
use turso::Connection;

/// Calculates Jaccard Word-Set Overlap Similarity between two strings.
pub fn jaccard_similarity(s1: &str, s2: &str) -> f32 {
    let w1: HashSet<String> = s1
        .to_lowercase()
        .split_whitespace()
        .map(|s| s.replace(|c: char| c.is_ascii_punctuation() || c == '।', ""))
        .filter(|s| !s.is_empty())
        .collect();
    let w2: HashSet<String> = s2
        .to_lowercase()
        .split_whitespace()
        .map(|s| s.replace(|c: char| c.is_ascii_punctuation() || c == '।', ""))
        .filter(|s| !s.is_empty())
        .collect();

    if w1.is_empty() && w2.is_empty() {
        return 1.0;
    }
    if w1.is_empty() || w2.is_empty() {
        return 0.0;
    }

    let intersection = w1.intersection(&w2).count() as f32;
    let union = w1.union(&w2).count() as f32;
    intersection / union
}

const FACTUAL_DEDUP_COLLECTIONS: &[&str] = &[
    "Identity",
    "Constraints",
    "Directives",
    "Profile",
    "Entities",
];

/// A claimed personal memory queue item pending stage 1 deduplication.
#[derive(Debug, Clone, PartialEq)]
pub struct Stage1Item {
    pub id: i64,
    pub fact: String,
    pub collection: String,
    pub source: String,
    pub session_id: String,
}

/// Atomically selects and claims candidate staged_pending items from the database.
async fn claim_staged_items(conn: &Connection, now: i64) -> Result<Vec<Stage1Item>> {
    let mut rows = conn
        .query(
            "SELECT id, fact, collection, source, session_id FROM personal_memory_queue
             WHERE status = 'staged_pending' ORDER BY created_at ASC LIMIT ?",
            (STAGE1_BATCH_CEILING as i64,),
        )
        .await?;

    let mut candidate_items = Vec::new();
    while let Some(row) = rows.next().await? {
        candidate_items.push(Stage1Item {
            id: row.get::<i64>(0)?,
            fact: row.get::<String>(1)?,
            collection: row.get::<String>(2)?,
            source: row.get::<String>(3)?,
            session_id: row.get::<String>(4)?,
        });
    }

    let mut items = Vec::new();
    for item in candidate_items {
        let updated = conn.execute(
            "UPDATE personal_memory_queue SET status = ?, claimed_at = ? WHERE id = ? AND status = ?",
            (PM_QUEUE_STATUS_PROCESSING_DEDUP, now, item.id, PM_QUEUE_STATUS_STAGED_PENDING),
        )
        .await?;

        if updated > 0 {
            items.push(item);
        }
    }

    Ok(items)
}

/// Loads active DB facts and in-flight queue items into an in-memory collection map.
async fn load_active_and_queue_facts(
    conn: &Connection,
) -> Result<HashMap<String, Vec<(String, String, String)>>> {
    let mut active_facts_map: HashMap<String, Vec<(String, String, String)>> = HashMap::new();

    let mut db_rows = conn
        .query(
            "SELECT id, collection, fact FROM memory_facts WHERE status = 'active'",
            (),
        )
        .await?;
    while let Some(row) = db_rows.next().await? {
        let id: String = row.get(0)?;
        let coll: String = row.get(1)?;
        let fact: String = row.get(2)?;
        active_facts_map
            .entry(coll.clone())
            .or_default()
            .push((id, coll, fact));
    }

    let mut queue_rows = conn.query(
        "SELECT printf('item_%d', id), collection, fact FROM personal_memory_queue WHERE status IN ('deduped', 'embedded', 'evaluated', 'processing_embed', 'processing_eval')",
        (),
    ).await?;
    while let Some(row) = queue_rows.next().await? {
        let id: String = row.get(0)?;
        let coll: String = row.get(1)?;
        let fact: String = row.get(2)?;
        active_facts_map
            .entry(coll.clone())
            .or_default()
            .push((id, coll, fact));
    }

    Ok(active_facts_map)
}

/// Evaluates a single candidate item against the in-memory active facts map.
async fn dedup_item_against_active(
    conn: &Connection,
    item: &Stage1Item,
    active_facts_map: &mut HashMap<String, Vec<(String, String, String)>>,
    deduped_ids: &mut Vec<i64>,
    superseded_ids: &mut Vec<i64>,
) -> Result<()> {
    let trimmed_fact = item.fact.trim();
    if trimmed_fact.is_empty() {
        if let Err(e) = conn
            .execute("DELETE FROM personal_memory_queue WHERE id = ?", (item.id,))
            .await
        {
            log::warn!("[MemoryPipeline::Stage1] Failed to delete empty queue item: {}", e);
        }
        let log = DedupAuditLog {
            queue_item_id: item.id,
            item_fact: item.fact.clone(),
            item_collection: item.collection.clone(),
            stage: "stage1_jaccard".to_string(),
            action: "empty_fact_deleted".to_string(),
            matched_fact_id: String::new(),
            matched_fact_coll: String::new(),
            matched_fact: String::new(),
            score: 0.0,
        };
        if let Err(e) = crate::persistence::mutations::write_dedup_audit(conn, item.id, &log).await {
            log::warn!("[MemoryPipeline::Stage1] Failed to write empty fact dedup audit: {}", e);
        }
        return Ok(());
    }

    let incoming_priority = crate::core::constants::MemoryCollection::parse(&item.collection)
        .map(|c| c.priority())
        .unwrap_or(0);

    let matched = FACTUAL_DEDUP_COLLECTIONS.iter().find_map(|&coll| {
        active_facts_map.get(coll).and_then(|cand_list| {
            cand_list
                .iter()
                .find_map(|(cand_id, cand_coll, cand_fact)| {
                    let jacc_sim = jaccard_similarity(trimmed_fact, cand_fact);
                    if jacc_sim >= crate::services::memory::JACCARD_EXACT_MATCH_THRESHOLD {
                        Some((
                            cand_id.clone(),
                            cand_coll.clone(),
                            cand_fact.clone(),
                            jacc_sim,
                        ))
                    } else {
                        None
                    }
                })
        })
    });

    if let Some((matched_id, matched_coll, matched_fact, jacc_sim)) = matched {
        let existing_priority = crate::core::constants::MemoryCollection::parse(&matched_coll)
            .map(|c| c.priority())
            .unwrap_or(0);

        if incoming_priority <= existing_priority {
            superseded_ids.push(item.id);
            let log = DedupAuditLog {
                queue_item_id: item.id,
                item_fact: item.fact.clone(),
                item_collection: item.collection.clone(),
                stage: "stage1_jaccard".to_string(),
                action: "duplicate_dropped".to_string(),
                matched_fact_id: matched_id.clone(),
                matched_fact_coll: matched_coll.clone(),
                matched_fact: matched_fact.clone(),
                score: jacc_sim,
            };
            if let Err(e) = crate::persistence::mutations::write_dedup_audit(conn, item.id, &log).await {
                log::warn!("[MemoryPipeline::Stage1] Failed to write dedup audit: {}", e);
            }

            let cand_source = if matched_id.starts_with("item_") {
                "queue_in_flight".to_string()
            } else {
                "memory_facts".to_string()
            };
            let cand_log = super::CandidateAuditLog {
                item_id: item.id,
                item_fact: item.fact.clone(),
                item_collection: item.collection.clone(),
                cand_id: matched_id,
                cand_fact: matched_fact,
                cand_collection: matched_coll,
                candidate_source: cand_source,
                cosine_sim: jacc_sim,
                engine: "jaccard_exact".to_string(),
                nli_scores: None,
                edge_score: None,
                decision: "duplicate_dropped".to_string(),
                rejection_reason: Some("exact_jaccard_match".to_string()),
            };
            if let Err(e) = crate::persistence::mutations::write_candidate_audit(conn, item.id, &[cand_log]).await {
                log::warn!("[MemoryPipeline::Stage1] Failed to write candidate audit: {}", e);
            }
        } else {
            if !matched_id.starts_with("item_") {
                if let Err(e) = conn
                    .execute(
                        "UPDATE memory_facts SET status = 'superseded' WHERE id = ?",
                        (matched_id.as_str(),),
                    )
                    .await
                {
                    log::warn!("[MemoryPipeline::Stage1] Failed to supersede existing memory fact: {}", e);
                }
            }
            deduped_ids.push(item.id);
            let log = DedupAuditLog {
                queue_item_id: item.id,
                item_fact: item.fact.clone(),
                item_collection: item.collection.clone(),
                stage: "stage1_jaccard".to_string(),
                action: "superseded_existing".to_string(),
                matched_fact_id: matched_id.clone(),
                matched_fact_coll: matched_coll.clone(),
                matched_fact: matched_fact.clone(),
                score: jacc_sim,
            };
            if let Err(e) = crate::persistence::mutations::write_dedup_audit(conn, item.id, &log).await {
                log::warn!("[MemoryPipeline::Stage1] Failed to write dedup audit: {}", e);
            }

            let cand_source = if matched_id.starts_with("item_") {
                "queue_in_flight".to_string()
            } else {
                "memory_facts".to_string()
            };
            let cand_log = super::CandidateAuditLog {
                item_id: item.id,
                item_fact: item.fact.clone(),
                item_collection: item.collection.clone(),
                cand_id: matched_id,
                cand_fact: matched_fact,
                cand_collection: matched_coll,
                candidate_source: cand_source,
                cosine_sim: jacc_sim,
                engine: "jaccard_exact".to_string(),
                nli_scores: None,
                edge_score: None,
                decision: "superseded_existing".to_string(),
                rejection_reason: None,
            };
            if let Err(e) = crate::persistence::mutations::write_candidate_audit(conn, item.id, &[cand_log]).await {
                log::warn!("[MemoryPipeline::Stage1] Failed to write candidate audit: {}", e);
            }

            active_facts_map
                .entry(item.collection.clone())
                .or_default()
                .push((
                    format!("item_{}", item.id),
                    item.collection.clone(),
                    trimmed_fact.to_string(),
                ));
        }
    } else {
        deduped_ids.push(item.id);
        active_facts_map
            .entry(item.collection.clone())
            .or_default()
            .push((
                format!("item_{}", item.id),
                item.collection.clone(),
                trimmed_fact.to_string(),
            ));
    }

    Ok(())
}

/// Updates DB statuses for deduped and superseded item batches.
async fn commit_dedup_statuses(
    conn: &Connection,
    deduped_ids: &[i64],
    superseded_ids: &[i64],
) -> Result<()> {
    for id in deduped_ids {
        conn.execute(
            "UPDATE personal_memory_queue SET status = ? WHERE id = ?",
            (PM_QUEUE_STATUS_DEDUPED, *id),
        )
        .await?;
    }

    for id in superseded_ids {
        conn.execute(
            "UPDATE personal_memory_queue SET status = ? WHERE id = ?",
            (PM_QUEUE_STATUS_SUPERSEDED, *id),
        )
        .await?;
    }

    Ok(())
}

/// Stage 1: Dedup Worker (Batch Ceiling 128)
pub async fn run_stage1_dedup(conn: &Connection) -> Result<usize> {
    run_stage1_dedup_with_metrics(conn, "").await
}

/// Executes Stage 1 deduplication with structured metrics emission.
pub async fn run_stage1_dedup_with_metrics(conn: &Connection, run_id: &str) -> Result<usize> {
    let start_time = std::time::Instant::now();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;

    let items = claim_staged_items(conn, now).await?;
    if items.is_empty() {
        return Ok(0);
    }

    let items_claimed = items.len();
    log::info!(
        "[MemoryPipeline::Stage1] Claimed {} staged_pending items for deduplication",
        items_claimed
    );
    let session_id = items
        .first()
        .map(|i| i.session_id.clone())
        .unwrap_or_default();

    let mut active_facts_map = load_active_and_queue_facts(conn).await?;

    let mut deduped_ids = Vec::new();
    let mut superseded_ids = Vec::new();

    for item in &items {
        dedup_item_against_active(
            conn,
            item,
            &mut active_facts_map,
            &mut deduped_ids,
            &mut superseded_ids,
        )
        .await?;
    }

    commit_dedup_statuses(conn, &deduped_ids, &superseded_ids).await?;

    let processed_count = items.len();
    let error_count = items_claimed.saturating_sub(processed_count);
    let duration_ms = start_time.elapsed().as_millis();

    if !run_id.is_empty() {
        let metrics = super::PipelineStageMetrics {
            run_id: run_id.to_string(),
            stage_name: "stage1_dedup".to_string(),
            session_id,
            batch_seq: 0,
            items_claimed,
            error_count,
            duration_ms,
        };
        if let Err(e) = crate::persistence::mutations::record_stage_metrics(conn, &metrics).await {
            log::warn!("[MemoryPipeline::Stage1] Failed to record stage metrics: {}", e);
        }
    }

    Ok(processed_count)
}
