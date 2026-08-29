use crate::monitoring::SYSTEM_MONITOR_INTERVAL;
use std::sync::atomic::Ordering;
use sysinfo::{Pid, System};
use tauri::{AppHandle, Emitter, Manager};

/// Spawns the background system monitor task to collect and broadcast CPU and RAM statistics.
pub fn spawn_system_monitor(app: AppHandle) {
    let state_arc: std::sync::Arc<crate::core::state::AppState> = app
        .state::<std::sync::Arc<crate::core::state::AppState>>()
        .inner()
        .clone();
    let telemetry_tx = state_arc.telemetry.telemetry_tx.clone();
    let pid = sysinfo::get_current_pid().ok();

    tauri::async_runtime::spawn(async move {
        log::info!("[Monitoring::SystemMonitor] System monitor task started");
        let mut sys = System::new_all();

        loop {
            tokio::time::sleep(SYSTEM_MONITOR_INTERVAL).await;

            sys.refresh_all();

            let system_cpu = sys.global_cpu_info().cpu_usage();
            let system_ram_pct = (sys.used_memory() as f32 / sys.total_memory() as f32) * 100.0;
            let (vox_cpu, vox_ram_mb, thread_count) = collect_process_metrics(&sys, pid);

            update_shared_metrics(
                &state_arc,
                system_cpu,
                system_ram_pct,
                vox_cpu,
                vox_ram_mb,
                thread_count,
            );

            emit_system_stats(&app, &sys, system_cpu, system_ram_pct, vox_cpu, vox_ram_mb, thread_count);

            if let Err(e) = telemetry_tx.try_send(
                crate::monitoring::aggregator::TelemetryEvent::SystemHealth {
                    system_cpu,
                    system_ram_pct,
                    vox_cpu,
                    vox_ram_mb,
                },
            ) {
                log::warn!("[Monitoring::SystemMonitor] Failed to send SystemHealth to telemetry aggregator: {}", e);
            }
        }
    });
}

fn collect_process_metrics(sys: &System, pid: Option<Pid>) -> (f32, u32, u32) {
    let target_pid = match pid {
        Some(p) => p,
        None => return (0.0, 0, 0),
    };

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

        if is_descendant_process(sys, p_pid, target_pid) {
            total_memory += proc.memory();
            total_cpu += proc.cpu_usage();
            total_threads += proc.tasks().map(|t| t.len()).unwrap_or(0) as u32;
        }
    }

    let cpu_cores = sys.cpus().len().max(1) as f32;
    (
        total_cpu / cpu_cores,
        (total_memory / 1024 / 1024) as u32,
        total_threads,
    )
}

fn is_descendant_process(sys: &System, p_pid: Pid, target_pid: Pid) -> bool {
    if p_pid == target_pid {
        return true;
    }
    let mut curr = sys.process(p_pid);
    while let Some(proc) = curr {
        if let Some(parent_pid) = proc.parent() {
            if parent_pid == target_pid {
                return true;
            }
            curr = sys.process(parent_pid);
        } else {
            break;
        }
    }
    false
}

fn update_shared_metrics(
    state: &crate::core::state::AppState,
    system_cpu: f32,
    system_ram_pct: f32,
    vox_cpu: f32,
    vox_ram_mb: u32,
    thread_count: u32,
) {
    state
        .telemetry
        .latest_sys_cpu
        .store(system_cpu.to_bits(), Ordering::Relaxed);
    state
        .telemetry
        .latest_sys_ram
        .store(system_ram_pct.to_bits(), Ordering::Relaxed);
    state
        .telemetry
        .latest_vox_cpu
        .store(vox_cpu.to_bits(), Ordering::Relaxed);
    state
        .telemetry
        .latest_vox_ram
        .store(vox_ram_mb, Ordering::Relaxed);
    state
        .telemetry
        .latest_threads
        .store(thread_count, Ordering::Relaxed);
}

fn emit_system_stats(
    app: &AppHandle,
    sys: &System,
    system_cpu: f32,
    system_ram_pct: f32,
    vox_cpu: f32,
    vox_ram_mb: u32,
    thread_count: u32,
) {
    if let Err(e) = app.emit(
        "system_stats",
        serde_json::json!({
            "system_cpu": system_cpu,
            "system_ram_pct": system_ram_pct,
            "vox_cpu": vox_cpu,
            "vox_ram_mb": vox_ram_mb,
            "threads": thread_count,
            "total_memory_gb": sys.total_memory() / 1024 / 1024 / 1024,
            "cpu_count": sys.cpus().len(),
        }),
    ) {
        log::warn!("[Monitoring::SystemMonitor] Failed to emit system_stats: {}", e);
    }
}

