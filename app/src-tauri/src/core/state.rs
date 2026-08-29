use crate::core::settings::VoxSettings;
use crate::services::audio::AudioStream;
use crate::services::stt::SttCommand;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, RwLock};
use tokio::sync::Mutex;

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

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
#[repr(u32)]
pub enum InteractionState {
    Idle = 0,
    Ready = 1,
    Listening = 2,
    Thinking = 3,
    Speaking = 4,
    Paused = 5,
    Error = 6,
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
            _ => InteractionState::Idle,
        }
    }
}

impl From<InteractionState> for u32 {
    fn from(state: InteractionState) -> Self {
        state as u32
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct TelemetryData {
    pub energy: f32,
    pub vad_prob: f32,
    pub low: f32,
    pub mid: f32,
    pub high: f32,
}

pub enum VadCommand {
    UpdateThreshold(f32),
    UpdateNoiseGate(f32),
    UpdateMode(crate::core::settings::InteractionMode),
    UpdateAudioMode(crate::core::settings::AudioOutputMode),
    SetOperationalMode(crate::services::vad::VadOperationalMode),
    SetAudioSink(Option<tokio::sync::mpsc::Sender<Vec<f32>>>),
    StartWindowValidation,
    StopWindowValidation {
        response_tx: tokio::sync::oneshot::Sender<crate::services::vad::VadValidationResult>,
    },
    Shutdown,
    StartRealtime {
        tx: tokio::sync::mpsc::Sender<Vec<i16>>,
        is_ptt: bool,
    },
    StopRealtime,
}

pub struct VoxEngine {
    pub audio_stream: AudioStream,
    pub stt_tx: std::sync::mpsc::Sender<SttCommand>,
    pub vad_tx: std::sync::mpsc::Sender<VadCommand>,
    pub llm_tx: Option<std::sync::mpsc::Sender<crate::services::llm::LlmCommand>>,
    pub tts_tx: Option<std::sync::mpsc::Sender<crate::services::tts::TtsCommand>>,
    pub telemetry_tx: crossbeam_channel::Sender<crate::monitoring::aggregator::TelemetryEvent>,
    pub pipeline_tx: std::sync::mpsc::Sender<crate::core::events::VoxEvent>,
    pub playback_engine: Arc<crate::services::audio::PlaybackEngine>,
    pub stt_handle: Option<std::thread::JoinHandle<()>>,
    pub vad_handle: Option<std::thread::JoinHandle<()>>,
    pub llm_handle: Option<std::thread::JoinHandle<()>>,
    pub tts_handle: Option<std::thread::JoinHandle<()>>,
    pub orchestrator_handle: Option<std::thread::JoinHandle<()>>,
    pub forwarder_handle: Option<tauri::async_runtime::JoinHandle<()>>,
}

pub struct PipelineAtomics {
    pub cancel_flag: Arc<AtomicBool>,
    pub is_paused: Arc<AtomicBool>,
    pub playback_active: Arc<AtomicBool>,
    pub llm_generating: Arc<AtomicBool>,
    pub tts_generating: Arc<AtomicBool>,
    pub turn_id: Arc<AtomicU32>,
    pub is_engaged: Arc<AtomicBool>,
    pub transcript_history: Arc<parking_lot::Mutex<VecDeque<String>>>,
    pub playback_underruns: Arc<std::sync::atomic::AtomicU64>,
    pub is_assistant_speaking: Arc<AtomicBool>,
    pub current_state_atomic: Arc<std::sync::atomic::AtomicU32>,
    pub engine_shutdown: Arc<AtomicBool>,
}

impl Default for PipelineAtomics {
    fn default() -> Self {
        Self::new()
    }
}

impl PipelineAtomics {
    pub fn new() -> Self {
        Self {
            cancel_flag: Arc::new(AtomicBool::new(false)),
            is_paused: Arc::new(AtomicBool::new(false)),
            playback_active: Arc::new(AtomicBool::new(false)),
            llm_generating: Arc::new(AtomicBool::new(false)),
            tts_generating: Arc::new(AtomicBool::new(false)),
            turn_id: Arc::new(AtomicU32::new(0)),
            is_engaged: Arc::new(AtomicBool::new(false)),
            transcript_history: Arc::new(parking_lot::Mutex::new(VecDeque::with_capacity(
                crate::core::constants::TRANSCRIPT_HISTORY_LIMIT,
            ))),
            playback_underruns: Arc::new(std::sync::atomic::AtomicU64::new(0)),
            is_assistant_speaking: Arc::new(AtomicBool::new(false)),
            current_state_atomic: Arc::new(std::sync::atomic::AtomicU32::new(
                InteractionState::Idle as u32,
            )),
            engine_shutdown: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Returns the current interaction state derived from the canonical atomic state.
    pub fn state(&self) -> InteractionState {
        InteractionState::from(self.current_state_atomic.load(Ordering::Relaxed))
    }

    /// Updates internal interaction state atomics.
    pub fn set_state(&self, new_state: InteractionState) {
        self.is_assistant_speaking.store(
            new_state == InteractionState::Speaking,
            Ordering::Relaxed,
        );
        self.current_state_atomic
            .store(new_state as u32, Ordering::Relaxed);
    }
}

pub struct MemoryAppState {
    pub graph_version: Arc<AtomicU64>,
    pub pipeline_paused: Arc<AtomicBool>,
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
            pipeline_paused: Arc::new(AtomicBool::new(false)),
        }
    }
}

pub struct AppState {
    pub engine: Mutex<Option<VoxEngine>>,
    pub realtime_engine: Mutex<Option<crate::services::realtime::engine::RealtimeEngine>>,
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
    pub is_dictation_enabled: Arc<AtomicBool>,
    pub is_llm_loaded: Arc<AtomicBool>,
    pub is_tts_loaded: Arc<AtomicBool>,
    pub is_stt_loaded: Arc<AtomicBool>,
    pub is_vad_loaded: Arc<AtomicBool>,
    pub is_embedder_loaded: Arc<AtomicBool>,
    pub is_query_classifier_loaded: Arc<AtomicBool>,
    pub is_intra_edge_classifier_loaded: Arc<AtomicBool>,
    pub is_inter_edge_classifier_loaded: Arc<AtomicBool>,
    pub is_translit_loaded: Arc<AtomicBool>,
    pub is_sleeping: Arc<AtomicBool>,
    pub runtime_status: Arc<std::sync::atomic::AtomicU32>,
    pub main_window_destroyed: Arc<AtomicBool>,
    pub persist_tx: parking_lot::Mutex<
        Option<crossbeam_channel::Sender<crate::persistence::events::PersistenceEvent>>,
    >,
    pub memory_tx: parking_lot::Mutex<
        Option<crossbeam_channel::Sender<crate::persistence::memory_worker::MemoryWorkerEvent>>,
    >,
    pub dropped_persistence_events: Arc<std::sync::atomic::AtomicU64>,
    pub monitoring: Arc<crate::monitoring::runtime_state::MonitoringState>,
    pub model_manager: Arc<crate::setup::model_manager::ModelManager>,
    pub manifest: Arc<tokio::sync::RwLock<Option<crate::setup::manifest::VoxManifest>>>,
    pub cpu_governor: parking_lot::Mutex<String>,
    pub cpu_governor_optimal: Arc<AtomicBool>,
    pub setup_running: Arc<Mutex<bool>>,
    pub conversation_manager: Arc<parking_lot::Mutex<crate::services::memory::ConversationManager>>,
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
        let dictation_enabled = settings.dictation.enabled;
        let initial_ctx_size = settings.llm.context_window as usize;
        telemetry.is_private_mode.store(settings.history.private_mode, Ordering::Relaxed);

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
            is_dictation_enabled: Arc::new(AtomicBool::new(dictation_enabled)),
            is_llm_loaded: Arc::new(AtomicBool::new(false)),
            is_tts_loaded: Arc::new(AtomicBool::new(false)),
            is_stt_loaded: Arc::new(AtomicBool::new(false)),
            is_vad_loaded: Arc::new(AtomicBool::new(false)),
            is_embedder_loaded: Arc::new(AtomicBool::new(false)),
            is_query_classifier_loaded: Arc::new(AtomicBool::new(false)),
            is_intra_edge_classifier_loaded: Arc::new(AtomicBool::new(false)),
            is_inter_edge_classifier_loaded: Arc::new(AtomicBool::new(false)),
            is_translit_loaded: Arc::new(AtomicBool::new(false)),
            is_sleeping: Arc::new(AtomicBool::new(false)),
            runtime_status: Arc::new(AtomicU32::new(RuntimeStatus::Initializing as u32)),
            main_window_destroyed: Arc::new(AtomicBool::new(false)),
            persist_tx: parking_lot::Mutex::new(None),
            memory_tx: parking_lot::Mutex::new(None),
            dropped_persistence_events: Arc::new(std::sync::atomic::AtomicU64::new(0)),
            monitoring: Arc::new(crate::monitoring::runtime_state::MonitoringState::new()),
            model_manager,
            manifest,
            cpu_governor: parking_lot::Mutex::new("ondemand".into()),
            cpu_governor_optimal: Arc::new(AtomicBool::new(true)),
            setup_running: Arc::new(Mutex::new(false)),
            conversation_manager: Arc::new(parking_lot::Mutex::new(
                crate::services::memory::ConversationManager::new(initial_ctx_size),
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
