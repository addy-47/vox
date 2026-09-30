//! ============================================================================
//! evals/common/reporting.rs — Evaluation Reporting & Run Directory Management
//! ============================================================================

use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{anyhow, Result};

/// Generates a timestamped evaluation run ID: `YYYYMMDD_HHMMSS_<short_uuid>`.
pub fn generate_run_id() -> String {
    let now = chrono::Utc::now();
    let uuid_str = uuid::Uuid::new_v4().to_string();
    let short_uuid = &uuid_str[..8];
    format!("{}_{}", now.format("%Y%m%d_%H%M%S"), short_uuid)
}

/// Creates the base run directory `<base_dir>/<run_id>`.
pub fn create_run_directory(base_dir: &Path, run_id: &str) -> Result<PathBuf> {
    let run_dir = base_dir.join(run_id);
    fs::create_dir_all(&run_dir).map_err(|e| {
        anyhow!(
            "Failed to create evaluation run directory at {:?}: {}",
            run_dir,
            e
        )
    })?;
    Ok(run_dir)
}

/// Creates a sub-directory for a specific case: `<run_dir>/case_XX`.
pub fn create_case_directory(run_dir: &Path, case_idx: usize) -> Result<PathBuf> {
    let case_dir = run_dir.join(format!("case_{:02}", case_idx));
    fs::create_dir_all(&case_dir).map_err(|e| {
        anyhow!(
            "Failed to create case directory at {:?}: {}",
            case_dir,
            e
        )
    })?;
    Ok(case_dir)
}

/// Writes a Markdown report file in `<case_dir>/<filename>`.
pub fn write_markdown_report(case_dir: &Path, filename: &str, content: &str) -> Result<PathBuf> {
    let file_path = case_dir.join(filename);
    fs::write(&file_path, content).map_err(|e| {
        anyhow!(
            "Failed to write markdown report to {:?}: {}",
            file_path,
            e
        )
    })?;
    Ok(file_path)
}
