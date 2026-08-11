// use tauri::AppHandle;
use crate::core::settings::VoxSettings;
use crate::services::audio::AudioStream;
use crate::services::stt::SttCommand;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, RwLock};
use tokio::sync::Mutex;

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum InteractionOwner {
    Tray = 0,
    MainWindow = 1,
    Ptt = 2,
    Wizard = 3,
}

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize)]
pub enum RuntimeStatus {
    Initializing,
    Ready,
    Error,
}

impl From<u32> for InteractionOwner {
    fn from(v: u32) -> Self {
        match v {
            1 => InteractionOwner::MainWindow,
            2 => InteractionOwner::Ptt,
            3 => InteractionOwner::Wizard,
            _ => InteractionOwner::Tray,
        }
    }
}

impl From<u8> for InteractionOwner {
    fn from(v: u8) -> Self {
        match v {
            1 => InteractionOwner::MainWindow,
            2 => InteractionOwner::Ptt,
            3 => InteractionOwner::Wizard,
            _ => InteractionOwner::Tray,
        }
    }
}

impl From<InteractionOwner> for u8 {
    fn from(owner: InteractionOwner) -> Self {
        owner as u8
    }
}

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize)]
pub enum InteractionState {
    Idle,
    Listening,
    UserSpeaking,
    Thinking,
    AssistantSpeaking,
    Interrupted,
    MaintainingContext,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct TelemetryData {
    pub energy: f32,
    pub vad_prob: f32,
    pub low: f32,
    pub mid: f32,
    pub high: f32,
}

// ─── VadCommand ───────────────────────────────────────────────────────────────

/// Hot-update commands sent to the VAD worker thread via a dedicated channel.
///
/// This avoids locking `AppState.settings` on the real-time audio path.
/// The VAD thread holds its own local copies of threshold values and updates
/// them only when it receives a command here.
pub enum VadCommand {
    /// Update the VAD speech/silence classification threshold (0.0–1.0).
    UpdateThreshold(f32),
    /// Update the PTT noise gate floor to suppress sub-threshold RMS.
    UpdateNoiseGate(f32),
    /// Update the interaction mode (Passive, PTT, etc.)
    UpdateMode(crate::core::settings::InteractionMode),
    /// Update the interaction owner (Tray, MainWindow, Ptt)
    UpdateOwner(InteractionOwner),
    /// Update the audio output mode (Speaker, Headset) for mic ducking logic
    UpdateAudioMode(crate::core::settings::AudioOutputMode),
    /// Gracefully shutdown the VAD worker.
    Shutdown,
    /// Enable realtime S2S audio routing.
    ///
    /// `is_ptt` controls how the VAD actor routes audio:
    /// - `false` (Passive): every chunk is forwarded to Gemini immediately;
    ///   Gemini's own cloud VAD handles speech detection.
    /// - `true` (PTT): forwarding is gated on `ptt.speech_detected`;
    ///   the client gates silence to prevent hallucinations.
    StartRealtime {
        tx: tokio::sync::mpsc::Sender<Vec<i16>>,
        is_ptt: bool,
    },
    /// Disable realtime S2S audio routing.
    StopRealtime,
}

// ─── VoxEngine ────────────────────────────────────────────────────────────────

pub struct VoxEngine {
    pub audio_stream: AudioStream,
    pub stt_tx: std::sync::mpsc::Sender<SttCommand>,
    /// Channel to send hot-updates to the VAD worker without locking AppState.
    pub vad_tx: std::sync::mpsc::Sender<VadCommand>,
    pub telemetry_tx: crossbeam_channel::Sender<crate::monitoring::aggregator::TelemetryEvent>,
    pub pipeline_tx: std::sync::mpsc::Sender<crate::core::events::VoxEvent>,
    pub playback_engine: Arc<crate::services::audio::PlaybackEngine>,

