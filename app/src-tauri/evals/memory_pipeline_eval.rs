//! ============================================================================
//! memory_pipeline_eval.rs — Shared-DB compaction, ingestion, and consolidation eval
//! ============================================================================
//! Category     : Evaluation
//! Component    : Memory compaction, ingestion, and personal-memory consolidation
//! Prerequisites: qwen3.5:4b on the configured Ollama server; local MiniLM-L12 ONNX model
//! Execution    : cargo run --release --bin memory_pipeline_eval -- --help
//! Metrics      : Compaction crossings, queue drain, fact continuity, personal-memory versions,
//!                cross-case state continuity, failures, and latency
//! ============================================================================

#[path = "common/mod.rs"]
mod common;

use std::{
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

use anyhow::{Context, Result};
use clap::Parser;
use common::{
    calibration::{load_count_calibration, write_count_calibration, CountCalibration},
    consolidation_judge::{judge_consolidation, JudgeSettings},
    db, paths,
    pipeline_report::{
        case_start_ms, failed_case_report, failed_setup_report, fetch_compaction_observations,
        validate_compaction_ranges, write_final_report, CaseReport, ConsolidationObservation,
        ConsolidationOutcome, FinalReportConfig, IngestionCycleReport, SuggestionObservation,
        TurnTelemetry,
    },
    preservation::untargeted_preservation,
    report, settings_cfg,
    turns::DatasetTurn,
};
use turso::Connection;
use vox_lib::{
    core::{
        defaults::{DEFAULT_LLM_MAX_OUTPUT_TOKENS, DEFAULT_SYSTEM_PROMPT_MODULAR},
        settings::VoxSettings,
    },
    persistence::{
        facts::fetch_active_facts_by_type,
        personal_memory::{
            fetch_pending_suggestions, get_personal_memory, PersonalMemorySuggestionRecord,
        },
        queue::has_unfinished_items,
        sessions::fetch_session_continuation,
        VoxDb,
    },
    services::{
        harness::{CompactionParams, CompactionStage, ContextBudgetStage, ContextStatus, Harness},
        llm::{actor::create_llm_provider_from_llm_settings, LlmCommand, LlmProvider},
        memory::{
            ingestion::run_ingestion_cycle,
            ml::embedder::unload_embedder,
            personal::{consolidate_personal_memory, resolve_memory_suggestions},
        },
    },
};

const DEFAULT_DB_PATH: &str = "evals/assets/memory_pipeline.db";
const DEFAULT_SERVER_URL: &str = "http://100.67.98.126:11434/v1";
const DEFAULT_SERVER_MODEL: &str = "qwen3.5:9b";
const DEFAULT_SERVER_PROVIDER: &str = "ollama";
const TOP_LEVEL_TIMEOUT_SECS: u64 = 6 * 60 * 60;
const STAGE_TIMEOUT_SECS: u64 = 10 * 60;
const PROVIDER_PREFLIGHT_TIMEOUT_SECS: u64 = 30;
const MAX_INGESTION_CYCLES: usize = 512;

/// Name and input-turn count per case.
const EXPECTED_CASES: &[(&str, usize)] = &[
    ("case_01_under_threshold_035_turns", 35),
    ("case_02_under_threshold_075_turns", 75),
    ("case_03_under_threshold_090_turns", 90),
    ("case_04_one_crossing_180_turns", 180),
    ("case_05_one_crossing_200_turns", 200),
    ("case_06_one_crossing_220_turns", 220),
    ("case_07_two_crossings_350_turns", 350),
    ("case_08_two_crossings_450_turns", 450),
    ("case_09_three_crossings_500_turns", 500),
    ("case_10_three_crossings_550_turns", 550),
    ("case_11_three_crossings_600_turns", 600),
    ("case_12_three_crossings_620_turns", 620),
    ("case_13_three_crossings_650_turns", 650),
    ("case_14_three_crossings_680_turns", 680),
];

#[derive(Parser, Debug, Clone)]
#[command(
    name = "memory_pipeline_eval",
    about = "Sequential shared-DB compaction, ingestion, and consolidation evaluation"
)]
struct Args {
    #[arg(long)]
    dataset_dir: Option<PathBuf>,
    #[arg(long, default_value = DEFAULT_DB_PATH)]
    db_path: PathBuf,
    #[arg(long)]
    case: Option<u32>,
    #[arg(long, default_value_t = 8192)]
    context_window: u32,
    #[arg(long, default_value = DEFAULT_SERVER_URL)]
    server_url: String,
    #[arg(long, default_value = DEFAULT_SERVER_MODEL)]
    server_model: String,
    #[arg(long, default_value = DEFAULT_SERVER_PROVIDER)]
    server_provider: String,
    #[arg(long, default_value = "")]
    server_api_key: String,
    #[arg(long, default_value_t = false)]
    no_judge: bool,
    #[arg(long)]
    judge_model: Option<String>,
    #[arg(long)]
    judge_url: Option<String>,
    #[arg(long)]
    reject_probe_case: Option<u32>,
    #[arg(long, default_value_t = false)]
    write_calibration: bool,
}

