use anyhow::Result;
use turso::Connection;
use crate::core::constants::{
    PM_QUEUE_STATUS_DEDUPED, PM_QUEUE_STATUS_EMBEDDED, PM_QUEUE_STATUS_PROCESSING_EMBED,
    PM_QUEUE_STATUS_SUPERSEDED, PM_RELATION_SUPERSEDES,
};
use crate::persistence::{encode_f32_blob, mutations, queries};
use crate::services::memory::embedder::{ensure_embedder_loaded, generate_embedding};
use super::batch_result::RelationEdge;

pub const STAGE2_BATCH_SIZE: usize = 16;
pub const SOFT_VECTOR_DEDUP_THRESHOLD: f32 = 0.95;

#[derive(Debug, Clone)]
pub struct Stage2Item {
    pub id: i64,
    pub fact: String,
    pub collection: String,
}

/// Stage 2: Embedding & Soft Vector Deduplication Worker (Batch Size 16)
/// Atomically claims `deduped` items, runs MiniLM-L12 ONNX vector embedding,
/// evaluates Phase 2 soft vector deduplication (cos >= 0.95), stores vector BLOB,
/// and transitions to `embedded` or `superseded`.
pub async fn run_stage2_embed(conn: &Connection) -> Result<usize> {
    run_stage2_embed_with_metrics(conn, "").await
}

pub async fn run_stage2_embed_with_metrics(conn: &Connection, run_id: &str) -> Result<usize> {
    let start_time = std::time::Instant::now();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;

    // 1. Select candidate deduped items
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

    if candidate_items.is_empty() {
        return Ok(0);
    }

    // 2. Atomically claim candidate items in DB
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

    if items.is_empty() {
        return Ok(0);
    }

    let items_claimed = items.len();
    ensure_embedder_loaded(true)?;

    let mut processed_count = 0;

    for item in items {
        let embedding_res = generate_embedding(&item.fact);
        match embedding_res {
            Ok(Some(vec)) => {
                let blob_bytes = encode_f32_blob(&vec);

                // Phase 2 Cross-Collection Soft Vector Deduplication Check (cos >= 0.95) & Priority Resolution
                let soft_dups = queries::fetch_cross_collection_candidates(
                    conn,
                    &vec,
                    SOFT_VECTOR_DEDUP_THRESHOLD,
                    Some(1),
                )
                .await
                .unwrap_or_default();

                if let Some((match_id, match_fact, match_coll, sim)) = soft_dups.first() {
                    let incoming_priority = crate::core::constants::MemoryCollection::parse(&item.collection)
                        .map(|c| c.priority())
                        .unwrap_or(0);
                    let existing_priority = crate::core::constants::MemoryCollection::parse(match_coll)
                        .map(|c| c.priority())
                        .unwrap_or(0);

                    if incoming_priority <= existing_priority {
                        // Lower or equal priority incoming item is superseded by existing DB fact
                        let rel = vec![RelationEdge {
                            from_id: format!("item_{}", item.id),
                            to_id: match_id.clone(),
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
                            matched_fact: match_fact.clone(),
                            score: *sim,
                        };
                        let _ = crate::persistence::mutations::write_dedup_audit(conn, item.id, &log).await;
                    } else {
                        // Higher priority incoming item supersedes existing lower priority DB fact
                        conn.execute(
                            "UPDATE memory_facts SET status = 'superseded' WHERE id = ?",
                            (match_id.as_str(),),
                        )
                        .await?;
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
                            matched_fact: match_fact.clone(),
                            score: *sim,
                        };
                        let _ = crate::persistence::mutations::write_dedup_audit(conn, item.id, &log).await;
                    }
                } else {
                    conn.execute(
                        "UPDATE personal_memory_queue SET status = ?, vector = ? WHERE id = ?",
                        (PM_QUEUE_STATUS_EMBEDDED, blob_bytes, item.id),
                    )
                    .await?;
                }

                processed_count += 1;
            }
            Ok(None) | Err(_) => {
                log::warn!("[Stage2Embed] Failed to generate embedding for queue item {}", item.id);
                mutations::mark_job_failed(conn, item.id, "Embedding generation failed").await;
            }
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
        let _ = crate::persistence::mutations::record_stage_metrics(conn, &metrics).await;
    }

    Ok(processed_count)
}
