use super::batch_result::{BatchEvaluationResult, CandidateAuditLog, RelationEdge};
use crate::core::constants::{
    is_valid_inter_collection_pair, PM_QUEUE_STATUS_EMBEDDED, PM_QUEUE_STATUS_EVALUATED,
    PM_QUEUE_STATUS_PROCESSING_EVAL, PM_QUEUE_STATUS_SUPERSEDED, PM_RELATION_CONFLICTS,
    PM_RELATION_SUPERSEDES, PM_RELATION_SUPPORTS, PM_SEMANTIC_GRAPH_COLLECTIONS,
};
use crate::persistence::{decode_f32_blob, queries};
use crate::services::memory::classifiers::inter_edge_classifier;
use crate::services::memory::classifiers::intra_edge_classifier::{
    classify_batch, ensure_nli_loaded, relation_from_result, NliRelation, NLI_MODEL_DIR,
};
use anyhow::Result;
use turso::Connection;

pub const STAGE3_BATCH_SIZE: usize = 16;
pub const SAME_COLLECTION_CANDIDATE_SEARCH: f32 = 0.60;  // Intra-collection NLI requires high similarity
pub const INTER_COLLECTION_CANDIDATE_SEARCH: f32 = 0.40; // Inter-collection cross-domain edges have lower similarity
pub const SUBFLOOR_CANDIDATE_FLOOR: f32 = 0.25;

pub const NLI_CONTRADICTION_CONFIDENCE_THRESHOLD: f32 = 0.85;
pub const NLI_CONTRADICTION_MARGIN_THRESHOLD: f32 = 0.20;
pub const NLI_ENTAILMENT_CONFIDENCE_THRESHOLD: f32 = 0.85;

#[derive(Debug, Clone)]
pub struct Stage3Item {
    pub id: i64,
    pub fact: String,
    pub collection: String,
    pub session_id: String,
    pub vector: Vec<f32>,
}

/// Synchronous CPU worker function for Sub-Branch A (DeBERTa-v3 NLI Engine).
fn eval_subbranch_a_nli_sync(
    item: &Stage3Item,
    nli_candidates: &[(String, String, f32)],
) -> (Vec<RelationEdge>, Vec<CandidateAuditLog>) {
    let mut relations = Vec::new();
    let mut logs = Vec::new();
    let is_nli_domain = matches!(
        item.collection.as_str(),
        "Identity" | "Directives" | "Constraints"
    );
    if nli_candidates.is_empty() || !is_nli_domain {
        return (relations, logs);
    }

    if ensure_nli_loaded(NLI_MODEL_DIR).is_err() {
        return (relations, logs);
    }

    let pairs: Vec<(&str, &str)> = nli_candidates
        .iter()
        .map(|(_, cand_fact, _)| (cand_fact.as_str(), item.fact.as_str()))
        .collect();

    let nli_results = match classify_batch(&pairs) {
        Ok(res) => res,
        Err(_) => return (relations, logs),
    };

    for ((cand_id, cand_fact, sim), nli_res) in nli_candidates.iter().zip(nli_results.iter()) {
        log::debug!(
            "[Stage3Eval NLI] Premise (old): '{}' | Hypothesis (new): '{}' | Res: c={:.3}, e={:.3}, n={:.3}",
            cand_fact,
            item.fact,
            nli_res.contradiction,
            nli_res.entailment,
            nli_res.neutral
        );
        let relation = relation_from_result(nli_res);
        let mut decision = "NONE".to_string();
        let mut rejection_reason = None;

        match relation {
            NliRelation::Conflicts => {
                let confident_score = nli_res.contradiction
                    >= NLI_CONTRADICTION_CONFIDENCE_THRESHOLD
                    && (nli_res.contradiction - nli_res.neutral)
                        >= NLI_CONTRADICTION_MARGIN_THRESHOLD;

                if confident_score {
                    let (fwd, inv) =
                        if item.collection == "Identity" || item.collection == "Directives" {
                            (PM_RELATION_SUPERSEDES, "superseded_by")
                        } else {
                            (PM_RELATION_CONFLICTS, "conflicts_with")
                        };
                    decision = fwd.to_string();
                    relations.push(RelationEdge {
                        from_id: format!("item_{}", item.id),
                        to_id: cand_id.clone(),
                        relation: fwd.to_string(),
                        source: "NLI".to_string(),
                    });
                    relations.push(RelationEdge {
                        from_id: cand_id.clone(),
                        to_id: format!("item_{}", item.id),
                        relation: inv.to_string(),
                        source: "NLI".to_string(),
                    });
                } else {
                    rejection_reason = Some("below_nli_confidence".to_string());
                }
            }
            NliRelation::Supports => {
                if nli_res.entailment >= NLI_ENTAILMENT_CONFIDENCE_THRESHOLD {
                    decision = PM_RELATION_SUPPORTS.to_string();
                    relations.push(RelationEdge {
                        from_id: format!("item_{}", item.id),
                        to_id: cand_id.clone(),
                        relation: PM_RELATION_SUPPORTS.to_string(),
                        source: "NLI".to_string(),
                    });
                    relations.push(RelationEdge {
                        from_id: cand_id.clone(),
                        to_id: format!("item_{}", item.id),
                        relation: "supported_by".to_string(),
                        source: "NLI".to_string(),
                    });
                } else {
                    rejection_reason = Some("below_nli_confidence".to_string());
                }
            }
            NliRelation::Neutral => {
                rejection_reason = Some("nli_neutral".to_string());
            }
        }

        let cand_source = if cand_id.starts_with("item_") {
            "queue_in_flight".to_string()
        } else {
            "memory_facts".to_string()
        };

        logs.push(CandidateAuditLog {
            item_id: item.id,
            item_fact: item.fact.clone(),
            item_collection: item.collection.clone(),
            cand_id: cand_id.clone(),
            cand_fact: cand_fact.clone(),
            cand_collection: item.collection.clone(),
            candidate_source: cand_source,
            cosine_sim: *sim,
            engine: "NLI".to_string(),
            nli_scores: Some([nli_res.contradiction, nli_res.entailment, nli_res.neutral]),
            edge_score: None,
            decision,
            rejection_reason,
        });
    }

    (relations, logs)
}

