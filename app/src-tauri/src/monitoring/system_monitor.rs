use tauri::{AppHandle, Emitter, Manager};
use sysinfo::System;

pub fn spawn_system_monitor(app: AppHandle) {
    let state_arc: std::sync::Arc<crate::core::state::AppState> = app.state::<std::sync::Arc<crate::core::state::AppState>>().inner().clone();
    let telemetry_tx = state_arc.telemetry_tx.clone();
    let pid = sysinfo::get_current_pid().ok();

    tauri::async_runtime::spawn(async move {
        tracing::info!("[Telemetry] Dual-Stream System monitor started (Throttled: 10s).");
        let mut sys = System::new_all();
        
        loop {
            // High Throttle as per Architect Correction: 30000ms (30 seconds)
            tokio::time::sleep(std::time::Duration::from_millis(30000)).await;
            
            sys.refresh_all();
            
            // 1. Global System Metrics
            let system_cpu = sys.global_cpu_info().cpu_usage();
            let system_ram_pct = (sys.used_memory() as f32 / sys.total_memory() as f32) * 100.0;
            
            // 2. Vox Process Metrics (recursive aggregation of parent + child Webview processes)
            let (vox_cpu, vox_ram_mb, thread_count): (f32, u32, u32) = if let Some(target_pid) = pid {
                let mut total_memory: u64 = 0;
                let mut total_cpu: f32 = 0.0;
                let mut total_threads: u32 = 0;

                for (&p_pid, proc) in sys.processes() {
                    #[cfg(target_os = "linux")]
                    {
                        if proc.tasks().is_none() {
                            continue;
                        }
                    }

                    let mut is_descendant = false;
                    
                    if p_pid == target_pid {
                        is_descendant = true;
                    } else {
                        let mut curr = proc;
                        while let Some(parent_pid) = curr.parent() {
                            if parent_pid == target_pid {
                                is_descendant = true;
                                break;
                            }
                            if let Some(parent_proc) = sys.process(parent_pid) {
                                curr = parent_proc;
                            } else {
                                break;
                            }
                        }
                    }

                    if is_descendant {
                        total_memory += proc.memory();
                        total_cpu += proc.cpu_usage();
                        total_threads += proc.tasks().map(|t| t.len()).unwrap_or(0) as u32;
                    }
                }

                (
                    total_cpu / sys.cpus().len() as f32,
                    (total_memory / 1024 / 1024) as u32,
                    total_threads,
                )
            } else {
                (0.0, 0, 0)
            };

            // Update Shared Atomics for real-time monitoring
            use std::sync::atomic::Ordering;
            state_arc.latest_sys_cpu.store(system_cpu.to_bits(), Ordering::Relaxed);
            state_arc.latest_sys_ram.store(system_ram_pct.to_bits(), Ordering::Relaxed);
            state_arc.latest_vox_cpu.store(vox_cpu.to_bits(), Ordering::Relaxed);
            state_arc.latest_vox_ram.store(vox_ram_mb, Ordering::Relaxed);
            state_arc.latest_threads.store(thread_count, Ordering::Relaxed);

            // Emit to IPC for real-time dashboard (Monitoring Page)
            let _ = app.emit("system_stats", serde_json::json!({
                "system_cpu": system_cpu,
                "system_ram_pct": system_ram_pct,
                "vox_cpu": vox_cpu,
                "vox_ram_mb": vox_ram_mb,
                "threads": thread_count,
                "total_memory_gb": sys.total_memory() / 1024 / 1024 / 1024,
                "cpu_count": sys.cpus().len(),
            }));

            // Send to aggregator for structured telemetry
            let _ = telemetry_tx.send(crate::monitoring::aggregator::TelemetryEvent::SystemHealth {
                system_cpu,
                system_ram_pct,
                vox_cpu,
                vox_ram_mb,
            });
        }
    });
}
