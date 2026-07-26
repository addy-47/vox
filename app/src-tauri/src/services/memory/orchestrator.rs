use anyhow::Result;
use turso::Connection;
use crate::core::settings::MemorySettings;
use crate::core::constants::{
    PM_RELATION_CONFLICTS, PM_RELATION_SUPPORTS, PM_RELATION_SUPERSEDES,
    PM_CLASS_C_COLLECTIONS, PM_CLASS_A_COLLECTIONS, PM_CLASS_B_COLLECTIONS,
    inter_collection_edge,
};
use crate::persistence::{queries, mutations};
use crate::services::memory::deduplication::{jaccard_similarity, is_exact_duplicate};
use crate::services::memory::embedder::{ensure_embedder_loaded, generate_embedding};
use crate::services::memory::llm_edge_classifier;
use crate::services::memory::nli::{ensure_nli_loaded, classify_pair, relation_from_result, NliRelation, NLI_MODEL_DIR};

/// MiniLM-L12 calibrated intra-collection NLI candidate search threshold (Class A).
pub const SAME_COLLECTION_CANDIDATE_SEARCH: f32 = 0.40;
/// MiniLM-L12 calibrated inter-collection LLM edge candidate search threshold (Class B).
pub const INTER_COLLECTION_CANDIDATE_SEARCH: f32 = 0.55;

#[derive(Debug, Clone, PartialEq)]
pub enum PipelineOutcome {
    NoWork,
    Merged { fact_id: String, merged_into: String },
    Ingested { fact_id: String, relations: Vec<String> },
}

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

