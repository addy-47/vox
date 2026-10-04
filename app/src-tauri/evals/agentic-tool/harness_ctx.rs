//! ============================================================================
//! evals/agentic-tool/harness_ctx.rs — Isolated AppState, Tauri handle and LLM worker wiring
//! ============================================================================
//! Category     : Evaluation
//! Component    : evals harness
//! Prerequisites: see evals/README.md
//! Execution    : cargo bench --bench agentic_tool_eval --release -- --help
//! Metrics      : see summary.md in the run directory
//! ============================================================================

use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering::Relaxed},
        mpsc, Arc,
    },
};

use anyhow::{anyhow, Context as _, Result};
use parking_lot::Mutex;
use serde::Serialize;
use vox_lib::{
    core::{events::VoxEvent, state::AppState},
    monitoring::telemetry::{TelemetryAggregator, TelemetryAggregatorHandles, TelemetryState},
    pipeline::{assistant::TurnAccumulator, RoutingContext},
    services::{
        harness::{chassis::Harness, TurnExecutionRequest, TurnOutcome},
        llm::{spawn_llm_worker, ConnectionConfig, LlmCommand, LlmProvider, RemoteTransport},
    },
    utils::paths,
};

#[allow(dead_code)]
pub struct EvalHarnessContext {
    pub state: Arc<AppState>,
    pub tauri_app: tauri::AppHandle<tauri::test::MockRuntime>,
    pub session_id: i64,
    pub run_id: String,
    pub results_dir: PathBuf,
    pub event_tx: mpsc::Sender<VoxEvent>,
    pub event_rx: Arc<Mutex<mpsc::Receiver<VoxEvent>>>,
    pub llm_tx: mpsc::Sender<LlmCommand>,
    pub llm_provider: Arc<dyn LlmProvider>,
    pub db: Arc<vox_lib::persistence::VoxDb>,
    /// TTS settings, so the eval drives the same provider the app would.
    pub tts_settings: vox_lib::services::tts::TtsSettings,
}

pub struct EvalConfig {
    pub eval_name: String,
    pub server_url: String,
    pub server_model: String,
    pub server_api_key: Option<String>,
    pub server_provider: Option<String>,
    pub context_window: u32,
}

impl EvalConfig {
    pub fn validate(&self) -> Result<()> {
        if self.server_url.trim().is_empty() {
            return Err(anyhow!("server_url cannot be empty"));
        }
        if self.server_model.trim().is_empty() {
            return Err(anyhow!("server_model cannot be empty"));
        }
        if self.context_window == 0 {
            return Err(anyhow!("context_window must be > 0"));
        }
        Ok(())
    }
}

/// Several fields hold `f32::to_bits()`, so a numeric zero default would decode
/// as a denormal. Mirrors the `src/lib.rs` bootstrap exactly.
fn build_telemetry_state() -> Arc<TelemetryState> {
    let f0 = || Arc::new(AtomicU32::new(0f32.to_bits()));
    let u0 = || Arc::new(AtomicU32::new(0));

    let latest_energy = f0();
    let latest_vad_prob = u0();
    let (latest_low, latest_mid, latest_high) = (f0(), f0(), f0());
    let (latest_playback_energy, latest_playback_low, latest_playback_mid, latest_playback_high) =
        (f0(), f0(), f0(), f0());
    let (latest_sys_cpu, latest_sys_ram, latest_vox_cpu) = (f0(), f0(), f0());
    let latest_vox_ram = Arc::new(AtomicU32::new(0));
    let (latest_stt_ms, latest_ttft_ms, latest_voice_latency_ms, latest_threads) =
        (u0(), u0(), u0(), u0());
    let (latest_tts_rtf, latest_playback_start_ms, latest_persistence_rate) = (f0(), u0(), f0());
    let is_db_healthy = Arc::new(AtomicBool::new(true));
    let is_private_mode = Arc::new(AtomicBool::new(false));
    let dropped_telemetry_events = Arc::new(AtomicU64::new(0));

    let (worker, telemetry_tx) = TelemetryAggregator::new(TelemetryAggregatorHandles {
        latest_energy: Arc::clone(&latest_energy),
        latest_vad_prob: Arc::clone(&latest_vad_prob),
        latest_low: Arc::clone(&latest_low),
        latest_mid: Arc::clone(&latest_mid),
        latest_high: Arc::clone(&latest_high),
        latest_sys_cpu: Arc::clone(&latest_sys_cpu),
        latest_sys_ram: Arc::clone(&latest_sys_ram),
        latest_vox_cpu: Arc::clone(&latest_vox_cpu),
        latest_vox_ram: Arc::clone(&latest_vox_ram),
        dropped_events: Arc::clone(&dropped_telemetry_events),
    });
    worker.start();

    Arc::new(TelemetryState {
        telemetry_tx,
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
        dropped_telemetry_events,
    })
}

