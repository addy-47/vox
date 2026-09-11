use std::{
    collections::VecDeque,
    sync::{atomic::Ordering, Arc, RwLock},
    thread,
    time::{SystemTime, UNIX_EPOCH},
};

use crate::{
    core::{
        settings::LlmActiveProvider,
        state::{AppState, InteractionOwner, InteractionState},
    },
    monitoring::{COLLECTOR_TICK_INTERVAL, MAX_SNAPSHOT_HISTORY},
    services::{memory::is_embedder_loaded, translit::is_transliteration_engine_loaded},
    utils::check_cpu_governor,
};

/// A normalized, read-only snapshot of the Vox engine runtime state.
/// This is the primary source of truth for the frontend monitoring UI.
#[derive(Debug, Clone, serde::Serialize)]
pub struct RuntimeSnapshot {
    /// Current pipeline state (Idle, Ready, Listening, Thinking, Speaking, Paused, Error)
    pub pipeline_state: String,
    /// Ephemeral turn ID for the current interaction.
    pub current_turn_id: u32,
    /// Persistent conversation session ID. 0 if inactive (Tray mode).
    pub conversation_id: u64,

    /// System activity flags.
    pub playback_active: bool,

    /// System resource utilization.
    pub system_cpu_usage: f32,
    pub system_ram_mb: u32,
    pub vox_cpu_usage: f32,
    pub vox_ram_mb: u32,
    pub total_ram_mb: u32,
    pub cpu_cores: u32,

    /// Real-time VAD characteristics.
    pub vad_energy: f32,
    pub vad_probability: f32,

    /// Latency metrics for the last completed turn.
    pub stt_latency_ms: Option<u32>,
    pub ttft_ms: Option<u32>,
    pub total_voice_latency_ms: Option<u32>,

    /// Persistence health.
    pub persistence_queue_depth: usize,
    pub dropped_persistence_events: u64,

    /// Playback health.
    pub playback_buffer_samples: usize,
    pub playback_underruns: u64,

    /// Current interaction owner (Dictation, Assistant).
    pub active_owner: String,

    /// Extended Monitoring Metrics
    pub active_threads: u32,
    pub tts_rtf: Option<f32>,
    pub playback_start_ms: Option<u32>,
    pub persistence_writes_per_sec: f32,
    pub is_db_healthy: bool,

    // Tier Status (Model Residency)
    pub is_llm_loaded: bool,
    pub llm_provider_kind: String,
    pub is_tts_loaded: bool,
    pub is_stt_loaded: bool,
    pub is_vad_loaded: bool,
    pub is_embedder_loaded: bool,
    pub is_query_classifier_loaded: bool,
    pub is_intra_edge_classifier_loaded: bool,
    pub is_inter_edge_classifier_loaded: bool,
    pub is_translit_loaded: bool,

    /// CPU frequency governor (Linux only, e.g. "powersave", "performance"). Empty string if unavailable.
    pub cpu_governor: String,
    /// Whether the CPU governor is optimal ("performance"). False if unknown/non-Linux.
    pub cpu_governor_optimal: bool,

    /// Optional per-WebView RAM breakdown in MB (Measured via sysinfo descendant enumeration)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub main_webview_ram_mb: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tray_webview_ram_mb: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wizard_webview_ram_mb: Option<u32>,

    /// Unix timestamp of the snapshot in milliseconds.
    pub timestamp_ms: u64,
}

/// Shared thread-safe state for runtime monitoring.
pub struct MonitoringState {
    history: Arc<RwLock<VecDeque<RuntimeSnapshot>>>,
    latest: Arc<RwLock<Option<RuntimeSnapshot>>>,
}

impl Default for MonitoringState {
    fn default() -> Self {
        Self::new()
    }
}

impl MonitoringState {
    /// Creates a new empty MonitoringState instance.
    pub fn new() -> Self {
        Self {
            history: Arc::new(RwLock::new(VecDeque::with_capacity(MAX_SNAPSHOT_HISTORY))),
            latest: Arc::new(RwLock::new(None)),
        }
    }

    /// Adds a new snapshot to the history, evicting the oldest if capacity is exceeded.
    pub fn push(&self, snapshot: RuntimeSnapshot) {
        let mut latest = self.latest.write().unwrap_or_else(|e| e.into_inner());
        *latest = Some(snapshot.clone());

        let mut history = self.history.write().unwrap_or_else(|e| e.into_inner());
        history.push_back(snapshot);
        if history.len() > MAX_SNAPSHOT_HISTORY {
            history.pop_front();
        }
    }

