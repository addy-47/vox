use anyhow::Result;
use turso::Connection;
use crate::core::constants::{
    inter_collection_edge, PM_QUEUE_STATUS_EMBEDDED, PM_QUEUE_STATUS_EVALUATED, PM_QUEUE_STATUS_PROCESSING_EVAL,
    PM_QUEUE_STATUS_SUPERSEDED, PM_RELATION_CONFLICTS, PM_RELATION_SUPERSEDES, PM_RELATION_SUPPORTS,
    PM_SEMANTIC_GRAPH_COLLECTIONS,
};
use crate::persistence::{decode_f32_blob, queries};
use crate::services::memory::edge_classifier;
use crate::services::memory::nli::{classify_pair, ensure_nli_loaded, relation_from_result, NliRelation, NLI_MODEL_DIR};
use super::batch_result::{BatchEvaluationResult, RelationEdge};

pub const STAGE3_BATCH_SIZE: usize = 16;
pub const SAME_COLLECTION_CANDIDATE_SEARCH: f32 = 0.40;
pub const INTER_COLLECTION_CANDIDATE_SEARCH: f32 = 0.55;

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
    nli_candidates: &[(String, String)],
) -> Vec<RelationEdge> {
    let mut relations = Vec::new();
    let is_nli_domain = matches!(
        item.collection.as_str(),
        "Identity" | "Directives" | "Constraints"
    );
    if nli_candidates.is_empty() || !is_nli_domain {
        return relations;
    }

    if ensure_nli_loaded(NLI_MODEL_DIR).is_err() {
        return relations;
    }

    for (cand_id, cand_fact) in nli_candidates {
        if let Ok(nli_res) = classify_pair(&item.fact, cand_fact) {
            log::debug!(
                "[Stage3Eval NLI] Premise: '{}' | Hypothesis: '{}' | Res: c={:.3}, e={:.3}, n={:.3}",
                item.fact,
                cand_fact,
                nli_res.contradiction,
                nli_res.entailment,
                nli_res.neutral
            );
            let relation = relation_from_result(&nli_res);
            match relation {
                NliRelation::Conflicts => {
                    let (fwd, inv) = if item.collection == "Identity" || item.collection == "Directives" {
                        (PM_RELATION_SUPERSEDES, "superseded_by")
                    } else {
                        (PM_RELATION_CONFLICTS, "conflicts_with")
                    };
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
                }
                NliRelation::Supports => {
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
                }
                NliRelation::Neutral => {}
            }
        }
    }

    relations
}

/// Synchronous CPU worker function for Sub-Branch B (ModernBERT Edge Classifier Engine).
fn eval_subbranch_b_edges_sync(
    item: &Stage3Item,
    edge_candidates: &[(String, String, String)],
) -> Vec<RelationEdge> {
    let mut relations = Vec::new();
    if edge_candidates.is_empty() {
        return relations;
    }

    for (cand_id, cand_fact, cand_coll) in edge_candidates {
        if let Some((forward_edge, inverse_edge)) = inter_collection_edge(&item.collection, cand_coll) {
            match edge_classifier::classify_edge(
                &item.collection,
                &item.fact,
                None,
                cand_coll,
                cand_fact,
                None,
                forward_edge,
            ) {
                Ok(Some(pred_edge)) => {
                    relations.push(RelationEdge {
                        from_id: format!("item_{}", item.id),
                        to_id: cand_id.clone(),
                        relation: pred_edge.to_string(),
                        source: "LLM".to_string(),
                    });
                    relations.push(RelationEdge {
                        from_id: cand_id.clone(),
                        to_id: format!("item_{}", item.id),
                        relation: inverse_edge.to_string(),
                        source: "LLM".to_string(),
                    });
                }
                Ok(None) | Err(_) => {}
            }
        }
    }

    relations
}

/// Stage 3: Unified Edge & State Evaluation Stage (Batch Size 16)
/// Atomically claims `embedded` items, offloads CPU ONNX tasks to `spawn_blocking` threads,
/// runs Sub-Branch A (NLI) and Sub-Branch B (Edge Classifier) concurrently via `tokio::join!`,
/// merges output into `BatchEvaluationResult`, and commits atomic `status = 'evaluated'`.
pub async fn run_stage3_eval(conn: &Connection) -> Result<usize> {
    run_stage3_eval_with_metrics(conn, "").await
}

pub async fn run_stage3_eval_with_metrics(conn: &Connection, run_id: &str) -> Result<usize> {
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
        let vec_blob: Vec<u8> = row.get(4)?;
        candidate_items.push(Stage3Item {
            id: row.get::<i64>(0)?,
            fact: row.get::<String>(1)?,
            collection: row.get::<String>(2)?,
            session_id: row.get::<String>(3)?,
            vector: decode_f32_blob(&vec_blob),
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
    let session_id = items.first().map(|i| i.session_id.clone()).unwrap_or_default();

    let mut processed_count = 0;
    let mut superseded_count = 0;
    let mut total_relations_created = 0;

    for item in items {
        // Fetch candidates for Sub-Branch A and Sub-Branch B without K-cap (Spec Behavioral Invariants #3)
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
            .filter(|&tgt| inter_collection_edge(&item.collection, tgt).is_some())
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
        let handle_a = tokio::task::spawn_blocking(move || eval_subbranch_a_nli_sync(&item_a, &cand_a));

        let item_b = item.clone();
        let cand_b = edge_candidates.clone();
        let handle_b = tokio::task::spawn_blocking(move || eval_subbranch_b_edges_sync(&item_b, &cand_b));

        let (nli_res, edge_res) = tokio::join!(handle_a, handle_b);

        let mut all_relations = nli_res.unwrap_or_else(|_| Vec::new());
        all_relations.extend(edge_res.unwrap_or_else(|_| Vec::new()));

        total_relations_created += all_relations.len();

        let item_target_id = format!("item_{}", item.id);
        let is_superseded = all_relations.iter().any(|rel| rel.to_id == item_target_id && rel.relation == PM_RELATION_SUPERSEDES);

        if is_superseded {
            superseded_count += 1;
        }

        let eval_result = BatchEvaluationResult {
            item_id: item.id,
            is_superseded,
            superseded_by: None,
            relations: all_relations,
        };

        let json_str = serde_json::to_string(&eval_result.relations).unwrap_or_else(|_| "[]".to_string());
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

        processed_count += 1;
    }

    let duration_ms = start_time.elapsed().as_millis();

    if !run_id.is_empty() {
        let metrics = super::metrics::PipelineStageMetrics {
            run_id: run_id.to_string(),
            stage_name: "stage3_eval".to_string(),
            session_id,
            items_claimed,
            items_processed: processed_count,
            items_superseded: superseded_count,
            relations_created: total_relations_created,
            duration_ms,
            error_count: 0,
        };
        let _ = crate::persistence::mutations::record_stage_metrics(conn, &metrics).await;
    }

    Ok(processed_count)
}