    // Lifecycle handles for deterministic cleanup
    pub stt_handle: Option<std::thread::JoinHandle<()>>,
    pub vad_handle: Option<std::thread::JoinHandle<()>>,
    pub orchestrator_handle: Option<std::thread::JoinHandle<()>>,
}

// ─── PttState ─────────────────────────────────────────────────────────────────

pub struct PttState {
    pub is_recording: std::sync::atomic::AtomicBool,
    pub turn_id: Arc<AtomicU32>,
    pub audio_buffer: parking_lot::Mutex<Vec<f32>>,
    pub samples_since_partial: std::sync::atomic::AtomicUsize,
    pub samples_since_waveform: std::sync::atomic::AtomicUsize,
    pub speech_detected: std::sync::atomic::AtomicBool,
    pub ptt_start_ms: std::sync::atomic::AtomicU64,
}

// ─── PipelineAtomics ──────────────────────────────────────────────────────────

/// Phase 4 shared atomics — checked on every inference iteration for cancellation.
///
/// Using `Arc<AtomicBool>` (not channels or async) because llama.cpp and onnxruntime
/// execute in blocking C++ loops that cannot be interrupted via Rust async primitives.
pub struct PipelineAtomics {
    /// Set to `true` to abort the current LLM + TTS + Playback turn immediately.
    pub cancel_flag: Arc<AtomicBool>,
    /// Set to `true` to temporarily freeze audio routing and playback.
    pub is_paused: Arc<AtomicBool>,
    /// `true` while the CPAL output stream is actively draining audio.
    /// In Speaker mode, the VAD loop drops mic frames while this is set.
    pub playback_active: Arc<AtomicBool>,
    /// `true` while the LLM worker is generating tokens.
    pub llm_generating: Arc<AtomicBool>,
    /// `true` while the TTS worker is synthesizing audio.
    pub tts_generating: Arc<AtomicBool>,
    /// Monotonically increasing turn counter. Increments on every TranscriptFinal.
    /// Used to reject stale pipeline events. NEVER persisted.
    /// Persistence identity is AppState::conversation_id.
    pub turn_id: Arc<AtomicU32>,
    /// Current interaction state (Idle, Listening, etc.)
    pub state: Arc<parking_lot::Mutex<InteractionState>>,
    /// `true` if the main application is "engaged" (active interaction).
    /// If false, the pipeline remains in a dormant, STT-only state.
    pub is_engaged: Arc<AtomicBool>,
    /// In-memory history of recent transcripts. Bridge to Phase 6.3 persistence.
    pub transcript_history: Arc<parking_lot::Mutex<VecDeque<String>>>,
    /// Track playback underruns for monitoring.
    pub playback_underruns: Arc<std::sync::atomic::AtomicU64>,
    /// `true` while the system is in AssistantSpeaking state.
    pub is_assistant_speaking: Arc<AtomicBool>,
    /// Atomic representation of InteractionState for lock-free monitoring.
    pub current_state_atomic: Arc<std::sync::atomic::AtomicU32>,
    /// Set to `true` when the entire engine is shutting down.
    /// All background threads must exit their main loops when this is set.
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
            state: Arc::new(parking_lot::Mutex::new(InteractionState::Idle)),
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

    /// Update internal state and emit IPC event to the **owning** window only.
    pub fn update_interaction_state(
        &self,
        new_state: InteractionState,
        owner: InteractionOwner,
        app_handle: &tauri::AppHandle,
    ) {
        let mut state_lock = self.state.lock();
        if *state_lock != new_state {
            log::debug!(
                "[Pipeline] State changed -> {:?} (Owner: {:?})",
                new_state,
                owner
            );
            *state_lock = new_state;

            // Update atomic flags for lock-free access in monitoring and audio callback
            self.is_assistant_speaking.store(
                new_state == InteractionState::AssistantSpeaking,
                Ordering::Relaxed,
            );
            self.current_state_atomic
                .store(new_state as u32, Ordering::Relaxed);

            let target = match owner {
                InteractionOwner::Tray => "tray",
                InteractionOwner::MainWindow | InteractionOwner::Ptt => "main",
                InteractionOwner::Wizard => "wizard",
            };
            let _ = tauri::Emitter::emit_to(app_handle, target, "state_changed", new_state);
        }
    }
}

// ─── MemoryAppState ───────────────────────────────────────────────────────────

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

// ─── AppState ─────────────────────────────────────────────────────────────────

pub struct AppState {
    pub engine: Mutex<Option<VoxEngine>>,
    pub realtime_engine: Mutex<Option<crate::services::realtime::engine::RealtimeEngine>>,
    pub owner: Arc<AtomicU32>,
    pub hud_visible: Mutex<bool>,
    pub memory: MemoryAppState,

