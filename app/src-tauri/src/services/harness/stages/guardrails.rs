use std::hash::{Hash, Hasher};

use serde_json::Value;

use crate::services::harness::stages::tools::ToolExecutionOutcome;

/// Compact representation of a completed tool call for pattern and loop detection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolCallFingerprint {
    pub step_index: u32,
    pub tool_name: String,
    pub args_hash: u64,
    pub result_hash: u64,
    pub is_error: bool,
}

/// Turn-scoped guardrail state maintaining call fingerprints and a monotonic strike counter.
#[derive(Debug, Clone, Default)]
pub struct TurnGuardrailsState {
    pub fingerprints: Vec<ToolCallFingerprint>,
    pub strike_count: u8,
}

/// Action directive determined by guardrail evaluation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GuardrailAction {
    Proceed,
    Strike1Warning { warning_msg: String },
    Strike2CircuitBreak,
}

/// Guardrail engine for detecting non-productive tool loops and ping-pong patterns.
#[derive(Debug, Clone, Default)]
pub struct GuardrailsStage;

impl GuardrailsStage {
    /// Evaluates a completed batch of tool outcomes against the turn's sliding window.
    pub fn evaluate_batch(
        &self,
        _turn_id: u32,
        _step_index: u32,
        _state: &mut TurnGuardrailsState,
        _outcomes: &[ToolExecutionOutcome],
    ) -> GuardrailAction {
        GuardrailAction::Proceed
    }
}

/// Computes a deterministic 64-bit hash of a JSON value using canonical key ordering.
pub fn compute_canonical_json_hash(val: &Value) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    let canonical = serde_json::to_string(val).unwrap_or_else(|_| "{}".to_string());
    canonical.hash(&mut hasher);
    hasher.finish()
}

/// Computes a deterministic 64-bit hash of a string slice.
pub fn compute_str_hash(data: &str) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    data.hash(&mut hasher);
    hasher.finish()
}
