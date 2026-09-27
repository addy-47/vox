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
        ConsolidationOutcome, EngineReplayObservation, FinalReportConfig, IngestionCycleReport,
        ReanchorArithmeticObservation, RejectedCandidate, StructureDefectRecord,
        StructureObservation, SuggestionObservation, TurnTelemetry,
    },
    preservation::untargeted_preservation,
    report, settings_cfg,
    structure::{
        account_operations, classify_fact_coverage, heading_count_preserved, measure_document,
        verify_reanchor_arithmetic, verify_section_membership,
    },
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
            personal::{
            apply_patch_operations, consolidate_personal_memory, resolve_memory_suggestions,
            MemoryPatchOperation,
        },
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
    /// Run only the first N cases of the ladder on a fresh shared DB. Unlike `--case`,
    /// which selects a single case in isolation, this preserves the cumulative document
    /// evolution the incremental index-addressed path depends on: case 1 is the only
    /// cold-start (Prompt 1) case, and every later case is an incremental (Prompt 2)
    /// cycle on the document the earlier cases accumulated.
    #[arg(long)]
    max_cases: Option<u32>,
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
    let is_subset = args.case.is_some() || args.max_cases.is_some();
    // A prefix run of two or more cases still has to demonstrate the incremental
    // index-addressed path, because that is the only thing the new patch engine does.
    // A single-case run in isolation, and a one-case prefix, cannot: both are cold-start only.
    let requires_incremental_coverage =
        args.case.is_none() && args.max_cases.unwrap_or(u32::MAX) >= 2;
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
                            " path={} staged={} dropped_ops={} clamped={}/chain={} absent_facts={} partial_facts={} hdg_ok={} unexplained_lost={} reanchor={:?}(meaningful={},shifted={}) judge={} | struct: ok={} hdg={} bul={} dup_hdg={} dup_bul={} above_hdg={} | engine: replay_applicable={} replay_match={} | sect: viol={} | gate_rejected={} blocked_ops={}",
                            observation.consolidation_path,
                            observation.staged_suggestions,
                            observation.dropped_operations,
                            observation.clamped_operations,
                            observation.engine_replay.chained_section_anchors,
                            observation.absent_candidate_facts.len(),
                            observation.partial_candidate_facts.len(),
                            observation.heading_count_valid,
                            observation.unexplained_lost_lines.len(),
                            observation.reanchor_valid,
                            observation.reanchor_arithmetic.probe_meaningful,
                            observation.reanchor_arithmetic.shifted.len(),
                            observation
                                .judge
                                .as_ref()
                                .map(|judge| format!("{}:{}", judge.model, judge.verdict))
                                .unwrap_or_else(|| "skipped".to_string()),
                            observation.accepted_structure.contract_satisfied,
                            observation.accepted_structure.headings_total,
                            observation.accepted_structure.bullets_total,
                            observation.accepted_structure.duplicate_headings.len(),
                            observation.accepted_structure.duplicate_bullets.len(),
                            observation.accepted_structure.bullets_above_first_heading,
                            observation.engine_replay.replay_applicable,
                            observation.engine_replay.matches_committed_document,
                            observation.section_membership_violations.len(),
                            observation.structure_gate_rejected,
                            observation
                                .rejected_candidate
                                .as_ref()
                                .map_or(0, |rejected| rejected.blocked_operations),
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
    let all_passed = cases_passed && (!requires_incremental_coverage || memory_pipeline_exercised);
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
    let is_subset = args.case.is_some() || args.max_cases.is_some();
    let selected_cases = select_cases(cases, args.case, args.max_cases)?;
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