    /// Gets the most recent snapshot if available.
    pub fn get_latest(&self) -> Option<RuntimeSnapshot> {
        let guard = self.latest.read().unwrap_or_else(|e| e.into_inner());
        guard.clone()
    }

    /// Gets the full history of recorded snapshots.
    pub fn get_history(&self) -> Vec<RuntimeSnapshot> {
        let guard = self.history.read().unwrap_or_else(|e| e.into_inner());
        guard.iter().cloned().collect()
    }

    /// Clears all recorded snapshot history and latest state.
    pub fn clear(&self) {
        let mut history = self.history.write().unwrap_or_else(|e| e.into_inner());
        history.clear();
        let mut latest = self.latest.write().unwrap_or_else(|e| e.into_inner());
        *latest = None;
    }
}

/// Spawn the Monitoring Collector on a dedicated OS thread.
pub fn spawn_monitoring_collector(state: Arc<AppState>) {
    thread::Builder::new()
        .name("vox-monitor".to_string())
        .spawn(move || {
            log::info!("[Monitoring::Collector] Collector worker started (10Hz)");

            let mut sys = sysinfo::System::new_with_specifics(
                sysinfo::RefreshKind::new()
                    .with_memory(sysinfo::MemoryRefreshKind::everything())
                    .with_cpu(sysinfo::CpuRefreshKind::everything()),
            );
            sys.refresh_memory();
            let total_ram_mb = (sys.total_memory() / 1024 / 1024) as u32;
            let cpu_cores = sys.cpus().len() as u32;

            let mut tick_count: u64 = 0;
            loop {
                tick_count = tick_count.wrapping_add(1);
                if tick_count.is_multiple_of(50) {
                    if let Some(governor) = check_cpu_governor() {
                        let is_optimal = governor == "performance";
                        *state.cpu_governor.lock() = governor;
                        state
                            .cpu_governor_optimal
                            .store(is_optimal, Ordering::Relaxed);
                    }
                }
                let threads = state.telemetry.latest_threads.load(Ordering::Relaxed);
                let snapshot = collect_snapshot(&state, threads, total_ram_mb, cpu_cores);
                state.monitoring.push(snapshot);
                thread::sleep(COLLECTOR_TICK_INTERVAL);
            }
        })
        .expect("[Monitoring::Collector] Failed to spawn monitoring collector thread");
}

/// Maps the pipeline state atomic to its frontend display string.
fn map_pipeline_state_string(state_u32: u32) -> String {
    match InteractionState::from(state_u32) {
        InteractionState::Idle => "Idle".into(),
        InteractionState::Ready => "Ready".into(),
        InteractionState::Listening => "Listening".into(),
        InteractionState::Thinking => "Thinking".into(),
        InteractionState::Speaking => "Speaking".into(),
        InteractionState::Paused => "Paused".into(),
        InteractionState::Error => "Error".into(),
        InteractionState::Sleeping => "Sleeping".into(),
        InteractionState::Working => "Working".into(),
    }
}

/// Reads the current playback buffer length without blocking the audio path.
fn get_playback_buffer_samples(state: &AppState) -> usize {
    if let Ok(lock) = state.engine.try_lock() {
        if let Some(engine) = lock.as_ref() {
            engine.playback_engine.buffer_len()
        } else {
            0
        }
    } else {
        0
    }
}

/// Resolves the active LLM provider kind for snapshot display.
fn get_llm_provider_kind(state: &AppState) -> String {
    let settings = match state.settings.read() {
        Ok(s) => s,
        Err(_) => return "embedded".to_string(),
    };
    match settings.llm.active {
        LlmActiveProvider::Embedded => "embedded".to_string(),
        LlmActiveProvider::Server => {
            if let Some(ref name) = settings.llm.server.provider_name {
                format!("server:{}", name.to_lowercase())
            } else {
                "server".to_string()
            }
        }
        LlmActiveProvider::Cloud => {
            if let Some(ref name) = settings.llm.cloud.provider_name {
                format!("cloud:{}", name.to_lowercase())
            } else {
                "cloud".to_string()
            }
        }
    }
}

