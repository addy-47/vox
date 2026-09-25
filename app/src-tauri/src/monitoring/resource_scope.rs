use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
};

use sysinfo::{
    CpuRefreshKind, MemoryRefreshKind, Pid, ProcessRefreshKind, RefreshKind, System, UpdateKind,
};

const BYTES_PER_KIB: u64 = 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResourceScopeKind {
    Application,
    TauriDev,
}

impl ResourceScopeKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Application => "application",
            Self::TauriDev => "tauri_dev",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessDescriptor {
    pub pid: u32,
    pub parent_pid: Option<u32>,
    pub command: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResourceScopeRoot {
    pub pid: u32,
    pub kind: ResourceScopeKind,
}

#[derive(Debug, Clone, Default)]
pub struct ProcessMemoryBytes {
    pub rss: u64,
    pub pss: u64,
    pub private_dirty: u64,
    pub anon: u64,
    pub file: u64,
    pub shmem: u64,
    pub swap: u64,
}

#[derive(Debug, Clone)]
pub struct ScopedProcess {
    pub pid: u32,
    pub parent_pid: Option<u32>,
    pub name: String,
    pub command: String,
    pub start_time: u64,
    pub cpu_percent: f32,
    pub memory: ProcessMemoryBytes,
    pub thread_count: u32,
    pub is_runtime: bool,
    pub is_dev_tool: bool,
}

#[derive(Debug, Clone, Default)]
pub struct ProcessScopeSnapshot {
    pub rss_bytes: u64,
    pub pss_bytes: u64,
    pub private_dirty_bytes: u64,
    pub anon_bytes: u64,
    pub file_bytes: u64,
    pub shmem_bytes: u64,
    pub swap_bytes: u64,
    pub runtime_rss_bytes: u64,
    pub runtime_pss_bytes: u64,
    pub dev_tool_rss_bytes: u64,
    pub dev_tool_pss_bytes: u64,
    pub cpu_percent: f32,
    pub thread_count: u32,
    pub processes: Vec<ScopedProcess>,
}

#[derive(Debug, Clone, Default)]
pub struct LinuxCgroupSnapshot {
    pub current_bytes: u64,
    pub swap_bytes: u64,
    pub anon_bytes: u64,
    pub file_bytes: u64,
    pub kernel_bytes: u64,
    pub shmem_bytes: u64,
    pub shmem_thp_bytes: u64,
}

#[derive(Debug, Clone)]
pub struct ResourceScopeSnapshot {
    pub root_pid: u32,
    pub kind: ResourceScopeKind,
    pub process: ProcessScopeSnapshot,
    pub cgroup: Option<LinuxCgroupSnapshot>,
}

impl ResourceScopeSnapshot {
    pub fn resident_bytes(&self) -> u64 {
        self.cgroup
            .as_ref()
            .map(|cgroup| cgroup.current_bytes)
            .unwrap_or(self.process.pss_bytes)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ResourceScopeSampler {
    current_pid: Pid,
}

impl ResourceScopeSampler {
    pub fn new(current_pid: Pid) -> Self {
        Self { current_pid }
    }

    pub fn sample(&self, system: &System, cpu_cores: usize) -> ResourceScopeSnapshot {
        let root = resolve_scope_root(system, self.current_pid);
        let process = collect_process_scope(system, self.current_pid, root, cpu_cores);
        ResourceScopeSnapshot {
            root_pid: root.pid,
            kind: root.kind,
            process,
            cgroup: read_cgroup_snapshot(self.current_pid.as_u32()),
        }
    }
}

pub fn new_resource_system() -> System {
    System::new_with_specifics(refresh_kind())
}

pub fn refresh_resource_system(system: &mut System) {
    system.refresh_cpu_specifics(CpuRefreshKind::everything().without_frequency());
    system.refresh_memory_specifics(MemoryRefreshKind::new().with_ram());
    system.refresh_processes_specifics(process_refresh_kind());
}

pub fn is_tauri_dev_command(command: &str) -> bool {
    let normalized = command.to_ascii_lowercase();
    normalized.contains("pnpm.mjs tauri dev")
        || (normalized.contains("@tauri-apps/cli") && normalized.contains("tauri.js dev"))
        || normalized == "tauri dev"
}

pub fn select_scope_root(current_pid: u32, descriptors: &[ProcessDescriptor]) -> ResourceScopeRoot {
    let by_pid: HashMap<u32, &ProcessDescriptor> =
        descriptors.iter().map(|entry| (entry.pid, entry)).collect();
    let mut cursor = Some(current_pid);
    let mut selected = ResourceScopeRoot {
        pid: current_pid,
        kind: ResourceScopeKind::Application,
    };

    while let Some(pid) = cursor {
        let Some(descriptor) = by_pid.get(&pid) else {
            break;
        };
        if is_tauri_dev_command(&descriptor.command) {
            selected = ResourceScopeRoot {
                pid,
                kind: ResourceScopeKind::TauriDev,
            };
        }
        if descriptor.parent_pid == Some(pid) {
            break;
        }
        cursor = descriptor.parent_pid;
    }

    selected
}

pub fn parse_cgroup_memory(
    current_bytes: u64,
    memory_stat: &str,
    swap_bytes: u64,
) -> LinuxCgroupSnapshot {
    let values = parse_key_values(memory_stat);
    LinuxCgroupSnapshot {
        current_bytes,
        swap_bytes,
        anon_bytes: values.get("anon").copied().unwrap_or_default(),
        file_bytes: values.get("file").copied().unwrap_or_default(),
        kernel_bytes: values.get("kernel").copied().unwrap_or_default(),
        shmem_bytes: values.get("shmem").copied().unwrap_or_default(),
        shmem_thp_bytes: values.get("shmem_thp").copied().unwrap_or_default(),
    }
}

fn refresh_kind() -> RefreshKind {
    RefreshKind::new()
        .with_processes(process_refresh_kind())
        .with_memory(MemoryRefreshKind::new().with_ram())
        .with_cpu(CpuRefreshKind::everything().without_frequency())
}

fn process_refresh_kind() -> ProcessRefreshKind {
    ProcessRefreshKind::new()
        .with_cpu()
        .with_memory()
        .with_cmd(UpdateKind::OnlyIfNotSet)
}

fn resolve_scope_root(system: &System, current_pid: Pid) -> ResourceScopeRoot {
    let mut descriptors = Vec::new();
    let mut cursor = Some(current_pid);
    while let Some(pid) = cursor {
        let Some(process) = system.process(pid) else {
            break;
        };
        let parent_pid = process.parent();
        descriptors.push(ProcessDescriptor {
            pid: pid.as_u32(),
            parent_pid: parent_pid.map(Pid::as_u32),
            command: process_command(process),
        });
        if parent_pid == Some(pid) {
            break;
        }
        cursor = parent_pid;
    }
    select_scope_root(current_pid.as_u32(), &descriptors)
}

fn collect_process_scope(
    system: &System,
    current_pid: Pid,
    root: ResourceScopeRoot,
    cpu_cores: usize,
) -> ProcessScopeSnapshot {
    let mut snapshot = ProcessScopeSnapshot::default();

    for (&pid, process) in system.processes() {
        #[cfg(target_os = "linux")]
        if process.thread_kind().is_some() {
            continue;
        }

        if !is_descendant_process(system, pid, root_pid(root)) {
            continue;
        }

        let mut memory = read_process_memory(pid.as_u32());
        if memory.rss == 0 {
            memory.rss = process.memory();
        }
        if memory.pss == 0 {
            memory.pss = memory.rss;
        }

        let is_runtime = is_descendant_process(system, pid, current_pid);
        let is_dev_tool = root.pid != current_pid.as_u32() && !is_runtime;
        let cpu_percent = process.cpu_usage().max(0.0);
        let thread_count = process.tasks().map(|tasks| tasks.len()).unwrap_or(1) as u32;

        snapshot.rss_bytes += memory.rss;
        snapshot.pss_bytes += memory.pss;
        snapshot.private_dirty_bytes += memory.private_dirty;
        snapshot.anon_bytes += memory.anon;
        snapshot.file_bytes += memory.file;
        snapshot.shmem_bytes += memory.shmem;
        snapshot.swap_bytes += memory.swap;
        snapshot.cpu_percent += cpu_percent;
        snapshot.thread_count += thread_count;

        if is_dev_tool {
            snapshot.dev_tool_rss_bytes += memory.rss;
            snapshot.dev_tool_pss_bytes += memory.pss;
        } else {
            snapshot.runtime_rss_bytes += memory.rss;
            snapshot.runtime_pss_bytes += memory.pss;
        }

        snapshot.processes.push(ScopedProcess {
            pid: pid.as_u32(),
            parent_pid: process.parent().map(Pid::as_u32),
            name: process.name().to_string(),
            command: process_command(process),
            start_time: process.start_time(),
            cpu_percent,
            memory,
            thread_count,
            is_runtime,
            is_dev_tool,
        });
    }

    snapshot.cpu_percent /= cpu_cores.max(1) as f32;
    snapshot
        .processes
        .sort_by(|left, right| right.memory.rss.cmp(&left.memory.rss));
    snapshot
}

fn root_pid(root: ResourceScopeRoot) -> Pid {
    Pid::from_u32(root.pid)
}

fn is_descendant_process(system: &System, pid: Pid, target: Pid) -> bool {
    if pid == target {
        return true;
    }
    let mut cursor = system.process(pid);
    while let Some(process) = cursor {
        let Some(parent) = process.parent() else {
            break;
        };
        if parent == target {
            return true;
        }
        cursor = system.process(parent);
    }
    false
}

fn process_command(process: &sysinfo::Process) -> String {
    if process.cmd().is_empty() {
        return process.name().to_string();
    }
    process
        .cmd()
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(target_os = "linux")]
fn read_process_memory(pid: u32) -> ProcessMemoryBytes {
    let status = fs::read_to_string(format!("/proc/{pid}/status")).unwrap_or_default();
    let smaps = fs::read_to_string(format!("/proc/{pid}/smaps_rollup")).unwrap_or_default();
    ProcessMemoryBytes {
        rss: read_kib_field(&status, "VmRSS") * BYTES_PER_KIB,
        pss: read_kib_field(&smaps, "Pss") * BYTES_PER_KIB,
        private_dirty: read_kib_field(&smaps, "Private_Dirty") * BYTES_PER_KIB,
        anon: read_kib_field(&status, "RssAnon") * BYTES_PER_KIB,
        file: read_kib_field(&status, "RssFile") * BYTES_PER_KIB,
        shmem: read_kib_field(&status, "RssShmem") * BYTES_PER_KIB,
        swap: read_kib_field(&status, "VmSwap") * BYTES_PER_KIB,
    }
}

#[cfg(not(target_os = "linux"))]
fn read_process_memory(_pid: u32) -> ProcessMemoryBytes {
    ProcessMemoryBytes::default()
}

#[cfg(target_os = "linux")]
fn read_kib_field(content: &str, field: &str) -> u64 {
    content
        .lines()
        .find_map(|line| line.strip_prefix(&format!("{field}:")))
        .and_then(|value| value.split_whitespace().next())
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or_default()
}

#[cfg(target_os = "linux")]
fn read_cgroup_snapshot(pid: u32) -> Option<LinuxCgroupSnapshot> {
    let cgroup = read_cgroup_path(pid)?;
    let current_bytes = fs::read_to_string(cgroup.join("memory.current"))
        .ok()?
        .trim()
        .parse::<u64>()
        .ok()?;
    let swap_bytes = fs::read_to_string(cgroup.join("memory.swap.current"))
        .ok()
        .and_then(|value| value.trim().parse::<u64>().ok())
        .unwrap_or_default();
    let memory_stat = fs::read_to_string(cgroup.join("memory.stat")).ok()?;
    Some(parse_cgroup_memory(current_bytes, &memory_stat, swap_bytes))
}

#[cfg(not(target_os = "linux"))]
fn read_cgroup_snapshot(_pid: u32) -> Option<LinuxCgroupSnapshot> {
    None
}

#[cfg(target_os = "linux")]
fn read_cgroup_path(pid: u32) -> Option<PathBuf> {
    let content = fs::read_to_string(format!("/proc/{pid}/cgroup")).ok()?;
    let relative = content
        .lines()
        .find_map(|line| line.strip_prefix("0::"))?
        .trim_start_matches('/');
    if relative.is_empty() {
        return None;
    }
    Some(Path::new("/sys/fs/cgroup").join(relative))
}

fn parse_key_values(content: &str) -> HashMap<String, u64> {
    content
        .lines()
        .filter_map(|line| {
            let mut parts = line.split_whitespace();
            let key = parts.next()?;
            let value = parts.next()?.parse::<u64>().ok()?;
            Some((key.to_string(), value))
        })
        .collect()
}
