//! ============================================================================
//! report.rs — Run-id + structured JSON report writer for the memory ladder
//! ============================================================================
//! Category     : Evaluation (shared harness, not a runnable eval)
//! Component    : evals/common
//! Prerequisites: None
//! Execution    : Included via #[path] from evals/memory_*_eval.rs
//! Metrics      : N/A (writes evals/results/<eval>/<run_id>/)
//! ============================================================================

use anyhow::{Context, Result};
use serde_json::{json, Value};

/// Generates a `<YYYYMMDD_HHMMSS>_<short_uuid>` run id per style-guide §8.2.
pub fn new_run_id() -> String {
    let ts = chrono::Utc::now().format("%Y%m%d_%H%M%S").to_string();
    let uuid = uuid::Uuid::new_v4().to_string().replace('-', "");
    format!("{ts}_{}", &uuid[..8])
}

/// Collects OS / CPU / RAM facts for the report metadata block.
pub fn system_info() -> Value {
    use sysinfo::System;
    let mut sys = System::new_all();
    sys.refresh_all();
    let rss_mb = sysinfo::get_current_pid()
        .ok()
        .and_then(|pid| sys.process(pid))
        .map(|p| p.memory() as f64 / 1024.0 / 1024.0)
        .unwrap_or(0.0);
    json!({
        "os": System::long_os_version().unwrap_or_else(|| "unknown".to_string()),
        "cpu_cores": sys.cpus().len(),
        "total_ram_mb": sys.total_memory() as f64 / 1024.0 / 1024.0,
        "process_rss_mb": rss_mb,
    })
}

/// Reads an integer score from a `TAG: <0-100>` line, tolerating markdown
/// emphasis and surrounding prose on the same line.
pub fn extract_score(report: &str, tag: &str) -> Option<u32> {
    for line in report.lines() {
        let trimmed = line
            .trim()
            .trim_start_matches(['#', '*', '_', ' ', '-', '`']);
        let Some(rest) = trimmed.strip_prefix(tag) else {
            continue;
        };
        let digits: String = rest
            .trim_start_matches([':', ' ', '*', '`'])
            .chars()
            .take_while(|c| c.is_ascii_digit())
            .collect();
        if let Ok(score) = digits.parse::<u32>() {
            return Some(score.min(100));
        }
    }
    None
}

/// Collects the items of a repeated `TAG: <item>` block. `TAG: NONE` yields an
/// empty list. A missing tag yields an empty list too, which is deliberately
/// indistinguishable from "NONE" — callers must not read an absent tag as an
/// all-clear, only as an unproven claim.
pub fn extract_tagged_items(report: &str, tag: &str) -> Vec<String> {
    let mut items = Vec::new();
    let mut in_block = false;
    for line in report.lines() {
        let trimmed = line.trim();
        let is_tag = trimmed
            .trim_start_matches(['#', '*', '_', ' ', '-', '`'])
            .starts_with(tag);
        if is_tag {
            in_block = true;
            let value = trimmed
                .trim_start_matches(['#', '*', '_', ' ', '-', '`'])
                .strip_prefix(tag)
                .unwrap_or_default()
                .trim_start_matches([':', ' ', '*', '`'])
                .trim();
            if !value.is_empty() && !value.eq_ignore_ascii_case("none") {
                items.push(value.to_string());
            }
            continue;
        }
        // Continuation lines of a multi-item block are indented list items.
        if in_block && (trimmed.starts_with('-') || trimmed.starts_with('*')) && !trimmed.is_empty()
        {
            let value = trimmed.trim_start_matches(['-', '*', ' ']).trim();
            if !value.is_empty() && !value.eq_ignore_ascii_case("none") {
                items.push(value.to_string());
            }
        } else if !trimmed.is_empty() {
            in_block = false;
        }
    }
    items
}

/// Writes `report.json` under `results/<eval_name>/<run_id>/` and mirrors it
/// to `results/<eval_name>/latest.json`. Returns the run directory.
pub fn write_report(
    eval_name: &str,
    run_id: &str,
    mut payload: Value,
) -> Result<std::path::PathBuf> {
    let base = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("evals/results")
        .join(eval_name);
    let run_dir = base.join(run_id);
    std::fs::create_dir_all(&run_dir)
        .with_context(|| format!("Failed to create {}", run_dir.display()))?;

    let meta = payload
        .as_object_mut()
        .context("Report payload must be an object")?;
    meta.insert("run_id".to_string(), json!(run_id));
    meta.insert(
        "timestamp_utc".to_string(),
        json!(chrono::Utc::now().to_rfc3339()),
    );
    meta.insert("system_info".to_string(), system_info());

    let report_path = run_dir.join("report.json");
    std::fs::write(&report_path, serde_json::to_string_pretty(&payload)?)
        .with_context(|| format!("Failed to write {}", report_path.display()))?;
    std::fs::write(
        base.join("latest.json"),
        serde_json::to_string_pretty(&payload)?,
    )
    .context("Failed to write latest.json")?;
    Ok(run_dir)
}
