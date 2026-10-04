//! ============================================================================
//! evals/common/reporting.rs — Run/case directories, report and summary writing
//! ============================================================================
//! Category     : Utility Module
//! Component    : evals harness
//! Prerequisites: see evals/README.md
//! Execution    : cargo bench --bench agentic_tool_eval --release -- --help
//! Metrics      : see summary.md in the run directory
//! ============================================================================

use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{anyhow, Context, Result};

pub fn generate_run_id() -> String {
    let now = chrono::Utc::now();
    format!(
        "{}_{}",
        now.format("%Y%m%d_%H%M%S"),
        &uuid::Uuid::new_v4().to_string()[..8]
    )
}

pub fn create_run_directory(base_dir: &Path, run_id: &str) -> Result<PathBuf> {
    let d = base_dir.join(run_id);
    fs::create_dir_all(&d).with_context(|| format!("Failed to create {:?}", d))?;
    Ok(d)
}

pub fn create_case_directory(run_dir: &Path, case_name: &str) -> Result<PathBuf> {
    let d = run_dir.join(case_name);
    fs::create_dir_all(&d).with_context(|| format!("Failed to create {:?}", d))?;
    Ok(d)
}

pub fn write_markdown_report(case_dir: &Path, filename: &str, content: &str) -> Result<PathBuf> {
    let p = case_dir.join(filename);
    fs::write(&p, content).with_context(|| format!("Failed to write {:?}", p))?;
    Ok(p)
}

pub fn write_report(
    eval_name: &str,
    run_id: &str,
    payload: &serde_json::Value,
) -> Result<Vec<PathBuf>> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("evals/results")
        .join(eval_name)
        .join(run_id)
        .join("report.json");
    let body = serde_json::to_string_pretty(payload)?;
    if let Some(p) = path.parent() {
        fs::create_dir_all(p)?;
    }
    fs::write(&path, body).with_context(|| format!("Failed to write {:?}", path))?;
    Ok(vec![path])
}

pub fn markdown_table(headers: &[&str], rows: &[String]) -> String {
    if rows.is_empty() {
        return String::new();
    }
    let sep: Vec<&str> = headers.iter().map(|_| "---").collect();
    let mut out = format!("| {} |\n| {} |\n", headers.join(" | "), sep.join(" | "));
    for r in rows {
        out.push_str(&format!("| {} |\n", r));
    }
    out
}

pub fn write_summary_markdown(
    run_dir: &Path,
    title: &str,
    preamble: &str,
    sections: &[(String, String)],
) -> Result<PathBuf> {
    let mut out = format!("# {}\n\n", title);
    if !preamble.trim().is_empty() {
        out.push_str(preamble.trim());
        out.push_str("\n\n");
    }
    for (h, b) in sections {
        if b.trim().is_empty() {
            continue;
        }
        out.push_str(&format!("## {}\n\n{}\n\n", h, b.trim()));
    }
    write_markdown_report(run_dir, "summary.md", &out)
}

/// Anchored to the manifest dir so invocation from the repo root and from
/// `app/src-tauri/` land in the same place.
pub fn resolve_output_dir(explicit: Option<&Path>, eval_name: &str) -> PathBuf {
    match explicit {
        Some(p) => p.to_path_buf(),
        None => PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("evals/results")
            .join(eval_name),
    }
}

pub fn require_dir(path: &Path) -> Result<()> {
    if !path.is_dir() {
        return Err(anyhow!("Expected directory {:?} does not exist", path));
    }
    Ok(())
}
