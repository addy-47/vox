use tauri::{AppHandle, Manager};
use tokio::sync::Mutex;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32};
use crate::services::audio::AudioStream;
use crate::services::stt::SttCommand;
use crate::core::settings::VoxSettings;

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

pub struct VoxEngine {
    pub audio_stream: AudioStream,
    pub stt_tx: std::sync::mpsc::Sender<SttCommand>,
    pub telemetry_tx: std::sync::mpsc::Sender<TelemetryData>,
    pub pipeline_tx: std::sync::mpsc::Sender<crate::core::events::VoxEvent>,
}

pub struct PttState {
    pub is_recording:          Mutex<bool>,
    pub session_id:            Arc<AtomicU32>,
    pub audio_buffer:          Mutex<Vec<f32>>,
    pub samples_since_partial: Mutex<usize>,
    pub samples_since_waveform: Mutex<usize>,
}

/// Phase 4 shared atomics — checked on every inference iteration for cancellation.
///
/// Using Arc<AtomicBool> (not channels or async) because llama.cpp and onnxruntime
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
    /// Monotonically increasing turn counter. Used to reject stale pipeline events.
    pub session_id:      Arc<AtomicU32>,
    /// Current interaction state (Idle, Listening, etc.)
    pub state:           Arc<std::sync::Mutex<InteractionState>>,
    /// `true` if the main application is "engaged" (active interaction).
    /// If false, the pipeline remains in a dormant, STT-only state.
    pub is_engaged:      Arc<AtomicBool>,
}

impl PipelineAtomics {
    pub fn new() -> Self {
        Self {
            cancel_flag:     Arc::new(AtomicBool::new(false)),
            playback_active: Arc::new(AtomicBool::new(false)),
            llm_generating:  Arc::new(AtomicBool::new(false)),
            tts_generating:  Arc::new(AtomicBool::new(false)),
            session_id:      Arc::new(AtomicU32::new(0)),
            state:           Arc::new(std::sync::Mutex::new(InteractionState::Idle)),
            is_engaged:      Arc::new(AtomicBool::new(false)),
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

pub struct AppState {
    pub engine:       Mutex<Option<VoxEngine>>,
    pub owner:        Mutex<InteractionOwner>,
    pub hud_visible:  Mutex<bool>,
    pub settings:     Mutex<VoxSettings>,
    pub hud_menu_item: Mutex<Option<tauri::menu::CheckMenuItem<tauri::Wry>>>,
    pub ptt:          PttState,
    pub config_dir:   std::path::PathBuf,
    /// Phase 4: shared pipeline cancellation and status atomics.
    pub pipeline:     PipelineAtomics,
}


impl AppState {
    pub fn new(app: &AppHandle) -> Self {
        let config_dir = app.path().home_dir().map(|mut p| {
            p.push(".vox");
            p
        }).unwrap_or_else(|_| {
            let mut p = std::env::current_dir().unwrap_or_default();
            p.push(".vox");
            p
        });

        let settings = VoxSettings::load(&config_dir);

        Self {
            engine:    Mutex::new(None),
            owner:     Mutex::new(InteractionOwner::Tray),
            hud_visible: Mutex::new(true),
            settings:  Mutex::new(settings),
            hud_menu_item: Mutex::new(None),
            ptt: PttState {
                is_recording:           Mutex::new(false),
                session_id:             Arc::new(AtomicU32::new(0)),
                audio_buffer:           Mutex::new(Vec::new()),
                samples_since_partial:  Mutex::new(0),
                samples_since_waveform: Mutex::new(0),
            },
            config_dir,
            pipeline: PipelineAtomics::new(),
        }
    }

    pub async fn save_settings(&self) -> anyhow::Result<()> {
        let settings = self.settings.lock().await;
        let json = serde_json::to_string_pretty(&*settings)?;
        let path = self.config_dir.join("settings.json");
        tokio::fs::write(path, json).await?;
        Ok(())
    }
}
