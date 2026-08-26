use super::batch_result::RelationEdge;
use crate::core::constants::{
    PM_QUEUE_STATUS_DEDUPED, PM_QUEUE_STATUS_EMBEDDED, PM_QUEUE_STATUS_PROCESSING_EMBED,
    PM_QUEUE_STATUS_SUPERSEDED, PM_RELATION_SUPERSEDES,
};
use crate::persistence::{encode_f32_blob, mutations, queries};
use crate::services::memory::embedder::{ensure_embedder_loaded, generate_embedding};
use crate::services::memory::{SOFT_VECTOR_DEDUP_THRESHOLD, STAGE2_BATCH_SIZE};
use anyhow::Result;
use turso::Connection;

/// Claimed item pending stage 2 vector embedding and soft deduplication.
#[derive(Debug, Clone)]
pub struct Stage2Item {
    pub id: i64,
    pub fact: String,
    pub collection: String,
}

/// Atomically selects and claims candidate deduped items from the queue.
async fn claim_deduped_items(conn: &Connection, now: i64) -> Result<Vec<Stage2Item>> {
    let mut rows = conn
        .query(
            "SELECT id, fact, collection FROM personal_memory_queue
             WHERE status = 'deduped' ORDER BY created_at ASC LIMIT ?",
            (STAGE2_BATCH_SIZE as i64,),
        )
        .await?;

    let mut candidate_items = Vec::new();
    while let Some(row) = rows.next().await? {
        candidate_items.push(Stage2Item {
            id: row.get::<i64>(0)?,
            fact: row.get::<String>(1)?,
            collection: row.get::<String>(2)?,
        });
    }

    let mut items = Vec::new();
    for item in candidate_items {
        let updated = conn.execute(
            "UPDATE personal_memory_queue SET status = ?, claimed_at = ? WHERE id = ? AND status = ?",
            (PM_QUEUE_STATUS_PROCESSING_EMBED, now, item.id, PM_QUEUE_STATUS_DEDUPED),
        )
        .await?;

        if updated > 0 {
            items.push(item);
        }
    }

    Ok(items)
}

