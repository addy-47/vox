use serde::{Deserialize, Serialize};

/// Operational metrics recorded for each pipeline stage execution run.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct PipelineStageMetrics {
    pub run_id: String,
    pub stage_name: String,
    pub session_id: String,
    pub items_claimed: usize,
    pub items_processed: usize,
    pub items_superseded: usize,
    pub relations_created: usize,
    pub duration_ms: u128,
    pub error_count: usize,
}
