pub mod runner;
pub mod stage1_dedup;
pub mod stage2_embed;
pub mod stage3_eval;
pub mod stage4_commit;

pub use runner::{
    drain_pipeline_queue, drain_pipeline_queue_with_run_id, run_pipeline_cycle,
    run_pipeline_cycle_with_id_seq,
};
use serde::{Deserialize, Serialize};
pub use stage1_dedup::run_stage1_dedup;
pub use stage2_embed::run_stage2_embed;
pub use stage3_eval::{run_stage3_eval, run_stage3_eval_with_metrics_seq};
pub use stage4_commit::run_stage4_commit;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RelationEdge {
    pub from_id: String,
    pub to_id: String,
    pub relation: String,
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DedupAuditLog {
    pub queue_item_id: i64,
    pub item_fact: String,
    pub item_collection: String,
    pub stage: String,           // "stage1_jaccard" | "stage2_soft_vector"
    pub action: String, // "duplicate_dropped" | "superseded_lower_priority" | "superseded_existing"
    pub matched_fact_id: String, // available for both stages
    pub matched_fact_coll: String,
    pub matched_fact: String,
    pub score: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CandidateAuditLog {
    pub item_id: i64,
    pub item_fact: String,
    pub item_collection: String,
    pub cand_id: String,
    pub cand_fact: String,
    pub cand_collection: String,
    pub candidate_source: String, // "memory_facts" | "queue_in_flight" | "subfloor"
    pub cosine_sim: f32,
    pub engine: String,               // "NLI" | "ModernBERT" | "subfloor"
    pub nli_scores: Option<[f32; 3]>, // [contradiction, entailment, neutral] — NLI branch only
    pub edge_score: Option<f32>,      // ModernBERT confidence — ModernBERT branch only
    pub decision: String,
    pub rejection_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct BatchEvaluationResult {
    pub item_id: i64,
    pub is_superseded: bool,
    pub superseded_by: Option<String>,
    pub relations: Vec<RelationEdge>,
    pub candidate_logs: Vec<CandidateAuditLog>,
}

/// Operational metrics recorded for each pipeline stage execution run.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct PipelineStageMetrics {
    pub run_id: String,
    pub stage_name: String,
    pub session_id: String,
    pub batch_seq: usize,
    pub items_claimed: usize,
    pub error_count: usize,
    pub duration_ms: u128,
}