/// Generates embedding vector and resolves soft vector deduplication against active facts.
async fn process_stage2_item(conn: &Connection, item: &Stage2Item) -> Result<bool> {
    if item.collection == "Narrative" {
        conn.execute(
            "UPDATE personal_memory_queue SET status = ?, vector = NULL WHERE id = ?",
            (PM_QUEUE_STATUS_EMBEDDED, item.id),
        )
        .await?;
        return Ok(true);
    }

    let embedding_res = generate_embedding(&item.fact);
    match embedding_res {
        Ok(Some(vec)) => {
            let blob_bytes = encode_f32_blob(&vec);

            let soft_dups = queries::fetch_cross_collection_candidates(
                conn,
                &vec,
                SOFT_VECTOR_DEDUP_THRESHOLD,
                None,
            )
            .await
            .unwrap_or_default();

            let best_match = soft_dups.iter().max_by(|a, b| {
                let prio_a = crate::core::constants::MemoryCollection::parse(&a.2)
                    .map(|c| c.priority())
                    .unwrap_or(0);
                let prio_b = crate::core::constants::MemoryCollection::parse(&b.2)
                    .map(|c| c.priority())
                    .unwrap_or(0);
                prio_a
                    .cmp(&prio_b)
                    .then_with(|| a.3.partial_cmp(&b.3).unwrap_or(std::cmp::Ordering::Equal))
            });

            if let Some((match_id, match_fact, match_coll, sim)) = best_match {
                let incoming_priority =
                    crate::core::constants::MemoryCollection::parse(&item.collection)
                        .map(|c| c.priority())
                        .unwrap_or(0);
                let existing_priority = crate::core::constants::MemoryCollection::parse(match_coll)
                    .map(|c| c.priority())
                    .unwrap_or(0);

                if incoming_priority <= existing_priority {
                    let rel = vec![RelationEdge {
                        from_id: match_id.clone(),
                        to_id: format!("item_{}", item.id),
                        relation: PM_RELATION_SUPERSEDES.to_string(),
                        source: "Embedding".to_string(),
                    }];
                    let rel_json = serde_json::to_string(&rel).unwrap_or_else(|_| "[]".to_string());

                    conn.execute(
                        "UPDATE personal_memory_queue SET status = ?, vector = ?, relations_json = ? WHERE id = ?",
                        (PM_QUEUE_STATUS_SUPERSEDED, blob_bytes, rel_json, item.id),
                    )
                    .await?;

                    let log = super::batch_result::DedupAuditLog {
                        queue_item_id: item.id,
                        item_fact: item.fact.clone(),
                        item_collection: item.collection.clone(),
                        stage: "stage2_soft_vector".to_string(),
                        action: "superseded_lower_priority".to_string(),
                        matched_fact_id: match_id.clone(),
                        matched_fact_coll: match_coll.clone(),
                        matched_fact: match_fact.clone(),
                        score: *sim,
                    };
                    if let Err(e) = crate::persistence::mutations::write_dedup_audit(conn, item.id, &log).await {
                        log::warn!("[MemoryPipeline::Stage2] Failed to write dedup audit: {}", e);
                    }
                } else {
                    for (m_id, _, _, _) in &soft_dups {
                        if !m_id.starts_with("item_") {
                            if let Err(e) = conn
                                .execute(
                                    "UPDATE memory_facts SET status = 'superseded' WHERE id = ?",
                                    (m_id.as_str(),),
                                )
                                .await
                            {
                                log::warn!("[MemoryPipeline::Stage2] Failed to supersede existing memory fact: {}", e);
                            }
                        }
                    }
                    conn.execute(
                        "UPDATE personal_memory_queue SET status = ?, vector = ? WHERE id = ?",
                        (PM_QUEUE_STATUS_EMBEDDED, blob_bytes, item.id),
                    )
                    .await?;

                    let log = super::batch_result::DedupAuditLog {
                        queue_item_id: item.id,
                        item_fact: item.fact.clone(),
                        item_collection: item.collection.clone(),
                        stage: "stage2_soft_vector".to_string(),
                        action: "superseded_existing".to_string(),
                        matched_fact_id: match_id.clone(),
                        matched_fact_coll: match_coll.clone(),
                        matched_fact: match_fact.clone(),
                        score: *sim,
                    };
                    if let Err(e) = crate::persistence::mutations::write_dedup_audit(conn, item.id, &log).await {
                        log::warn!("[MemoryPipeline::Stage2] Failed to write dedup audit: {}", e);
                    }
                }
            } else {
                conn.execute(
                    "UPDATE personal_memory_queue SET status = ?, vector = ? WHERE id = ?",
                    (PM_QUEUE_STATUS_EMBEDDED, blob_bytes, item.id),
                )
                .await?;
            }

            Ok(true)
        }
        Ok(None) | Err(_) => {
            log::warn!(
                "[Stage2Embed] Failed to generate embedding for queue item {}",
                item.id
            );
            mutations::mark_job_failed(conn, item.id, "Embedding generation failed").await;
            Ok(false)
        }
    }
}

/// Stage 2: Embedding & Soft Vector Deduplication Worker (Batch Size 16)
pub async fn run_stage2_embed(conn: &Connection) -> Result<usize> {
    run_stage2_embed_with_metrics(conn, "").await
}

/// Executes Stage 2 embedding with metrics recording.
pub async fn run_stage2_embed_with_metrics(conn: &Connection, run_id: &str) -> Result<usize> {
    let start_time = std::time::Instant::now();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;

    let items = claim_deduped_items(conn, now).await?;
    if items.is_empty() {
        return Ok(0);
    }

    let items_claimed = items.len();
    log::info!(
        "[MemoryPipeline::Stage2] Claimed {} deduped items for MiniLM embedding generation",
        items_claimed
    );
    ensure_embedder_loaded(true)?;

    let mut processed_count = 0;
    for item in items {
        if process_stage2_item(conn, &item).await? {
            processed_count += 1;
        }
    }

    let duration_ms = start_time.elapsed().as_millis();

    if !run_id.is_empty() {
        let metrics = super::metrics::PipelineStageMetrics {
            run_id: run_id.to_string(),
            stage_name: "stage2_embed".to_string(),
            session_id: String::new(),
            batch_seq: 0,
            items_claimed,
            error_count: 0,
            duration_ms,
        };
        if let Err(e) = crate::persistence::mutations::record_stage_metrics(conn, &metrics).await {
            log::warn!("[MemoryPipeline::Stage2] Failed to record stage metrics: {}", e);
        }
    }

    Ok(processed_count)
}
