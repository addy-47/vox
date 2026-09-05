use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, RwLock};

use tokio::sync::Mutex;

use crate::core::events::VoxEvent;
use crate::core::settings::VoxSettings;
use crate::services::audio::AudioStream;
use crate::services::stt::SttCommand;

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum InteractionOwner {
    Dictation = 0,
    Assistant = 1,
}

impl From<u32> for InteractionOwner {
    fn from(v: u32) -> Self {
        match v {
            1 => InteractionOwner::Assistant,
            _ => InteractionOwner::Dictation,
        }
    }
}

impl From<InteractionOwner> for u32 {
    fn from(owner: InteractionOwner) -> Self {
        owner as u32
    }
}

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize)]
pub enum RuntimeStatus {
    Initializing,
    Ready,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[repr(u32)]
pub enum InteractionState {
    Idle = 0,
    Ready = 1,
    Listening = 2,
    Thinking = 3,
    Speaking = 4,
    Paused = 5,
    Error = 6,
    Sleeping = 7,
}

impl From<u32> for InteractionState {
    fn from(v: u32) -> Self {
        match v {
            1 => InteractionState::Ready,
            2 => InteractionState::Listening,
            3 => InteractionState::Thinking,
            4 => InteractionState::Speaking,
            5 => InteractionState::Paused,
            6 => InteractionState::Error,
            7 => InteractionState::Sleeping,
            _ => InteractionState::Idle,
        }
    }
}

impl From<InteractionState> for u32 {
    fn from(state: InteractionState) -> Self {
        state as u32
    }
}

pub struct VoxEngine {
    pub audio_stream: AudioStream,
    pub stt_tx: mpsc::Sender<SttCommand>,
    pub vad_tx: mpsc::Sender<crate::services::vad::VadCommand>,
    pub llm_tx: Option<mpsc::Sender<crate::services::llm::LlmCommand>>,
    pub tts_tx: Option<mpsc::Sender<crate::services::tts::TtsCommand>>,
    pub telemetry_tx: crossbeam_channel::Sender<crate::monitoring::aggregator::TelemetryEvent>,
    pub pipeline_tx: mpsc::Sender<VoxEvent>,
    pub playback_engine: Arc<crate::services::audio::PlaybackEngine>,
    pub stt_handle: Option<std::thread::JoinHandle<()>>,
    pub vad_handle: Option<std::thread::JoinHandle<()>>,
    pub llm_handle: Option<std::thread::JoinHandle<()>>,
    pub tts_handle: Option<std::thread::JoinHandle<()>>,
    pub orchestrator_handle: Option<std::thread::JoinHandle<()>>,
}

pub struct PipelineAtomics {
    pub cancel_flag: Arc<AtomicBool>,
    pub turn_id: Arc<AtomicU32>,
    pub transcript_history: Arc<parking_lot::Mutex<VecDeque<String>>>,
    pub playback_underruns: Arc<AtomicU64>,
    pub pending_synthesis_jobs: Arc<AtomicU32>,
    pub current_state_atomic: Arc<AtomicU32>,
    pub state_tx: tokio::sync::watch::Sender<InteractionState>,
    pub state_rx: tokio::sync::watch::Receiver<InteractionState>,
    pub dictation_state_atomic: Arc<AtomicU32>,
    pub dictation_state_tx: tokio::sync::watch::Sender<InteractionState>,
    pub dictation_state_rx: tokio::sync::watch::Receiver<InteractionState>,
    pub ingestion_gate: Arc<AtomicBool>,
    pub turn_token: Arc<parking_lot::Mutex<tokio_util::sync::CancellationToken>>,
    pub engine_shutdown: Arc<AtomicBool>,
}

impl Default for PipelineAtomics {
    fn default() -> Self {
        Self::new()
    }
}

impl PipelineAtomics {
    pub fn new() -> Self {
        let (state_tx, state_rx) = tokio::sync::watch::channel(InteractionState::Idle);
        let (dictation_state_tx, dictation_state_rx) =
            tokio::sync::watch::channel(InteractionState::Idle);
        Self {
            cancel_flag: Arc::new(AtomicBool::new(false)),
            turn_id: Arc::new(AtomicU32::new(0)),
            transcript_history: Arc::new(parking_lot::Mutex::new(VecDeque::with_capacity(
                crate::core::constants::TRANSCRIPT_HISTORY_LIMIT,
            ))),
            playback_underruns: Arc::new(AtomicU64::new(0)),
            pending_synthesis_jobs: Arc::new(AtomicU32::new(0)),
            current_state_atomic: Arc::new(AtomicU32::new(InteractionState::Idle as u32)),
            state_tx,
            state_rx,
            dictation_state_atomic: Arc::new(AtomicU32::new(InteractionState::Idle as u32)),
            dictation_state_tx,
            dictation_state_rx,
            ingestion_gate: Arc::new(AtomicBool::new(false)),
            turn_token: Arc::new(parking_lot::Mutex::new(
                tokio_util::sync::CancellationToken::new(),
            )),
            engine_shutdown: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Recomputes the lock-free audio ingestion gate based on dual-track states.
    /// Invariant: Gate Is Open <=> (assistant in {Ready, Listening, Thinking, Speaking}) || (dictation in {Ready, Listening, Thinking})
    pub fn update_ingestion_gate(&self) {
        let a = InteractionState::from(self.current_state_atomic.load(Ordering::Relaxed));
        let d = InteractionState::from(self.dictation_state_atomic.load(Ordering::Relaxed));
        let open = matches!(
            a,
            InteractionState::Ready
                | InteractionState::Listening
                | InteractionState::Thinking
                | InteractionState::Speaking
        ) || matches!(
            d,
            InteractionState::Ready | InteractionState::Listening | InteractionState::Thinking
        );
        self.ingestion_gate.store(open, Ordering::Relaxed);
    }

    /// Returns the current interaction state derived from the canonical atomic state.
    pub fn state(&self) -> InteractionState {
        InteractionState::from(self.current_state_atomic.load(Ordering::SeqCst))
    }

    /// Updates internal interaction state atomics and notifies all observers.
    pub fn set_state(&self, new_state: InteractionState) {
        self.current_state_atomic
            .store(new_state as u32, Ordering::SeqCst);
        self.update_ingestion_gate();
        if let Err(e) = self.state_tx.send(new_state) {
            log::warn!(
                "[Pipeline::State] Failed to broadcast state to observers: {}",
                e
            );
        }
    }

    /// Subscribes to the broadcast interaction state channel for multi-consumer fanout.
    pub fn subscribe_state(&self) -> tokio::sync::watch::Receiver<InteractionState> {
        self.state_tx.subscribe()
    }

    /// Returns the current dictation state derived from the canonical atomic state.
    pub fn dictation_state(&self) -> InteractionState {
        InteractionState::from(self.dictation_state_atomic.load(Ordering::SeqCst))
    }

    /// Updates internal dictation state atomics and notifies all observers.
    pub fn set_dictation_state(&self, new_state: InteractionState) {
        self.dictation_state_atomic
            .store(new_state as u32, Ordering::SeqCst);
        self.update_ingestion_gate();
        if let Err(e) = self.dictation_state_tx.send(new_state) {
            log::warn!(
                "[Pipeline::State] Failed to broadcast dictation state: {}",
                e
            );
        }
    }

    /// Subscribes to the broadcast dictation state channel for multi-consumer fanout.
    pub fn subscribe_dictation_state(&self) -> tokio::sync::watch::Receiver<InteractionState> {
        self.dictation_state_tx.subscribe()
    }

    /// Returns a clone of the current turn's cancellation token.
    pub fn turn_token(&self) -> tokio_util::sync::CancellationToken {
        self.turn_token.lock().clone()
    }

    /// Cancels the active turn's token and returns a fresh cancellation token without allocating a new turn ID.
    /// Use this for session-level re-arming (e.g. on resume or test clip cancellation).
    pub fn rearm_turn_token(&self) -> tokio_util::sync::CancellationToken {
        let mut guard = self.turn_token.lock();
        guard.cancel();
        let new_token = tokio_util::sync::CancellationToken::new();
        *guard = new_token.clone();
        new_token
    }

    /// Atomically allocates the next monotonic turn ID.
    pub fn next_turn_id(&self) -> u32 {
        self.turn_id.fetch_add(1, Ordering::Relaxed) + 1
    }

    /// Returns the current turn ID without incrementing.
    pub fn peek_turn_id(&self) -> u32 {
        self.turn_id.load(Ordering::Relaxed)
    }

    /// Atomically increments turn_id and rotates the per-turn cancellation token.
    pub fn next_turn(&self) -> (u32, tokio_util::sync::CancellationToken) {
        let id = self.next_turn_id();
        let tok = self.rearm_turn_token();
        (id, tok)
    }

    /// Cancels the current turn's cancellation token without allocating a new turn.
    pub fn cancel_current_turn(&self) {
        self.turn_token.lock().cancel();
    }
}

pub struct MemoryAppState {
    pub graph_version: Arc<AtomicU64>,
    pub user_paused_ingestion: Arc<AtomicBool>,
}

impl Default for MemoryAppState {
    fn default() -> Self {
        Self::new()
    }
}

impl MemoryAppState {
    pub fn new() -> Self {
        Self {
            graph_version: Arc::new(AtomicU64::new(1)),
            user_paused_ingestion: Arc::new(AtomicBool::new(false)),
        }
    }
}

pub struct AppState {
    pub engine: Mutex<Option<VoxEngine>>,
    pub realtime_engine: Mutex<Option<crate::services::realtime::RealtimeActor>>,
    pub owner: Arc<AtomicU32>,
    pub hud_visible: Arc<AtomicBool>,
    pub memory: MemoryAppState,
    pub settings: Arc<RwLock<VoxSettings>>,
    pub hud_menu_item: parking_lot::Mutex<Option<tauri::menu::CheckMenuItem<tauri::Wry>>>,
    pub pipeline: PipelineAtomics,
    pub save_debounce: Mutex<Option<tauri::async_runtime::JoinHandle<()>>>,
    pub _log_guard: Option<tracing_appender::non_blocking::WorkerGuard>,
    pub telemetry: Arc<TelemetryState>,
    pub dictation_last_transcript: parking_lot::Mutex<Option<String>>,
    pub conversation_id: Arc<AtomicU64>,
    pub runtime_status: Arc<AtomicU32>,
    pub main_window_destroyed: Arc<AtomicBool>,
    pub persist_tx: parking_lot::Mutex<
        Option<crossbeam_channel::Sender<crate::persistence::events::PersistenceEvent>>,
    >,
    pub memory_tx: parking_lot::Mutex<
        Option<crossbeam_channel::Sender<crate::persistence::events::MemoryWorkerEvent>>,
    >,
    pub dropped_persistence_events: Arc<AtomicU64>,
    pub monitoring: Arc<crate::monitoring::runtime_state::MonitoringState>,
    pub model_manager: Arc<crate::setup::model_manager::ModelManager>,
    pub manifest: Arc<tokio::sync::RwLock<Option<crate::setup::manifest::VoxManifest>>>,
    pub cpu_governor: parking_lot::Mutex<String>,
    pub cpu_governor_optimal: Arc<AtomicBool>,
    pub setup_running: Arc<Mutex<bool>>,
    pub conversation_manager:
        Arc<parking_lot::Mutex<crate::services::harness::ConversationManager>>,
    pub llm_provider: Arc<parking_lot::RwLock<Option<Arc<dyn crate::services::llm::LlmProvider>>>>,
    pub event_tx: parking_lot::Mutex<Option<mpsc::Sender<VoxEvent>>>,
    pub pipeline_accumulator:
        Arc<parking_lot::Mutex<crate::pipeline::assistant::accumulator::TurnAccumulator>>,
}

/// Telemetry handles and health atomics bundled for AppState and monitoring workers.
#[derive(Clone)]
pub struct TelemetryState {
    pub telemetry_tx: crossbeam_channel::Sender<crate::monitoring::aggregator::TelemetryEvent>,
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

impl AppState {
    pub fn new<R: tauri::Runtime>(
        app_handle: &tauri::AppHandle<R>,
        log_guard: Option<tracing_appender::non_blocking::WorkerGuard>,
        telemetry: Arc<TelemetryState>,
    ) -> Self {
        let settings = VoxSettings::load();
        telemetry
            .is_private_mode
            .store(settings.history.private_mode, Ordering::Relaxed);

        let model_manager = Arc::new(crate::setup::model_manager::ModelManager::new(Some(
            app_handle.clone(),
        )));
        let manifest = Arc::new(tokio::sync::RwLock::new(None));

        Self {
            engine: Mutex::new(None),
            realtime_engine: Mutex::new(None),
            owner: Arc::new(AtomicU32::new(InteractionOwner::Dictation as u32)),
            hud_visible: Arc::new(AtomicBool::new(true)),
            memory: MemoryAppState::new(),
            settings: Arc::new(RwLock::new(settings)),
            hud_menu_item: parking_lot::Mutex::new(None),
            pipeline: PipelineAtomics::new(),
            save_debounce: Mutex::new(None),
            _log_guard: log_guard,
            telemetry: Arc::clone(&telemetry),
            dictation_last_transcript: parking_lot::Mutex::new(None),
            conversation_id: Arc::new(AtomicU64::new(0)),
            runtime_status: Arc::new(AtomicU32::new(RuntimeStatus::Initializing as u32)),
            main_window_destroyed: Arc::new(AtomicBool::new(false)),
            persist_tx: parking_lot::Mutex::new(None),
            memory_tx: parking_lot::Mutex::new(None),
            dropped_persistence_events: Arc::new(AtomicU64::new(0)),
            monitoring: Arc::new(crate::monitoring::runtime_state::MonitoringState::new()),
            model_manager,
            manifest,
            cpu_governor: parking_lot::Mutex::new("ondemand".into()),
            cpu_governor_optimal: Arc::new(AtomicBool::new(true)),
            setup_running: Arc::new(Mutex::new(false)),
            conversation_manager: Arc::new(parking_lot::Mutex::new(
                crate::services::harness::ConversationManager::new(),
            )),
            llm_provider: Arc::new(parking_lot::RwLock::new(None)),
            event_tx: parking_lot::Mutex::new(None),
            pipeline_accumulator: Arc::new(parking_lot::Mutex::new(
                crate::pipeline::assistant::accumulator::TurnAccumulator::new(),
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Tests bidirectional conversions between u32 and InteractionOwner with unknown value fallback.
    #[test]
    fn test_interaction_owner_conversions() {
        assert_eq!(InteractionOwner::from(0), InteractionOwner::Dictation);
        assert_eq!(InteractionOwner::from(1), InteractionOwner::Assistant);
        assert_eq!(InteractionOwner::from(42), InteractionOwner::Dictation);
        assert_eq!(
            InteractionOwner::from(u32::MAX),
            InteractionOwner::Dictation
        );

        assert_eq!(u32::from(InteractionOwner::Dictation), 0);
        assert_eq!(u32::from(InteractionOwner::Assistant), 1);
    }
}
