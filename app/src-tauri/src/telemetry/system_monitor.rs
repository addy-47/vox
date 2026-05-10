use tauri::{AppHandle, Emitter};
use std::time::Instant;
use crate::core::constants::SYSTEM_STATS_INTERVAL;

pub fn spawn_system_monitor(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        log::info!("[Telemetry] System monitor started.");
        let mut last_cpu_time: u64 = 0;
        let mut last_check = Instant::now();
        
        loop {
            tokio::time::sleep(SYSTEM_STATS_INTERVAL).await;
            
            let stats = std::fs::read_to_string("/proc/self/stat").ok();
            let cpu_usage = if let Some(s) = stats {
                let parts: Vec<&str> = s.split_whitespace().collect();
                if parts.len() > 14 {
                    let utime: u64 = parts[13].parse().unwrap_or(0);
                    let stime: u64 = parts[14].parse().unwrap_or(0);
                    let total = utime + stime;
                    let diff = if last_cpu_time > 0 { total - last_cpu_time } else { 0 };
                    last_cpu_time = total;
                    let elapsed = last_check.elapsed().as_secs_f32();
                    last_check = Instant::now();
                    (diff as f32 / 100.0) / elapsed * 100.0
                } else { 0.0 }
            } else { 0.0 };

            let status = std::fs::read_to_string("/proc/self/status").ok();
            let mem_rss_mb = if let Some(s) = status {
                s.lines()
                    .find(|l| l.starts_with("VmRSS:"))
                    .and_then(|l| l.split_whitespace().nth(1))
                    .and_then(|v| v.parse::<u64>().ok())
                    .map(|kb| kb / 1024)
                    .unwrap_or(0)
            } else { 0 };

            let _ = app.emit("system_stats", serde_json::json!({
                "cpu_usage": cpu_usage,
                "memory_used_mb": mem_rss_mb,
            }));
        }
    });
}
