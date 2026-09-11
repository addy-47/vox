use std::{
    sync::{
        atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering},
        Arc,
    },
    thread::Builder,
};

use crossbeam_channel::{bounded, Receiver, Sender};
use serde::Serialize;
use sysinfo::{Pid, System};
use tauri::{AppHandle, Manager};

use crate::{
    core::{
        events::{emit_ipc, emit_ipc_to, IpcEvent, SystemStatsPayload, TelemetryData},
        state::{AppState, AppWindow, InteractionOwner, InteractionState},
    },
    monitoring::{
        SYSTEM_MONITOR_INTERVAL, TELEMETRY_AGGREGATOR_CHANNEL_CAPACITY, TELEMETRY_EMITTER_INTERVAL,
    },
};

/// Structured telemetry events emitted by various engine subsystems.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", content = "data")]
pub enum TelemetryEvent {
    /// Periodic system resource utilization
    SystemHealth {
        system_cpu: f32,
        system_ram_pct: f32,
        vox_cpu: f32,
        vox_ram_mb: u32,
    },
    /// Real-time audio signal characteristics (VAD hot-path)
    AudioEnergy {
        energy: f32,
        vad_prob: f32,
        low: f32,
        mid: f32,
        high: f32,
    },
}

/// Telemetry handles and health atomics bundled for AppState and monitoring workers.
#[derive(Clone)]
pub struct TelemetryState {
    pub telemetry_tx: crossbeam_channel::Sender<TelemetryEvent>,
    pub latest_energy: Arc<AtomicU32>,
    pub latest_vad_prob: Arc<AtomicU32>,
    pub latest_low: Arc<AtomicU32>,
    pub latest_mid: Arc<AtomicU32>,
    pub latest_high: Arc<AtomicU32>,
    pub latest_playback_energy: Arc<AtomicU32>,
    pub latest_playback_low: Arc<AtomicU32>,
    pub latest_playback_mid: Arc<AtomicU32>,
    pub latest_playback_high: Arc<AtomicU32>,
    pub latest_sys_cpu: Arc<AtomicU32>,
    pub latest_sys_ram: Arc<AtomicU32>,
    pub latest_vox_cpu: Arc<AtomicU32>,
    pub latest_vox_ram: Arc<AtomicU32>,
    pub latest_stt_ms: Arc<AtomicU32>,
    pub latest_ttft_ms: Arc<AtomicU32>,
    pub latest_voice_latency_ms: Arc<AtomicU32>,
    pub latest_threads: Arc<AtomicU32>,
    pub latest_tts_rtf: Arc<AtomicU32>,
    pub latest_playback_start_ms: Arc<AtomicU32>,
    pub latest_persistence_rate: Arc<AtomicU32>,
    pub is_db_healthy: Arc<AtomicBool>,
    pub is_private_mode: Arc<AtomicBool>,
    pub dropped_telemetry_events: Arc<AtomicU64>,
}

/// A dedicated background worker that aggregates telemetry events.
pub struct TelemetryAggregator {
    rx: Receiver<TelemetryEvent>,
    latest_energy: Arc<AtomicU32>,
    latest_vad_prob: Arc<AtomicU32>,
    latest_low: Arc<AtomicU32>,
    latest_mid: Arc<AtomicU32>,
    latest_high: Arc<AtomicU32>,
    latest_sys_cpu: Arc<AtomicU32>,
    latest_sys_ram: Arc<AtomicU32>,
    latest_vox_cpu: Arc<AtomicU32>,
    latest_vox_ram: Arc<AtomicU32>,
    dropped_events: Arc<AtomicU64>,
}

/// Target atomics updated by the telemetry aggregator loop.
pub struct TelemetryAggregatorHandles {
    pub latest_energy: Arc<AtomicU32>,
    pub latest_vad_prob: Arc<AtomicU32>,
    pub latest_low: Arc<AtomicU32>,
    pub latest_mid: Arc<AtomicU32>,
    pub latest_high: Arc<AtomicU32>,
    pub latest_sys_cpu: Arc<AtomicU32>,
    pub latest_sys_ram: Arc<AtomicU32>,
    pub latest_vox_cpu: Arc<AtomicU32>,
    pub latest_vox_ram: Arc<AtomicU32>,
    pub dropped_events: Arc<AtomicU64>,
}