pub async fn setup_isolated_eval_context(
    cfg: &EvalConfig,
    results_root: &std::path::Path,
) -> Result<EvalHarnessContext> {
    cfg.validate()?;
    paths::init();

    let run_id = crate::common::reporting::generate_run_id();
    let results_dir = results_root.join(&cfg.eval_name).join(&run_id);
    std::fs::create_dir_all(&results_dir)
        .with_context(|| format!("Failed to create results dir {:?}", results_dir))?;

    let db_path = results_dir.join("eval_vox.db");
    let db = Arc::new(
        vox_lib::persistence::VoxDb::open(&db_path)
            .await
            .map_err(|e| anyhow!("Failed to open eval db at {:?}: {}", db_path, e))?,
    );
    let conn = db
        .connect()
        .map_err(|e| anyhow!("Failed to connect to eval db: {}", e))?;
    vox_lib::persistence::schema::run_migrations(&conn)
        .await
        .map_err(|e| anyhow!("Failed to migrate eval db: {}", e))?;

    let app = tauri::test::mock_builder()
        .build(tauri::test::mock_context(tauri::test::noop_assets()))
        .map_err(|e| anyhow!("Failed to build mock Tauri app: {}", e))?;
    let tauri_app: tauri::AppHandle<tauri::test::MockRuntime> = app.handle().clone();

    let state = Arc::new(AppState::new(
        &tauri_app,
        None,
        build_telemetry_state(),
        Arc::clone(&db),
    ));
    state.conversation_id.store(1, Relaxed);

    let (event_tx, event_rx) = mpsc::channel::<VoxEvent>();

    let conn_cfg = ConnectionConfig::new(
        &cfg.server_url,
        &cfg.server_model,
        cfg.server_api_key.as_deref(),
        cfg.server_provider.as_deref(),
    );
    let llm_provider: Arc<dyn LlmProvider> = Arc::new(RemoteTransport::new(conn_cfg));
    let (llm_tx, llm_rx) = mpsc::channel::<LlmCommand>();
    spawn_llm_worker(llm_rx, Arc::clone(&llm_provider));

    let tts_settings = state
        .settings
        .read()
        .map_err(|e| anyhow!("Settings lock poisoned: {}", e))?
        .tts
        .clone();

    Ok(EvalHarnessContext {
        state,
        tauri_app,
        session_id: 1,
        run_id,
        results_dir,
        event_tx,
        event_rx: Arc::new(Mutex::new(event_rx)),
        llm_tx,
        llm_provider,
        db,
        tts_settings,
    })
}

pub fn build_modular_harness(
    ctx: &EvalHarnessContext,
    tool_filter: Option<&str>,
) -> Result<Harness> {
    let settings = ctx
        .state
        .settings
        .read()
        .map_err(|e| anyhow!("Settings lock poisoned: {}", e))?
        .clone();

    let harness = Harness::new_modular(
        Some(ctx.session_id),
        ctx.state.resolve_base_prompt(),
        None,
        &settings,
        ctx.llm_tx.clone(),
        true,
    );

    // `Harness` exposes no registry mutator, so isolation is verified rather than
    // enforced: confirm the requested tool exists instead of silently measuring
    // an unfiltered registry.
    if let Some(name) = tool_filter {
        let available: Vec<String> = harness
            .tool_registry()
            .canonical_definitions(vox_lib::core::events::PipelineMode::Modular)
            .into_iter()
            .map(|d| d.name)
            .collect();
        if !available.iter().any(|n| n == name) {
            return Err(anyhow!(
                "Tool '{}' not registered. Available: {:?}",
                name,
                available
            ));
        }
        log::info!(
            "[AgenticToolEval] target tool '{}' (available: {:?})",
            name,
            available
        );
    }

    Ok(harness)
}