/// Synchronous CPU worker function for Sub-Branch B (ModernBERT Edge Classifier Engine).
fn eval_subbranch_b_edges_sync(
    item: &Stage3Item,
    edge_candidates: &[(String, String, String, f32)],
) -> (Vec<RelationEdge>, Vec<CandidateAuditLog>) {
    let mut relations = Vec::new();
    let mut logs = Vec::new();
    if edge_candidates.is_empty() {
        return (relations, logs);
    }

    for (cand_id, cand_fact, cand_coll, sim) in edge_candidates {
        if is_valid_inter_collection_pair(&item.collection, cand_coll) {
            let mut decision = "NONE".to_string();
            let mut rejection_reason = None;
            let mut edge_score_val = None;

            match inter_edge_classifier::classify_edge(
                &item.collection,
                &item.fact,
                None,
                cand_coll,
                cand_fact,
                None,
            ) {
                Ok((Some(pred_edge), score)) => {
                    edge_score_val = Some(score);
                    decision = pred_edge.clone();
                    let inv_edge = crate::core::constants::inverse_edge_for_relation(&pred_edge);
                    relations.push(RelationEdge {
                        from_id: format!("item_{}", item.id),
                        to_id: cand_id.clone(),
                        relation: pred_edge,
                        source: "ModernBERT".to_string(),
                    });
                    relations.push(RelationEdge {
                        from_id: cand_id.clone(),
                        to_id: format!("item_{}", item.id),
                        relation: inv_edge.to_string(),
                        source: "ModernBERT".to_string(),
                    });
                }
                Ok((None, score)) => {
                    edge_score_val = Some(score);
                    rejection_reason = Some("below_edge_classifier_confidence".to_string());
                }
                Err(_) => {
                    rejection_reason = Some("below_edge_classifier_confidence".to_string());
                }
            }

            let cand_source = if cand_id.starts_with("item_") {
                "queue_in_flight".to_string()
            } else {
                "memory_facts".to_string()
            };

            logs.push(CandidateAuditLog {
                item_id: item.id,
                item_fact: item.fact.clone(),
                item_collection: item.collection.clone(),
                cand_id: cand_id.clone(),
                cand_fact: cand_fact.clone(),
                cand_collection: cand_coll.clone(),
                candidate_source: cand_source,
                cosine_sim: *sim,
                engine: "ModernBERT".to_string(),
                nli_scores: None,
                edge_score: edge_score_val,
                decision,
                rejection_reason,
            });
        }
    }

    (relations, logs)
}

/// Stage 3: Unified Edge & State Evaluation Stage (Batch Size 16)
/// Atomically claims `embedded` items, offloads CPU ONNX tasks to `spawn_blocking` threads,
/// runs Sub-Branch A (NLI) and Sub-Branch B (Edge Classifier) concurrently via `tokio::join!`,
/// merges output into `BatchEvaluationResult`, and commits atomic `status = 'evaluated'`.
pub async fn run_stage3_eval(conn: &Connection) -> Result<usize> {
    run_stage3_eval_with_metrics(conn, "").await
}

pub async fn run_stage3_eval_with_metrics(conn: &Connection, run_id: &str) -> Result<usize> {
    run_stage3_eval_with_metrics_seq(conn, run_id, 0).await
}

