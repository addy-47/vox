use tauri::AppHandle;
use tokio::sync::Mutex;
use std::sync::{Arc, RwLock};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64};
use crate::services::audio::AudioStream;
use crate::services::stt::SttCommand;
use crate::core::settings::VoxSettings;
use std::collections::VecDeque;

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize)]
pub enum InteractionOwner {
    Tray,
    MainWindow,
    Ptt,
}

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize)]
pub enum InteractionState {
    Idle,
    Listening,
    UserSpeaking,
    Thinking,
    AssistantSpeaking,
    Interrupted,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct TelemetryData {
    pub energy: f32,
    pub vad_prob: f32,
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
    /// Gracefully shutdown the VAD worker.
    Shutdown,
}

// ─── VoxEngine ────────────────────────────────────────────────────────────────

pub struct VoxEngine {
    pub audio_stream: AudioStream,
    pub stt_tx: std::sync::mpsc::Sender<SttCommand>,
    /// Channel to send hot-updates to the VAD worker without locking AppState.
    pub vad_tx: std::sync::mpsc::Sender<VadCommand>,
    pub telemetry_tx: crossbeam_channel::Sender<crate::telemetry::aggregator::TelemetryEvent>,
    pub pipeline_tx: std::sync::mpsc::Sender<crate::core::events::VoxEvent>,
}

// ─── PttState ─────────────────────────────────────────────────────────────────

pub struct PttState {
    pub is_recording:           Mutex<bool>,
    pub turn_id:                Arc<AtomicU32>,
    pub audio_buffer:           Mutex<Vec<f32>>,
    pub samples_since_partial:  Mutex<usize>,
    pub samples_since_waveform: Mutex<usize>,
}

// ─── PipelineAtomics ──────────────────────────────────────────────────────────

/// Phase 4 shared atomics — checked on every inference iteration for cancellation.
///
/// Using `Arc<AtomicBool>` (not channels or async) because llama.cpp and onnxruntime
/// execute in blocking C++ loops that cannot be interrupted via Rust async primitives.
pub struct PipelineAtomics {
    /// Set to `true` to abort the current LLM + TTS + Playback turn immediately.
    pub cancel_flag:     Arc<AtomicBool>,
    /// `true` while the CPAL output stream is actively draining audio.
    /// In Speaker mode, the VAD loop drops mic frames while this is set.
    pub playback_active: Arc<AtomicBool>,
    /// `true` while the LLM worker is generating tokens.
    pub llm_generating:  Arc<AtomicBool>,
    /// `true` while the TTS worker is synthesizing audio.
    pub tts_generating:  Arc<AtomicBool>,
    /// Monotonically increasing turn counter. Increments on every TranscriptFinal.
    /// Used to reject stale pipeline events. NEVER persisted.
    /// Persistence identity is AppState::conversation_id.
    pub turn_id:      Arc<AtomicU32>,
    /// Current interaction state (Idle, Listening, etc.)
    pub state:           Arc<std::sync::Mutex<InteractionState>>,
    /// `true` if the main application is "engaged" (active interaction).
    /// If false, the pipeline remains in a dormant, STT-only state.
    pub is_engaged:      Arc<AtomicBool>,
    /// In-memory history of recent transcripts. Bridge to Phase 6.3 persistence.
    pub transcript_history: Arc<std::sync::Mutex<VecDeque<String>>>,
}

impl PipelineAtomics {
    pub fn new() -> Self {
        Self {
            cancel_flag:        Arc::new(AtomicBool::new(false)),
            playback_active:    Arc::new(AtomicBool::new(false)),
            llm_generating:     Arc::new(AtomicBool::new(false)),
            tts_generating:     Arc::new(AtomicBool::new(false)),
            turn_id:            Arc::new(AtomicU32::new(0)),
            state:              Arc::new(std::sync::Mutex::new(InteractionState::Idle)),
            is_engaged:         Arc::new(AtomicBool::new(false)),
            transcript_history: Arc::new(std::sync::Mutex::new(
                VecDeque::with_capacity(crate::core::constants::TRANSCRIPT_HISTORY_LIMIT)
            )),
        }
    }