/// Master memory pipeline orchestrator driving 3-Class Taxonomy Ingestion Routing.
/// Processes one queued job from `personal_memory_queue`.
pub async fn process_one_queue_item(
    conn: &Connection,
    settings: &MemorySettings,
    cancel_flag: &Arc<AtomicBool>,
) -> Result<PipelineOutcome> {
    if !settings.pipeline_processing_enabled || cancel_flag.load(Ordering::Relaxed) {
        return Ok(PipelineOutcome::NoWork);
    }

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

    if fact.trim().is_empty() {
        let _ = conn
            .execute(
                "UPDATE personal_memory_queue SET status = 'completed' WHERE id = ?",
                (job_id,),
            )
            .await;
        return Ok(PipelineOutcome::NoWork);
    }

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

    // Check cancellation before heavy ONNX embedding inference
    if cancel_flag.load(Ordering::Relaxed) {
        let _ = conn.execute("UPDATE personal_memory_queue SET status = 'pending' WHERE id = ?", (job_id,)).await;
        log::info!("[Orchestrator] Interrupted before ONNX embedding generation. Reverted job to pending.");
        return Ok(PipelineOutcome::NoWork);
    }

    // ─── PHASE 1: Dual-Defense Fast Hard Deduplication & Embedding ────

    ensure_embedder_loaded(true)?;
    let embedding = match generate_embedding(&fact) {
        Ok(Some(v)) => v,
        Ok(None) => {
            mutations::mark_job_failed(conn, job_id, "Embedding generator returned None (not loaded)").await;
            return Ok(PipelineOutcome::NoWork);
        }
        Err(e) => {
            mutations::mark_job_failed(conn, job_id, &format!("Embedding generation failed: {}", e)).await;
            return Ok(PipelineOutcome::NoWork);
        }
    };

    // Fetch intra-collection candidates via SQL vector_distance_cos — pre-filtered by SAME_COLLECTION_CANDIDATE_SEARCH.
    // Returns (id, fact_text) tuples only; no Rust cosine decode or loop needed.
    let nli_candidate_pool: Vec<(String, String)> = match queries::fetch_intra_collection_candidates(
        conn, &collection, &embedding, SAME_COLLECTION_CANDIDATE_SEARCH, settings.top_k_facts as i64
    ).await {
        Ok(c) => c,
        Err(e) => {
            mutations::mark_job_failed(conn, job_id, &format!("Failed to fetch candidate vectors: {}", e)).await;
            return Ok(PipelineOutcome::NoWork);
        }
    };

    // Phase 1 O(1) Exact Dedup: run Jaccard against SQL-returned candidates (already cosine-pre-filtered).
    let mut exact_match = None;
    for (cand_id, cand_fact) in &nli_candidate_pool {
        let jacc_sim = jaccard_similarity(&fact, cand_fact);
        if is_exact_duplicate(1.0, jacc_sim) {
            log::info!("[Orchestrator] Phase 1 O(1) Identity Match. Jaccard: {}. Candidate: {:?}", jacc_sim, cand_id);
            exact_match = Some(cand_id.clone());
            break;
        }
    }

    if let Some(matched_cand_id) = exact_match {
        if let Err(e) = mutations::insert_exact_merged_fact(
            conn, job_id, &fact_id, &fact, &collection, &source, &session_id, &matched_cand_id, &embedding, now
        ).await {
            mutations::mark_job_failed(conn, job_id, &format!("Phase 1 Merge transaction failed: {}", e)).await;
            return Ok(PipelineOutcome::NoWork);
        } else {
            log::info!("[Orchestrator] Phase 1 O(1) Merge completed.");
            return Ok(PipelineOutcome::Merged {
                fact_id,
                merged_into: matched_cand_id,
            });
        }
    }

    // ─── 3-CLASS TAXONOMY INGESTION ROUTING ───────────────────────

    let is_class_a = PM_CLASS_A_COLLECTIONS.contains(&collection.as_str());
    let is_class_b = PM_CLASS_B_COLLECTIONS.contains(&collection.as_str());
    let is_class_c = PM_CLASS_C_COLLECTIONS.contains(&collection.as_str());

    let mut relations: Vec<(String, String, &'static str, &'static str)> = Vec::new();

    if is_class_a {
        // Class A (Identity, Context): Hard-dedup ONLY. Zero NLI. Zero LLM. Zero relation edges.
        log::info!("[Orchestrator] Class A isolated fact ({}). Zero NLI/LLM edge evaluation.", collection);
    } else if is_class_b {
        // Class B (Constraints, Tasks, Goals): Intra-collection NLI ONLY.
        // Candidates already fetched and cosine-pre-filtered by SQL vector_distance_cos above.
        if !nli_candidate_pool.is_empty() {
            if cancel_flag.load(Ordering::Relaxed) {
                let _ = conn.execute("UPDATE personal_memory_queue SET status = 'pending' WHERE id = ?", (job_id,)).await;
                log::info!("[Orchestrator] Interrupted before ONNX NLI classification. Reverted job to pending.");
                return Ok(PipelineOutcome::NoWork);
            }

            if let Err(e) = ensure_nli_loaded(NLI_MODEL_DIR) {
                mutations::mark_job_failed(conn, job_id, &format!("Failed to load NLI model: {}", e)).await;
                return Ok(PipelineOutcome::NoWork);
            }

            for (cand_id, cand_fact) in &nli_candidate_pool {
                match classify_pair(&fact, &cand_fact) {
                    Ok(nli_res) => {
                        let relation = relation_from_result(&nli_res);
                        match relation {
                            NliRelation::Conflicts => {
                                log::info!("[Orchestrator] NLI: Conflict detected between {} and candidate {}.", fact_id, cand_id);
                                relations.push((fact_id.clone(), cand_id.clone(), PM_RELATION_CONFLICTS, "NLI"));
                            }
                            NliRelation::Supports => {
                                if collection == "Tasks" || collection == "Goals" {
                                    // Tasks and Goals auto-resolve Entailment to SUPERSEDES
                                    log::info!("[Orchestrator] NLI: Tasks/Goals Entailment detected. Writing SUPERSEDES edge from {} to {}.", fact_id, cand_id);
                                    relations.push((fact_id.clone(), cand_id.clone(), PM_RELATION_SUPERSEDES, "NLI"));
                                    let _ = conn.execute("UPDATE memory_facts SET status = 'superseded' WHERE id = ?", (cand_id.clone(),)).await;
                                } else {
                                    // Constraints Entailment is refinement (SUPPORTS)
                                    log::info!("[Orchestrator] NLI: Constraints Entailment refinement detected from {} to {}.", fact_id, cand_id);
                                    relations.push((fact_id.clone(), cand_id.clone(), PM_RELATION_SUPPORTS, "NLI"));
                                }
                            }
                            NliRelation::Neutral => {}
                        }
                    }
                    Err(e) => {
                        mutations::mark_job_failed(conn, job_id, &format!("NLI classification error: {}", e)).await;
                        return Ok(PipelineOutcome::NoWork);
                    }
                }
            }
        }
    } else if is_class_c {
        // Class C (Skills, Preferences, Projects, Experiences, Relationships):
        // Intra-collection: Phase 1 hard-dedup only (already passed above).
        // Inter-collection: Candidate search via SQL vector_distance_cos at INTER_COLLECTION_CANDIDATE_SEARCH (0.55).
        let policy_targets: Vec<(&'static str, &'static str)> = PM_CLASS_C_COLLECTIONS
            .iter()
            .filter_map(|&tgt| inter_collection_edge(&collection, tgt).map(|(fwd, _inv)| (tgt, fwd)))
            .collect();

        if !policy_targets.is_empty() {
            let target_collections: Vec<&str> = policy_targets.iter().map(|(tgt, _)| *tgt).collect();
            // fetch_inter_collection_candidates returns (id, fact_text, collection) pre-filtered by threshold.
            let inter_candidates = match queries::fetch_inter_collection_candidates(
                conn,
                &target_collections,
                &embedding,
                INTER_COLLECTION_CANDIDATE_SEARCH,
                settings.top_k_facts as i64,
            ).await {
                Ok(c) => c,
                Err(e) => {
                    log::warn!("[Orchestrator] Failed to fetch inter-collection candidates for Class C: {}", e);
                    Vec::new()
                }
            };

            // Source fact's session context: resolve from the queue item's session_id directly
            // (fact hasn't been inserted into memory_facts yet at this point)
            let src_context = queries::fetch_session_context(conn, &session_id, now).await.unwrap_or(None);

            // No Rust cosine loop — all candidates are already above INTER_COLLECTION_CANDIDATE_SEARCH threshold.
            for (cand_id, cand_fact, cand_coll) in inter_candidates {
                if let Some((forward_edge, inverse_edge)) = inter_collection_edge(&collection, &cand_coll) {
                    let tgt_context = queries::fetch_fact_session_context(conn, &cand_id).await.unwrap_or(None);

                    match llm_edge_classifier::classify_edge(
                        &collection,
                        &fact,
                        src_context.as_deref(),
                        &cand_coll,
                        &cand_fact,
                        tgt_context.as_deref(),
                        forward_edge,
                    ) {
                        Ok(Some(pred_edge)) => {
                            log::info!(
                                "[Orchestrator] Class C Edge Generated (source='LLM'): {} ({}) -[{}]-> {} ({})",
                                fact_id, collection, pred_edge, cand_id, cand_coll
                            );
                            relations.push((fact_id.clone(), cand_id.clone(), forward_edge, "LLM"));
                            // Automatic Deterministic Inverse Edge (spec §5.4 & §6)
                            relations.push((cand_id.clone(), fact_id.clone(), inverse_edge, "LLM"));
                        }
                        Ok(None) => {}
                        Err(e) => {
                            log::warn!("[Orchestrator] Class C LLM classification error: {}", e);
                        }
                    }
                }
            }
        }
    }

    // ─── PHASE 3: Relation Mapping & Graph Persistence ───────────────

    let relation_names: Vec<String> = relations.iter().map(|(_, _, r, _)| r.to_string()).collect();

    if let Err(e) = mutations::insert_fact_with_vector_and_relations(
        conn, job_id, &fact_id, &fact, &collection, &source, &session_id, &embedding, relations, now
    ).await {
        mutations::mark_job_failed(conn, job_id, &format!("Phase 3 Persistence failed: {}", e)).await;
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
        let db = turso::Builder::new_local(":memory:").experimental_index_method(true).build().await?;
        let conn = db.connect()?;
        crate::persistence::schema::run_migrations(&conn).await?;

        let settings = MemorySettings::default();
        let cancel_flag = Arc::new(AtomicBool::new(false));
        let outcome = process_one_queue_item(&conn, &settings, &cancel_flag).await?;
        assert_eq!(outcome, PipelineOutcome::NoWork);
        Ok(())
    }

    #[tokio::test]
    async fn test_memory_ingestion_empty_and_whitespace() -> Result<()> {
        let db = turso::Builder::new_local(":memory:").experimental_index_method(true).build().await?;
        let conn = db.connect()?;
        crate::persistence::schema::run_migrations(&conn).await?;

        let settings = MemorySettings::default();
        let cancel_flag = Arc::new(AtomicBool::new(false));

        let test_cases = vec!["", "   ", "\n\t"];
        for (i, empty_fact) in test_cases.into_iter().enumerate() {
            conn.execute(
                "INSERT INTO personal_memory_queue (id, fact, collection, source, session_id, status, created_at)
                 VALUES (?, ?, 'Skills', 'test', 'sess_1', 'pending', 1000)",
                (i as i64 + 1, empty_fact.to_string()),
            )
            .await?;

            let outcome = process_one_queue_item(&conn, &settings, &cancel_flag).await?;
            assert_eq!(outcome, PipelineOutcome::NoWork);
        }

        // Verify zero facts inserted into memory_facts table
        let mut rows = conn.query("SELECT COUNT(*) FROM memory_facts", ()).await?;
        let count: i64 = rows.next().await?.unwrap().get(0)?;
        assert_eq!(count, 0, "No memory_facts should be created for empty/whitespace inputs");

        Ok(())
    }

    #[tokio::test]
    async fn test_memory_ingestion_class_a_direct_isolation() -> Result<()> {
        let db = turso::Builder::new_local(":memory:").experimental_index_method(true).build().await?;
        let conn = db.connect()?;
        crate::persistence::schema::run_migrations(&conn).await?;

        let settings = MemorySettings::default();
        let cancel_flag = Arc::new(AtomicBool::new(false));

        // Insert Class A facts: Identity and Context
        conn.execute(
            "INSERT INTO personal_memory_queue (id, fact, collection, source, session_id, status, created_at)
             VALUES (1, 'User prefers dark theme', 'Identity', 'test', 'sess_1', 'pending', 1000)",
            (),
        )
        .await?;

        conn.execute(
            "INSERT INTO personal_memory_queue (id, fact, collection, source, session_id, status, created_at)
             VALUES (2, 'User discussed Rust performance', 'Context', 'test', 'sess_1', 'pending', 1001)",
            (),
        )
        .await?;

        // Process item 1 (Identity)
        let outcome1 = process_one_queue_item(&conn, &settings, &cancel_flag).await?;
        match outcome1 {
            PipelineOutcome::Ingested { relations, .. } => {
                assert!(relations.is_empty(), "Class A items must bypass NLI/LLM and create 0 relation edges");
            }
            other => panic!("Expected Ingested outcome for Class A item 1, got {:?}", other),
        }

        // Process item 2 (Context)
        let outcome2 = process_one_queue_item(&conn, &settings, &cancel_flag).await?;
        match outcome2 {
            PipelineOutcome::Ingested { relations, .. } => {
                assert!(relations.is_empty(), "Class A items must bypass NLI/LLM and create 0 relation edges");
            }
            other => panic!("Expected Ingested outcome for Class A item 2, got {:?}", other),
        }

        // Verify zero relations created in memory_relations DB table
        let mut rel_rows = conn.query("SELECT COUNT(*) FROM memory_relations", ()).await?;
        let rel_count: i64 = rel_rows.next().await?.unwrap().get(0)?;
        assert_eq!(rel_count, 0, "Class A ingestion must create 0 memory_relations rows in DB");

        // Verify facts were directly stored in memory_facts
        let mut fact_rows = conn.query("SELECT COUNT(*) FROM memory_facts WHERE collection IN ('Identity', 'Context')", ()).await?;
        let fact_count: i64 = fact_rows.next().await?.unwrap().get(0)?;
        assert_eq!(fact_count, 2, "Class A facts must be directly ingested into memory_facts");

        Ok(())
    }
}
