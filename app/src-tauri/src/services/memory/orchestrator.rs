use anyhow::Result;
use turso::Connection;
use crate::core::settings::MemorySettings;
use crate::core::constants::{
    PM_RELATION_CONFLICTS, PM_RELATION_SUPPORTS, PM_RELATION_SIMILAR,
};
use crate::persistence::repository;
use crate::services::memory::deduplication::{jaccard_similarity, is_exact_duplicate};
use crate::services::memory::embedder::{ensure_embedder_loaded, generate_embedding, cosine_similarity};
use crate::services::memory::nli::{ensure_nli_loaded, classify_pair, relation_from_result, NliRelation};

#[derive(Debug, Clone, PartialEq)]
pub enum PipelineOutcome {
    NoWork,
    Merged { fact_id: String, merged_into: String },
    Ingested { fact_id: String, relations: Vec<String> },
}

/// Master memory pipeline orchestrator driving Phase 1 -> Phase 2 -> Phase 3 (v5 §5.1 / §5.3).
/// Processes one queued job from `personal_memory_queue`.
pub async fn process_one_queue_item(
    conn: &Connection,
    settings: &MemorySettings,
) -> Result<PipelineOutcome> {
    let mut rows = conn
        .query(
            "SELECT id, fact, collection, source, session_id FROM personal_memory_queue 
             WHERE status = 'pending' ORDER BY created_at ASC LIMIT 1",
            (),
        )
        .await?;

    let item = if let Some(row) = rows.next().await? {
        Some((
            row.get::<i64>(0)?,
            row.get::<String>(1)?,
            row.get::<String>(2)?,
            row.get::<String>(3)?,
            row.get::<String>(4)?,
        ))
    } else {
        None
    };

    let (job_id, fact, collection, source, session_id) = match item {
        Some(x) => x,
        None => return Ok(PipelineOutcome::NoWork),
    };

    conn.execute(
        "UPDATE personal_memory_queue SET status = 'processing' WHERE id = ?",
        (job_id,),
    )
    .await?;

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;
    let fact_id = format!("mem_{}_{}", now, uuid::Uuid::new_v4().simple());

    // ─── PHASE 1: Dual-Defense Fast Hard Deduplication & Embedding ────

    ensure_embedder_loaded(true)?;
    let embedding = match generate_embedding(&fact) {
        Ok(Some(v)) => v,
        Ok(None) => {
            repository::mark_job_failed(conn, job_id, "Embedding generator returned None (not loaded)").await;
            return Ok(PipelineOutcome::NoWork);
        }
        Err(e) => {
            repository::mark_job_failed(conn, job_id, &format!("Embedding generation failed: {}", e)).await;
            return Ok(PipelineOutcome::NoWork);
        }
    };

    let candidate_vectors = match repository::fetch_active_candidate_vectors(conn, &collection).await {
        Ok(c) => c,
        Err(e) => {
            repository::mark_job_failed(conn, job_id, &format!("Failed to fetch candidate vectors: {}", e)).await;
            return Ok(PipelineOutcome::NoWork);
        }
    };

    let mut scored_candidates = Vec::new();
    for (cand_id, cand_fact, emb_vector) in candidate_vectors {
        let sim = cosine_similarity(&embedding, &emb_vector);
        scored_candidates.push((sim, cand_id, cand_fact));
    }

    scored_candidates.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));

    let mut exact_match = None;
    for (sim, cand_id, cand_fact) in &scored_candidates {
        let jacc_sim = jaccard_similarity(&fact, cand_fact);
        if is_exact_duplicate(*sim, jacc_sim) {
            log::info!("[Orchestrator] Phase 1 O(1) Identity Match. Cosine: {}, Jaccard: {}. Candidate: {:?}", sim, jacc_sim, cand_id);
            exact_match = Some(cand_id.clone());
            break;
        }
    }

    if let Some(matched_cand_id) = exact_match {
        if let Err(e) = repository::insert_exact_merged_fact(
            conn, job_id, &fact_id, &fact, &collection, &source, &session_id, &matched_cand_id, &embedding, now
        ).await {
            repository::mark_job_failed(conn, job_id, &format!("Phase 1 Merge transaction failed: {}", e)).await;
            return Ok(PipelineOutcome::NoWork);
        } else {
            log::info!("[Orchestrator] Phase 1 O(1) Merge completed.");
            return Ok(PipelineOutcome::Merged {
                fact_id,
                merged_into: matched_cand_id,
            });
        }
    }

    // ─── PHASE 2: Candidate Retrieval & Multi-Tier NLI Routing ─────────

    let candidates: Vec<(f32, String, String)> = scored_candidates
        .into_iter()
        .take(settings.nli_candidate_limit as usize)
        .collect();

    let mut relations = Vec::new();
    let mut nli_pairs_to_classify = Vec::new();

    for (sim, cand_id, cand_fact) in candidates {
        if sim > 0.95 {
            log::info!("[Orchestrator] Multi-Tier Routing: Near-duplicate ({:.4}). Writing SIMILAR edge.", sim);
            relations.push((fact_id.clone(), cand_id.clone(), PM_RELATION_SIMILAR));
        } else if sim >= 0.65 {
            nli_pairs_to_classify.push((cand_id, cand_fact));
        }
    }

    if !nli_pairs_to_classify.is_empty() {
        if let Err(e) = ensure_nli_loaded(&settings.nli_model_name) {
            repository::mark_job_failed(conn, job_id, &format!("Failed to load NLI model: {}", e)).await;
            return Ok(PipelineOutcome::NoWork);
        }

        for (cand_id, cand_fact) in nli_pairs_to_classify {
            match classify_pair(&fact, &cand_fact) {
                Ok(nli_res) => {
                    let relation = relation_from_result(&nli_res, settings);
                    match relation {
                        NliRelation::Conflicts => {
                            log::info!("[Orchestrator] NLI: Conflict detected between enqueued fact and candidate fact.");
                            relations.push((fact_id.clone(), cand_id.clone(), PM_RELATION_CONFLICTS));
                        }
                        NliRelation::Supports => {
                            log::info!("[Orchestrator] NLI: Supports relationship detected between enqueued fact and candidate fact.");
                            relations.push((fact_id.clone(), cand_id.clone(), PM_RELATION_SUPPORTS));
                        }
                        NliRelation::Neutral => {}
                    }
                }
                Err(e) => {
                    repository::mark_job_failed(conn, job_id, &format!("NLI classification error: {}", e)).await;
                    return Ok(PipelineOutcome::NoWork);
                }
            }
        }
    }

    // ─── PHASE 3: Relation Mapping & Graph Persistence ───────────────

    let relation_names: Vec<String> = relations.iter().map(|(_, _, r)| r.to_string()).collect();

    if let Err(e) = repository::insert_fact_with_vector_and_relations(
        conn, job_id, &fact_id, &fact, &collection, &source, &session_id, &embedding, relations, now
    ).await {
        repository::mark_job_failed(conn, job_id, &format!("Phase 3 Persistence failed: {}", e)).await;
        Ok(PipelineOutcome::NoWork)
    } else {
        log::info!("[Orchestrator] Phase 3 Persistence succeeded for fact_id={}", fact_id);
        Ok(PipelineOutcome::Ingested {
            fact_id,
            relations: relation_names,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_orchestrator_empty_queue() -> Result<()> {
        let db = turso::Builder::new_local(":memory:").build().await?;
        let conn = db.connect()?;
        crate::persistence::schema::run_migrations(&conn).await?;

        let settings = MemorySettings::default();
        let outcome = process_one_queue_item(&conn, &settings).await?;
        assert_eq!(outcome, PipelineOutcome::NoWork);
        Ok(())
    }
}