    /// Update internal state and emit IPC event to the **owning** window only.
    pub fn update_interaction_state(&self, new_state: InteractionState, owner: InteractionOwner, app_handle: &tauri::AppHandle) {
        let mut state_lock = self.state.lock().unwrap();
        if *state_lock != new_state {
            log::debug!("[Pipeline] State changed -> {:?} (Owner: {:?})", new_state, owner);
            *state_lock = new_state;

            let target = match owner {
                InteractionOwner::Tray => "tray",
                InteractionOwner::MainWindow | InteractionOwner::Ptt => "main",
            };
            let _ = tauri::Emitter::emit_to(app_handle, target, "state_changed", new_state);
        }
    }
}

// ─── AppState ─────────────────────────────────────────────────────────────────

pub struct AppState {
    pub engine:        Mutex<Option<VoxEngine>>,
    pub owner:         Mutex<InteractionOwner>,
    pub hud_visible:   Mutex<bool>,

    /// Settings protected by RwLock for concurrent read access.
    ///
    /// # Concurrency Contract
    /// - IPC handlers acquire `write()` only when mutating settings
    /// - NO real-time thread (VAD, STT, LLM, TTS, Playback callback) may call `read()` on the hot path
    /// - Hot-path settings (VAD threshold, noise gate) are snapshotted into worker-local variables
    ///   at startup and updated via `VadCommand` / other worker channels on change
    pub settings:      Arc<RwLock<VoxSettings>>,

    pub hud_menu_item: Mutex<Option<tauri::menu::CheckMenuItem<tauri::Wry>>>,
    pub ptt:           PttState,

    /// Phase 4: shared pipeline cancellation and status atomics.
    pub pipeline:      PipelineAtomics,

    /// Debounce handle for settings disk writes.
    /// Cancelled and respawned on each `update_setting` IPC call.
    pub save_debounce: Mutex<Option<tauri::async_runtime::JoinHandle<()>>>,

    /// Async log writer guard. Must be held to ensure logs are flushed.
    pub _log_guard:    Option<tracing_appender::non_blocking::WorkerGuard>,
    /// Structured telemetry bus (crossbeam — lock-free, safe for hot-path threads).
    pub telemetry_tx:  crossbeam_channel::Sender<crate::telemetry::aggregator::TelemetryEvent>,
    /// Long-lived conversation session ID. 0 = no active session (Tray mode).
    /// Created on Engage, destroyed on Disengage. Persistence worker ignores events with id == 0.
    pub conversation_id: Arc<AtomicU64>,
    /// Persistence worker channel. None if persistence is disabled.
    pub persist_tx: Option<crossbeam_channel::Sender<crate::persistence::events::PersistenceEvent>>,
}

impl AppState {
    pub fn new(_app: &AppHandle, log_guard: Option<tracing_appender::non_blocking::WorkerGuard>, telemetry_tx: crossbeam_channel::Sender<crate::telemetry::aggregator::TelemetryEvent>) -> Self {
        // paths::init() must have been called before AppState::new()
        let settings = VoxSettings::load();

        Self {
            engine:        Mutex::new(None),
            owner:         Mutex::new(InteractionOwner::Tray),
            hud_visible:   Mutex::new(true),
            settings:      Arc::new(RwLock::new(settings)),
            hud_menu_item: Mutex::new(None),
            ptt: PttState {
                is_recording:           Mutex::new(false),
                turn_id:                Arc::new(AtomicU32::new(0)),
                audio_buffer:           Mutex::new(Vec::new()),
                samples_since_partial:  Mutex::new(0),
                samples_since_waveform: Mutex::new(0),
            },
            pipeline:      PipelineAtomics::new(),
            save_debounce: Mutex::new(None),
            _log_guard:    log_guard,
            telemetry_tx,
            conversation_id: Arc::new(AtomicU64::new(0)),
            persist_tx: None,
        }
    }
}
