//! ============================================================================
//! evals/common/stage_dump.rs — Per-stage artifact persistence, run manifest and results mirror
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
use serde_json::Value;

pub mod names {
    pub const STAGE_1_FANOUT: &str = "stage_1_fanout.json";
    pub const STAGE_1B_FETCH: &str = "stage_1b_fetch.json";
    pub const STAGE_1C_EXTRACT: &str = "stage_1c_extract.json";
    pub const STAGE_2_CHUNK: &str = "stage_2_chunk.json";
    pub const STAGE_2_RANK: &str = "stage_2_rank.json";
    pub const RAW_NEXUS_METRICS: &str = "raw_nexus_metrics.json";
    pub const EVIDENCE_XML: &str = "evidence.xml";
    pub const EVIDENCE_TXT: &str = "evidence.txt";
    pub const METRICS: &str = "metrics.json";
    pub const MANIFEST: &str = "manifest.json";
    pub const SUMMARY_JSON: &str = "summary.json";
    pub const SUMMARY_MD: &str = "summary.md";
    pub const QA_REPORT: &str = "qa_report.md";
    pub const RENDER_LOG: &str = "render_log.json";
    pub const AUDIO_SUMMARY: &str = "audio_summary.json";
    pub const CONTEXT_LLM_SAW: &str = "context_llm_saw.json";
    pub const REQUESTS_DIR: &str = "requests";
    pub const EVIDENCE: &str = "evidence.xml";
    pub const PIPELINE_EVENTS: &str = "pipeline_events.json";
    pub const TOOL_CALLS: &str = "tool_calls.json";
}

pub fn write_json(dir: &Path, filename: &str, value: &Value) -> Result<PathBuf> {
    let path = dir.join(filename);
    let body = serde_json::to_string_pretty(value)
        .with_context(|| format!("Failed to serializing {}", filename))?;
    fs::write(&path, body).with_context(|| format!("Failed to write {:?}", path))?;
    Ok(path)
}

/// Written with no transformation so the file is byte-comparable with what the model saw.
pub fn write_verbatim(dir: &Path, filename: &str, body: &str) -> Result<PathBuf> {
    let path = dir.join(filename);
    fs::write(&path, body)
        .with_context(|| format!("Failed to write {} to {:?}", filename, path))?;
    Ok(path)
}

pub fn read_json(dir: &Path, filename: &str) -> Result<Value> {
    let path = dir.join(filename);
    let body = fs::read_to_string(&path).with_context(|| format!("Failed to read {:?}", path))?;
    serde_json::from_str(&body).with_context(|| format!("Failed to parse {:?}", path))
}

pub fn build_run_manifest(
    eval_name: &str,
    run_id: &str,
    argv: &[String],
    extra: Value,
) -> Result<Value> {
    let git = |args: &[&str]| -> Option<String> {
        std::process::Command::new("git")
            .args(args)
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
    };
    let sha = git(&["rev-parse", "--short", "HEAD"]).unwrap_or_else(|| "unknown".to_string());
    let dirty = git(&["status", "--porcelain"])
        .map(|s| !s.is_empty())
        .unwrap_or(false);

    Ok(serde_json::json!({
        "eval": eval_name,
        "run_id": run_id,
        "timestamp_utc": chrono::Utc::now().to_rfc3339(),
        "created_utc": chrono::Utc::now().to_rfc3339(),
        "git": { "sha": sha, "dirty": dirty },
        "system_info": system_info(),
        "versions": { "vox_lib": env!("CARGO_PKG_VERSION") },
        "argv": argv,
        "config": extra,
    }))
}

/// OS, CPU count, physical RAM and this process's current RSS (§8.2).
pub fn system_info() -> Value {
    use sysinfo::{MemoryRefreshKind, RefreshKind, System};
    let mut sys =
        System::new_with_specifics(RefreshKind::new().with_memory(MemoryRefreshKind::everything()));
    sys.refresh_memory();
    serde_json::json!({
        "os": std::env::consts::OS,
        "arch": std::env::consts::ARCH,
        "cpu_count": std::thread::available_parallelism().map(|n| n.get()).unwrap_or(0),
        "physical_ram_bytes": sys.total_memory(),
        "available_ram_bytes": sys.available_memory(),
        "process_rss_bytes": process_rss_bytes(),
    })
}

/// Reads this process's RSS directly from /proc, avoiding a full System refresh.
pub fn process_rss_bytes() -> u64 {
    std::fs::read_to_string("/proc/self/statm")
        .ok()
        .and_then(|s| {
            s.split_whitespace()
                .nth(1)
                .and_then(|pages| pages.parse::<u64>().ok())
        })
        .map(|pages| pages * page_size())
        .unwrap_or(0)
}

fn page_size() -> u64 {
    // Linux reports statm in pages; 4 KiB is the page size on every target Vox ships.
    4096
}

/// Mirrors the run summary to `latest.json` in the tool's base results directory
/// (§8.2) so CI never has to scan run subdirectories.
pub fn write_latest(results_root: &Path, eval_name: &str, summary: &Value) -> Result<PathBuf> {
    let dir = results_root.join(eval_name);
    std::fs::create_dir_all(&dir)?;
    let path = dir.join("latest.json");
    std::fs::write(&path, serde_json::to_string_pretty(summary)?)
        .with_context(|| format!("Failed to write latest.json at {:?}", path))?;
    Ok(path)
}

/// Bounded preview that preserves the true length, so token accounting is never
/// derived from truncated data.
pub fn preview(text: &str, max_chars: usize) -> Value {
    let total = text.chars().count();
    if total <= max_chars {
        return serde_json::json!({ "text": text, "total_chars": total, "truncated": false });
    }
    serde_json::json!({
        "head": text.chars().take(max_chars).collect::<String>(),
        "tail": text.chars().skip(total.saturating_sub(max_chars / 4)).collect::<String>(),
        "total_chars": total,
        "truncated": true,
    })
}

pub fn estimate_tokens(text: &str) -> usize {
    vox_lib::services::memory::ml::estimate_tokens(text)
}

/// Naive host extraction for source-diversity counting only — not a security boundary.
pub fn apex_domain(url: &str) -> String {
    let rest = url.split_once("://").map(|(_, r)| r).unwrap_or(url);
    let host = rest.split(['/', '?', '#']).next().unwrap_or("");
    let host = host.rsplit_once('@').map(|(_, h)| h).unwrap_or(host);
    host.split(':')
        .next()
        .unwrap_or(host)
        .trim_start_matches("www.")
        .to_lowercase()
}

pub fn require_artifacts(dir: &Path, filenames: &[&str]) -> Result<()> {
    let missing: Vec<&str> = filenames
        .iter()
        .copied()
        .filter(|f| !dir.join(f).exists())
        .collect();
    if !missing.is_empty() {
        return Err(anyhow!(
            "Missing artifacts in {:?}: {:?}. Run is incomplete; its metrics are not trustworthy.",
            dir,
            missing
        ));
    }
    Ok(())
}