/// Builds a single runtime snapshot from the shared telemetry atomics and engine state.
fn collect_snapshot(
    state: &AppState,
    threads: u32,
    total_ram_mb: u32,
    cpu_cores: u32,
) -> RuntimeSnapshot {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;

    let pa = &state.pipeline;
    let pipeline_state = map_pipeline_state_string(pa.current_state_atomic.load(Ordering::Relaxed));
    let owner_enum: InteractionOwner = state.owner.load(Ordering::Relaxed).into();
    let owner = format!("{:?}", owner_enum);
    let buffer_samples = get_playback_buffer_samples(state);
    let sys_ram_pct = f32::from_bits(state.telemetry.latest_sys_ram.load(Ordering::Relaxed));
    let llm_provider_kind = get_llm_provider_kind(state);

    RuntimeSnapshot {
        pipeline_state,
        current_turn_id: pa.turn_id.load(Ordering::Relaxed),
        conversation_id: state.conversation_id.load(Ordering::Relaxed),

        playback_active: pa.state() == InteractionState::Speaking,

        system_cpu_usage: f32::from_bits(state.telemetry.latest_sys_cpu.load(Ordering::Relaxed)),
        system_ram_mb: (sys_ram_pct * 0.01 * total_ram_mb as f32) as u32,
        vox_cpu_usage: f32::from_bits(state.telemetry.latest_vox_cpu.load(Ordering::Relaxed)),
        vox_ram_mb: state.telemetry.latest_vox_ram.load(Ordering::Relaxed),
        total_ram_mb,
        cpu_cores,

        vad_energy: f32::from_bits(state.telemetry.latest_energy.load(Ordering::Relaxed)),
        vad_probability: f32::from_bits(state.telemetry.latest_vad_prob.load(Ordering::Relaxed)),

        stt_latency_ms: Some(state.telemetry.latest_stt_ms.load(Ordering::Relaxed))
            .filter(|&v| v > 0),
        ttft_ms: Some(state.telemetry.latest_ttft_ms.load(Ordering::Relaxed)).filter(|&v| v > 0),
        total_voice_latency_ms: Some(
            state
                .telemetry
                .latest_voice_latency_ms
                .load(Ordering::Relaxed),
        )
        .filter(|&v| v > 0),

        persistence_queue_depth: state
            .persist_tx
            .lock()
            .as_ref()
            .map(|tx| tx.len())
            .unwrap_or(0),
        dropped_persistence_events: state.dropped_persistence_events.load(Ordering::Relaxed),

        playback_buffer_samples: buffer_samples,
        playback_underruns: pa.playback_underruns.load(Ordering::Relaxed),

        active_owner: owner,

        active_threads: threads,
        tts_rtf: {
            let bits = state.telemetry.latest_tts_rtf.load(Ordering::Relaxed);
            let val = f32::from_bits(bits);
            if val > 0.0 {
                Some(val)
            } else {
                None
            }
        },
        playback_start_ms: Some(
            state
                .telemetry
                .latest_playback_start_ms
                .load(Ordering::Relaxed),
        )
        .filter(|&v| v > 0),
        persistence_writes_per_sec: f32::from_bits(
            state
                .telemetry
                .latest_persistence_rate
                .load(Ordering::Relaxed),
        ),
        is_db_healthy: state.telemetry.is_db_healthy.load(Ordering::Relaxed),

        is_llm_loaded: state
            .engine
            .try_lock()
            .map(|e| e.as_ref().map(|eng| eng.llm_tx.is_some()).unwrap_or(false))
            .unwrap_or(false),
        llm_provider_kind,
        is_tts_loaded: state
            .engine
            .try_lock()
            .map(|e| e.as_ref().map(|eng| eng.tts_tx.is_some()).unwrap_or(false))
            .unwrap_or(false),
        is_stt_loaded: state
            .engine
            .try_lock()
            .map(|e| {
                e.as_ref()
                    .map(|eng| eng.stt_handle.is_some())
                    .unwrap_or(false)
            })
            .unwrap_or(false),
        is_vad_loaded: state
            .engine
            .try_lock()
            .map(|e| {
                e.as_ref()
                    .map(|eng| eng.vad_handle.is_some())
                    .unwrap_or(false)
            })
            .unwrap_or(false),
        is_embedder_loaded: is_embedder_loaded(),
        is_query_classifier_loaded: false,
        is_intra_edge_classifier_loaded: false,
        is_inter_edge_classifier_loaded: false,
        is_translit_loaded: is_transliteration_engine_loaded(),
        cpu_governor: state.cpu_governor.lock().clone(),
        cpu_governor_optimal: state.cpu_governor_optimal.load(Ordering::Relaxed),

        main_webview_ram_mb: None,
        tray_webview_ram_mb: None,
        wizard_webview_ram_mb: None,

        timestamp_ms: now,
    }
}