#[derive(Debug, Clone)]
struct EvalCase {
    name: String,
    ordinal: usize,
    manual_compaction: bool,
    turns: Vec<DatasetTurn>,
}

#[derive(Debug, Clone)]
struct RunConfig {
    calibration: CountCalibration,
    judge_enabled: bool,
    judge_model: String,
    judge_url: String,
    reject_probe_case: Option<u32>,
}

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();
    let args = Args::parse();
    let started = Instant::now();
    let result = tokio::time::timeout(
        Duration::from_secs(TOP_LEVEL_TIMEOUT_SECS),
        run(args.clone()),
    )
    .await;
    match result {
        Ok(result) => result,
        Err(_) => {
            let dataset_dir = paths::resolve(args.dataset_dir.clone().unwrap_or_else(|| {
                PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("../../sandbox/datasets/eval-sessions")
            }));
            let db_path = paths::resolve(args.db_path.clone());
            let run_id = report::new_run_id();
            let failure = failed_setup_report("Memory pipeline eval top-level timeout exceeded");
            let cfg = FinalReportConfig {
                eval_name: "memory_pipeline",
                run_id: &run_id,
                server_url: &args.server_url,
                server_model: &args.server_model,
                server_provider: &args.server_provider,
                db_path: &db_path,
                dataset_dir: &dataset_dir,
                context_window: args.context_window,
                no_judge: args.no_judge,
                judge_model: args.judge_model.as_deref(),
                judge_url: args.judge_url.as_deref(),
                is_subset: args.case.is_some(),
                run_passed: false,
                total_latency_s: started.elapsed().as_secs_f64(),
                config_source: "unavailable",
            };
            write_final_report(&cfg, &[failure])?;
            anyhow::bail!("Memory pipeline eval top-level timeout exceeded")
        }
    }
}