    /// Settings protected by RwLock for concurrent read access.
    ///
    /// # Concurrency Contract
    /// - IPC handlers acquire `write()` only when mutating settings
    /// - NO real-time thread (VAD, STT, LLM, TTS, Playback callback) may call `read()` on the hot path
    /// - Hot-path settings (VAD threshold, noise gate) are snapshotted into worker-local variables
    ///   at startup and updated via `VadCommand` / other worker channels on change
    pub settings: Arc<RwLock<VoxSettings>>,

    pub hud_menu_item: Mutex<Option<tauri::menu::CheckMenuItem<tauri::Wry>>>,
    pub ptt: PttState,

    /// Phase 4: shared pipeline cancellation and status atomics.
    pub pipeline: PipelineAtomics,

    /// Debounce handle for settings disk writes.
    /// Cancelled and respawned on each `update_setting` IPC call.
    pub save_debounce: Mutex<Option<tauri::async_runtime::JoinHandle<()>>>,

    /// Async log writer guard. Must be held to ensure logs are flushed.
    pub _log_guard: Option<tracing_appender::non_blocking::WorkerGuard>,
    /// Structured telemetry bus (crossbeam — lock-free, safe for hot-path threads).
    pub telemetry_tx: crossbeam_channel::Sender<crate::monitoring::aggregator::TelemetryEvent>,
    /// Long-lived conversation session ID. 0 = no active session (Tray mode).
    /// Created on Engage, destroyed on Disengage. Persistence worker ignores events with id == 0.
    pub conversation_id: Arc<AtomicU64>,

    /// Latest VAD characteristics for monitoring (Atomic f32 via bit storage).
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
    pub latest_tts_rtf: Arc<AtomicU32>, // f32 bits
    pub latest_playback_start_ms: Arc<AtomicU32>,
    pub latest_persistence_rate: Arc<AtomicU32>, // f32 bits
    pub is_db_healthy: Arc<AtomicBool>,
    pub is_private_mode: Arc<AtomicBool>,
    pub is_llm_loaded: Arc<AtomicBool>,
    pub is_tts_loaded: Arc<AtomicBool>,
    pub is_stt_loaded: Arc<AtomicBool>,
    pub is_vad_loaded: Arc<AtomicBool>,
    pub is_sleeping: Arc<AtomicBool>,
    pub runtime_status: Arc<std::sync::atomic::AtomicU32>, // RuntimeStatus as u32

    /// Persistence worker channel. None if persistence is disabled or hibernating.
    pub persist_tx: parking_lot::Mutex<
        Option<crossbeam_channel::Sender<crate::persistence::events::PersistenceEvent>>,
    >,
    /// Background memory worker channel. None if memory worker is disabled.
    pub memory_tx: parking_lot::Mutex<
        Option<crossbeam_channel::Sender<crate::persistence::memory_worker::MemoryWorkerEvent>>,
    >,
    /// Track dropped persistence events for monitoring.
    pub dropped_persistence_events: Arc<std::sync::atomic::AtomicU64>,
    /// Track dropped telemetry events to prevent I/O blocking on hot-paths.
    pub dropped_telemetry_events: Arc<std::sync::atomic::AtomicU64>,
    /// Monitoring state (snapshots + history).
    pub monitoring: Arc<crate::monitoring::runtime_state::MonitoringState>,

    /// Phase 7 Setup
    pub model_manager: Arc<crate::setup::model_manager::ModelManager>,
    pub manifest: Arc<tokio::sync::RwLock<Option<crate::setup::manifest::VoxManifest>>>,