pub fn build_turn_request(
    ctx: &EvalHarnessContext,
    turn_id: u32,
    query: String,
    tts_tx: &mpsc::Sender<vox_lib::services::tts::TtsCommand>,
    pending_synthesis_jobs: &Arc<std::sync::atomic::AtomicU32>,
) -> TurnExecutionRequest<tauri::test::MockRuntime> {
    TurnExecutionRequest {
        query,
        turn_id,
        owner: vox_lib::core::state::InteractionOwner::Assistant,
        cancel: tokio_util::sync::CancellationToken::new(),
        routing_ctx: RoutingContext::from_app_state(&ctx.state),
        app: ctx.tauri_app.clone(),
        app_state: Arc::clone(&ctx.state),
        db: Arc::clone(&ctx.db),
        accumulator: Arc::new(Mutex::new(TurnAccumulator::new())),
        tts_tx: Some(tts_tx.clone()),
        llm_tx: Some(ctx.llm_tx.clone()),
        pipeline_tx: Some(ctx.event_tx.clone()),
        pending_synthesis_jobs: Arc::clone(pending_synthesis_jobs),
        provider: Some(Arc::clone(&ctx.llm_provider)),
    }
}

#[derive(Debug, Default, Serialize)]
pub struct EventTrace {
    pub names: Vec<String>,
    pub counts: std::collections::BTreeMap<String, usize>,
}

impl EventTrace {
    pub fn record(&mut self, name: impl Into<String>) {
        let name = name.into();
        *self.counts.entry(name.clone()).or_insert(0) += 1;
        self.names.push(name);
    }
    #[allow(dead_code)]
    pub fn len(&self) -> usize {
        self.names.len()
    }
    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.names.is_empty()
    }
    #[allow(dead_code)]
    pub fn contains(&self, name: &str) -> bool {
        self.counts.contains_key(name)
    }
}

#[allow(dead_code)]
pub fn drain_events(
    rx: &Mutex<mpsc::Receiver<VoxEvent>>,
    trace: &mut EventTrace,
    max_events: usize,
) -> bool {
    let rx = rx.lock();
    let mut drained = 0;
    while drained < max_events {
        match rx.try_recv() {
            Ok(e) => {
                trace.record(event_name(&e));
                drained += 1;
            }
            Err(mpsc::TryRecvError::Empty) => return true,
            Err(mpsc::TryRecvError::Disconnected) => return false,
        }
    }
    true
}

pub fn drain_events_after_turn(rx: &Mutex<mpsc::Receiver<VoxEvent>>, grace_ms: u64) -> EventTrace {
    let rx = rx.lock();
    let mut trace = EventTrace::default();
    let deadline = std::time::Instant::now() + std::time::Duration::from_millis(grace_ms);
    loop {
        let mut progressed = false;
        while let Ok(e) = rx.try_recv() {
            trace.record(event_name(&e));
            progressed = true;
        }
        if !progressed || std::time::Instant::now() >= deadline {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    trace
}

/// Derived from `Debug` so a new event variant is never silently dropped.
pub fn event_name(event: &VoxEvent) -> String {
    format!("{:?}", event)
        .split(['{', '('])
        .next()
        .unwrap_or("Unknown")
        .trim()
        .to_string()
}

#[allow(dead_code)]
pub fn outcome_text(outcome: &TurnOutcome) -> String {
    match outcome {
        TurnOutcome::Completed {
            assistant_response, ..
        } => assistant_response.clone(),
        TurnOutcome::Cancelled { .. } => "[CANCELLED]".to_string(),
        TurnOutcome::Error { message, .. } => format!("[ERROR: {}]", message),
        TurnOutcome::DuplicateIgnored { .. } => "[DUPLICATE]".to_string(),
    }
}

pub fn persist_event_trace(ctx: &EvalHarnessContext, trace: &EventTrace) -> Result<PathBuf> {
    let p = ctx
        .results_dir
        .join(crate::common::stage_dump::names::PIPELINE_EVENTS);
    std::fs::write(&p, serde_json::to_string_pretty(trace)?)?;
    Ok(p)
}