async fn run(args: Args) -> Result<()> {
    let started = Instant::now();
    let run_id = report::new_run_id();
    let eval_name = "memory_pipeline";
    let dataset_dir = paths::resolve(args.dataset_dir.clone().unwrap_or_else(|| {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../sandbox/datasets/eval-sessions")
    }));
    let db_path = paths::resolve(args.db_path.clone());
    let is_subset = args.case.is_some();
    let prepared = prepare_run(&args, &dataset_dir).await;
    let (selected_cases, is_subset, settings, provider, _db, conn) = match prepared {
        Ok(prepared) => prepared,
        Err(error) => {
            let failure = failed_setup_report(&format!("{error:#}"));
            let cfg = FinalReportConfig {
                eval_name,
                run_id: &run_id,
                server_url: &args.server_url,
                server_model: &args.server_model,
                server_provider: &args.server_provider,
                db_path: &db_path,
                dataset_dir: &dataset_dir,
                context_window: args.context_window,
                no_judge: args.no_judge,
                judge_model: args.judge_model.as_deref(),
                judge_url: args.judge_url.as_deref(),
                is_subset,
                run_passed: false,
                total_latency_s: started.elapsed().as_secs_f64(),
                config_source: "unavailable",
            };
            write_final_report(&cfg, &[failure])?;
            return Err(error);
        }
    };
    let config = RunConfig {
        calibration: load_count_calibration(&args.server_model, args.context_window),
        judge_enabled: !args.no_judge,
        judge_model: args
            .judge_model
            .clone()
            .unwrap_or_else(|| args.server_model.clone()),
        judge_url: args
            .judge_url
            .clone()
            .unwrap_or_else(|| args.server_url.clone()),
        reject_probe_case: args.reject_probe_case,
    };
    println!(
        "[config] executor={} judge={} judge_url={} calibration={} reject_probe_case={:?}",
        args.server_model,
        config.judge_model,
        config.judge_url,
        config.calibration.source,
        config.reject_probe_case
    );
    let mut reports = Vec::with_capacity(selected_cases.len());
    let mut previous_session_start = None;

    for case in selected_cases.iter() {
        let case_started = Instant::now();
        if !is_subset {
            if let Some(previous) = previous_session_start {
                anyhow::ensure!(
                    case.ordinal != 1 && case_start_ms(case.ordinal) - previous >= 86_400_000,
                    "Session timestamps do not have a one-day gap before {}",
                    case.name
                );
            }
        }
        previous_session_start = Some(case_start_ms(case.ordinal));
        let report = run_case(case, &conn, provider.as_ref(), &settings, &config).await;
        match report {
            Ok(report) => {
                let consolidation_line = report
                    .consolidation
                    .as_ref()
                    .map(|observation| {
                        format!(
                            " staged={} skipped_ops={} unexplained_lost={} reanchor={:?} judge={}",
                            observation.staged_suggestions,
                            observation.skipped_operations,
                            observation.unexplained_lost_lines.len(),
                            observation.reanchor_valid,
                            observation
                                .judge
                                .as_ref()
                                .map(|judge| format!("{}:{}", judge.model, judge.verdict))
                                .unwrap_or_else(|| "skipped".to_string())
                        )
                    })
                    .unwrap_or_else(|| " no_candidates".to_string());
                println!(
                    "[case] {}: status={} compactions={} expected_calibrated={:?} delta={:?} ingestion_cycles={} queue_after={} |{} elapsed={}ms",
                    report.case,
                    report.status,
                    report.actual_compactions,
                    report.expected_compactions_calibrated,
                    report.count_calibration_delta,
                    report.ingestion_cycles,
                    report.queue_after,
                    consolidation_line,
                    case_started.elapsed().as_millis()
                );
                reports.push(report);
            }
            Err(error) => {
                let failure =
                    failed_case_report(&case.name, case.turns.len(), &format!("{error:#}"));
                println!(
                    "[case] {}: status=FAILED compactions={} elapsed={}ms error={error}",
                    failure.case,
                    failure.actual_compactions,
                    case_started.elapsed().as_millis()
                );
                reports.push(failure);
                unload_embedder();
                let cfg = FinalReportConfig {
                    eval_name,
                    run_id: &run_id,
                    server_url: &args.server_url,
                    server_model: &args.server_model,
                    server_provider: &args.server_provider,
                    db_path: &db_path,
                    dataset_dir: &dataset_dir,
                    context_window: args.context_window,
                    no_judge: args.no_judge,
                    judge_model: args.judge_model.as_deref(),
                    judge_url: args.judge_url.as_deref(),
                    is_subset,
                    run_passed: false,
                    total_latency_s: started.elapsed().as_secs_f64(),
                    config_source: &config.calibration.source,
                };
                write_final_report(&cfg, &reports)?;
                return Err(error.context(format!("case {} failed", case.name)));
            }
        }
    }

    unload_embedder();
    let cases_passed = reports.iter().all(|report| {
        report.status == "passed" && report.error.is_none() && report.failed_queue_items == 0
    });
    let memory_pipeline_exercised = reports
        .iter()
        .any(|report| report.memory_consolidation_exercised);
    let all_passed = cases_passed && (is_subset || memory_pipeline_exercised);
    if args.write_calibration {
        let case_compactions: Vec<(String, u32)> = reports
            .iter()
            .map(|r| (r.case.clone(), r.actual_compactions))
            .collect();
        write_count_calibration(
            &args.server_model,
            args.context_window,
            &case_compactions,
            &run_id,
        )?;
    }

    let payload_status = if is_subset {
        if all_passed {
            "passed_subset"
        } else {
            "failed_subset"
        }
    } else if all_passed {
        "passed"
    } else {
        "failed"
    };
    let cfg = FinalReportConfig {
        eval_name,
        run_id: &run_id,
        server_url: &args.server_url,
        server_model: &args.server_model,
        server_provider: &args.server_provider,
        db_path: &db_path,
        dataset_dir: &dataset_dir,
        context_window: args.context_window,
        no_judge: args.no_judge,
        judge_model: args.judge_model.as_deref(),
        judge_url: args.judge_url.as_deref(),
        is_subset,
        run_passed: all_passed,
        total_latency_s: started.elapsed().as_secs_f64(),
        config_source: &config.calibration.source,
    };
    write_final_report(&cfg, &reports)?;
    if !all_passed && !is_subset {
        anyhow::bail!("One or more eval cases failed");
    }
    println!(
        "Eval complete: status={payload_status} db={}",
        db_path.display()
    );
    Ok(())
}

async fn prepare_run(
    args: &Args,
    dataset_dir: &Path,
) -> Result<(
    Vec<EvalCase>,
    bool,
    VoxSettings,
    Box<dyn LlmProvider>,
    VoxDb,
    Connection,
)> {
    let cases = load_cases(dataset_dir).await?;
    validate_manifest(&cases)?;
    let is_subset = args.case.is_some();
    let selected_cases = select_cases(cases, args.case)?;
    anyhow::ensure!(
        args.context_window > DEFAULT_LLM_MAX_OUTPUT_TOKENS,
        "Context window must exceed reserved output tokens"
    );
    let mut settings = settings_cfg::server_llm_settings(
        &args.server_url,
        &args.server_model,
        args.context_window,
        settings_cfg::optional_api_key(&args.server_api_key),
        settings_cfg::optional_provider(&args.server_provider),
    );
    settings.working_memory.auto_compaction = true;
    settings.personal_memory.context_retrieval_enabled = false;
    let provider = create_llm_provider_from_llm_settings(
        &settings_cfg::llm_settings_of(&settings),
        Path::new(""),
    )
    .map_err(|e| anyhow::anyhow!("Failed to build Ollama provider: {e}"))?;
    tokio::time::timeout(
        Duration::from_secs(PROVIDER_PREFLIGHT_TIMEOUT_SECS),
        provider.health_check(),
    )
    .await
    .context("Provider preflight timed out")?
    .context("Provider preflight failed")?;
    let models = tokio::time::timeout(
        Duration::from_secs(PROVIDER_PREFLIGHT_TIMEOUT_SECS),
        provider.list_models(),
    )
    .await
    .context("Provider model preflight timed out")?
    .context("Provider model preflight failed")?;
    anyhow::ensure!(
        models.iter().any(|model| model.id == args.server_model),
        "Configured model is not available on the provider: {}",
        args.server_model
    );
    let (database, conn) = db::open_fresh_eval_db(&paths::resolve(args.db_path.clone())).await?;
    Ok((
        selected_cases,
        is_subset,
        settings,
        provider,
        database,
        conn,
    ))
}