    /// CPU frequency governor (Linux). Checked once at startup. Empty string if unavailable.
    pub cpu_governor: parking_lot::Mutex<String>,
    /// Whether the CPU governor is optimal ("performance"). True if unavailable (non-Linux).
    pub cpu_governor_optimal: Arc<AtomicBool>,
    pub setup_running: Arc<Mutex<bool>>,
    pub conversation_manager: Arc<parking_lot::Mutex<crate::services::memory::ConversationManager>>,
}

impl AppState {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        app_handle: &tauri::AppHandle,
        log_guard: Option<tracing_appender::non_blocking::WorkerGuard>,
        telemetry_tx: crossbeam_channel::Sender<crate::monitoring::aggregator::TelemetryEvent>,
        latest_energy: Arc<AtomicU32>,
        latest_vad_prob: Arc<AtomicU32>,
        latest_low: Arc<AtomicU32>,
        latest_mid: Arc<AtomicU32>,
        latest_high: Arc<AtomicU32>,
        latest_playback_energy: Arc<AtomicU32>,
        latest_playback_low: Arc<AtomicU32>,
        latest_playback_mid: Arc<AtomicU32>,
        latest_playback_high: Arc<AtomicU32>,
        latest_sys_cpu: Arc<AtomicU32>,
        latest_sys_ram: Arc<AtomicU32>,
        latest_vox_cpu: Arc<AtomicU32>,
        latest_vox_ram: Arc<AtomicU32>,
        latest_stt_ms: Arc<AtomicU32>,
        latest_ttft_ms: Arc<AtomicU32>,
        latest_voice_latency_ms: Arc<AtomicU32>,
        latest_threads: Arc<AtomicU32>,
        latest_tts_rtf: Arc<AtomicU32>,
        latest_playback_start_ms: Arc<AtomicU32>,
        latest_persistence_rate: Arc<AtomicU32>,
        is_db_healthy: Arc<AtomicBool>,
        is_private_mode: Arc<AtomicBool>,
        dropped_telemetry_events: Arc<AtomicU64>,
    ) -> Self {
        // paths::init() must have been called before AppState::new()
        let settings = VoxSettings::load();
        let initial_ctx_size = settings.llm.ctx_size as usize;
        is_private_mode.store(settings.persistence.private_mode, Ordering::Relaxed);

        let model_manager = Arc::new(crate::setup::model_manager::ModelManager::new(Some(
            app_handle.clone(),
        )));
        let manifest = Arc::new(tokio::sync::RwLock::new(None));

        Self {
            engine: Mutex::new(None),
            realtime_engine: Mutex::new(None),
            owner: Arc::new(AtomicU32::new(if settings.setup.completed {
                InteractionOwner::Tray as u32
            } else {
                InteractionOwner::Wizard as u32
            })),
            hud_visible: Mutex::new(true),
            memory: MemoryAppState::new(),
            settings: Arc::new(RwLock::new(settings)),
            hud_menu_item: Mutex::new(None),
            ptt: PttState {
                is_recording: std::sync::atomic::AtomicBool::new(false),
                turn_id: Arc::new(AtomicU32::new(0)),
                audio_buffer: parking_lot::Mutex::new(Vec::new()),
                samples_since_partial: std::sync::atomic::AtomicUsize::new(0),
                samples_since_waveform: std::sync::atomic::AtomicUsize::new(0),
                speech_detected: std::sync::atomic::AtomicBool::new(false),
                ptt_start_ms: std::sync::atomic::AtomicU64::new(0),
            },
            pipeline: PipelineAtomics::new(),
            save_debounce: Mutex::new(None),
            _log_guard: log_guard,
            telemetry_tx,
            conversation_id: Arc::new(AtomicU64::new(0)),
            latest_energy,
            latest_vad_prob,
            latest_low,
            latest_mid,
            latest_high,
            latest_playback_energy,
            latest_playback_low,
            latest_playback_mid,
            latest_playback_high,
            latest_sys_cpu,
            latest_sys_ram,
            latest_vox_cpu,
            latest_vox_ram,
            latest_stt_ms,
            latest_ttft_ms,
            latest_voice_latency_ms,
            latest_threads,
            latest_tts_rtf,
            latest_playback_start_ms,
            latest_persistence_rate,
            is_db_healthy,
            is_private_mode,
            is_llm_loaded: Arc::new(AtomicBool::new(false)),
            is_tts_loaded: Arc::new(AtomicBool::new(false)),
            is_stt_loaded: Arc::new(AtomicBool::new(false)),
            is_vad_loaded: Arc::new(AtomicBool::new(false)),
            is_sleeping: Arc::new(AtomicBool::new(false)),
            runtime_status: Arc::new(AtomicU32::new(RuntimeStatus::Initializing as u32)),
            persist_tx: parking_lot::Mutex::new(None),
            memory_tx: parking_lot::Mutex::new(None),
            dropped_persistence_events: Arc::new(std::sync::atomic::AtomicU64::new(0)),
            dropped_telemetry_events,
            monitoring: Arc::new(crate::monitoring::runtime_state::MonitoringState::new()),
            model_manager,
            manifest,
            cpu_governor: parking_lot::Mutex::new(String::new()),
            cpu_governor_optimal: Arc::new(AtomicBool::new(true)), // optimistic default
            setup_running: Arc::new(Mutex::new(false)),
            conversation_manager: Arc::new(parking_lot::Mutex::new(
                crate::services::memory::ConversationManager::new(initial_ctx_size),
            )),
        }
    }
}
