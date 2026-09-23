use std::{
    fmt::{Display, Formatter, Result},
    sync::{
        atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering},
        mpsc::Sender,
        Arc, RwLock,
    },
};

use crossbeam_channel::Sender as CrossbeamSender;
use parking_lot::{Mutex as ParkingMutex, RwLock as ParkingRwLock};
use serde::{Deserialize, Serialize};
use tauri::{
    async_runtime::JoinHandle,
    menu::CheckMenuItem,
    AppHandle, Runtime, Wry,
};
use tokio::sync::{Mutex as TokioMutex, RwLock as TokioRwLock};
use tracing_appender::non_blocking::WorkerGuard;

pub use crate::{
    core::engine::VoxEngine, monitoring::telemetry::TelemetryState, pipeline::PipelineAtomics,
    services::memory::MemoryAppState,
};
use crate::{
    core::{
        events::VoxEvent,
        metrics::TurnMetricsCollector,
        settings::{PipelineMode, VoxSettings},
    },
    monitoring::snapshots::MonitoringState,
    persistence::{VoxDb, PersistenceEvent},
    pipeline::assistant::TurnAccumulator,
    services::{harness::Harness, llm::LlmProvider, realtime::RealtimeActor},
    setup::{manifest::VoxManifest, model_manager::ModelManager},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
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

impl Display for AppWindow {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        write!(f, "{}", self.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub enum RuntimeStatus {
    Initializing,
    Ready,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
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
    Working = 8,
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
            8 => InteractionState::Working,
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
    pub engine: TokioMutex<Option<VoxEngine>>,
    pub realtime_engine: TokioMutex<Option<RealtimeActor>>,
    pub owner: Arc<AtomicU32>,
    pub hud_visible: Arc<AtomicBool>,
    pub memory: MemoryAppState,
    pub settings: Arc<RwLock<VoxSettings>>,
    pub hud_menu_item: ParkingMutex<Option<CheckMenuItem<Wry>>>,
    pub pipeline: PipelineAtomics,
    pub save_debounce: TokioMutex<Option<JoinHandle<()>>>,
    pub _log_guard: Option<WorkerGuard>,
    pub telemetry: Arc<TelemetryState>,
    pub dictation_last_transcript: ParkingMutex<Option<String>>,
    pub conversation_id: Arc<AtomicU64>,
    pub runtime_status: Arc<AtomicU32>,
    pub main_window_destroyed: Arc<AtomicBool>,
    pub persist_tx: ParkingMutex<Option<CrossbeamSender<PersistenceEvent>>>,
    pub dropped_persistence_events: Arc<AtomicU64>,
    pub monitoring: Arc<MonitoringState>,
    pub model_manager: Arc<ModelManager>,
    pub manifest: Arc<TokioRwLock<Option<VoxManifest>>>,
    pub cpu_governor: ParkingMutex<String>,
    pub cpu_governor_optimal: Arc<AtomicBool>,
    pub setup_running: Arc<TokioMutex<bool>>,
    pub harness: Arc<ParkingMutex<Option<Harness>>>,
    pub llm_provider: Arc<ParkingRwLock<Option<Arc<dyn LlmProvider>>>>,
    pub event_tx: ParkingMutex<Option<Sender<VoxEvent>>>,
    pub pipeline_accumulator: Arc<ParkingMutex<TurnAccumulator>>,
    pub turn_metrics: Arc<TurnMetricsCollector>,
    pub db: Arc<VoxDb>,
}

impl AppState {
    pub fn new<R: Runtime>(
        app_handle: &AppHandle<R>,
        log_guard: Option<WorkerGuard>,
        telemetry: Arc<TelemetryState>,
        db: Arc<VoxDb>,
    ) -> Self {
        let settings = VoxSettings::load();
        telemetry
            .is_private_mode
            .store(settings.working_memory.private_mode, Ordering::Relaxed);

        let model_manager = Arc::new(ModelManager::new(Some(app_handle.clone())));
        let manifest = Arc::new(TokioRwLock::new(None));

        Self {
            engine: TokioMutex::new(None),
            realtime_engine: TokioMutex::new(None),
            owner: Arc::new(AtomicU32::new(InteractionOwner::Dictation as u32)),
            hud_visible: Arc::new(AtomicBool::new(true)),
            memory: MemoryAppState::new(),
            settings: Arc::new(RwLock::new(settings)),
            hud_menu_item: ParkingMutex::new(None),
            pipeline: PipelineAtomics::new(),
            save_debounce: TokioMutex::new(None),
            _log_guard: log_guard,
            telemetry: Arc::clone(&telemetry),
            dictation_last_transcript: ParkingMutex::new(None),
            conversation_id: Arc::new(AtomicU64::new(0)),
            runtime_status: Arc::new(AtomicU32::new(RuntimeStatus::Initializing as u32)),
            main_window_destroyed: Arc::new(AtomicBool::new(false)),
            persist_tx: ParkingMutex::new(None),
            dropped_persistence_events: Arc::new(AtomicU64::new(0)),
            monitoring: Arc::new(MonitoringState::new()),
            model_manager,
            manifest,
            cpu_governor: ParkingMutex::new("ondemand".into()),
            cpu_governor_optimal: Arc::new(AtomicBool::new(true)),
            setup_running: Arc::new(TokioMutex::new(false)),
            harness: Arc::new(ParkingMutex::new(None)),
            llm_provider: Arc::new(ParkingRwLock::new(None)),
            event_tx: ParkingMutex::new(None),
            pipeline_accumulator: Arc::new(ParkingMutex::new(TurnAccumulator::new())),
            turn_metrics: Arc::new(TurnMetricsCollector::new()),
            db,
        }
    }

    /// Dynamically resolves the base system prompt according to active pipeline mode.
    pub fn resolve_base_prompt(&self) -> String {
        let settings = self.settings.read().unwrap_or_else(|p| p.into_inner());
        match settings.interaction.pipeline_mode {
            PipelineMode::Modular => settings.persona.modular_prompt.clone(),
            PipelineMode::Realtime => settings.persona.realtime_prompt.clone(),
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
