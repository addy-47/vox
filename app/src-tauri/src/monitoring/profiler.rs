use std::{
    env::current_dir,
    fs::{create_dir_all, write, OpenOptions},
    io::Write,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

use serde::Serialize;
use sysinfo::System;

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
    pub total_pss_mb: Option<f32>,
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
fn read_linux_proc_memory(pid: u32) -> (Option<f32>, Option<f32>, Option<f32>) {
    let mut private_anon_mb = None;
    let mut shared_file_mb = None;
    let mut pss_mb = None;

    let status_path = format!("/proc/{}/status", pid);
    if let Ok(content) = std::fs::read_to_string(&status_path) {
        for line in content.lines() {
            if line.starts_with("RssAnon:") {
                if let Some(val) = parse_kb_line(line) {
                    private_anon_mb = Some((val as f32 / 1024.0 * 100.0).round() / 100.0);
                }
            } else if line.starts_with("RssFile:") {
                if let Some(val) = parse_kb_line(line) {
                    shared_file_mb = Some((val as f32 / 1024.0 * 100.0).round() / 100.0);
                }
            }
        }
    }

    let smaps_path = format!("/proc/{}/smaps_rollup", pid);
    if let Ok(content) = std::fs::read_to_string(&smaps_path) {
        for line in content.lines() {
            if line.starts_with("Pss:") {
                if let Some(val) = parse_kb_line(line) {
                    pss_mb = Some((val as f32 / 1024.0 * 100.0).round() / 100.0);
                    break;
                }
            }
        }
    }

    (private_anon_mb, shared_file_mb, pss_mb)
}

#[cfg(not(target_os = "linux"))]
fn read_linux_proc_memory(_pid: u32) -> (Option<f32>, Option<f32>, Option<f32>) {
    (None, None, None)
}

fn parse_kb_line(line: &str) -> Option<u64> {
    line.split_whitespace().nth(1)?.parse::<u64>().ok()
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

fn extract_descendant_processes(
    sys: &System,
    target: sysinfo::Pid,
) -> (Vec<ProcessMemoryEntry>, f32, Option<f32>, f32, u64) {
    let mut process_tree = Vec::new();
    let mut main_ram = 0.0;
    let mut network_ram = None;
    let mut other_ram = 0.0;
    let mut total_bytes = 0;

    for (&p_pid, proc) in sys.processes() {
        #[cfg(target_os = "linux")]
        if proc.tasks().is_none() {
            continue;
        }

        let is_main = p_pid == target;
        let mut is_descendant = is_main;
        if !is_main {
            let mut curr = proc;
            while let Some(parent_pid) = curr.parent() {
                if parent_pid == target {
                    is_descendant = true;
                    break;
                }
                match sys.process(parent_pid) {
                    Some(parent_proc) => curr = parent_proc,
                    None => break,
                }
            }
        }

        if is_descendant {
            let pid_u32 = p_pid.as_u32();
            let (private_anon, shared_file, pss) = read_linux_proc_memory(pid_u32);
            let is_devtools = is_devtools_process(pid_u32);
            let mem_bytes = proc.memory();
            total_bytes += mem_bytes;
            let mem_mb = mem_bytes as f32 / 1024.0 / 1024.0;
            let cpu = proc.cpu_usage();
            let name = proc.name().to_string();

            let role = if is_main {
                main_ram = mem_mb;
                "Main Process (Rust Core)".to_string()
            } else if is_devtools {
                "WebKit Inspector / DevTools".to_string()
            } else if name.contains("WebKitWeb") || name.contains("WebProcess") {
                "WebKit WebProcess".to_string()
            } else if name.contains("WebKitNetwork") || name.contains("NetworkProcess") {
                network_ram = Some(mem_mb);
                "WebKit NetworkProcess".to_string()
            } else {
                other_ram += mem_mb;
                "Child Process".to_string()
            };

            process_tree.push(ProcessMemoryEntry {
                pid: pid_u32,
                parent_pid: proc.parent().map(|p| p.as_u32()),
                name,
                memory_mb: (mem_mb * 100.0).round() / 100.0,
                private_ram_mb: private_anon,
                shared_ram_mb: shared_file,
                pss_mb: pss,
                is_devtools,
                cpu_usage: (cpu * 10.0).round() / 10.0,
                start_time: proc.start_time(),
                is_main_process: is_main,
                role,
            });
        }
    }

    (process_tree, main_ram, network_ram, other_ram, total_bytes)
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
            !p.is_main_process && !p.is_devtools && (p.name.contains("WebKitWeb") || p.name.contains("WebProcess"))
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
    let mut sys = System::new_all();
    sys.refresh_all();

    let target_pid = sysinfo::get_current_pid().ok();
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

    let (mut tree, main_ram, net_ram, other_ram, total_bytes) = match target_pid {
        Some(target) => extract_descendant_processes(&sys, target),
        None => (Vec::new(), 0.0, None, 0.0, 0),
    };

    let (main_web, tray_web, wizard_web) =
        assign_webview_roles(&mut tree, has_main, has_tray, has_wizard);

    tree.sort_by(|a, b| {
        b.is_main_process
            .cmp(&a.is_main_process)
            .then_with(|| a.pid.cmp(&b.pid))
    });

    let total_vox_mb = (total_bytes as f32 / 1024.0 / 1024.0 * 100.0).round() / 100.0;
    let mut core_app_ram = 0.0;
    let mut devtools_ram = 0.0;
    let mut total_pss = 0.0;

    for entry in &tree {
        if let Some(pss) = entry.pss_mb {
            total_pss += pss;
        }
        if entry.is_devtools {
            devtools_ram += entry.memory_mb;
        } else {
            core_app_ram += entry.memory_mb;
        }
    }

    ProfilerSnapshot {
        total_vox_ram_mb: total_vox_mb,
        core_app_ram_mb: (core_app_ram * 100.0).round() / 100.0,
        devtools_ram_mb: if devtools_ram > 0.0 {
            Some((devtools_ram * 100.0).round() / 100.0)
        } else {
            None
        },
        total_pss_mb: if total_pss > 0.0 {
            Some((total_pss * 100.0).round() / 100.0)
        } else {
            None
        },
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
        accuracy: "Measured (OS-level RSS & PSS via /proc & sysinfo)",
    }
}

/// Persists and logs a structured frontend memory profile event.
pub fn persist_memory_profile_event(event: &MemoryProfileLogEvent) -> Result<(), String> {
    if event.event_type != "poll" {
        log::info!(
            target: "memory_profiler",
            "[MEMORY_PROFILE] Route: {} | Event: {} | Core App: {:?}MB | Total RSS: {:.1}MB | DevTools: {:?}MB | PSS: {:?}MB | Peak: {:?}MB | WebViews: Main={:?}MB | DOM Nodes: {} | Components: {:?}",
            event.route,
            event.event_type,
            event.core_app_ram_mb,
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
