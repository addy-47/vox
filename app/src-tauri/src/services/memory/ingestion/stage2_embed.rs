use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::Result;
use turso::Connection;
use uuid::Uuid;

use super::{
    is_near_miss, DedupDecision, DedupNearMiss, DedupStage, SOFT_VECTOR_DEDUP_THRESHOLD,
    STAGE2_BATCH_SIZE,
};
use crate::{
    persistence::{
        deactivate_observation,
        facts::{
            fetch_active_vectors_by_type, insert_observation, insert_vector, ObservationRecord,
        },
        fetch_session_project_id,
        queue::claim_pending_queue_batch,
        record_queue_item_failure, update_queue_item_status, QueueItem,
    },
    services::memory::{cosine_similarity, ensure_embedder_loaded, generate_embeddings_batch},
};

/// Strongly-typed lifecycle state for a single Stage 2 item's telemetry.
#[derive(Debug, Clone, Default)]
pub struct Stage2ItemTelemetry {
    pub decisions: Vec<DedupDecision>,
    pub near_misses: Vec<DedupNearMiss>,
}

/// Summary metrics returned after running a Stage 2 semantic cosine deduplication pass.
#[derive(Debug, Clone, Default)]
pub struct Stage2Summary {
    pub processed: usize,
    pub inserted: usize,
    pub duplicates_deactivated: usize,
    pub errors: usize,
    /// Telemetry only. Records each merge with the cosine that triggered it.
    pub decisions: Vec<DedupDecision>,
    /// Telemetry only. Records borderline pairs that were deliberately left alone.
    pub near_misses: Vec<DedupNearMiss>,
}

/// Executes Stage 2 semantic deduplication using the default MiniLM ONNX embedder singleton
/// with batched ONNX tensor inference offloaded to a background blocking thread.
pub async fn run_stage2_cosine_dedup(conn: &Connection) -> Result<Stage2Summary> {
    run_stage2_cosine_dedup_with_batch_embedder(conn, |texts| {
        if let Err(e) = ensure_embedder_loaded(true) {
            log::warn!("[Stage2Dedup] Failed to ensure embedder loaded: {}", e);
        }
        generate_embeddings_batch(texts)
    })
    .await
}

/// Executes Stage 2 semantic deduplication with a custom single-item embedding function.
pub async fn run_stage2_cosine_dedup_with_embedder<F>(
    conn: &Connection,
    embed_fn: F,
) -> Result<Stage2Summary>
where
    F: Fn(&str) -> Result<Option<Vec<f32>>> + Send + Sync + 'static,
{
    run_stage2_cosine_dedup_with_batch_embedder(conn, move |texts| {
        let mut results = Vec::with_capacity(texts.len());
        for text in texts {
            let vec = embed_fn(text)?
                .ok_or_else(|| anyhow::anyhow!("Text embedder model is not loaded or available"))?;
            results.push(vec);
        }
        Ok(Some(results))
    })
    .await
}

/// Executes Stage 2 semantic deduplication with a custom or injected batch embedding function.
pub async fn run_stage2_cosine_dedup_with_batch_embedder<F>(
    conn: &Connection,
    embed_fn: F,
) -> Result<Stage2Summary>
where
    F: Fn(&[&str]) -> Result<Option<Vec<Vec<f32>>>> + Send + Sync + 'static,
{
    let items =
        claim_pending_queue_batch(conn, "stage1_done", "stage2_processing", STAGE2_BATCH_SIZE)
            .await?;

    if items.is_empty() {
        return Ok(Stage2Summary::default());
    }

    let texts: Vec<String> = items.iter().map(|it| it.text.clone()).collect();
    let embeddings_res = tokio::task::spawn_blocking(move || {
        let str_refs: Vec<&str> = texts.iter().map(|s| s.as_str()).collect();
        embed_fn(&str_refs)
    })
    .await?;

    let embeddings = match embeddings_res {
        Ok(Some(vecs)) if vecs.len() == items.len() => vecs,
        Ok(Some(vecs)) => {
            anyhow::bail!(
                "Batch embedder returned mismatched vector count: expected {}, got {}",
                items.len(),
                vecs.len()
            );
        }
        Ok(None) => {
            anyhow::bail!("Text embedder model is not loaded or available");
        }
        Err(e) => {
            anyhow::bail!("Batch embedding failed: {}", e);
        }
    };

    let mut summary = Stage2Summary::default();

    for (item, embedding) in items.into_iter().zip(embeddings) {
        match process_stage2_item_with_embedding(conn, &item, &embedding).await {
            Ok(telemetry) => {
                let deactivated = telemetry.decisions.len();
                if let Err(e) = update_queue_item_status(conn, item.id, "completed", None).await {
                    log::warn!(
                        "[Memory::Ingestion::Stage2] Failed to mark item {} as completed: {}",
                        item.id,
                        e
                    );
                    summary.errors += 1;
                } else {
                    summary.processed += 1;
                    summary.inserted += 1;
                    summary.duplicates_deactivated += deactivated;
                    summary.decisions.extend(telemetry.decisions);
                    summary.near_misses.extend(telemetry.near_misses);
                }
            }
            Err(e) => {
                log::warn!(
                    "[Memory::Ingestion::Stage2] Error processing item {}: {}",
                    item.id,
                    e
                );
                if let Err(rec_err) = record_queue_item_failure(
                    conn,
                    item.id,
                    item.retry_count,
                    &e.to_string(),
                )
                .await
                {
                    log::warn!(
                        "[Memory::Ingestion::Stage2] Failed to record failure for item {}: {}",
                        item.id,
                        rec_err
                    );
                }
                summary.errors += 1;
            }
        }
    }

    log::info!(
        "[Memory::Ingestion::Stage2] Batch cycle completed: {} processed, {} inserted, {} deactivated, {} errors",
        summary.processed,
        summary.inserted,
        summary.duplicates_deactivated,
        summary.errors
    );

    Ok(summary)
}

