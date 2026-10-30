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
#[cfg(desktop)]
use tauri::menu::CheckMenuItem;
#[cfg(desktop)]
use tauri::Wry;
use tauri::{async_runtime::JoinHandle, AppHandle, Manager, Runtime, State};
use tokio::sync::{Mutex as TokioMutex, RwLock as TokioRwLock};
use tracing_appender::non_blocking::WorkerGuard;

use crate::{
    config::VoxSettings,
    core::{
        events::{emit_ipc, IpcEvent, PipelineMode, VoxEvent},
        metrics::TurnMetricsCollector,
    },
    monitoring::snapshots::MonitoringState,
    persistence::{PersistenceEvent, VoxDb},
    pipeline::assistant::TurnAccumulator,
    services::{
        harness::{
            stages::tools::web_search::{default_engines, VoxEmbedder, DEFAULT_MIN_SCORE},
            Harness,
        },
        llm::LlmProvider,
        realtime::RealtimeActor,
        remote::{
            security::{PairingCoordinator, PairingPrompt, TrustStore},
            session::{RemoteLinkState, RemoteSessionManager, SessionRole, StateChangeCallback},
        },
    },
    setup::{manifest::VoxManifest, model_manager::ModelManager},
};
pub use crate::{
    core::engine::VoxEngine, monitoring::telemetry::TelemetryState, pipeline::PipelineAtomics,
    services::memory::MemoryAppState,
};

/// Pending inbound pairing request awaiting host user approval.
pub struct PendingPairingState {
    pub device_id: String,
    pub display_name: String,
    pub prompt: PairingPrompt,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AppWindow {
    Main,
    Tray,
    Wizard,
}

impl AppWindow {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Main => "main",
            Self::Tray => "tray",
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

impl InteractionState {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Idle => "Idle",
            Self::Ready => "Ready",
            Self::Listening => "Listening",
            Self::Thinking => "Thinking",
            Self::Speaking => "Speaking",
            Self::Paused => "Paused",
            Self::Error => "Error",
            Self::Sleeping => "Sleeping",
            Self::Working => "Working",
        }
    }
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
    pub settings: Arc<std::sync::RwLock<VoxSettings>>,
    #[cfg(desktop)]
    pub hud_menu_item: ParkingMutex<Option<CheckMenuItem<Wry>>>,
    #[cfg(not(desktop))]
    pub hud_menu_item: ParkingMutex<Option<()>>,
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
    pub restart_requested: Arc<AtomicBool>,
    pub restart_runner: Arc<AtomicBool>,
    pub worker_warmup_claimed: Arc<AtomicBool>,
    pub restart_in_flight: Arc<AtomicBool>,
    pub ingestion_cancel: Arc<ParkingMutex<Option<tokio_util::sync::CancellationToken>>>,
    pub compaction_lock: Arc<TokioMutex<()>>,
    pub nexus_search: Arc<nexus::NexusSearch>,
    pub remote_manager: Arc<RemoteSessionManager>,
    pub remote_audio_ingestor: Arc<ParkingMutex<Option<Arc<dyn crate::services::remote::audio::NetworkAudioIngestor + Send + Sync>>>>,
    pub pairing_coordinator: Arc<PairingCoordinator>,
    pub pending_pairing: Arc<ParkingMutex<Option<PendingPairingState>>>,
    pub trust_store: Arc<TrustStore>,
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
            .store(settings.memory.private_mode, Ordering::Relaxed);

        let model_manager = Arc::new(ModelManager::new(Some(app_handle.clone())));
        let manifest = Arc::new(TokioRwLock::new(None));

        let trust_store = Arc::new(TrustStore::new());
        let remote_manager = {
            let session_role = if cfg!(desktop) {
                SessionRole::Host
            } else {
                SessionRole::Controller
            };

            let app_handle_for_remote = app_handle.clone();
            let on_state_change: StateChangeCallback = Arc::new(move |state, peer| {
                let dto = state.to_dto(
                    peer.map(|p| p.device_id.as_str()),
                    peer.map(|p| p.display_name.as_str()),
                );
                let _ = emit_ipc(
                    &app_handle_for_remote,
                    IpcEvent::RemoteStateChanged(dto),
                );

                // Hot-swap audio capture source on remote connect/disconnect (US-RC-02).
                let new_state = state.clone();
                let app = app_handle_for_remote.clone();
                if new_state == RemoteLinkState::Connected
                    || new_state == RemoteLinkState::Paired
                {
                    tauri::async_runtime::spawn(async move {
                        let st: State<'_, Arc<AppState>> = app.state();
                        let running = st.engine.lock().await.is_some();
                        if running {
                            let _ = crate::ipc::pipeline::restart_engine_inner(&app, &st, false).await;
                        }
                    });
                }
            });

            let manager = Arc::new(RemoteSessionManager::with_callback(
                session_role,
                Arc::clone(&db),
                on_state_change,
            ));
            manager.set_trust_store(Arc::clone(&trust_store));

            if session_role == SessionRole::Host && !settings.remote.accepts_controllers {
                manager.set_state(RemoteLinkState::Disabled);
            }

            manager
        };

        Self {
            engine: TokioMutex::new(None),
            realtime_engine: TokioMutex::new(None),
            owner: Arc::new(AtomicU32::new(InteractionOwner::Dictation as u32)),
            hud_visible: Arc::new(AtomicBool::new(true)),
            memory: MemoryAppState::new(),
            settings: Arc::new(RwLock::new(settings.clone())),
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
            db: db.clone(),
            restart_requested: Arc::new(AtomicBool::new(false)),
            restart_runner: Arc::new(AtomicBool::new(false)),
            worker_warmup_claimed: Arc::new(AtomicBool::new(false)),
            restart_in_flight: Arc::new(AtomicBool::new(false)),
            ingestion_cancel: Arc::new(ParkingMutex::new(None)),
            compaction_lock: Arc::new(TokioMutex::new(())),
            nexus_search: {
                let fanout_policy = nexus::model::FanoutPolicy {
                    min_reporting_engines: 2,
                    min_distinct_domains: 2,
                    min_candidate_hits: 4,
                    max_fanout_deadline_ms: 2200,
                };
                // B1: min_score must be explicit here — WebSearchState::new is no
                // longer on the execution path, so the AppState singleton is the
                // only live RankingPolicy. Relying on ..Default::default() would
                // silently disable the relevance floor if the crate default moves.
                let ranking_policy = nexus::model::RankingPolicy {
                    two_stage_reranking: true,
                    min_score: DEFAULT_MIN_SCORE,
                    ..Default::default()
                };
                Arc::new(
                    nexus::NexusSearch::builder()
                        .with_engines(default_engines())
                        .with_fanout_policy(fanout_policy)
                        .with_ranking_policy(ranking_policy)
                        .with_fetch_concurrency(5)
                        .with_embedder(Arc::new(VoxEmbedder))
                        .build()
                        .expect("Failed to initialize NexusSearch client"),
                )
            },
            remote_manager,
            remote_audio_ingestor: Arc::new(ParkingMutex::new(None)),
            pairing_coordinator: Arc::new(PairingCoordinator::new()),
            pending_pairing: Arc::new(ParkingMutex::new(None)),
            trust_store,
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