async fn load_cases(dataset_dir: &Path) -> Result<Vec<EvalCase>> {
    let mut paths = Vec::new();
    for entry in std::fs::read_dir(dataset_dir)
        .with_context(|| format!("Failed to read dataset directory {}", dataset_dir.display()))?
    {
        let path = entry?.path();
        if path.extension().and_then(|value| value.to_str()) == Some("json")
            && path
                .file_name()
                .and_then(|value| value.to_str())
                .is_some_and(|name| name.starts_with("case_"))
        {
            paths.push(path);
        }
    }
    paths.sort();

    let mut cases = Vec::with_capacity(paths.len());
    for (ordinal, path) in paths.into_iter().enumerate() {
        let name = path
            .file_stem()
            .and_then(|value| value.to_str())
            .context("Case filename has no UTF-8 stem")?
            .to_string();
        let manual_compaction = name.contains("under_threshold");
        anyhow::ensure!(
            name.contains("under_threshold")
                || name.contains("one_crossing")
                || name.contains("two_crossings")
                || name.contains("three_crossings"),
            "Unknown case class in filename: {name}"
        );
        let raw = std::fs::read_to_string(&path)
            .with_context(|| format!("Failed to read case {}", path.display()))?;
        let case_turns: Vec<DatasetTurn> = serde_json::from_str(&raw)
            .with_context(|| format!("Failed to parse case {}", path.display()))?;
        anyhow::ensure!(!case_turns.is_empty(), "Case {} is empty", name);
        anyhow::ensure!(
            case_turns
                .iter()
                .enumerate()
                .all(|(index, turn)| turn.turn == (index as u32) + 1),
            "Case {name} has non-contiguous turn IDs"
        );
        cases.push(EvalCase {
            name,
            ordinal: ordinal + 1,
            manual_compaction,
            turns: case_turns,
        });
    }
    anyhow::ensure!(
        !cases.is_empty(),
        "No eval cases found in {}",
        dataset_dir.display()
    );
    Ok(cases)
}

fn validate_manifest(cases: &[EvalCase]) -> Result<()> {
    anyhow::ensure!(
        cases.len() == EXPECTED_CASES.len(),
        "Expected {} eval cases, found {}",
        EXPECTED_CASES.len(),
        cases.len()
    );
    let total_turns = cases.iter().map(|case| case.turns.len()).sum::<usize>();
    anyhow::ensure!(
        total_turns == 5200,
        "Expected 5200 total eval turns, found {total_turns}"
    );
    for (expected_name, expected_turns) in EXPECTED_CASES {
        let case = cases
            .iter()
            .find(|case| case.name == *expected_name)
            .with_context(|| format!("Missing expected eval case {expected_name}"))?;
        anyhow::ensure!(
            case.turns.len() == *expected_turns,
            "Case {expected_name} has {} turns, expected {expected_turns}",
            case.turns.len()
        );
    }
    Ok(())
}

fn select_cases(cases: Vec<EvalCase>, case: Option<u32>) -> Result<Vec<EvalCase>> {
    let Some(case) = case else {
        return Ok(cases);
    };
    let selected = cases
        .into_iter()
        .find(|candidate| candidate.ordinal == case as usize)
        .with_context(|| format!("Case {case} is out of range"))?;
    Ok(vec![selected])
}

