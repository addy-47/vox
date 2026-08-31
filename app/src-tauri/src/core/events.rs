use crate::core::state::InteractionOwner;
use crate::setup::model_manager::ModelSetupStatus;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Runtime};

// Re-export subsystem events for centralized SSOT registry discovery (2. use/imports)
pub use crate::monitoring::aggregator::TelemetryEvent;
pub use crate::persistence::events::{MemoryWorkerEvent, PersistenceEvent};

// ─── 1. Internal Pipeline Actor-to-Router Bus ────────────────────────────────
#[derive(Debug, Clone)]
pub enum VoxEvent {
    SpeechStart {
        turn_id: u32,
    },
    SpeechEnd {
        turn_id: u32,
        audio_buffer: Vec<f32>,
    },
    TranscriptPartial {
        turn_id: u32,
        text: String,
    },
    TranscriptFinal {
        turn_id: u32,
        text: String,
    },
    LlmToken {
        turn_id: u32,
        token: String,
    },
    LlmFinished {
        turn_id: u32,
    },
    TtsChunk {
        turn_id: u32,
        samples: Vec<f32>,
    },
    TtsFinished {
        turn_id: u32,
        rtf: f32,
    },
    PlaybackStarted {
        turn_id: u32,
    },
    PlaybackFinished {
        turn_id: u32,
    },
    Cancelled {
        turn_id: u32,
    },
    Interrupted {
        turn_id: u32,
    },
    Error {
        turn_id: u32,
        message: String,
    },
    Shutdown,
}

// ─── 2. Strongly Typed Payload Types for IPC ──────────────────────────────────
/// Unified payload emitted on `state_changed` — SSOT for all pipeline + dictation state transitions.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StateChangedPayload {
    pub owner: InteractionOwner,
    pub state: String,
    pub turn_id: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoiceErrorPayload {
    pub message: String,
    pub source: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub owner: Option<InteractionOwner>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemStatsPayload {
    pub system_cpu: f32,
    pub system_ram_pct: f32,
    pub vox_cpu: f32,
    pub vox_ram_mb: u32,
    pub threads: u32,
    pub total_memory_gb: u64,
    pub cpu_count: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct TelemetryData {
    pub energy: f32,
    pub vad_prob: f32,
    pub low: f32,
    pub mid: f32,
    pub high: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscriptPayload {
    pub turn_id: u32,
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub owner: Option<InteractionOwner>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmTokenPayload {
    pub turn_id: u32,
    pub token: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ToastLevel {
    Success,
    Warning,
    Error,
    Info,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToastPayload {
    pub title: String,
    pub message: String,
    pub level: ToastLevel,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,
}

// ─── 3. Canonical IPC Event Registry ──────────────────────────────────────────
/// Strongly-typed universal Tauri IPC event enum.
/// Every IPC event emitted to any webview window must have a canonical entry here.
#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum IpcEvent {
    StateChanged(StateChangedPayload),
    TranscriptPartial(TranscriptPayload),
    TranscriptFinal(TranscriptPayload),
    LlmToken(LlmTokenPayload),
    VoiceError(VoiceErrorPayload),
    ModelProgress(ModelSetupStatus),
    Telemetry(TelemetryData),
    SystemStats(SystemStatsPayload),
    SettingsUpdated,
    ToggleTray,
    ShowToast(ToastPayload),
}

impl IpcEvent {
    /// Returns the canonical event string identifier for Tauri IPC.
    pub const fn name(&self) -> &'static str {
        match self {
            Self::StateChanged(_) => "state_changed",
            Self::TranscriptPartial(_) => "transcript_partial",
            Self::TranscriptFinal(_) => "transcript_final",
            Self::LlmToken(_) => "llm_token",
            Self::VoiceError(_) => "voice_error",
            Self::ModelProgress(_) => "model_progress",
            Self::Telemetry(_) => "telemetry",
            Self::SystemStats(_) => "system_stats",
            Self::SettingsUpdated => "settings-updated",
            Self::ToggleTray => "toggle_tray",
            Self::ShowToast(_) => "show_toast",
        }
    }
}

/// Emits a canonical IPC event broadcast to all webview windows.
pub fn emit_ipc<R: Runtime>(app: &AppHandle<R>, event: IpcEvent) -> Result<(), tauri::Error> {
    let name = event.name();
    match event {
        IpcEvent::StateChanged(payload) => app.emit(name, payload),
        IpcEvent::TranscriptPartial(payload) => app.emit(name, payload),
        IpcEvent::TranscriptFinal(payload) => app.emit(name, payload),
        IpcEvent::LlmToken(payload) => app.emit(name, payload),
        IpcEvent::VoiceError(payload) => app.emit(name, payload),
        IpcEvent::ModelProgress(payload) => app.emit(name, payload),
        IpcEvent::Telemetry(payload) => app.emit(name, payload),
        IpcEvent::SystemStats(payload) => app.emit(name, payload),
        IpcEvent::SettingsUpdated => app.emit(name, ()),
        IpcEvent::ToggleTray => app.emit(name, ()),
        IpcEvent::ShowToast(payload) => app.emit(name, payload),
    }
}

/// Emits a canonical IPC event targeted to a specific webview window (e.g. "main" or "tray").
pub fn emit_ipc_to<R: Runtime>(
    app: &AppHandle<R>,
    target: &str,
    event: IpcEvent,
) -> Result<(), tauri::Error> {
    let name = event.name();
    match event {
        IpcEvent::StateChanged(payload) => app.emit_to(target, name, payload),
        IpcEvent::TranscriptPartial(payload) => app.emit_to(target, name, payload),
        IpcEvent::TranscriptFinal(payload) => app.emit_to(target, name, payload),
        IpcEvent::LlmToken(payload) => app.emit_to(target, name, payload),
        IpcEvent::VoiceError(payload) => app.emit_to(target, name, payload),
        IpcEvent::ModelProgress(payload) => app.emit_to(target, name, payload),
        IpcEvent::Telemetry(payload) => app.emit_to(target, name, payload),
        IpcEvent::SystemStats(payload) => app.emit_to(target, name, payload),
        IpcEvent::SettingsUpdated => app.emit_to(target, name, ()),
        IpcEvent::ToggleTray => app.emit_to(target, name, ()),
        IpcEvent::ShowToast(payload) => app.emit_to(target, name, payload),
    }
}
