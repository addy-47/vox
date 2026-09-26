use std::{
    env::current_dir,
    fs::{create_dir_all, write, OpenOptions},
    io::Write,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

use serde::Serialize;

use super::resource_scope::{
    new_resource_system, refresh_resource_system, ResourceScopeSampler, ResourceScopeSnapshot,
};

/// Process memory entry capturing RSS, CPU, and assigned role in the application tree.
#[derive(Debug, Clone, serde::Deserialize, Serialize)]
pub struct ProcessMemoryEntry {
    pub pid: u32,
    pub parent_pid: Option<u32>,
    pub name: String,
    pub memory_mb: f32,
    #[serde(default)]
    pub private_ram_mb: Option<f32>,
    #[serde(default)]
    pub shared_ram_mb: Option<f32>,
    #[serde(default)]
    pub pss_mb: Option<f32>,
    #[serde(default)]
    pub is_devtools: bool,
    #[serde(default)]
    pub is_dev_tool: bool,
    pub cpu_usage: f32,
    pub start_time: u64,
    pub is_main_process: bool,
    pub role: String,
}

/// Comprehensive profiler snapshot capturing system and per-webview memory breakdown.
#[derive(Debug, Clone, Serialize)]
pub struct ProfilerSnapshot {
    pub total_vox_ram_mb: f32,
    pub core_app_ram_mb: f32,
    pub devtools_ram_mb: Option<f32>,
    pub development_tools_ram_mb: f32,
    pub scope_kind: String,
    pub runtime_pss_mb: f32,
    pub dev_tool_pss_mb: f32,
    pub total_pss_mb: Option<f32>,
    pub cgroup_current_mb: Option<f32>,
    pub cgroup_swap_mb: Option<f32>,
    pub cgroup_anon_mb: Option<f32>,
    pub cgroup_file_mb: Option<f32>,
    pub cgroup_kernel_mb: Option<f32>,
    pub cgroup_shmem_mb: Option<f32>,
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

/// Telemetry event payload recorded during frontend route transitions and component renders.
#[derive(Debug, Clone, serde::Deserialize, Serialize)]
pub struct MemoryProfileLogEvent {
    pub route: String,
    pub event_type: String,
    pub baseline_ram_mb: Option<f32>,
    pub current_ram_mb: f32,
    #[serde(default)]
    pub core_app_ram_mb: Option<f32>,
    #[serde(default)]
    pub devtools_ram_mb: Option<f32>,
    #[serde(default)]
    pub development_tools_ram_mb: Option<f32>,
    #[serde(default)]
    pub scope_kind: Option<String>,
    #[serde(default)]
    pub runtime_pss_mb: Option<f32>,
    #[serde(default)]
    pub dev_tool_pss_mb: Option<f32>,
    #[serde(default)]
    pub cgroup_current_mb: Option<f32>,
    #[serde(default)]
    pub cgroup_swap_mb: Option<f32>,
    #[serde(default)]
    pub cgroup_anon_mb: Option<f32>,
    #[serde(default)]
    pub cgroup_file_mb: Option<f32>,
    #[serde(default)]
    pub cgroup_kernel_mb: Option<f32>,
    #[serde(default)]
    pub cgroup_shmem_mb: Option<f32>,
    #[serde(default)]
    pub total_pss_mb: Option<f32>,
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
    pub css_indicators: Option<serde_json::Value>,
    #[serde(default)]
    pub three_metrics: Option<serde_json::Value>,
    #[serde(default)]
    pub js_heap: Option<serde_json::Value>,
    #[serde(default)]
    pub process_tree: Option<Vec<ProcessMemoryEntry>>,
}

/// Robustly resolves the workspace `temp` directory across any execution working directory.
pub fn resolve_temp_dir() -> PathBuf {
    let candidates = [
        PathBuf::from("temp"),
        PathBuf::from("../temp"),
        PathBuf::from("../../temp"),
    ];
    for candidate in &candidates {
        if candidate.is_dir() {
            return candidate.clone();
        }
    }
    if let Ok(mut dir) = current_dir() {
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
    let fallback = PathBuf::from("temp");
    if let Err(e) = create_dir_all(&fallback) {
        log::warn!(
            target: "memory_profiler",
            "Failed to create fallback temp directory {:?}: {}",
            fallback,
            e
        );
    }
    fallback
}

/// Sanitizes route path into a clean page identifier (e.g. "/history" -> "history", "/" -> "home").
pub fn sanitize_page_name(route: &str) -> String {
    let clean = route.trim_matches('/').replace('/', "_").to_lowercase();
    let sanitized: String = clean
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '_' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect();
    if sanitized.is_empty() {
        "home".to_string()
    } else {
        sanitized
    }
}

#[cfg(target_os = "linux")]
fn is_devtools_process(pid: u32) -> bool {
    let cmdline_path = format!("/proc/{}/cmdline", pid);
    if let Ok(content) = std::fs::read_to_string(&cmdline_path) {
        let lower = content.to_lowercase();
        return lower.contains("inspector") || lower.contains("devtools");
    }
    false
}

#[cfg(not(target_os = "linux"))]
fn is_devtools_process(_pid: u32) -> bool {
    false
}

fn build_process_memory_entries(
    scope: &ResourceScopeSnapshot,
    current_pid: u32,
) -> (Vec<ProcessMemoryEntry>, f32, Option<f32>, f32) {
    let mut process_tree = Vec::new();
    let mut main_ram = 0.0;
    let mut network_ram = None;
    let mut other_ram = 0.0;

    for process in &scope.process.processes {
        let is_main = process.pid == current_pid;
        let is_devtools = is_devtools_process(process.pid);
        let mem_mb = bytes_to_mb(process.memory.rss);
        let role = if is_main {
            main_ram = mem_mb;
            "Main Process (Rust Core)".to_string()
        } else if process.is_dev_tool {
            "Development Tool".to_string()
        } else if is_devtools {
            "WebKit Inspector / DevTools".to_string()
        } else if process.name.contains("WebKitWeb") || process.name.contains("WebProcess") {
            "WebKit WebProcess".to_string()
        } else if process.name.contains("WebKitNetwork") || process.name.contains("NetworkProcess")
        {
            network_ram = Some(mem_mb);
            "WebKit NetworkProcess".to_string()
        } else {
            other_ram += mem_mb;
            "Child Process".to_string()
        };

        process_tree.push(ProcessMemoryEntry {
            pid: process.pid,
            parent_pid: process.parent_pid,
            name: process.name.clone(),
            memory_mb: mem_mb,
            private_ram_mb: optional_bytes_to_mb(process.memory.anon),
            shared_ram_mb: optional_bytes_to_mb(process.memory.file),
            pss_mb: optional_bytes_to_mb(process.memory.pss),
            is_devtools,
            is_dev_tool: process.is_dev_tool,
            cpu_usage: (process.cpu_percent * 10.0).round() / 10.0,
            start_time: process.start_time,
            is_main_process: is_main,
            role,
        });
    }

    (process_tree, main_ram, network_ram, other_ram)
}

fn bytes_to_mb(bytes: u64) -> f32 {
    ((bytes as f32 / 1024.0 / 1024.0) * 100.0).round() / 100.0
}

fn optional_bytes_to_mb(bytes: u64) -> Option<f32> {
    (bytes > 0).then(|| bytes_to_mb(bytes))
}

fn optional_positive(value: f32) -> Option<f32> {
    (value > 0.0).then_some((value * 100.0).round() / 100.0)
}

fn assign_webview_roles(
    process_tree: &mut [ProcessMemoryEntry],
    has_main: bool,
    has_tray: bool,
    has_wizard: bool,
) -> (Option<f32>, Option<f32>, Option<f32>) {
    let mut web_pids: Vec<(u32, u64, f32)> = process_tree
        .iter()
        .filter(|p| {
            !p.is_main_process
                && !p.is_devtools
                && !p.is_dev_tool
                && (p.name.contains("WebKitWeb") || p.name.contains("WebProcess"))
        })
        .map(|p| (p.pid, p.start_time, p.memory_mb))
        .collect();

    web_pids.sort_by(|a, b| a.1.cmp(&b.1).then_with(|| a.0.cmp(&b.0)));

    let mut main_ram = None;
    let mut tray_ram = None;
    let mut wizard_ram = None;
    let mut idx = 0;

    let roles = [
        (has_main, "Main WebView (Primary UI)", &mut main_ram),
        (has_tray, "Tray WebView (HUD Overlay)", &mut tray_ram),
        (has_wizard, "Wizard WebView (Setup Window)", &mut wizard_ram),
    ];

    for (active, role_name, ram_slot) in roles {
        if active && idx < web_pids.len() {
            *ram_slot = Some(web_pids[idx].2);
            if let Some(entry) = process_tree.iter_mut().find(|p| p.pid == web_pids[idx].0) {
                entry.role = role_name.to_string();
            }
            idx += 1;
        }
    }

    while idx < web_pids.len() {
        if let Some(entry) = process_tree.iter_mut().find(|p| p.pid == web_pids[idx].0) {
            entry.role = "WebKit Inspector / DevTools".to_string();
            entry.is_devtools = true;
        }
        idx += 1;
    }

    (main_ram, tray_ram, wizard_ram)
}

/// Public helper for gathering profiler snapshot metrics with known window states.
pub fn collect_profiler_snapshot(
    has_main: bool,
    has_tray: bool,
    has_wizard: bool,
) -> ProfilerSnapshot {
    let mut sys = new_resource_system();
    refresh_resource_system(&mut sys);
    let target_pid = match sysinfo::get_current_pid() {
        Ok(pid) => pid,
        Err(error) => {
            log::warn!("[Profiler] Failed to resolve current process: {}", error);
            sysinfo::Pid::from_u32(0)
        }
    };
    let scope = ResourceScopeSampler::new(target_pid).sample(&sys, sys.cpus().len());
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;

    let total_sys_ram = (sys.total_memory() / 1024 / 1024) as u32;
    let used_sys_ram = (sys.used_memory() / 1024 / 1024) as u32;
    let sys_pct = if sys.total_memory() > 0 {
        (sys.used_memory() as f32 / sys.total_memory() as f32) * 100.0
    } else {
        0.0
    };

    let (mut tree, main_ram, net_ram, other_ram) =
        build_process_memory_entries(&scope, target_pid.as_u32());
    let (main_web, tray_web, wizard_web) =
        assign_webview_roles(&mut tree, has_main, has_tray, has_wizard);
    tree.sort_by(|a, b| {
        b.is_main_process
            .cmp(&a.is_main_process)
            .then_with(|| a.pid.cmp(&b.pid))
    });

    let mut core_app_ram = 0.0;
    let mut devtools_ram = 0.0;
    let mut development_tools_ram = 0.0;
    for entry in &tree {
        if entry.is_devtools {
            devtools_ram += entry.memory_mb;
        } else if entry.is_dev_tool {
            development_tools_ram += entry.memory_mb;
        } else {
            core_app_ram += entry.memory_mb;
        }
    }

    ProfilerSnapshot {
        total_vox_ram_mb: bytes_to_mb(scope.process.rss_bytes),
        core_app_ram_mb: (core_app_ram * 100.0).round() / 100.0,
        devtools_ram_mb: optional_positive(devtools_ram),
        development_tools_ram_mb: development_tools_ram,
        scope_kind: scope.kind.as_str().to_string(),
        runtime_pss_mb: bytes_to_mb(scope.process.runtime_pss_bytes),
        dev_tool_pss_mb: bytes_to_mb(scope.process.dev_tool_pss_bytes),
        total_pss_mb: optional_positive(bytes_to_mb(scope.process.pss_bytes)),
        cgroup_current_mb: scope
            .cgroup
            .as_ref()
            .map(|cgroup| bytes_to_mb(cgroup.current_bytes)),
        cgroup_swap_mb: scope
            .cgroup
            .as_ref()
            .map(|cgroup| bytes_to_mb(cgroup.swap_bytes)),
        cgroup_anon_mb: scope
            .cgroup
            .as_ref()
            .map(|cgroup| bytes_to_mb(cgroup.anon_bytes)),
        cgroup_file_mb: scope
            .cgroup
            .as_ref()
            .map(|cgroup| bytes_to_mb(cgroup.file_bytes)),
        cgroup_kernel_mb: scope
            .cgroup
            .as_ref()
            .map(|cgroup| bytes_to_mb(cgroup.kernel_bytes)),
        cgroup_shmem_mb: scope
            .cgroup
            .as_ref()
            .map(|cgroup| bytes_to_mb(cgroup.shmem_bytes)),
        main_process_ram_mb: (main_ram * 100.0).round() / 100.0,
        main_webview_ram_mb: main_web,
        tray_webview_ram_mb: tray_web,
        wizard_webview_ram_mb: wizard_web,
        network_process_ram_mb: net_ram,
        other_children_ram_mb: (other_ram * 100.0).round() / 100.0,
        total_system_ram_mb: total_sys_ram,
        used_system_ram_mb: used_sys_ram,
        system_ram_pct: (sys_pct * 10.0).round() / 10.0,
        process_tree: tree,
        timestamp_ms: now,
        accuracy: "Measured (full launch process tree, PSS, and cgroup RAM)",
    }
}

/// Persists and logs a structured frontend memory profile event.
pub fn persist_memory_profile_event(event: &MemoryProfileLogEvent) -> Result<(), String> {
    if event.event_type != "poll" {
        log::info!(
            target: "memory_profiler",
            "[MEMORY_PROFILE] Route: {} | Event: {} | Scope: {:?} | Runtime PSS: {:?}MB | Dev PSS: {:?}MB | Cgroup: {:?}MB | Shmem: {:?}MB | Total RSS: {:.1}MB | DevTools: {:?}MB | PSS: {:?}MB | Peak: {:?}MB | WebViews: Main={:?}MB | DOM Nodes: {} | Components: {:?}",
            event.route,
            event.event_type,
            event.scope_kind,
            event.runtime_pss_mb,
            event.dev_tool_pss_mb,
            event.cgroup_current_mb,
            event.cgroup_shmem_mb,
            event.current_ram_mb,
            event.devtools_ram_mb,
            event.total_pss_mb,
            event.peak_ram_mb,
            event.main_webview_ram_mb,
            event.dom_node_count,
            event.active_components,
        );
    }

    let serialized = serde_json::to_string(event).map_err(|e| e.to_string())?;
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

    if let Ok(mut file) = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&file_path)
    {
        if let Err(e) = writeln!(file, "{}", serialized) {
            log::warn!(target: "memory_profiler", "Failed to write to snapshot JSONL: {}", e);
        }
    } else {
        log::warn!(target: "memory_profiler", "Failed to open snapshot JSONL file at {:?}", file_path);
    }

    if let Err(e) = write(temp_dir.join("memory_profile_latest.json"), &serialized) {
        log::warn!(target: "memory_profiler", "Failed to write latest snapshot JSON: {}", e);
    }

    Ok(())
}