impl TelemetryAggregator {
    /// Creates a new TelemetryAggregator and bounded Sender channel.
    pub fn new(handles: TelemetryAggregatorHandles) -> (Self, Sender<TelemetryEvent>) {
        let (tx, rx) = bounded(TELEMETRY_AGGREGATOR_CHANNEL_CAPACITY);
        (
            Self {
                rx,
                latest_energy: handles.latest_energy,
                latest_vad_prob: handles.latest_vad_prob,
                latest_low: handles.latest_low,
                latest_mid: handles.latest_mid,
                latest_high: handles.latest_high,
                latest_sys_cpu: handles.latest_sys_cpu,
                latest_sys_ram: handles.latest_sys_ram,
                latest_vox_cpu: handles.latest_vox_cpu,
                latest_vox_ram: handles.latest_vox_ram,
                dropped_events: handles.dropped_events,
            },
            tx,
        )
    }

    /// Spawns the aggregator loop on a dedicated OS thread.
    pub fn start(self) {
        Builder::new()
            .name("vox-telemetry".to_string())
            .spawn(move || {
                log::info!("[Monitoring::Aggregator] Aggregator worker started");

                while let Ok(event) = self.rx.recv() {
                    self.handle_event(event);
                }
                log::info!("[Monitoring::Aggregator] Channel disconnected. Aggregator exiting");
            })
            .expect("[Monitoring::Aggregator] Failed to spawn aggregator thread");
    }

    /// Applies a single telemetry event to the latest-value atomics.
    fn handle_event(&self, event: TelemetryEvent) {
        match event {
            TelemetryEvent::SystemHealth {
                system_cpu,
                system_ram_pct,
                vox_cpu,
                vox_ram_mb,
            } => {
                log::debug!(
                    "[Monitoring::Aggregator] System health sys_cpu={}% sys_ram={}% vox_cpu={}% vox_ram={}MB",
                    system_cpu,
                    system_ram_pct,
                    vox_cpu,
                    vox_ram_mb
                );
                self.latest_sys_cpu
                    .store(system_cpu.to_bits(), Ordering::Relaxed);
                self.latest_sys_ram
                    .store(system_ram_pct.to_bits(), Ordering::Relaxed);
                self.latest_vox_cpu
                    .store(vox_cpu.to_bits(), Ordering::Relaxed);
                self.latest_vox_ram.store(vox_ram_mb, Ordering::Relaxed);

                let dropped = self.dropped_events.load(Ordering::Relaxed);
                if dropped > 0 {
                    log::warn!(
                        "[Monitoring::Aggregator] Dropped {} telemetry events due to channel saturation",
                        dropped
                    );
                }
            }
            TelemetryEvent::AudioEnergy {
                energy,
                vad_prob,
                low,
                mid,
                high,
            } => {
                log::debug!(
                    "[Monitoring::Aggregator] Audio energy e={} vad={} low={} mid={} high={}",
                    energy,
                    vad_prob,
                    low,
                    mid,
                    high
                );
                self.latest_energy
                    .store(energy.to_bits(), Ordering::Relaxed);
                self.latest_vad_prob
                    .store(vad_prob.to_bits(), Ordering::Relaxed);
                self.latest_low.store(low.to_bits(), Ordering::Relaxed);
                self.latest_mid.store(mid.to_bits(), Ordering::Relaxed);
                self.latest_high.store(high.to_bits(), Ordering::Relaxed);
            }
        }
    }
}

