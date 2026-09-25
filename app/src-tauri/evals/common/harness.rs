//! ============================================================================
//! harness.rs — Shared Eval Context & Isolated Subsystem Setup
//! ============================================================================
//! Category     : Evaluation Common Helper
//! Component    : evals/common/harness.rs
//! Prerequisites: None
//! ============================================================================

use std::{
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering},
        mpsc, Arc,
    },
};

use anyhow::{Context, Result};
use turso::Connection;
use vox_lib::{
    core::{
        events::VoxEvent,
        settings::{LlmActiveProvider, VoxSettings},
        state::{AppState, TelemetryState},
    },
    persistence::{sessions::create_session, worker::spawn_persistence_worker, VoxDb},
    services::llm::{
        actor::{create_llm_provider_from_llm_settings, spawn_llm_worker},
        LlmCommand, LlmProvider,
    },
};

use super::{db, report};

/// Standard configuration for initializing an isolated eval context.
#[derive(Clone, Debug)]
pub struct EvalConfig {
    pub eval_name: String,
    pub server_url: String,
    pub server_model: String,
    pub server_api_key: Option<String>,
    pub server_provider: Option<String>,
    pub context_window: u32,
}

/// Encapsulates all handles, states, and channels of an isolated eval execution.
pub struct EvalContext {
    pub run_id: String,
    pub results_dir: PathBuf,
    pub tauri_app: tauri::AppHandle<tauri::test::MockRuntime>,
    pub state: Arc<AppState>,
    pub db: Arc<VoxDb>,
    pub conn: Connection,
    pub session_id: i64,
    pub llm_tx: mpsc::Sender<LlmCommand>,
    pub event_tx: mpsc::Sender<VoxEvent>,
    pub event_rx: mpsc::Receiver<VoxEvent>,
}

/// Creates a fresh, isolated eval context with fresh SQLite DB, mock AppHandle,
/// TelemetryState, persistence worker, and LLM provider worker thread.
pub async fn setup_isolated_eval_context(cfg: &EvalConfig, base_dir: &Path) -> Result<EvalContext> {
    let run_id = report::new_run_id();
    let results_dir = base_dir.join(&cfg.eval_name).join(&run_id);
    tokio::fs::create_dir_all(&results_dir).await?;

    let db_path = results_dir.join("eval.db");
    let (db, conn) = db::open_fresh_eval_db(&db_path).await?;
    let db_arc = Arc::new(db);

    let session_id = create_session(&conn, None).await?;

    let mut settings = VoxSettings::default();
    settings.llm.active = LlmActiveProvider::Server;
    settings.llm.server.base_url = cfg.server_url.clone();
    settings.llm.server.model = cfg.server_model.clone();
    settings.llm.server.api_key = cfg.server_api_key.clone();
    settings.llm.server.provider_name = cfg.server_provider.clone();
    settings.llm.context_window = cfg.context_window;

    let tauri_app = tauri::test::mock_app().handle().clone();
    let (telemetry_tx, _telemetry_rx) = crossbeam_channel::unbounded();
    let telemetry = Arc::new(TelemetryState {
        telemetry_tx,
        latest_energy: Arc::new(AtomicU32::new(0)),
        latest_vad_prob: Arc::new(AtomicU32::new(0)),
        latest_low: Arc::new(AtomicU32::new(0)),
        latest_mid: Arc::new(AtomicU32::new(0)),
        latest_high: Arc::new(AtomicU32::new(0)),
        latest_playback_energy: Arc::new(AtomicU32::new(0)),
        latest_playback_low: Arc::new(AtomicU32::new(0)),
        latest_playback_mid: Arc::new(AtomicU32::new(0)),
        latest_playback_high: Arc::new(AtomicU32::new(0)),
        latest_sys_cpu: Arc::new(AtomicU32::new(0)),
        latest_sys_ram: Arc::new(AtomicU32::new(0)),
        latest_vox_cpu: Arc::new(AtomicU32::new(0)),
        latest_vox_ram: Arc::new(AtomicU32::new(0)),
        latest_stt_ms: Arc::new(AtomicU32::new(0)),
        latest_ttft_ms: Arc::new(AtomicU32::new(0)),
        latest_voice_latency_ms: Arc::new(AtomicU32::new(0)),
        latest_threads: Arc::new(AtomicU32::new(0)),
        latest_tts_rtf: Arc::new(AtomicU32::new(0)),
        latest_playback_start_ms: Arc::new(AtomicU32::new(0)),
        latest_persistence_rate: Arc::new(AtomicU32::new(0)),
        is_db_healthy: Arc::new(AtomicBool::new(true)),
        is_private_mode: Arc::new(AtomicBool::new(false)),
        dropped_telemetry_events: Arc::new(AtomicU64::new(0)),
    });

    let state = Arc::new(AppState::new(
        &tauri_app,
        None,
        telemetry,
        Arc::clone(&db_arc),
    ));
    *state.settings.write().unwrap() = settings.clone();
    state
        .conversation_id
        .store(session_id as u64, Ordering::Relaxed);

    let persist_tx = spawn_persistence_worker(
        Arc::clone(&db_arc),
        Arc::clone(&state.telemetry.is_db_healthy),
        Arc::clone(&state.telemetry.latest_persistence_rate),
        Arc::clone(&state.telemetry.is_private_mode),
    );
    *state.persist_tx.lock() = Some(persist_tx);

    let (llm_tx, llm_rx) = mpsc::channel::<LlmCommand>();
    let llm_provider =
        create_llm_provider_from_llm_settings(&settings.llm, &vox_lib::utils::paths::model_dir(""))
            .map_err(|e| anyhow::anyhow!(e))
            .context("Failed to create LLM provider")?;
    let llm_provider_arc: Arc<dyn LlmProvider> = llm_provider.into();
    std::thread::Builder::new()
        .name("vox-eval-llm-worker".to_string())
        .spawn(move || {
            spawn_llm_worker(llm_rx, llm_provider_arc);
        })
        .context("Failed to spawn LLM worker thread")?;

    let (event_tx, event_rx) = mpsc::channel::<VoxEvent>();

    Ok(EvalContext {
        run_id,
        results_dir,
        tauri_app,
        state,
        db: db_arc,
        conn,
        session_id,
        llm_tx,
        event_tx,
        event_rx,
    })
}
