use std::path::PathBuf;
use std::fs;
use chrono::Local;
use serde::Serialize;
use sysinfo::System;

#[derive(Serialize, Default, Debug, Clone)]
pub struct MemorySnapshot {
    pub rss_mb: u64,
    pub virt_mb: u64,
}

#[derive(Serialize, Default, Debug)]
pub struct BenchMetrics {
    pub stt_latency_ms: u32,
    pub stt_rtf: f32,
    pub llm_ttft_ms: u32,
    pub llm_tps: f32,
    pub tts_latency_ms: u32,
    pub tts_rtf: f32,
    pub e2e_latency_ms: u32, // Time to first sound
    
    pub ram_start: MemorySnapshot,
    pub ram_peak: MemorySnapshot,
    pub ram_end: MemorySnapshot,
}

pub struct BenchReporter {
    pub run_dir: PathBuf,
    pub metrics: BenchMetrics,
}

impl BenchReporter {
    pub fn new() -> Self {
        let timestamp = Local::now().format("%Y%m%d_%H%M%S").to_string();
        let mut run_dir = PathBuf::from("outputs");
        run_dir.push(format!("run_{}", timestamp));
        
        fs::create_dir_all(&run_dir).expect("Failed to create run directory");

        Self {
            run_dir,
            metrics: BenchMetrics::default(),
        }
    }

    pub fn write_artifact(&self, filename: &str, content: &str) {
        let path = self.run_dir.join(filename);
        fs::write(path, content).expect("Failed to write artifact");
    }

    pub fn save_report(&self, latency_report: serde_json::Value) {
        let path = self.run_dir.join("metrics.json");
        
        let report = serde_json::json!({
            "latency": latency_report,
            "performance": {
                "stt_rtf": self.metrics.stt_rtf,
                "llm_tps": self.metrics.llm_tps,
                "tts_rtf": self.metrics.tts_rtf,
            },
            "resources": {
                "ram_start_mb": self.metrics.ram_start,
                "ram_peak_mb": self.metrics.ram_peak,
                "ram_end_mb": self.metrics.ram_end,
            }
        });

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
    
    pub fn update_peak_ram(&mut self) {
        let current = Self::get_memory_snapshot();
        if current.rss_mb > self.metrics.ram_peak.rss_mb {
            self.metrics.ram_peak = current;
        }
    }
}
