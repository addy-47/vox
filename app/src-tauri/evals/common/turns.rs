//! ============================================================================
//! turns.rs — Frozen turn fixture loader for the memory-ladder evals
//! ============================================================================
//! Category     : Evaluation (shared harness, not a runnable eval)
//! Component    : evals/common
//! Prerequisites: evals/datasets/turns_session2.json
//! Execution    : Included via #[path] from evals/memory_*_eval.rs
//! Metrics      : N/A
//! ============================================================================

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

/// One conversation turn from the frozen eval fixture.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DatasetTurn {
    pub turn: u32,
    pub user: String,
    pub assistant: String,
}

/// Loads the frozen 100-turn session fixture pinned under evals/.
pub fn load_session_turns() -> Result<Vec<DatasetTurn>> {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/evals/datasets/turns_session2.json"
    );
    let raw = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read turn fixture at {path}"))?;
    serde_json::from_str(&raw).context("Failed to parse turn fixture JSON")
}

/// Loads the 300-turn session used as the trip feed: at the 8192 floor the
/// 85% line needs ~6700 tracked tokens, which the 100-turn fixture cannot
/// reach. Turns are fed in order until the genuine trip; leftovers are omitted.
pub fn load_trip_turns() -> Result<Vec<DatasetTurn>> {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/evals/datasets/dataset_session_2.json"
    );
    let raw = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read trip fixture at {path}"))?;
    serde_json::from_str(&raw).context("Failed to parse trip fixture JSON")
}
