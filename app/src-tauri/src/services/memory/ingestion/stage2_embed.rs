use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::Result;
use turso::Connection;
use uuid::Uuid;

use super::{SOFT_VECTOR_DEDUP_THRESHOLD, STAGE2_BATCH_SIZE};
use crate::{
    persistence::{
        deactivate_fact,
        facts::{fetch_active_vectors_by_type, insert_fact, insert_vector, FactRecord},
        fetch_session_project_id,
        queue::claim_pending_queue_batch,
        record_queue_item_failure, update_queue_item_status, QueueItem,
    },
    services::memory::{cosine_similarity, ensure_embedder_loaded, generate_embeddings_batch},
};

/// Summary metrics returned after running a Stage 2 semantic cosine deduplication pass.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Stage2Summary {
    pub processed: usize,
    pub inserted: usize,
    pub duplicates_deactivated: usize,
    pub errors: usize,
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

    for (item, embedding) in items.into_iter().zip(embeddings.into_iter()) {
        match process_stage2_item_with_embedding(conn, &item, &embedding).await {
            Ok((inserted, deactivated)) => {
                if let Err(e) = update_queue_item_status(conn, item.id, "completed", None).await {
                    log::warn!(
                        "[Memory::Ingestion::Stage2] Failed to mark item {} as completed: {}",
                        item.id,
                        e
                    );
                    summary.errors += 1;
                } else {
                    summary.processed += 1;
                    summary.inserted += inserted;
                    summary.duplicates_deactivated += deactivated;
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
                    "stage1_done",
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

/// Deduplicates against active vectors and commits a single fact and vector to storage.
async fn process_stage2_item_with_embedding(
    conn: &Connection,
    item: &QueueItem,
    embedding: &[f32],
) -> Result<(usize, usize)> {
    let active_vectors = fetch_active_vectors_by_type(conn, &item.fact_type).await?;
    let mut deactivated = 0;

    for (existing_fact_id, vec) in active_vectors {
        let similarity = cosine_similarity(embedding, &vec);
        if similarity >= SOFT_VECTOR_DEDUP_THRESHOLD {
            log::info!(
                "[Memory::Ingestion::Stage2] Cosine duplicate found (sim={:.3}). Deactivating older fact {} for incoming item {}",
                similarity,
                existing_fact_id,
                item.id
            );
            deactivate_fact(conn, &existing_fact_id).await?;
            deactivated += 1;
        }
    }

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;
    let fact_id = format!("fact_{}_{}", now, Uuid::new_v4().simple());

    let project_id = resolve_project_id(conn, item.session_id).await?;

    let fact = FactRecord {
        id: fact_id.clone(),
        session_id: item.session_id,
        compaction_id: item.compaction_id,
        fact_type: item.fact_type.clone(),
        text: item.text.clone(),
        status: "active".to_string(),
        created_at: now,
        updated_at: now,
    };

    insert_fact(conn, &fact).await?;
    insert_vector(
        conn,
        &fact_id,
        &item.fact_type,
        "active",
        project_id.as_deref(),
        embedding,
    )
    .await?;

    Ok((1, deactivated))
}

/// Queries the parent project ID for a given session ID if present.
async fn resolve_project_id(conn: &Connection, session_id: Option<i64>) -> Result<Option<String>> {
    let sid = match session_id {
        Some(s) => s,
        None => return Ok(None),
    };

    fetch_session_project_id(conn, sid).await
}

#[cfg(test)]
mod tests {
    use turso::Builder;

    use super::*;
    use crate::persistence::{
        compactions::record_compaction_start,
        facts::{
            fetch_active_facts_by_type, fetch_active_vectors_by_type, insert_fact, insert_vector,
            FactRecord,
        },
        queue::enqueue_fact,
        schema::recreate_schema,
        sessions::create_session,
    };

    #[tokio::test]
    async fn test_stage2_cosine_dedup_winner_takes_all() {
        let db = Builder::new_local(":memory:").build().await.unwrap();
        let conn = db.connect().unwrap();
        recreate_schema(&conn).await.unwrap();

        let session_id = create_session(&conn, Some("default")).await.unwrap();
        let compaction_id = record_compaction_start(&conn, session_id, "soft", 0, 5)
            .await
            .unwrap();

        let base_vector = vec![0.5f32; 384];
        let old_fact = FactRecord {
            id: "fact_old_vec".to_string(),
            session_id: Some(session_id),
            compaction_id,
            fact_type: "workdone".to_string(),
            text: "Refactored audio ring buffer".to_string(),
            status: "active".to_string(),
            created_at: 1000,
            updated_at: 1000,
        };
        insert_fact(&conn, &old_fact).await.unwrap();
        insert_vector(
            &conn,
            &old_fact.id,
            "workdone",
            "active",
            Some("default"),
            &base_vector,
        )
        .await
        .unwrap();

        let q_id = enqueue_fact(
            &conn,
            Some(session_id),
            compaction_id,
            "workdone",
            "Rewrote audio ring buffer implementation",
        )
        .await
        .unwrap();

        conn.execute(
            "UPDATE memory_ingestion_queue SET status = 'stage1_done' WHERE id = ?",
            (q_id,),
        )
        .await
        .unwrap();

        let mock_vec = base_vector.clone();
        let summary =
            run_stage2_cosine_dedup_with_embedder(&conn, move |_| Ok(Some(mock_vec.clone())))
                .await
                .unwrap();

        assert_eq!(summary.processed, 1);
        assert_eq!(summary.inserted, 1);
        assert_eq!(summary.duplicates_deactivated, 1);
        assert_eq!(summary.errors, 0);

        let active_facts = fetch_active_facts_by_type(&conn, "workdone").await.unwrap();
        assert_eq!(active_facts.len(), 1);
        assert_ne!(active_facts[0].id, "fact_old_vec");
        assert_eq!(
            active_facts[0].text,
            "Rewrote audio ring buffer implementation"
        );

        let active_vecs = fetch_active_vectors_by_type(&conn, "workdone")
            .await
            .unwrap();
        assert_eq!(active_vecs.len(), 1);
        assert_eq!(active_vecs[0].0, active_facts[0].id);

        let mut rows = conn
            .query(
                "SELECT status FROM memory_ingestion_queue WHERE id = ?",
                (q_id,),
            )
            .await
            .unwrap();
        let row = rows.next().await.unwrap().unwrap();
        let status: String = row.get(0).unwrap();
        assert_eq!(status, "completed");
    }
}
