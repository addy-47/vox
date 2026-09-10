use std::{
    collections::VecDeque,
    sync::{
        atomic::{AtomicBool, AtomicU32, AtomicU64},
        Arc, RwLock,
    },
};

use crate::monitoring::{
    aggregator::TelemetryEvent, snapshot::RuntimeSnapshot, MAX_SNAPSHOT_HISTORY,
};

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

/// Shared thread-safe state for runtime monitoring.
pub struct MonitoringState {
    history: Arc<RwLock<VecDeque<RuntimeSnapshot>>>,
    latest: Arc<RwLock<Option<RuntimeSnapshot>>>,
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

impl Default for MonitoringState {
    fn default() -> Self {
        Self::new()
    }
}