fn select_cases(
    cases: Vec<EvalCase>,
    case: Option<u32>,
    max_cases: Option<u32>,
) -> Result<Vec<EvalCase>> {
    if let Some(case) = case {
        let selected = cases
            .into_iter()
            .find(|candidate| candidate.ordinal == case as usize)
            .with_context(|| format!("Case {case} is out of range"))?;
        return Ok(vec![selected]);
    }
    let Some(max_cases) = max_cases else {
        return Ok(cases);
    };
    anyhow::ensure!(max_cases > 0, "--max-cases must be at least 1");
    anyhow::ensure!(
        max_cases as usize <= cases.len(),
        "--max-cases {max_cases} exceeds the {} cases in the ladder",
        cases.len()
    );
    // Keep the ladder prefix in order: the personal-memory document accumulates across
    // cases, so a prefix must start at case 1 to reach the cold-start path first.
    let mut prefix: Vec<EvalCase> = cases.into_iter().take(max_cases as usize).collect();
    anyhow::ensure!(
        prefix.first().is_some_and(|first| first.ordinal == 1),
        "--max-cases prefix must begin at case 1"
    );
    prefix.shrink_to_fit();
    Ok(prefix)
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

    // Mechanical gate. `structure_valid` is the accumulation-side counterpart to the
    // loss-only preservation check: without it this gate cannot fail for the append-only
    // degeneration that the previous two consolidation approaches both produced, and a
    // visibly degrading document would read as `passed`.
    let structure_valid = consolidation.as_ref().is_none_or(|outcome| {
        let observation = &outcome.observation;
        observation.accepted_structure.contract_satisfied
            && !observation.structure_gate_rejected
            && (!observation.engine_replay.replay_applicable
                || observation.engine_replay.matches_committed_document)
    });

    let patch_fidelity_valid = consolidation.as_ref().is_none_or(|outcome| {
        let observation = &outcome.observation;
        observation.dropped_operations == 0
            && observation.reanchor_arithmetic.version_mismatch.is_empty()
            && observation.section_membership_violations.is_empty()
    });

    let consolidation_valid = consolidation.as_ref().is_none_or(|outcome| {
        let observation = &outcome.observation;
        // Staging immutability is a Prompt 2/3 invariant. Prompt 1 commits the synthesized
        // document directly, so on the cold-start path the document is *expected* to change;
        // requiring it to be untouched there would assert the opposite of the spec.
        let staging_immutable = if observation.consolidation_path == "cold_start" {
            true
        } else {
            observation.document_unchanged_after_staging
        };
        staging_immutable
            && observation.candidate_partition_valid
            && observation.pending_anchored_to_base_version
            && observation.untargeted_preservation_valid
            && observation.unexplained_lost_lines.is_empty()
            && observation.dropped_operations == 0
            && observation.facts_left_active == 0
            && observation.reanchor_valid != Some(false)
            && observation.reanchor_arithmetic.arithmetic_valid
            && observation.section_membership_violations.is_empty()
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
            && structure_valid
            && patch_fidelity_valid
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

    // The two consolidation paths carry different invariants. Naming the path keeps the
    // path-specific assertions from being scored against the wrong contract.
    let consolidation_path = if base_document.trim().is_empty() {
        "cold_start"
    } else if staged.is_empty() {
        "staged_no_edits"
    } else {
        "incremental"
    };

    // Real-engine accounting replaces the previous stub, which only tested for non-empty
    // content and therefore could not fail for an out-of-range index.
    let op_tuples: Vec<(String, u32, String)> = staged
        .iter()
        .map(|suggestion| {
            (
                suggestion.op.clone(),
                suggestion.target_index,
                suggestion.content.clone(),
            )
        })
        .collect();
    let mut engine = account_operations(base_document, &op_tuples);

    let pending_anchored_to_base_version = staged
        .iter()
        .all(|suggestion| suggestion.base_memory_version == base_version);

    let active_left = db::count_facts_by_status(conn, "personal", "active").await?;
    let candidate_partition_valid = active_left == 0;

    let mut reanchor_probe_exercised = false;
    let mut reanchor_valid: Option<bool> = None;
    let mut reanchor_arithmetic = ReanchorArithmeticObservation {
        probe_exercised: false,
        probe_meaningful: false,
        resolved_op: String::new(),
        resolved_index: 0,
        remaining_pending_count: 0,
        shifted: Vec::new(),
        missed_shifts: Vec::new(),
        spurious_shifts: Vec::new(),
        version_mismatch: Vec::new(),
        arithmetic_valid: true,
    };
    let mut structure_gate_rejected = false;
    let mut accepted = after_stage.clone();
    let mut expected_version = base_version;

    if staged.len() >= 2 {
        reanchor_probe_exercised = true;
        // Pick the probe target so the §5.3-B arithmetic is actually exercised. A `replace`
        // shifts no index by definition, and an `insert_after`/`delete` anchored at or past
        // the highest remaining index also shifts nothing — so accepting `staged[0]` blindly
        // yields a probe that cannot fail. Prefer a shift-triggering op whose index sits
        // strictly below another staged suggestion's index.
        let max_index = staged
            .iter()
            .map(|suggestion| suggestion.target_index)
            .max()
            .unwrap_or(0);
        let probe_position = staged
            .iter()
            .position(|suggestion| {
                let op = suggestion.op.as_str();
                (op == "insert_after" || op == "delete") && suggestion.target_index < max_index
            })
            .unwrap_or(0);
        let resolved = staged[probe_position].clone();
        // Sample the remaining rows *before* the accept so the arithmetic can be
        // recomputed independently rather than inferred from the base_version bump alone.
        let remaining_before: Vec<(String, String, u32)> = staged
            .iter()
            .enumerate()
            .filter(|(position, _)| *position != probe_position)
            .map(|(_, suggestion)| {
                (
                    suggestion.id.clone(),
                    suggestion.op.clone(),
                    suggestion.target_index,
                )
            })
            .collect();

        match resolve_memory_suggestions(conn, None, Some(&resolved.id), "accept").await {
            Ok(first) => {
                let remaining = fetch_pending_suggestions(conn, None).await?;
                let remaining_after: Vec<(String, String, u32)> = remaining
                    .iter()
                    .map(|suggestion| {
                        (
                            suggestion.id.clone(),
                            suggestion.op.clone(),
                            suggestion.target_index,
                        )
                    })
                    .collect();
                let arithmetic = verify_reanchor_arithmetic(
                    &resolved.op,
                    resolved.target_index,
                    &remaining_before,
                    &remaining_after,
                );
                // Non-vacuous only when the shift rule was actually supposed to fire: the
                // resolved op must be shift-triggering and some remaining row must sit above
                // the accepted index. A vacuous probe is reported as unverified, never as proof.
                let probe_meaningful = (resolved.op == "insert_after" || resolved.op == "delete")
                    && remaining_before
                        .iter()
                        .any(|(_, _, index)| *index > resolved.target_index);
                if !probe_meaningful {
                    println!(
                        "[consolidation] re-anchor probe was VACUOUS (op={} index={} remaining={}): no shift was due, so §5.3-B arithmetic is UNVERIFIED this cycle",
                        resolved.op,
                        resolved.target_index,
                        remaining_before.len()
                    );
                }
                reanchor_arithmetic = ReanchorArithmeticObservation {
                    probe_exercised: true,
                    probe_meaningful,
                    resolved_op: resolved.op.clone(),
                    resolved_index: resolved.target_index,
                    remaining_pending_count: remaining.len(),
                    shifted: arithmetic.shifted,
                    missed_shifts: arithmetic.missed_shifts,
                    spurious_shifts: arithmetic.spurious_shifts,
                    version_mismatch: remaining
                        .iter()
                        .filter(|suggestion| suggestion.base_memory_version != first.version)
                        .map(|suggestion| suggestion.id.clone())
                        .collect(),
                    arithmetic_valid: arithmetic.arithmetic_valid,
                };
                reanchor_valid = Some(
                    reanchor_arithmetic.version_mismatch.is_empty()
                        && reanchor_arithmetic.arithmetic_valid
                        && remaining.len() == staged.len() - 1,
                );
                expected_version = first.version;
                accepted = first;
            }
            Err(error) if is_structure_gate(&error) => {
                // The gate fired: the patched candidate violated the §5.1 contract and
                // nothing was committed. This is a production-correct protection and a
                // model-output defect at the same time. Do not abort the ladder — record it.
                println!(
                    "[consolidation] structure gate rejected the case-{} accept: {error}",
                    base_version
                );
                structure_gate_rejected = true;
                reanchor_probe_exercised = false;
            }
            Err(error) => {
                return Err(error).context("Single-suggestion accept failed");
            }
        }
    }

    if !structure_gate_rejected {
        // The §5.1 structure gate and the heading-loss gate both refuse to commit rather than
        // aborting the cycle, so the batch accept can legitimately fail. That is production
        // behaving correctly, not an eval error: record what was blocked and keep the ladder
        // running, exactly as on the probe path above.
        match resolve_memory_suggestions(conn, None, None, "accept").await {
            Ok(record) => {
                accepted = record;
            }
            Err(error) if is_structure_gate(&error) => {
                println!(
                    "[consolidation] structure gate rejected the case-{} batch accept: {error}",
                    base_version
                );
                structure_gate_rejected = true;
            }
            Err(error) => {
                return Err(error).context("Accept-all suggestion resolution failed");
            }
        }
    }
    let latency_ms = started.elapsed().as_millis();
    anyhow::ensure!(
        structure_gate_rejected || accepted.version == expected_version + 1,
        "Accept-all produced version {} but {} was expected",
        accepted.version,
        expected_version + 1
    );

    // The gate must have left the active version and every suggestion row untouched.
    let gate_protected_state = if structure_gate_rejected {
        let still_pending = fetch_pending_suggestions(conn, None).await?;
        let current = get_personal_memory(conn, None).await?;
        still_pending.len() == staged.len()
            && current.version == base_version
            && current.content.trim() == base_document.trim()
    } else {
        true
    };

    let still_pending = fetch_pending_suggestions(conn, None).await?;
    let accepted_count = still_pending.len() as u32;
    let consolidated_in_db = db::count_facts_by_status(conn, "personal", "consolidated").await?;
    let rejected_in_db = db::count_facts_by_status(conn, "personal", "rejected").await?;
    let active_after = db::count_facts_by_status(conn, "personal", "active").await?;

    // Section membership is resolved before the operation records are built, because each
    // record carries its landing section alongside what the engine did with its index.
    let section_violations =
        verify_section_membership(base_document, &op_tuples, &accepted.content);
    let operations: Vec<SuggestionObservation> = staged
        .iter()
        .enumerate()
        .map(|(position, suggestion)| {
            let trace = engine.per_operation.get(position);
            let membership = section_violations
                .iter()
                .find(|violation| violation.content.trim() == suggestion.content.trim());
            SuggestionObservation {
                op: suggestion.op.clone(),
                target_index: suggestion.target_index,
                content: suggestion.content.clone(),
                engine_would_match: trace
                    .is_some_and(|trace| !trace.engine_action.starts_with("dropped")),
                element_count_at_stage: trace.map_or(0, |trace| trace.element_count_at_stage),
                index_in_bounds: trace.is_some_and(|trace| trace.index_in_bounds),
                engine_action: trace
                    .map_or("unaccounted", |trace| trace.engine_action)
                    .to_string(),
                landed_under_section: membership.and_then(|v| v.landed_under.clone()),
                intended_under_section: membership.and_then(|v| v.intended_under.clone()),
            }
        })
        .collect();
    let dropped_operations = operations
        .iter()
        .filter(|operation| !operation.engine_would_match)
        .count() as u32;
    let clamped_operations = engine.clamped_to_append;
    // INVARIANT 5.3-A retires every candidate fact when suggestions are staged, whether or not
    // the model proposed an edit for it. A fact it silently declined to write down is then gone
    // for good, and nothing else in this harness can see that.
    let fact_coverage = classify_fact_coverage(&candidate_texts, &accepted.content);
    let absent_candidate_facts: Vec<(String, f64)> = fact_coverage
        .absent
        .iter()
        .map(|fact| (fact.text.clone(), fact.coverage))
        .collect();
    let partial_candidate_facts: Vec<(String, f64)> = fact_coverage
        .partial
        .iter()
        .chain(fact_coverage.represented.iter())
        .map(|fact| (fact.text.clone(), fact.coverage))
        .collect();
    // A `replace` aimed at a heading index silently deletes that section.
    let (heading_count_valid, base_headings, accepted_headings, minimum_allowed_headings) =
        heading_count_preserved(base_document, &op_tuples, &accepted.content);
    if !heading_count_valid {
        println!(
            "[consolidation] SECTION LOSS: base had {base_headings} heading(s), accepted has {accepted_headings}, minimum allowed {minimum_allowed_headings} (a 'replace' on a heading index erases its section)"
        );
    }
    let section_membership_violations: Vec<SuggestionObservation> = section_violations
        .iter()
        .filter_map(|violation| {
            operations
                .iter()
                .find(|operation| operation.content.trim() == violation.content.trim())
                .cloned()
        })
        .collect();

    // When a gate refused the commit, reconstruct locally what it prevented. Without this the
    // report says only that something was blocked; with it, the blocked damage is visible.
    let mut rejected_candidate: Option<RejectedCandidate> = None;
    if structure_gate_rejected {
        let blocked = fetch_pending_suggestions(conn, None).await?;
        let blocked_ops: Vec<MemoryPatchOperation> = blocked
            .iter()
            .map(|suggestion| MemoryPatchOperation {
                op: suggestion.op.clone(),
                index: suggestion.target_index,
                text: suggestion.content.clone(),
            })
            .collect();
        if let Ok(candidate) = apply_patch_operations(&accepted.content, &blocked_ops) {
            let candidate = candidate.document;
            let measured = measure_document(&candidate);
            let blocked_tuples: Vec<(String, u32, String)> = blocked_ops
                .iter()
                .map(|op| (op.op.clone(), op.index, op.text.clone()))
                .collect();
            let (headings_valid, base_h, accepted_h, min_h) =
                heading_count_preserved(&accepted.content, &blocked_tuples, &candidate);
            println!(
                "[consolidation] gate blocked {} op(s): candidate would have had {} heading(s) vs {} in the active document (min allowed {min_h}, structure_ok={}, headings_valid={headings_valid})",
                blocked_ops.len(),
                accepted_h,
                base_h,
                measured.contract_satisfied(),
            );
            rejected_candidate = Some(RejectedCandidate {
                blocked_operations: blocked_ops.len() as u32,
                candidate_headings: accepted_h,
                active_headings: base_h,
                minimum_allowed_headings: min_h,
                headings_valid,
                candidate_structure_ok: measured.contract_satisfied(),
                candidate_nameless_headings: measured.nameless_headings.clone(),
                candidate_duplicate_headings: measured.duplicate_headings.clone(),
            });
        }
    }

    let (untargeted_total, untargeted_preserved, unexplained_lost_lines) =
        untargeted_preservation(base_document, &accepted.content, &operations);

    // Accumulation-side structure measurement. The preservation check above is loss-only
    // and is structurally blind to an append-only bullet dump; these checks are not.
    let base_structure = measure_document(base_document);
    let accepted_structure = measure_document(&accepted.content);

    // Independent replay: the engine's own output must equal what production committed,
    // otherwise every per-operation reading above is untrustworthy. On the cold-start path
    // there is no operation to replay — Prompt 1 synthesizes the document whole — so the
    // comparison is recorded as not applicable instead of being scored as a mismatch.
    let replay_applicable = engine.operations_total > 0;
    engine.engine_replay_matches_committed =
        !structure_gate_rejected && engine.engine_replay_document.trim() == accepted.content.trim();
    let engine_replay = EngineReplayObservation {
        operations_total: engine.operations_total,
        dropped_by_engine: engine.dropped_by_engine,
        clamped_to_append: engine.clamped_to_append,
        chained_section_anchors: engine.chained_section_anchors,
        replay_applicable,
        matches_committed_document: engine.engine_replay_matches_committed,
        replay_chars: engine.engine_replay_document.len(),
        committed_chars: accepted.content.len(),
        replay_headings: engine
            .engine_replay_document
            .lines()
            .filter(|line| line.trim_start().starts_with('#'))
            .map(|line| line.trim().to_string())
            .collect(),
    };

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
            consolidation_path: consolidation_path.to_string(),
            staged_suggestions: staged.len() as u32,
            candidate_facts: candidate_count as i64,
            candidate_fact_texts: candidate_texts,
            document_unchanged_after_staging,
            candidate_partition_valid,
            pending_anchored_to_base_version,
            operations,
            dropped_operations,
            clamped_operations,
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
            absent_candidate_facts,
            partial_candidate_facts,
            heading_count_valid,
            base_headings,
            accepted_headings,
            minimum_allowed_headings,
            reject_probe: false,
            document_unchanged_after_reject: false,
            accepted_structure: to_structure_observation(&accepted_structure, &base_structure),
            base_structure: to_structure_observation(&base_structure, &base_structure),
            engine_replay,
            reanchor_arithmetic,
            section_membership_violations,
            structure_gate_rejected: structure_gate_rejected && !gate_protected_state,
            rejected_candidate,
            judge: judge_observation,
        },
        accepted_document: accepted.content,
        latency_ms,
    }))
}