/// Spawns the background system monitor task to collect and broadcast CPU and RAM statistics.
pub fn spawn_system_monitor(app: AppHandle) {
    let state_arc: Arc<AppState> = app.state::<Arc<AppState>>().inner().clone();
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

            emit_system_stats(
                &app,
                &sys,
                system_cpu,
                system_ram_pct,
                vox_cpu,
                vox_ram_mb,
                thread_count,
            );

            if let Err(e) = telemetry_tx.try_send(TelemetryEvent::SystemHealth {
                system_cpu,
                system_ram_pct,
                vox_cpu,
                vox_ram_mb,
            }) {
                log::warn!("[Monitoring::SystemMonitor] Failed to send SystemHealth to telemetry aggregator: {}", e);
            }
        }
    });
}

/// Spawns periodic background task pushing audio and VAD telemetry to active window.
pub fn spawn_telemetry_emitter(app: AppHandle) {
    let state = app.state::<Arc<AppState>>().inner().clone();

    tauri::async_runtime::spawn(async move {
        loop {
            tokio::time::sleep(TELEMETRY_EMITTER_INTERVAL).await;

            if state.pipeline.state() == InteractionState::Paused {
                continue;
            }

            let (energy, low, mid, high) = get_current_audio_levels(&state);
            let vad_prob = f32::from_bits(state.telemetry.latest_vad_prob.load(Ordering::Relaxed));
            let target = get_target_window(&state);

            if let Err(e) = emit_ipc_to(
                &app,
                target,
                IpcEvent::Telemetry(TelemetryData {
                    energy,
                    vad_prob,
                    low,
                    mid,
                    high,
                }),
            ) {
                log::warn!(
                    "[Monitoring::TelemetryEmitter] Failed to emit telemetry event: {}",
                    e
                );
            }
        }
    });
}

/// Sums CPU, RAM, and thread counts across the current process and its descendants.
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

/// Reports whether a process is the target or descends from it via parent links.
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

/// Stores the latest system and process metrics into the shared telemetry atomics.
fn update_shared_metrics(
    state: &AppState,
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

/// Emits the collected system stats to the frontend via IPC.
fn emit_system_stats(
    app: &AppHandle,
    sys: &System,
    system_cpu: f32,
    system_ram_pct: f32,
    vox_cpu: f32,
    vox_ram_mb: u32,
    thread_count: u32,
) {
    let payload = SystemStatsPayload {
        system_cpu,
        system_ram_pct,
        vox_cpu,
        vox_ram_mb,
        threads: thread_count,
        total_memory_gb: sys.total_memory() / 1024 / 1024 / 1024,
        cpu_count: sys.cpus().len(),
    };

    if let Err(e) = emit_ipc(app, IpcEvent::SystemStats(payload)) {
        log::warn!(
            "[Monitoring::SystemMonitor] Failed to emit system_stats: {}",
            e
        );
    }
}

/// Reads the current audio band levels, preferring playback levels while speaking.
fn get_current_audio_levels(state: &AppState) -> (f32, f32, f32, f32) {
    if state.pipeline.state() == InteractionState::Speaking {
        (
            f32::from_bits(
                state
                    .telemetry
                    .latest_playback_energy
                    .load(Ordering::Relaxed),
            ),
            f32::from_bits(state.telemetry.latest_playback_low.load(Ordering::Relaxed)),
            f32::from_bits(state.telemetry.latest_playback_mid.load(Ordering::Relaxed)),
            f32::from_bits(state.telemetry.latest_playback_high.load(Ordering::Relaxed)),
        )
    } else {
        (
            f32::from_bits(state.telemetry.latest_energy.load(Ordering::Relaxed)),
            f32::from_bits(state.telemetry.latest_low.load(Ordering::Relaxed)),
            f32::from_bits(state.telemetry.latest_mid.load(Ordering::Relaxed)),
            f32::from_bits(state.telemetry.latest_high.load(Ordering::Relaxed)),
        )
    }
}

/// Resolves the frontend window that owns the current interaction.
fn get_target_window(state: &AppState) -> AppWindow {
    let owner_enum: InteractionOwner = state.owner.load(Ordering::Relaxed).into();
    match owner_enum {
        InteractionOwner::Assistant => AppWindow::Main,
        InteractionOwner::Dictation => AppWindow::Tray,
    }
}