pub async fn run_stage3_eval_with_metrics_seq(
    conn: &Connection,
    run_id: &str,
    batch_seq: usize,
) -> Result<usize> {
    let start_time = std::time::Instant::now();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;

    // 1. Select candidate embedded items
    let mut rows = conn
        .query(
            "SELECT id, fact, collection, session_id, vector FROM personal_memory_queue
             WHERE status = 'embedded' ORDER BY created_at ASC LIMIT ?",
            (STAGE3_BATCH_SIZE as i64,),
        )
        .await?;

    let mut candidate_items = Vec::new();
    while let Some(row) = rows.next().await? {
        let vec_blob_opt: Option<Vec<u8>> = row.get(4).unwrap_or(None);
        let vector = vec_blob_opt
            .map(|blob| decode_f32_blob(&blob))
            .unwrap_or_default();

        candidate_items.push(Stage3Item {
            id: row.get::<i64>(0)?,
            fact: row.get::<String>(1)?,
            collection: row.get::<String>(2)?,
            session_id: row.get::<String>(3)?,
            vector,
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
            (PM_QUEUE_STATUS_PROCESSING_EVAL, now, item.id, PM_QUEUE_STATUS_EMBEDDED),
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
    tracing::info!(
        "[MemoryPipeline] [Stage 3 NLI/Edge Eval] Claimed {} embedded items for DeBERTa/ModernBERT evaluation",
        items_claimed
    );
    let session_id = items
        .first()
        .map(|i| i.session_id.clone())
        .unwrap_or_default();

    let mut processed_count = 0;

    for item in &items {
        // Fetch candidates combining active memory_facts AND in-flight personal_memory_queue items
        let nli_candidates = queries::fetch_intra_collection_candidates(
            conn,
            &item.collection,
            &item.vector,
            SAME_COLLECTION_CANDIDATE_SEARCH,
            None,
        )
        .await
        .unwrap_or_default();

        let policy_targets: Vec<&'static str> = PM_SEMANTIC_GRAPH_COLLECTIONS
            .iter()
            .copied()
            .filter(|&tgt| is_valid_inter_collection_pair(&item.collection, tgt))
            .collect();

        let edge_candidates = if !policy_targets.is_empty() {
            queries::fetch_inter_collection_candidates(
                conn,
                &policy_targets,
                &item.vector,
                INTER_COLLECTION_CANDIDATE_SEARCH,
                None,
            )
            .await
            .unwrap_or_default()
        } else {
            Vec::new()
        };

        // Offload CPU inference to spawn_blocking threads for true concurrency
        let item_a = item.clone();
        let cand_a = nli_candidates.clone();
        let handle_a =
            tokio::task::spawn_blocking(move || eval_subbranch_a_nli_sync(&item_a, &cand_a));

        let item_b = item.clone();
        let cand_b = edge_candidates.clone();
        let handle_b =
            tokio::task::spawn_blocking(move || eval_subbranch_b_edges_sync(&item_b, &cand_b));

        let (res_a, res_b) = tokio::join!(handle_a, handle_b);

        let (nli_rels, nli_logs) = res_a.unwrap_or_else(|_| (Vec::new(), Vec::new()));
        let (edge_rels, edge_logs) = res_b.unwrap_or_else(|_| (Vec::new(), Vec::new()));

        let mut all_relations = nli_rels;
        all_relations.extend(edge_rels);

        let mut candidate_logs = nli_logs;
        candidate_logs.extend(edge_logs);

        let item_target_id = format!("item_{}", item.id);
        let is_superseded = all_relations
            .iter()
            .any(|rel| rel.to_id == item_target_id && rel.relation == PM_RELATION_SUPERSEDES);

        let eval_result = BatchEvaluationResult {
            item_id: item.id,
            is_superseded,
            superseded_by: None,
            relations: all_relations,
            candidate_logs,
        };

        let json_str =
            serde_json::to_string(&eval_result.relations).unwrap_or_else(|_| "[]".to_string());
        let new_status = if eval_result.is_superseded {
            PM_QUEUE_STATUS_SUPERSEDED
        } else {
            PM_QUEUE_STATUS_EVALUATED
        };

        conn.execute(
            "UPDATE personal_memory_queue SET status = ?, relations_json = ? WHERE id = ?",
            (new_status, json_str, item.id),
        )
        .await?;

        if !eval_result.candidate_logs.is_empty() {
            let _ = crate::persistence::mutations::write_candidate_audit(
                conn,
                item.id,
                &eval_result.candidate_logs,
            )
            .await;
        }

        processed_count += 1;
    }

    let duration_ms = start_time.elapsed().as_millis();

    if !run_id.is_empty() {
        let metrics = super::metrics::PipelineStageMetrics {
            run_id: run_id.to_string(),
            stage_name: "stage3_eval".to_string(),
            session_id,
            batch_seq,
            items_claimed,
            error_count: 0,
            duration_ms,
        };
        let _ = crate::persistence::mutations::record_stage_metrics(conn, &metrics).await;
    }

    Ok(processed_count)
}
