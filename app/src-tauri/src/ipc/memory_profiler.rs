use serde::Serialize;
use sysinfo::System;
use std::time::{SystemTime, UNIX_EPOCH};
use std::sync::OnceLock;

/// Session-unique JSONL path, fixed at first call so all events go into one file per app run.
fn session_jsonl_path() -> &'static String {
    static PATH: OnceLock<String> = OnceLock::new();
    PATH.get_or_init(|| {
        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let dir = std::path::Path::new("temp");
        let _ = std::fs::create_dir_all(dir);
        format!("temp/memory_profile_{}.jsonl", ts)
    })
}

#[derive(Debug, Clone, Serialize)]
pub struct ProcessMemoryEntry {
    pub pid: u32,
    pub parent_pid: Option<u32>,
    pub name: String,
    pub memory_mb: f32,
    pub cpu_usage: f32,
    pub start_time: u64,
    pub is_main_process: bool,
    pub role: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProfilerSnapshot {
    pub total_vox_ram_mb: f32,
    pub main_process_ram_mb: f32,
    pub main_webview_ram_mb: Option<f32>,
    pub tray_webview_ram_mb: Option<f32>,
    pub wizard_webview_ram_mb: Option<f32>,
    pub network_process_ram_mb: Option<f32>,
    pub other_children_ram_mb: f32,
    pub total_system_ram_mb: u32,
    pub used_system_ram_mb: u32,
    pub system_ram_pct: f32,
    pub process_tree: Vec<ProcessMemoryEntry>,
    pub timestamp_ms: u64,
    pub accuracy: &'static str,
}

fn collect_profiler_snapshot_internal() -> ProfilerSnapshot {
    let mut sys = System::new_all();
    sys.refresh_all();

    let target_pid = sysinfo::get_current_pid().ok();
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;

    let total_system_ram_mb = (sys.total_memory() / 1024 / 1024) as u32;
    let used_system_ram_mb = (sys.used_memory() / 1024 / 1024) as u32;
    let system_ram_pct = if sys.total_memory() > 0 {
        (sys.used_memory() as f32 / sys.total_memory() as f32) * 100.0
    } else {
        0.0
    };

    let mut process_tree: Vec<ProcessMemoryEntry> = Vec::new();
    let mut main_process_ram_mb: f32 = 0.0;
    let mut web_processes: Vec<ProcessMemoryEntry> = Vec::new();
    let mut network_process_ram_mb: Option<f32> = None;
    let mut other_children_ram_mb: f32 = 0.0;
    let mut total_vox_memory_bytes: u64 = 0;

    if let Some(target) = target_pid {
        // 1. Gather all descendants recursively
        for (&p_pid, proc) in sys.processes() {
            #[cfg(target_os = "linux")]
            {
                if proc.tasks().is_none() {
                    continue;
                }
            }

            let p_u32 = p_pid.as_u32();
            let mut is_descendant = false;
            let is_main = p_pid == target;

            if is_main {
                is_descendant = true;
            } else {
                let mut curr = proc;
                while let Some(parent_pid) = curr.parent() {
                    if parent_pid == target {
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
                let mem_bytes = proc.memory();
                total_vox_memory_bytes += mem_bytes;
                let mem_mb = mem_bytes as f32 / 1024.0 / 1024.0;
                let cpu = proc.cpu_usage();
                let name = proc.name().to_string();
                let parent_u32 = proc.parent().map(|p| p.as_u32());
                let start_time = proc.start_time();

                let mut role = "Child Process".to_string();
                if is_main {
                    role = "Main Process (Rust Core)".to_string();
                    main_process_ram_mb = mem_mb;
                } else if name.contains("WebKitWeb") || name.contains("WebProcess") {
                    role = "WebKit WebProcess".to_string();
                } else if name.contains("WebKitNetwork") || name.contains("NetworkProcess") {
                    role = "WebKit NetworkProcess".to_string();
                    network_process_ram_mb = Some(mem_mb);
                }

                let entry = ProcessMemoryEntry {
                    pid: p_u32,
                    parent_pid: parent_u32,
                    name,
                    memory_mb: (mem_mb * 100.0).round() / 100.0,
                    cpu_usage: (cpu * 10.0).round() / 10.0,
                    start_time,
                    is_main_process: is_main,
                    role,
                };

                if !is_main && (entry.name.contains("WebKitWeb") || entry.name.contains("WebProcess")) {
                    web_processes.push(entry.clone());
                } else if !is_main && !entry.name.contains("WebKitNetwork") && !entry.name.contains("NetworkProcess") {
                    other_children_ram_mb += mem_mb;
                }

                process_tree.push(entry);
            }
        }
    }

    // Sort WebProcesses chronologically by start_time (or PID as fallback)
    web_processes.sort_by(|a, b| {
        a.start_time.cmp(&b.start_time).then_with(|| a.pid.cmp(&b.pid))
    });

    let mut main_webview_ram_mb: Option<f32> = None;
    let mut tray_webview_ram_mb: Option<f32> = None;
    let mut wizard_webview_ram_mb: Option<f32> = None;

    if !web_processes.is_empty() {
        main_webview_ram_mb = Some(web_processes[0].memory_mb);
        // Tag role in process_tree
        if let Some(entry) = process_tree.iter_mut().find(|p| p.pid == web_processes[0].pid) {
            entry.role = "Main WebView (Primary UI)".to_string();
        }
    }

    if web_processes.len() > 1 {
        tray_webview_ram_mb = Some(web_processes[1].memory_mb);
        if let Some(entry) = process_tree.iter_mut().find(|p| p.pid == web_processes[1].pid) {
            entry.role = "Tray WebView (HUD Overlay)".to_string();
        }
    }

    if web_processes.len() > 2 {
        wizard_webview_ram_mb = Some(web_processes[2].memory_mb);
        if let Some(entry) = process_tree.iter_mut().find(|p| p.pid == web_processes[2].pid) {
            entry.role = "Wizard WebView (Setup Window)".to_string();
        }
    }

    // Sort complete process_tree with Main Process first, then WebViews, then others
    process_tree.sort_by(|a, b| {
        b.is_main_process.cmp(&a.is_main_process).then_with(|| a.pid.cmp(&b.pid))
    });

    let total_vox_ram_mb = (total_vox_memory_bytes as f32 / 1024.0 / 1024.0 * 100.0).round() / 100.0;

    ProfilerSnapshot {
        total_vox_ram_mb,
        main_process_ram_mb: (main_process_ram_mb * 100.0).round() / 100.0,
        main_webview_ram_mb,
        tray_webview_ram_mb,
        wizard_webview_ram_mb,
        network_process_ram_mb,
        other_children_ram_mb: (other_children_ram_mb * 100.0).round() / 100.0,
        total_system_ram_mb,
        used_system_ram_mb,
        system_ram_pct: (system_ram_pct * 10.0).round() / 10.0,
        process_tree,
        timestamp_ms: now,
        accuracy: "Measured (OS-level RSS via /proc & sysinfo)",
    }
}

/// Fetch an immediate, on-demand high-accuracy memory snapshot of the Vox process tree.
#[tauri::command]
pub async fn get_profiler_snapshot() -> Result<ProfilerSnapshot, String> {
    tokio::task::spawn_blocking(collect_profiler_snapshot_internal)
        .await
        .map_err(|e| format!("Failed to collect memory profiler snapshot: {e}"))
}

#[derive(Debug, Clone, serde::Deserialize, Serialize)]
pub struct MemoryProfileLogEvent {
    pub route: String,
    pub event_type: String,
    pub baseline_ram_mb: Option<f32>,
    pub current_ram_mb: f32,
    pub peak_ram_mb: Option<f32>,
    pub peak_delta_mb: Option<f32>,
    pub retained_ram_mb: Option<f32>,
    pub retained_delta_mb: Option<f32>,
    pub main_webview_ram_mb: Option<f32>,
    pub tray_webview_ram_mb: Option<f32>,
    pub active_components: Vec<String>,
    pub dom_node_count: usize,
    pub font_face_count: usize,
    pub timestamp_ms: u64,
}

/// Records a structured memory event.
/// - Lifecycle events (mount/peak/retained) → emitted to tracing log (vox2.log).
/// - Disk persistence to session JSONL is gated behind ENABLE_FILE_PERSISTENCE flag (disabled by default).
#[tauri::command]
pub async fn record_memory_profile_event(event: MemoryProfileLogEvent) -> Result<(), String> {
    tokio::task::spawn_blocking(move || {
        // 1. Emit structured tracing log (shows in terminal & vox2.log)
        if event.event_type != "poll" {
            tracing::info!(
                target: "memory_profiler",
                "[MEMORY_PROFILE] Route: {} | Event: {} | Current: {:.1}MB | Peak: {:?}MB (Δ{:?}MB) | Retained: {:?}MB (Δ{:?}MB) | WebViews: Main={:?}MB, Tray={:?}MB | DOM Nodes: {} | Components: {:?}",
                event.route,
                event.event_type,
                event.current_ram_mb,
                event.peak_ram_mb,
                event.peak_delta_mb,
                event.retained_ram_mb,
                event.retained_delta_mb,
                event.main_webview_ram_mb,
                event.tray_webview_ram_mb,
                event.dom_node_count,
                event.active_components,
            );
        }

        // 2. Disk persistence flag (disabled by default, can be flipped on for diagnostic runs)
        const ENABLE_FILE_PERSISTENCE: bool = false;

        if ENABLE_FILE_PERSISTENCE {
            if let Ok(serialized) = serde_json::to_string(&event) {
                use std::io::Write;
                let session_path = session_jsonl_path();
                if let Ok(mut file) = std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(session_path)
                {
                    let _ = writeln!(file, "{}", serialized);
                }

                // Also write latest for quick inspection
                let _ = std::fs::write("temp/memory_profile_latest.json", &serialized);
            }
        }
    })
    .await
    .map_err(|e| format!("Failed to record memory event: {e}"))
}

