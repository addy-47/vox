use super::batch_result::DedupAuditLog;
use crate::core::constants::{
    PM_QUEUE_STATUS_DEDUPED, PM_QUEUE_STATUS_PROCESSING_DEDUP, PM_QUEUE_STATUS_STAGED_PENDING,
    PM_QUEUE_STATUS_SUPERSEDED,
};
use crate::services::memory::deduplication::{is_exact_duplicate, jaccard_similarity};
use anyhow::Result;
use turso::Connection;

pub const STAGE1_BATCH_CEILING: usize = 128;

#[derive(Debug, Clone, PartialEq)]
pub struct Stage1Item {
    pub id: i64,
    pub fact: String,
    pub collection: String,
    pub source: String,
    pub session_id: String,
}

/// Stage 1: Dedup Worker (Batch Ceiling 128)
/// Atomically claims `staged_pending` items, executes Jaccard + soft vector deduplication,
/// and updates status to `deduped` or `superseded`.
pub async fn run_stage1_dedup(conn: &Connection) -> Result<usize> {
    run_stage1_dedup_with_metrics(conn, "").await
}

pub async fn run_stage1_dedup_with_metrics(conn: &Connection, run_id: &str) -> Result<usize> {
    let start_time = std::time::Instant::now();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;

    // 1. Select candidate staged_pending items
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

    if candidate_items.is_empty() {
        return Ok(0);
    }

    // 2. Atomically claim candidate items in DB
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

    if items.is_empty() {
        return Ok(0);
    }

    let items_claimed = items.len();
    let session_id = items
        .first()
        .map(|i| i.session_id.clone())
        .unwrap_or_default();

    // 2. PRE-FETCH ALL ACTIVE FACTS & QUEUE FACTS IN 2 QUERIES (0 SQL inside loop)
    let mut active_facts_map: std::collections::HashMap<String, Vec<(String, String, String)>> =
        std::collections::HashMap::new();
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

    // 3. Pure In-Memory Rust Comparison Loop across 5 Core Factual Collections
    const FACTUAL_DEDUP_COLLECTIONS: &[&str] = &[
        "Identity",
        "Constraints",
        "Directives",
        "Profile",
        "Entities",
    ];

    let mut deduped_ids = Vec::new();
    let mut superseded_ids = Vec::new();

    for item in &items {
        let trimmed_fact = item.fact.trim();
        if trimmed_fact.is_empty() {
            superseded_ids.push(item.id);
            let log = DedupAuditLog {
                queue_item_id: item.id,
                item_fact: item.fact.clone(),
                item_collection: item.collection.clone(),
                stage: "stage1_jaccard".to_string(),
                action: "empty_fact_dropped".to_string(),
                matched_fact_id: String::new(),
                matched_fact_coll: String::new(),
                matched_fact: String::new(),
                score: 0.0,
            };
            let _ = crate::persistence::mutations::write_dedup_audit(conn, item.id, &log).await;
            continue;
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
                        if is_exact_duplicate(0.0, jacc_sim) {
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
                let _ = crate::persistence::mutations::write_dedup_audit(conn, item.id, &log).await;

                let cand_source = if matched_id.starts_with("item_") {
                    "queue_in_flight".to_string()
                } else {
                    "memory_facts".to_string()
                };
                let cand_log = super::batch_result::CandidateAuditLog {
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
                let _ = crate::persistence::mutations::write_candidate_audit(conn, item.id, &[cand_log]).await;
            } else {
                // Higher priority incoming item supersedes existing lower-priority DB fact
                if !matched_id.starts_with("item_") {
                    let _ = conn
                        .execute(
                            "UPDATE memory_facts SET status = 'superseded' WHERE id = ?",
                            (matched_id.as_str(),),
                        )
                        .await;
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
                let _ = crate::persistence::mutations::write_dedup_audit(conn, item.id, &log).await;

                let cand_source = if matched_id.starts_with("item_") {
                    "queue_in_flight".to_string()
                } else {
                    "memory_facts".to_string()
                };
                let cand_log = super::batch_result::CandidateAuditLog {
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
                let _ = crate::persistence::mutations::write_candidate_audit(conn, item.id, &[cand_log]).await;

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
            // Add new unique fact to in-memory map for subsequent intra-batch item comparison
            active_facts_map
                .entry(item.collection.clone())
                .or_default()
                .push((
                    format!("item_{}", item.id),
                    item.collection.clone(),
                    trimmed_fact.to_string(),
                ));
        }
    }

    // 4. Batch Update Results in SQL Queries
    for id in &deduped_ids {
        conn.execute(
            "UPDATE personal_memory_queue SET status = ? WHERE id = ?",
            (PM_QUEUE_STATUS_DEDUPED, *id),
        )
        .await?;
    }

    for id in &superseded_ids {
        conn.execute(
            "UPDATE personal_memory_queue SET status = ? WHERE id = ?",
            (PM_QUEUE_STATUS_SUPERSEDED, *id),
        )
        .await?;
    }

    let processed_count = items.len();
    let duration_ms = start_time.elapsed().as_millis();

    if !run_id.is_empty() {
        let metrics = super::metrics::PipelineStageMetrics {
            run_id: run_id.to_string(),
            stage_name: "stage1_dedup".to_string(),
            session_id,
            batch_seq: 0,
            items_claimed,
            error_count: 0,
            duration_ms,
        };
        let _ = crate::persistence::mutations::record_stage_metrics(conn, &metrics).await;
    }

    Ok(processed_count)
}
