//! ============================================================================
//! evals/common/datasets.rs — Multi-turn Session Dataset Loader
//! ============================================================================

use std::{fs, path::PathBuf};

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};

/// Represents a single conversation turn from evaluation dataset JSON files.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionTurn {
    pub turn: u32,
    pub user: String,
    pub assistant: String,
}

/// Discovers candidate paths for the `sandbox/datasets/eval-sessions` directory.
fn resolve_eval_sessions_dir() -> Result<PathBuf> {
    let candidates = [
        PathBuf::from("sandbox/datasets/eval-sessions"),
        PathBuf::from("../../sandbox/datasets/eval-sessions"),
        PathBuf::from("../../../sandbox/datasets/eval-sessions"),
        PathBuf::from("evals/assets/datasets"),
    ];

    for c in &candidates {
        if c.is_dir() {
            return Ok(c.clone());
        }
    }

    Err(anyhow!(
        "Failed to locate eval-sessions directory in candidate paths: {:?}",
        candidates
    ))
}

/// Resolves the file for a given case index (1..=14).
pub fn resolve_case_file(case_idx: usize) -> Result<(String, PathBuf)> {
    let base = resolve_eval_sessions_dir()?;
    let prefix = format!("case_{:02}", case_idx);

    let entries = fs::read_dir(&base)
        .map_err(|e| anyhow!("Failed to read eval-sessions directory {:?}: {}", base, e))?;

    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with(&prefix) && name.ends_with(".json") {
            return Ok((name, entry.path()));
        }
    }

    Err(anyhow!(
        "Case index {} not found with prefix '{}' in {:?}",
        case_idx,
        prefix,
        base
    ))
}

/// Loads turns for a specific evaluation case by index (1..=14).
pub fn load_eval_case(case_idx: usize) -> Result<(String, Vec<SessionTurn>)> {
    let (case_name, case_path) = resolve_case_file(case_idx)?;
    let content = fs::read_to_string(&case_path)
        .map_err(|e| anyhow!("Failed to read case file {:?}: {}", case_path, e))?;

    let turns: Vec<SessionTurn> = serde_json::from_str(&content).map_err(|e| {
        anyhow!(
            "Failed to parse case turns JSON from {:?}: {}",
            case_path,
            e
        )
    })?;

    Ok((case_name, turns))
}