/// Deduplicates against active vectors and commits a single observation and vector to storage.
///
/// On a cosine match the incoming observation wins: the older stored observation is deactivated, and the
/// incoming one is inserted as `active`. Returns the telemetry for this item.
async fn process_stage2_item_with_embedding(
    conn: &Connection,
    item: &QueueItem,
    embedding: &[f32],
) -> Result<Stage2ItemTelemetry> {
    let active_vectors = fetch_active_vectors_by_type(conn, &item.observation_type).await?;
    let mut telemetry = Stage2ItemTelemetry::default();

    for (existing_observation_id, vec) in active_vectors {
        let similarity = cosine_similarity(embedding, &vec);
        if similarity >= SOFT_VECTOR_DEDUP_THRESHOLD {
            log::info!(
                "[Memory::Ingestion::Stage2] Cosine duplicate found (sim={:.3}). Deactivating older observation {} for incoming item {}",
                similarity,
                existing_observation_id,
                item.id
            );
            let text = observation_text(conn, &existing_observation_id).await;
            deactivate_observation(conn, &existing_observation_id).await?;
            telemetry.decisions.push(DedupDecision {
                stage: DedupStage::Stage2Cosine,
                incoming_queue_id: item.id,
                incoming_text: item.text.clone(),
                incoming_type: item.observation_type.clone(),
                deactivated_obs_id: existing_observation_id,
                deactivated_text: text,
                similarity,
            });
        } else if is_near_miss(similarity, SOFT_VECTOR_DEDUP_THRESHOLD) {
            let text = observation_text(conn, &existing_observation_id).await;
            telemetry.near_misses.push(DedupNearMiss {
                stage: DedupStage::Stage2Cosine,
                incoming_queue_id: item.id,
                incoming_text: item.text.clone(),
                active_obs_id: existing_observation_id,
                active_text: text,
                similarity,
            });
        }
    }

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;
    let observation_id = format!("fact_{}_{}", now, Uuid::new_v4().simple());

    let project_id = resolve_project_id(conn, item.session_id).await?;

    let observation = ObservationRecord {
        id: observation_id.clone(),
        session_id: item.session_id,
        compaction_id: item.compaction_id,
        observation_type: item.observation_type.clone(),
        text: item.text.clone(),
        status: "active".to_string(),
        created_at: now,
        updated_at: now,
    };

    insert_observation(conn, &observation).await?;
    insert_vector(
        conn,
        &observation_id,
        &item.observation_type,
        "active",
        project_id.as_deref(),
        embedding,
    )
    .await?;

    Ok(telemetry)
}

/// Reads an observation's text for telemetry reporting.
///
/// Telemetry must never be able to fail a deduplication pass, so a lookup
/// failure degrades to a placeholder rather than propagating.
async fn observation_text(conn: &Connection, observation_id: &str) -> String {
    let fallback = format!("<observation {} text unavailable>", observation_id);
    let mut rows = match conn
        .query(
            "SELECT text FROM memory_facts WHERE id = ?",
            (observation_id.to_string(),),
        )
        .await
    {
        Ok(r) => r,
        Err(e) => {
            log::warn!(
                "[Memory::Ingestion::Stage2] Telemetry text lookup failed for {}: {}",
                observation_id,
                e
            );
            return fallback;
        }
    };
    match rows.next().await {
        Ok(Some(row)) => row.get(0).unwrap_or(fallback),
        Ok(None) => fallback,
        Err(e) => {
            log::warn!(
                "[Memory::Ingestion::Stage2] Telemetry text row read failed for {}: {}",
                observation_id,
                e
            );
            fallback
        }
    }
}

/// Queries the parent project ID for a given session ID if present.
async fn resolve_project_id(conn: &Connection, session_id: Option<i64>) -> Result<Option<String>> {
    let sid = match session_id {
        Some(s) => s,
        None => return Ok(None),
    };

    fetch_session_project_id(conn, sid).await
}