/// True when the error is the §5.3 accept-path structure gate refusing a candidate document.
fn is_structure_gate(error: &vox_lib::services::memory::MemorySuggestionError) -> bool {
    matches!(
        error,
        vox_lib::services::memory::MemorySuggestionError::StructureGate(_)
    )
}

fn to_structure_observation(
    measured: &common::structure::StructureReport,
    base: &common::structure::StructureReport,
) -> StructureObservation {
    StructureObservation {
        bullets_total: measured.bullets_total,
        headings_total: measured.headings_total,
        base_bullets_total: base.bullets_total,
        base_headings_total: base.headings_total,
        nameless_headings: measured.nameless_headings.clone(),
        duplicate_headings: measured.duplicate_headings.clone(),
        duplicate_bullets: measured.duplicate_bullets.clone(),
        has_any_heading: measured.has_any_heading,
        is_empty: measured.is_empty,
        bullets_above_first_heading: measured.bullets_above_first_heading,
        bullets_per_section: measured.bullets_per_section.clone(),
        contract_satisfied: measured.contract_satisfied(),
        defects: measured
            .defects
            .iter()
            .map(|defect| StructureDefectRecord {
                kind: defect.kind.to_string(),
                detail: defect.detail.clone(),
            })
            .collect(),
    }
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
            consolidation_path: "reject_probe".to_string(),
            staged_suggestions: staged.len() as u32,
            candidate_facts: candidates.len() as i64,
            candidate_fact_texts: candidates.iter().map(|fact| fact.text.clone()).collect(),
            document_unchanged_after_staging: true,
            candidate_partition_valid: db::count_facts_by_status(conn, "personal", "active")
                .await?
                == 0,
            pending_anchored_to_base_version: true,
            operations: Vec::new(),
            dropped_operations: 0,
            clamped_operations: 0,
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
            absent_candidate_facts: Vec::new(),
            partial_candidate_facts: Vec::new(),
            heading_count_valid: true,
            base_headings: 0,
            accepted_headings: 0,
            minimum_allowed_headings: 0,
            reject_probe: true,
            document_unchanged_after_reject,
            accepted_structure: to_structure_observation(
                &measure_document(&rejected.content),
                &measure_document(base_document),
            ),
            base_structure: to_structure_observation(
                &measure_document(base_document),
                &measure_document(base_document),
            ),
            engine_replay: EngineReplayObservation {
                operations_total: 0,
                dropped_by_engine: 0,
                clamped_to_append: 0,
                chained_section_anchors: 0,
                // No operation was applied, so the committed document must equal the base.
                replay_applicable: false,
                matches_committed_document: true,
                replay_chars: base_document.len(),
                committed_chars: rejected.content.len(),
                replay_headings: Vec::new(),
            },
            reanchor_arithmetic: ReanchorArithmeticObservation {
                probe_exercised: false,
                probe_meaningful: false,
                resolved_op: String::new(),
                resolved_index: 0,
                remaining_pending_count: 0,
                shifted: Vec::new(),
                missed_shifts: Vec::new(),
                spurious_shifts: Vec::new(),
                version_mismatch: Vec::new(),
                arithmetic_valid: true,
            },
            section_membership_violations: Vec::new(),
            structure_gate_rejected: false,
            rejected_candidate: None,
            judge: None,
        },
        accepted_document: rejected.content,
        latency_ms: 0,
    }))
}
