use serde::Serialize;
use sysinfo::System;
use std::time::{SystemTime, UNIX_EPOCH};

/// Robustly resolves the workspace `temp` directory across any execution working directory.
pub fn resolve_temp_dir() -> std::path::PathBuf {
    let candidates = [
        std::path::PathBuf::from("temp"),
        std::path::PathBuf::from("../temp"),
        std::path::PathBuf::from("../../temp"),
    ];
    for candidate in &candidates {
        if candidate.is_dir() {
            return candidate.clone();
        }
    }
    if let Ok(mut dir) = std::env::current_dir() {
        for _ in 0..5 {
            let temp_candidate = dir.join("temp");
            if temp_candidate.is_dir() {
                return temp_candidate;
            }
            if !dir.pop() {
                break;
            }
        }
    }
    let fallback = std::path::PathBuf::from("temp");
    let _ = std::fs::create_dir_all(&fallback);
    fallback
}

/// Sanitizes route path into a clean page identifier (e.g. "/history" -> "history", "/" -> "home").
pub fn sanitize_page_name(route: &str) -> String {
    let clean = route.trim_matches('/').replace('/', "_").to_lowercase();
    let sanitized: String = clean
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '_' || c == '-' { c } else { '_' })
        .collect();
    if sanitized.is_empty() {
        "home".to_string()
    } else {
        sanitized
    }
}


#[derive(Debug, Clone, serde::Deserialize, Serialize)]
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

fn collect_profiler_snapshot_internal(has_main: bool, has_tray: bool, has_wizard: bool) -> ProfilerSnapshot {
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

    let mut proc_idx = 0;

    if has_main && proc_idx < web_processes.len() {
        main_webview_ram_mb = Some(web_processes[proc_idx].memory_mb);
        if let Some(entry) = process_tree.iter_mut().find(|p| p.pid == web_processes[proc_idx].pid) {
            entry.role = "Main WebView (Primary UI)".to_string();
        }
        proc_idx += 1;
    }

    if has_tray && proc_idx < web_processes.len() {
        tray_webview_ram_mb = Some(web_processes[proc_idx].memory_mb);
        if let Some(entry) = process_tree.iter_mut().find(|p| p.pid == web_processes[proc_idx].pid) {
            entry.role = "Tray WebView (HUD Overlay)".to_string();
        }
        proc_idx += 1;
    }

    if has_wizard && proc_idx < web_processes.len() {
        wizard_webview_ram_mb = Some(web_processes[proc_idx].memory_mb);
        if let Some(entry) = process_tree.iter_mut().find(|p| p.pid == web_processes[proc_idx].pid) {
            entry.role = "Wizard WebView (Setup Window)".to_string();
        }
        proc_idx += 1;
    }

    while proc_idx < web_processes.len() {
        if let Some(entry) = process_tree.iter_mut().find(|p| p.pid == web_processes[proc_idx].pid) {
            entry.role = "WebKit Auxiliary WebProcess".to_string();
        }
        proc_idx += 1;
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

/// Public helper for gathering profiler snapshot metrics with known window states.
pub fn collect_profiler_snapshot(has_main: bool, has_tray: bool, has_wizard: bool) -> ProfilerSnapshot {
    collect_profiler_snapshot_internal(has_main, has_tray, has_wizard)
}

/// Fetch an immediate, on-demand high-accuracy memory snapshot of the Vox process tree.
#[tauri::command]
pub async fn get_profiler_snapshot(app: tauri::AppHandle) -> Result<ProfilerSnapshot, String> {
    use tauri::Manager;
    let has_main = app.get_webview_window("main").is_some();
    let has_tray = app.get_webview_window("tray").is_some();
    let has_wizard = app.get_webview_window("wizard").is_some();

    tokio::task::spawn_blocking(move || {
        collect_profiler_snapshot_internal(has_main, has_tray, has_wizard)
    })
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
    #[serde(default)]
    pub process_tree: Option<Vec<ProcessMemoryEntry>>,
}

/// Records a structured memory event.
/// - Lifecycle events (mount/snapshot/peak/retained) → emitted to tracing log (vox2.log).
/// - Disk persistence writes structured logs to `temp/<timestamp>-<page>.jsonl`.
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

        // 2. Disk persistence to temp/<timestamp>-<page>.jsonl
        if let Ok(serialized) = serde_json::to_string(&event) {
            use std::io::Write;
            let temp_dir = resolve_temp_dir();
            let page = sanitize_page_name(&event.route);
            let ts = if event.timestamp_ms > 0 {
                event.timestamp_ms / 1000
            } else {
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs()
            };
            let filename = format!("{}-{}.jsonl", ts, page);
            let file_path = temp_dir.join(&filename);

            if let Ok(mut file) = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&file_path)
            {
                let _ = writeln!(file, "{}", serialized);
            } else {
                tracing::warn!(target: "memory_profiler", "Failed to open snapshot JSONL file at {:?}", file_path);
            }

            // Also update latest snapshot JSON for immediate inspection
            let _ = std::fs::write(temp_dir.join("memory_profile_latest.json"), &serialized);
        }
    })
    .await
    .map_err(|e| format!("Failed to record memory event: {e}"))
}


