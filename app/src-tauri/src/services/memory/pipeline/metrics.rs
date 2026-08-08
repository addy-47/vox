use serde::{Deserialize, Serialize};

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
