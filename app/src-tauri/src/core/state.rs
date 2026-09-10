use std::sync::{
    atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering},
    mpsc, Arc, RwLock,
};

use tokio::sync::Mutex;
use turso::Connection;

use crate::{
    core::{
        engine::VoxEngine,
        events::VoxEvent,
        settings::VoxSettings,
    },
    monitoring::runtime_state::{MonitoringState, TelemetryState},
    persistence::PersistenceEvent,
    pipeline::{assistant::accumulator::TurnAccumulator, PipelineAtomics},
    services::{
        harness::ConversationManager,
        llm::LlmProvider,
        memory::MemoryAppState,
        realtime::RealtimeActor,
    },
    setup::{manifest::VoxManifest, model_manager::ModelManager},
};

pub use crate::core::engine::VoxEngine;
pub use crate::monitoring::runtime_state::TelemetryState;
pub use crate::pipeline::PipelineAtomics;
pub use crate::services::memory::MemoryAppState;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum AppWindow {
    Main,
    Tray,
    Toast,
    Wizard,
}

impl AppWindow {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Main => "main",
            Self::Tray => "tray",
            Self::Toast => "toast",
            Self::Wizard => "wizard",
        }
    }
}

impl AsRef<str> for AppWindow {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl std::fmt::Display for AppWindow {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

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

pub struct AppState {
    pub engine: Mutex<Option<VoxEngine>>,
    pub realtime_engine: Mutex<Option<RealtimeActor>>,
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
    pub persist_tx: parking_lot::Mutex<Option<crossbeam_channel::Sender<PersistenceEvent>>>,
    pub dropped_persistence_events: Arc<AtomicU64>,
    pub monitoring: Arc<MonitoringState>,
    pub model_manager: Arc<ModelManager>,
    pub manifest: Arc<tokio::sync::RwLock<Option<VoxManifest>>>,
    pub cpu_governor: parking_lot::Mutex<String>,
    pub cpu_governor_optimal: Arc<AtomicBool>,
    pub setup_running: Arc<Mutex<bool>>,
    pub conversation_manager: Arc<parking_lot::Mutex<ConversationManager>>,
    pub llm_provider: Arc<parking_lot::RwLock<Option<Arc<dyn LlmProvider>>>>,
    pub event_tx: parking_lot::Mutex<Option<mpsc::Sender<VoxEvent>>>,
    pub pipeline_accumulator: Arc<parking_lot::Mutex<TurnAccumulator>>,
    pub db: Arc<Connection>,
}

impl AppState {
    pub fn new<R: tauri::Runtime>(
        app_handle: &tauri::AppHandle<R>,
        log_guard: Option<tracing_appender::non_blocking::WorkerGuard>,
        telemetry: Arc<TelemetryState>,
        db: Arc<Connection>,
    ) -> Self {
        let settings = VoxSettings::load();
        telemetry
            .is_private_mode
            .store(settings.history.private_mode, Ordering::Relaxed);

        let model_manager = Arc::new(ModelManager::new(Some(app_handle.clone())));
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
            dropped_persistence_events: Arc::new(AtomicU64::new(0)),
            monitoring: Arc::new(MonitoringState::new()),
            model_manager,
            manifest,
            cpu_governor: parking_lot::Mutex::new("ondemand".into()),
            cpu_governor_optimal: Arc::new(AtomicBool::new(true)),
            setup_running: Arc::new(Mutex::new(false)),
            conversation_manager: Arc::new(parking_lot::Mutex::new(ConversationManager::new())),
            llm_provider: Arc::new(parking_lot::RwLock::new(None)),
            event_tx: parking_lot::Mutex::new(None),
            pipeline_accumulator: Arc::new(parking_lot::Mutex::new(TurnAccumulator::new())),
            db,
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
