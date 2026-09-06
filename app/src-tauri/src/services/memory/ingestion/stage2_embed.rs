use super::RelationEdge;
use crate::persistence::{encode_f32_blob, mutations, queries};
use crate::services::memory::ml::embedder::{ensure_embedder_loaded, generate_embedding};
use crate::services::memory::{QueueStatus, Relation};
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
            (QueueStatus::ProcessingEmbed.as_str(), now, item.id, QueueStatus::Deduped.as_str()),
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
            (QueueStatus::Embedded.as_str(), item.id),
        )
        .await?;
        return Ok(true);
    }

    let fact_str = item.fact.clone();
    let embedding_res = tokio::task::spawn_blocking(move || generate_embedding(&fact_str)).await?;
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
            .map_err(|e| {
                log::warn!(
                    "[MemoryPipeline::Stage2] Failed to fetch cross-collection candidates for item {}: {}",
                    item.id,
                    e
                );
                e
            })?;

            let best_match = soft_dups.iter().max_by(|a, b| {
                let prio_a = crate::services::memory::MemoryCollection::parse(&a.2)
                    .map(|c| c.priority())
                    .unwrap_or(0);
                let prio_b = crate::services::memory::MemoryCollection::parse(&b.2)
                    .map(|c| c.priority())
                    .unwrap_or(0);
                prio_a
                    .cmp(&prio_b)
                    .then_with(|| a.3.partial_cmp(&b.3).unwrap_or(std::cmp::Ordering::Equal))
            });

            if let Some((match_id, match_fact, match_coll, sim)) = best_match {
                let incoming_priority =
                    crate::services::memory::MemoryCollection::parse(&item.collection)
                        .map(|c| c.priority())
                        .unwrap_or(0);
                let existing_priority =
                    crate::services::memory::MemoryCollection::parse(match_coll)
                        .map(|c| c.priority())
                        .unwrap_or(0);

                if incoming_priority <= existing_priority {
                    let rel = vec![RelationEdge {
                        from_id: match_id.clone(),
                        to_id: format!("item_{}", item.id),
                        relation: Relation::Supersedes.as_str().to_string(),
                        source: "Embedding".to_string(),
                    }];
                    let rel_json = serde_json::to_string(&rel).unwrap_or_else(|_| "[]".to_string());

                    conn.execute(
                        "UPDATE personal_memory_queue SET status = ?, vector = ?, relations_json = ? WHERE id = ?",
                        (QueueStatus::Superseded.as_str(), blob_bytes, rel_json, item.id),
                    )
                    .await?;

                    let log = super::DedupAuditLog {
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
                    if let Err(e) =
                        crate::persistence::mutations::write_dedup_audit(conn, item.id, &log).await
                    {
                        log::warn!(
                            "[MemoryPipeline::Stage2] Failed to write dedup audit: {}",
                            e
                        );
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
                        (QueueStatus::Embedded.as_str(), blob_bytes, item.id),
                    )
                    .await?;

                    let log = super::DedupAuditLog {
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
                    if let Err(e) =
                        crate::persistence::mutations::write_dedup_audit(conn, item.id, &log).await
                    {
                        log::warn!(
                            "[MemoryPipeline::Stage2] Failed to write dedup audit: {}",
                            e
                        );
                    }
                }
            } else {
                conn.execute(
                    "UPDATE personal_memory_queue SET status = ?, vector = ? WHERE id = ?",
                    (QueueStatus::Embedded.as_str(), blob_bytes, item.id),
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
    let mut error_count = 0;
    for item in items {
        match process_stage2_item(conn, &item).await {
            Ok(true) => processed_count += 1,
            Ok(false) => {}
            Err(e) => {
                log::error!(
                    "[MemoryPipeline::Stage2] Error embedding item {}: {}",
                    item.id,
                    e
                );
                error_count += 1;
            }
        }
    }

    let duration_ms = start_time.elapsed().as_millis();

    if !run_id.is_empty() {
        let metrics = super::PipelineStageMetrics {
            run_id: run_id.to_string(),
            stage_name: "stage2_embed".to_string(),
            session_id: String::new(),
            batch_seq: 0,
            items_claimed,
            error_count,
            duration_ms,
        };
        if let Err(e) = crate::persistence::mutations::record_stage_metrics(conn, &metrics).await {
            log::warn!(
                "[MemoryPipeline::Stage2] Failed to record stage metrics: {}",
                e
            );
        }
    }

    Ok(processed_count)
}