async fn run_case(
    case: &EvalCase,
    conn: &Connection,
    provider: &dyn vox_lib::services::llm::LlmProvider,
    settings: &vox_lib::core::settings::VoxSettings,
    config: &RunConfig,
) -> Result<CaseReport> {
    let session_start_ms = case_start_ms(case.ordinal);
    let facts_before = db::count_facts(conn).await?;
    let personal_memory_before = get_personal_memory(conn, None).await?;
    let session_id =
        db::create_session_at_timestamp(conn, session_start_ms, Some("default")).await?;
    let continuation = fetch_session_continuation(conn, session_id).await?;
    let personal_memory = continuation.personal_memory;
    let base_record = get_personal_memory(conn, None).await?;
    let base_document = base_record.content.trim().to_string();
    let base_version = base_record.version;
    let candidates = fetch_active_facts_by_type(conn, "personal").await?;
    let candidate_count = candidates.len();
    let pending_before_ids: std::collections::HashSet<String> =
        fetch_pending_suggestions(conn, None)
            .await?
            .into_iter()
            .map(|suggestion| suggestion.id)
            .collect();
    let judge_settings: Option<JudgeSettings> = JudgeSettings {
        enabled: config.judge_enabled,
        url: config.judge_url.clone(),
        model: config.judge_model.clone(),
    }
    .into();
    let budget = ContextBudgetStage::new(
        settings.llm.context_window as usize,
        settings.llm.max_output_tokens as usize,
    );
    let (llm_tx, _llm_rx) = std::sync::mpsc::channel::<LlmCommand>();
    let mut harness = Harness::new_modular(
        Some(session_id),
        DEFAULT_SYSTEM_PROMPT_MODULAR.to_string(),
        personal_memory.clone(),
        settings,
        llm_tx,
        false,
    );
    harness.seed_continuation(
        continuation.latest_summary,
        continuation.turns,
        continuation.title_is_set,
    );
    let assembled_prompt = harness.assembled_system_prompt();
    let personal_memory_injected = personal_memory
        .as_deref()
        .map(|content| !content.trim().is_empty() && assembled_prompt.contains("<user_identity>"))
        .unwrap_or(true);
    if personal_memory_before.content.trim().is_empty() {
        anyhow::ensure!(
            personal_memory_injected,
            "Blank personal memory unexpectedly injected a user identity block"
        );
    } else {
        let memory_prefix = personal_memory_before
            .content
            .trim()
            .chars()
            .take(80)
            .collect::<String>();
        anyhow::ensure!(
            assembled_prompt.contains(&memory_prefix),
            "Active personal memory was not carried into the next session prompt"
        );
    }

    let queue_before = db::count_queue_items(conn).await?;
    let mut crossing_turns = Vec::new();
    let mut critical_turns = Vec::new();
    let mut manual_compactions = 0;
    let mut compaction_latencies_ms = Vec::new();
    let mut turn_telemetry = Vec::with_capacity(case.turns.len());
    let mut empty_context_compactions = 0;

    for turn in &case.turns {
        let can_compact = harness.intake_recorded_turn(turn.user.clone());
        let tracked_tokens = budget.calculate_tracked_tokens(harness.messages());
        let (utilization, status) = budget.evaluate_utilization(tracked_tokens);
        let critical = status == ContextStatus::Critical;
        let near_miss = status == ContextStatus::SoftWarning;
        if critical {
            critical_turns.push(turn.turn);
        }
        let history = can_compact.then(|| harness.messages().to_vec());
        turn_telemetry.push(TurnTelemetry {
            turn: turn.turn,
            tracked_tokens,
            usable_budget: budget.usable_budget(),
            utilization_percent: utilization * 100.0,
            budget_status: format!("{status:?}"),
            near_miss,
            critical_eligible: critical && can_compact,
            history_messages: harness.messages().len(),
            compaction_triggered: critical && can_compact,
        });
        if let Some(history) = history {
            crossing_turns.push(turn.turn);
            let compaction_started = Instant::now();
            let params = CompactionParams {
                session_id,
                trigger_kind: "critical",
                from_turn_id: harness.from_turn_id(),
                to_turn_id: turn.turn,
                history_messages: &history,
                llm_settings: Some(&settings.llm),
                cancel: None,
            };
            let result = tokio::time::timeout(
                Duration::from_secs(STAGE_TIMEOUT_SECS),
                CompactionStage::run_and_persist(provider, conn, params),
            )
            .await
            .context("Production compaction call timed out")?
            .context("Production compaction failed")?;
            compaction_latencies_ms.push(compaction_started.elapsed().as_millis());
            if result.session_context.trim().is_empty() {
                empty_context_compactions += 1;
            }
            harness.apply_compaction_result(&Ok(result), turn.turn, &turn.user);
        }
        harness.commit_turn(turn.assistant.clone());
        db::persist_turn_at_timestamp(
            conn,
            session_id,
            turn.turn,
            &turn.user,
            &turn.assistant,
            session_start_ms + i64::from(turn.turn) * 60_000,
        )
        .await?;
    }

    if case.manual_compaction {
        let to_turn = case
            .turns
            .last()
            .map(|turn| turn.turn)
            .context("Manual compaction case has no turns")?;
        let history = harness.messages().to_vec();
        let compaction_started = Instant::now();
        let params = CompactionParams {
            session_id,
            trigger_kind: "manual",
            from_turn_id: harness.from_turn_id(),
            to_turn_id: to_turn,
            history_messages: &history,
            llm_settings: Some(&settings.llm),
            cancel: None,
        };
        let result = tokio::time::timeout(
            Duration::from_secs(STAGE_TIMEOUT_SECS),
            CompactionStage::run_and_persist(provider, conn, params),
        )
        .await
        .context("Production manual compaction call timed out")?
        .context("Production manual compaction failed")?;
        compaction_latencies_ms.push(compaction_started.elapsed().as_millis());
        manual_compactions += 1;
        if result.session_context.trim().is_empty() {
            empty_context_compactions += 1;
        }
        harness.apply_compaction_result(&Ok(result), to_turn, "");
    }

    let actual_compactions = db::count_case_compactions(conn, session_id).await?;
    anyhow::ensure!(
        actual_compactions as usize == crossing_turns.len() + manual_compactions as usize,
        "Compaction ledger count {actual_compactions} differs from critical plus manual count {}",
        crossing_turns.len() + manual_compactions as usize
    );
    let incomplete_compactions = db::count_incomplete_compactions(conn, session_id).await?;
    anyhow::ensure!(
        incomplete_compactions == 0,
        "{incomplete_compactions} compaction records are not completed"
    );
    let compaction_ledger = fetch_compaction_observations(conn, session_id).await?;
    let ledger_ranges_contiguous = validate_compaction_ranges(&compaction_ledger);
    let budget_invariants_valid = budget.usable_budget()
        == settings.llm.context_window as usize - settings.llm.max_output_tokens as usize
        && critical_turns == crossing_turns;
    let final_watermark_turn = compaction_ledger
        .last()
        .map(|observation| observation.to_turn)
        .unwrap_or(0);
    let final_watermark_valid = final_watermark_turn == 0
        || harness.from_turn_id() == final_watermark_turn.saturating_add(1);
    let queue_after_compaction = db::count_queue_items(conn).await?;
    let max_cycles = ((queue_after_compaction as usize / 16) + 2).clamp(1, MAX_INGESTION_CYCLES);
    let mut ingestion_cycle_reports = Vec::new();
    for cycle in 0..max_cycles {
        let before = db::count_queue_items(conn).await?;
        if cycle > 0 && before == 0 {
            break;
        }
        let cycle_started = Instant::now();
        let summary = tokio::time::timeout(
            Duration::from_secs(STAGE_TIMEOUT_SECS),
            run_ingestion_cycle(conn),
        )
        .await
        .context("Production ingestion cycle timed out")?
        .context("Production ingestion cycle failed")?;
        let after = db::count_queue_items(conn).await?;
        if before > 0 && after >= before {
            anyhow::bail!("Ingestion cycle made no queue progress: {before} -> {after}");
        }
        ingestion_cycle_reports.push(IngestionCycleReport {
            stage1_processed: summary.stage1.processed,
            stage1_duplicates_deactivated: summary.stage1.duplicates_deactivated,
            stage1_errors: summary.stage1.errors,
            stage2_processed: summary.stage2.processed,
            stage2_inserted: summary.stage2.inserted,
            stage2_duplicates_deactivated: summary.stage2.duplicates_deactivated,
            stage2_errors: summary.stage2.errors,
            latency_ms: cycle_started.elapsed().as_millis(),
        });
    }
    let ingestion_cycles = ingestion_cycle_reports.len() as u32;
    anyhow::ensure!(
        !has_unfinished_items(conn).await?,
        "Ingestion queue still has unfinished items after bounded drain"
    );
    let queue_after = db::count_queue_items(conn).await?;
    let failed_queue_items = db::count_failed_queue_items(conn).await?;
    anyhow::ensure!(
        failed_queue_items == 0,
        "{failed_queue_items} ingestion queue items are failed"
    );
    let queue_accounting_complete = queue_after == 0 && failed_queue_items == 0;
    let stage1_items_processed = ingestion_cycle_reports
        .iter()
        .map(|report| report.stage1_processed)
        .sum::<usize>();
    let stage2_items_processed = ingestion_cycle_reports
        .iter()
        .map(|report| report.stage2_processed)
        .sum::<usize>();
    let ingestion_items_processed = stage1_items_processed + stage2_items_processed;
    let ingestion_items_inserted = ingestion_cycle_reports
        .iter()
        .map(|report| report.stage2_inserted)
        .sum::<usize>();
    let ingestion_errors = ingestion_cycle_reports
        .iter()
        .map(|report| report.stage1_errors + report.stage2_errors)
        .sum::<usize>();
    anyhow::ensure!(
        ingestion_errors == 0,
        "Ingestion stages reported {ingestion_errors} errors"
    );
    let expected_queue_items = queue_after_compaction.max(0) as usize;
    let ingestion_accounting_valid = stage1_items_processed == expected_queue_items
        && stage2_items_processed == expected_queue_items
        && ingestion_items_inserted == expected_queue_items;
    anyhow::ensure!(
        ingestion_accounting_valid,
        "Ingestion accounting mismatch: queue={expected_queue_items} stage1={stage1_items_processed} stage2={stage2_items_processed} inserted={ingestion_items_inserted}"
    );

    let consolidation = if config.reject_probe_case == Some(case.ordinal as u32) {
        run_reject_probe(conn, &pending_before_ids, &base_document, base_version).await?
    } else {
        run_consolidation_and_accept(
            conn,
            provider,
            settings,
            &base_document,
            base_version,
            &pending_before_ids,
            judge_settings.as_ref(),
        )
        .await?
    };
    let consolidation_latency_ms = consolidation
        .as_ref()
        .map(|outcome| outcome.latency_ms)
        .unwrap_or_default();
    let memory_consolidation_exercised = consolidation
        .as_ref()
        .is_some_and(|outcome| outcome.observation.staged_suggestions > 0);
    let accepted_document = consolidation
        .as_ref()
        .map(|outcome| outcome.accepted_document.clone())
        .unwrap_or_else(|| base_document.clone());

    let facts_after = db::count_facts(conn).await?;
    let actual_compactions_u32 =
        u32::try_from(actual_compactions).context("Compaction count does not fit in u32")?;
    let expected_compactions_calibrated = config.calibration.per_case.get(&case.name).copied();
    let count_calibration_delta = expected_compactions_calibrated
        .map(|expected| i64::from(actual_compactions_u32) - i64::from(expected));
    let trigger_correctness_valid = budget_invariants_valid
        && ledger_ranges_contiguous
        && final_watermark_valid
        && empty_context_compactions == 0;

    let consolidation_valid = consolidation.as_ref().is_none_or(|outcome| {
        let observation = &outcome.observation;
        observation.document_unchanged_after_staging
            && observation.candidate_partition_valid
            && observation.pending_anchored_to_base_version
            && observation.untargeted_preservation_valid
            && observation.unexplained_lost_lines.is_empty()
            && observation.skipped_operations == 0
            && observation.facts_left_active == 0
            && observation.reanchor_valid != Some(false)
    });

    Ok(CaseReport {
        case: case.name.clone(),
        expected_compactions_calibrated,
        count_calibration_delta,
        count_calibration_source: config.calibration.source.clone(),
        actual_compactions: actual_compactions_u32,
        manual_compactions,
        trigger_correctness_valid,
        session_id,
        session_start_ms,
        input_turns: case.turns.len(),
        crossing_turns,
        critical_turns,
        budget_invariants_valid,
        compaction_latencies_ms,
        compaction_ledger,
        ledger_ranges_contiguous,
        final_watermark_valid,
        empty_context_compactions,
        ingestion_cycles,
        ingestion_items_processed,
        stage1_items_processed,
        stage2_items_processed,
        ingestion_items_inserted,
        queue_accounting_complete,
        ingestion_accounting_valid,
        ingestion_cycle_reports,
        consolidation_latency_ms,
        queue_before,
        queue_after_compaction,
        queue_after,
        failed_queue_items,
        facts_before,
        facts_after,
        personal_memory_version_before: personal_memory_before.version,
        personal_memory_chars_before: personal_memory_before.content.len(),
        personal_memory_chars_after: accepted_document.len(),
        personal_memory_injected,
        personal_facts_before_consolidation: candidate_count as i64,
        memory_consolidation_exercised,
        consolidation: consolidation
            .as_ref()
            .map(|outcome| outcome.observation.clone()),
        base_document,
        accepted_document,
        turn_telemetry,
        status: if trigger_correctness_valid
            && queue_accounting_complete
            && ingestion_accounting_valid
            && personal_memory_injected
            && consolidation_valid
        {
            "passed".to_string()
        } else {
            "failed_invariant".to_string()
        },
        error: None,
    })
}

