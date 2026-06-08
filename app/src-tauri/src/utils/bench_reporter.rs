use chrono::Local;
use serde::Serialize;
use std::fs;
use std::path::PathBuf;
use sysinfo::System;

#[derive(Serialize, Default, Debug, Clone)]
pub struct MemorySnapshot {
    pub rss_mb: u64,
    pub virt_mb: u64,
}

#[derive(Serialize, Default, Debug)]
pub struct BenchMetrics {} // Deprecated, keeping as placeholder if needed elsewhere

pub struct BenchReporter {
    pub run_dir: PathBuf,
}

impl BenchReporter {
    pub fn new() -> Self {
        let timestamp = Local::now().format("%Y%m%d_%H%M%S").to_string();
        let mut run_dir = PathBuf::from("outputs");
        run_dir.push(format!("run_{}", timestamp));

        fs::create_dir_all(&run_dir).expect("Failed to create run directory");

        Self { run_dir }
    }

    pub fn write_artifact(&self, filename: &str, content: &str) {
        let path = self.run_dir.join(filename);
        fs::write(path, content).expect("Failed to write artifact");
    }

    pub fn save_report(&self, report: serde_json::Value) {
        let path = self.run_dir.join("metrics.json");
        let json = serde_json::to_string_pretty(&report).expect("Failed to serialize report");
        fs::write(path, json).expect("Failed to write metrics.json");
    }

    pub fn get_memory_snapshot() -> MemorySnapshot {
        let mut sys = System::new_all();
        sys.refresh_all();
        let pid = sysinfo::get_current_pid().expect("Failed to get PID");
        if let Some(process) = sys.process(pid) {
            MemorySnapshot {
                rss_mb: process.memory() / 1024 / 1024,
                virt_mb: process.virtual_memory() / 1024 / 1024,
            }
        } else {
            MemorySnapshot::default()
        }
    }

    // Memory peak tracking moved to background thread in vox-bench.rs
}
