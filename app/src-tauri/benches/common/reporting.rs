//! ============================================================================
//! benches/common/reporting.rs — Structured Benchmark Results Persistence
//! ============================================================================

use std::{
    fs,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use sysinfo::{MemoryRefreshKind, ProcessRefreshKind, RefreshKind, System};

/// Returns active process memory (RSS) in megabytes.
pub fn get_process_memory_mb() -> u64 {
    let mut sys = System::new_with_specifics(
        RefreshKind::new().with_processes(ProcessRefreshKind::new().with_memory()),
    );
    let pid = sysinfo::get_current_pid().ok();
    if let Some(pid) = pid {
        if sys.refresh_process(pid) {
            if let Some(process) = sys.process(pid) {
                return process.memory() / 1024 / 1024;
            }
        }
    }
    0
}

/// System hardware and environment snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkSystemInfo {
    pub os: String,
    pub cpu_cores: usize,
    pub total_memory_mb: u64,
}

impl Default for BenchmarkSystemInfo {
    fn default() -> Self {
        let mut sys = System::new_with_specifics(
            RefreshKind::new().with_memory(MemoryRefreshKind::everything()),
        );
        sys.refresh_memory();
        Self {
            os: std::env::consts::OS.to_string(),
            cpu_cores: std::thread::available_parallelism()
                .map(|p| p.get())
                .unwrap_or(1),
            total_memory_mb: sys.total_memory() / 1024 / 1024,
        }
    }
}

/// Per-clip benchmark measurement.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClipBenchmarkResult {
    pub filename: String,
    pub lang: String,
    pub duration_s: f32,
    pub total_stream_time_ms: f64,
    pub final_post_speech_latency_ms: f64,
    pub rtf: f64,
    pub throughput_spl_s: f64,
    pub partials_emitted: usize,
    pub similarity: f64,
    pub hypothesis: String,
    pub ground_truth: String,
}

/// Aggregate summary for an engine execution run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EngineBenchmarkRun {
    pub engine_name: String,
    pub model_type: String,
    pub model_path: String,
    pub memory_rss_mb: u64,
    pub total_audio_s: f32,
    pub total_stream_time_ms: f64,
    pub avg_post_speech_latency_ms: f64,
    pub avg_rtf: f64,
    pub overall_throughput_spl_s: f64,
    pub avg_similarity: f64,
    pub clips: Vec<ClipBenchmarkResult>,
}

/// Comprehensive benchmark report artifact.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkReport {
    pub run_id: String,
    pub timestamp_utc: String,
    pub benchmark_name: String,
    pub system_info: BenchmarkSystemInfo,
    pub runs: Vec<EngineBenchmarkRun>,
}

/// Generates a timestamped run ID in the format `YYYYMMDD_HHMMSS_<short_uuid>`.
pub fn generate_run_id() -> String {
    let now = chrono::Utc::now();
    let uuid_str = uuid::Uuid::new_v4().to_string();
    let short_uuid = &uuid_str[..8];
    format!("{}_{}", now.format("%Y%m%d_%H%M%S"), short_uuid)
}

/// Saves the benchmark report artifact into `<base_dir>/<run_id>/report.json` and updates `<base_dir>/latest.json`.
pub fn save_benchmark_report(base_dir: &Path, report: &BenchmarkReport) -> Result<PathBuf, String> {
    let run_dir = base_dir.join(&report.run_id);
    fs::create_dir_all(&run_dir).map_err(|e| {
        format!(
            "Failed to create benchmark result dir at {:?}: {}",
            run_dir, e
        )
    })?;

    let report_path = run_dir.join("report.json");
    let json_data = serde_json::to_string_pretty(report)
        .map_err(|e| format!("Failed to serialize report JSON: {}", e))?;

    fs::write(&report_path, &json_data)
        .map_err(|e| format!("Failed to write report to {:?}: {}", report_path, e))?;

    // Also update/overwrite latest.json in base_dir
    let latest_path = base_dir.join("latest.json");
    let _ = fs::write(&latest_path, &json_data);

    Ok(report_path)
}