async fn run_consolidation_and_accept(
    conn: &Connection,
    provider: &dyn vox_lib::services::llm::LlmProvider,
    settings: &vox_lib::core::settings::VoxSettings,
    base_document: &str,
    base_version: i64,
    pending_before_ids: &std::collections::HashSet<String>,
    judge: Option<&JudgeSettings>,
) -> Result<Option<ConsolidationOutcome>> {
    let candidates = fetch_active_facts_by_type(conn, "personal").await?;
    if candidates.is_empty() {
        return Ok(None);
    }
    let candidate_texts: Vec<String> = candidates.iter().map(|fact| fact.text.clone()).collect();
    let candidate_count = candidates.len();

    let started = Instant::now();
    tokio::time::timeout(
        Duration::from_secs(STAGE_TIMEOUT_SECS),
        consolidate_personal_memory(conn, provider, None, None, Some(&settings.llm), None),
    )
    .await
    .context("Production personal-memory consolidation timed out")?
    .context("Production personal-memory consolidation failed")?;

    let after_stage = get_personal_memory(conn, None).await?;
    let document_unchanged_after_staging =
        after_stage.version == base_version && after_stage.content.trim() == base_document;

    let pending_after = fetch_pending_suggestions(conn, None).await?;
    let staged: Vec<PersonalMemorySuggestionRecord> = pending_after
        .into_iter()
        .filter(|suggestion| !pending_before_ids.contains(&suggestion.id))
        .collect();

    let operations: Vec<SuggestionObservation> = staged
        .iter()
        .map(|suggestion| {
            let engine_would_match = match suggestion.op.as_str() {
                "insert_after" | "insert" => !suggestion.content.trim().is_empty(),
                "replace" => !suggestion.content.trim().is_empty(),
                "delete" => true,
                _ => false,
            };
            SuggestionObservation {
                op: suggestion.op.clone(),
                target_index: suggestion.target_index,
                content: suggestion.content.clone(),
                engine_would_match,
            }
        })
        .collect();
    let skipped_operations = operations
        .iter()
        .filter(|operation| !operation.engine_would_match)
        .count() as u32;

    let pending_anchored_to_base_version = staged
        .iter()
        .all(|suggestion| suggestion.base_memory_version == base_version);

    let active_left = db::count_facts_by_status(conn, "personal", "active").await?;
    let candidate_partition_valid = active_left == 0;

    let mut reanchor_probe_exercised = false;
    let mut reanchor_valid: Option<bool> = None;
    let mut expected_version = base_version;
    if staged.len() >= 2 {
        reanchor_probe_exercised = true;
        let first = resolve_memory_suggestions(conn, None, Some(&staged[0].id), "accept")
            .await
            .context("Single-suggestion accept failed")?;
        let remaining = fetch_pending_suggestions(conn, None).await?;
        reanchor_valid = Some(
            remaining
                .iter()
                .all(|suggestion| suggestion.base_memory_version == first.version)
                && remaining.len() == staged.len() - 1,
        );
        expected_version = first.version;
    }

    let accepted = resolve_memory_suggestions(conn, None, None, "accept")
        .await
        .context("Accept-all suggestion resolution failed")?;
    let latency_ms = started.elapsed().as_millis();
    anyhow::ensure!(
        accepted.version == expected_version + 1,
        "Accept-all produced version {} but {} was expected",
        accepted.version,
        expected_version + 1
    );

    let still_pending = fetch_pending_suggestions(conn, None).await?;
    let accepted_count = still_pending.len() as u32;
    let consolidated_in_db = db::count_facts_by_status(conn, "personal", "consolidated").await?;
    let rejected_in_db = db::count_facts_by_status(conn, "personal", "rejected").await?;
    let active_after = db::count_facts_by_status(conn, "personal", "active").await?;

    let (untargeted_total, untargeted_preserved, unexplained_lost_lines) =
        untargeted_preservation(base_document, &accepted.content, &operations);

    let judge_observation = match judge {
        Some(settings) if settings.enabled => Some(
            judge_consolidation(
                settings,
                &candidate_texts,
                base_document,
                &operations,
                &accepted.content,
                Duration::from_secs(STAGE_TIMEOUT_SECS),
            )
            .await,
        ),
        _ => None,
    };

    Ok(Some(ConsolidationOutcome {
        observation: ConsolidationObservation {
            base_memory_version: base_version,
            staged_suggestions: staged.len() as u32,
            candidate_facts: candidate_count as i64,
            candidate_fact_texts: candidate_texts,
            document_unchanged_after_staging,
            candidate_partition_valid,
            pending_anchored_to_base_version,
            operations,
            skipped_operations,
            reanchor_probe_exercised,
            reanchor_valid,
            accepted_memory_version: accepted.version,
            expected_memory_version: expected_version + 1,
            untargeted_lines_total: untargeted_total,
            untargeted_lines_preserved: untargeted_preserved,
            untargeted_preservation_valid: unexplained_lost_lines.is_empty(),
            unexplained_lost_lines,
            suggestions_accepted: accepted_count,
            linked_facts_consolidated: consolidated_in_db as u32,
            linked_facts_rejected: rejected_in_db as u32,
            facts_left_active: active_after,
            reject_probe: false,
            document_unchanged_after_reject: false,
            judge: judge_observation,
        },
        accepted_document: accepted.content,
        latency_ms,
    }))
}

