use serde::{Deserialize, Serialize};

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