async fn run_reject_probe(
    conn: &Connection,
    pending_before_ids: &std::collections::HashSet<String>,
    base_document: &str,
    base_version: i64,
) -> Result<Option<ConsolidationOutcome>> {
    let candidates = fetch_active_facts_by_type(conn, "personal").await?;
    if candidates.is_empty() {
        return Ok(None);
    }
    let staged: Vec<PersonalMemorySuggestionRecord> = fetch_pending_suggestions(conn, None)
        .await?
        .into_iter()
        .filter(|suggestion| !pending_before_ids.contains(&suggestion.id))
        .collect();
    if staged.is_empty() {
        return Ok(None);
    }
    let rejected = resolve_memory_suggestions(conn, None, None, "reject")
        .await
        .context("Reject-all suggestion resolution failed")?;
    let still_pending = fetch_pending_suggestions(conn, None).await?;
    let document_unchanged_after_reject =
        rejected.version == base_version && rejected.content.trim() == base_document;

    Ok(Some(ConsolidationOutcome {
        observation: ConsolidationObservation {
            base_memory_version: base_version,
            staged_suggestions: staged.len() as u32,
            candidate_facts: candidates.len() as i64,
            candidate_fact_texts: candidates.iter().map(|fact| fact.text.clone()).collect(),
            document_unchanged_after_staging: true,
            candidate_partition_valid: db::count_facts_by_status(conn, "personal", "active")
                .await?
                == 0,
            pending_anchored_to_base_version: true,
            operations: Vec::new(),
            skipped_operations: 0,
            reanchor_probe_exercised: false,
            reanchor_valid: None,
            accepted_memory_version: rejected.version,
            expected_memory_version: base_version,
            untargeted_lines_total: 0,
            untargeted_lines_preserved: 0,
            untargeted_preservation_valid: true,
            unexplained_lost_lines: Vec::new(),
            suggestions_accepted: still_pending.len() as u32,
            linked_facts_consolidated: 0,
            linked_facts_rejected: db::count_facts_by_status(conn, "personal", "rejected").await?
                as u32,
            facts_left_active: db::count_facts_by_status(conn, "personal", "active").await?,
            reject_probe: true,
            document_unchanged_after_reject,
            judge: None,
        },
        accepted_document: rejected.content,
        latency_ms: 0,
    }))
}
