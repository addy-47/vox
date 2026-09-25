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
use chrono::{Duration as ChronoDuration, NaiveDate};
use clap::Parser;
use common::{db, report, settings_cfg, turns::DatasetTurn};
use serde::Serialize;
use turso::Connection;
use vox_lib::{
    core::{
        defaults::{DEFAULT_LLM_MAX_OUTPUT_TOKENS, DEFAULT_SYSTEM_PROMPT_MODULAR},
        settings::VoxSettings,
    },
    persistence::{
        facts::fetch_active_facts_by_type, personal_memory::get_personal_memory,
        queue::has_unfinished_items, sessions::fetch_session_continuation, VoxDb,
    },
    services::{
        harness::{CompactionParams, CompactionStage, ContextBudgetStage, ContextStatus, Harness},
        llm::{actor::create_llm_provider_from_llm_settings, LlmCommand, LlmProvider},
        memory::{
            ingestion::run_ingestion_cycle, ml::embedder::unload_embedder,
            personal::consolidate_personal_memory,
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
const EXPECTED_CASES: &[(&str, usize, u32)] = &[
    ("case_01_under_threshold_035_turns", 35, 1),
    ("case_02_under_threshold_075_turns", 75, 1),
    ("case_03_under_threshold_090_turns", 90, 1),
    ("case_04_one_crossing_180_turns", 180, 1),
    ("case_05_one_crossing_200_turns", 200, 1),
    ("case_06_one_crossing_220_turns", 220, 1),
    ("case_07_two_crossings_350_turns", 350, 2),
    ("case_08_two_crossings_450_turns", 450, 2),
    ("case_09_three_crossings_500_turns", 500, 3),
    ("case_10_three_crossings_550_turns", 550, 3),
    ("case_11_three_crossings_600_turns", 600, 3),
    ("case_12_three_crossings_620_turns", 620, 3),
    ("case_13_three_crossings_650_turns", 650, 3),
    ("case_14_three_crossings_680_turns", 680, 3),
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
}

#[derive(Debug, Clone)]
struct EvalCase {
    name: String,
    ordinal: usize,
    expected_compactions: u32,
    manual_compaction: bool,
    turns: Vec<DatasetTurn>,
}

#[derive(Debug, Serialize)]
struct TurnTelemetry {
    turn: u32,
    tracked_tokens: usize,
    usable_budget: usize,
    utilization_percent: f32,
    budget_status: String,
    near_miss: bool,
    critical_eligible: bool,
    history_messages: usize,
    compaction_triggered: bool,
}

#[derive(Debug, Serialize)]
struct IngestionCycleReport {
    stage1_processed: usize,
    stage1_duplicates_deactivated: usize,
    stage1_errors: usize,
    stage2_processed: usize,
    stage2_inserted: usize,
    stage2_duplicates_deactivated: usize,
    stage2_errors: usize,
    latency_ms: u128,
}

#[derive(Debug, Serialize)]
struct CompactionObservation {
    from_turn: u32,
    to_turn: u32,
    trigger_kind: String,
    status: String,
}

#[derive(Debug, Serialize)]
struct CaseReport {
    case: String,
    expected_compactions: u32,
    actual_compactions: u32,
    manual_compactions: u32,
    classification_match: bool,
    session_id: i64,
    session_start_ms: i64,
    input_turns: usize,
    crossing_turns: Vec<u32>,
    critical_turns: Vec<u32>,
    budget_invariants_valid: bool,
    compaction_latencies_ms: Vec<u128>,
    compaction_ledger: Vec<CompactionObservation>,
    ledger_ranges_contiguous: bool,
    final_watermark_valid: bool,
    empty_context_compactions: u32,
    ingestion_cycles: u32,
    ingestion_items_processed: usize,
    stage1_items_processed: usize,
    stage2_items_processed: usize,
    ingestion_items_inserted: usize,
    queue_accounting_complete: bool,
    ingestion_accounting_valid: bool,
    ingestion_cycle_reports: Vec<IngestionCycleReport>,
    consolidation_latency_ms: u128,
    queue_before: i64,
    queue_after_compaction: i64,
    queue_after: i64,
    failed_queue_items: i64,
    facts_before: i64,
    facts_after: i64,
    personal_memory_version_before: i64,
    personal_memory_version_after: i64,
    personal_memory_chars_before: usize,
    personal_memory_chars_after: usize,
    personal_memory_injected: bool,
    prior_memory_prefix_preserved: bool,
    prior_memory_anchors: Vec<String>,
    prior_memory_full_document_preserved: bool,
    personal_facts_before_consolidation: i64,
    memory_consolidation_exercised: bool,
    turn_telemetry: Vec<TurnTelemetry>,
    status: String,
    error: Option<String>,
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
            let dataset_dir = resolve_path(args.dataset_dir.clone().unwrap_or_else(|| {
                PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("../../sandbox/datasets/eval-sessions")
            }));
            let db_path = resolve_path(args.db_path.clone());
            let run_id = report::new_run_id();
            let failure = failed_setup_report("Memory pipeline eval top-level timeout exceeded");
            write_final_report(
                "memory_pipeline",
                &run_id,
                &args,
                &db_path,
                &dataset_dir,
                &[failure],
                args.case.is_some(),
                false,
                started.elapsed().as_secs_f64(),
            )?;
            anyhow::bail!("Memory pipeline eval top-level timeout exceeded")
        }
    }
}

async fn run(args: Args) -> Result<()> {
    let started = Instant::now();
    let run_id = report::new_run_id();
    let eval_name = "memory_pipeline";
    let dataset_dir = resolve_path(args.dataset_dir.clone().unwrap_or_else(|| {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../sandbox/datasets/eval-sessions")
    }));
    let db_path = resolve_path(args.db_path.clone());
    let is_subset = args.case.is_some();
    let prepared = prepare_run(&args, &dataset_dir).await;
    let (selected_cases, is_subset, settings, provider, _db, conn) = match prepared {
        Ok(prepared) => prepared,
        Err(error) => {
            let failure = failed_setup_report(&format!("{error:#}"));
            write_final_report(
                eval_name,
                &run_id,
                &args,
                &db_path,
                &dataset_dir,
                &[failure],
                is_subset,
                false,
                started.elapsed().as_secs_f64(),
            )?;
            return Err(error);
        }
    };
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
        let report = run_case(case, &conn, provider.as_ref(), &settings).await;
        match report {
            Ok(report) => {
                println!(
                    "[case] {}: status={} compactions={}/{} ingestion_cycles={} queue_after={} elapsed={}ms",
                    report.case,
                    report.status,
                    report.actual_compactions,
                    report.expected_compactions,
                    report.ingestion_cycles,
                    report.queue_after,
                    case_started.elapsed().as_millis()
                );
                reports.push(report);
            }
            Err(error) => {
                let failure = failed_case_report(case, &format!("{error:#}"));
                println!(
                    "[case] {}: status=FAILED compactions={} elapsed={}ms error={error}",
                    failure.case,
                    failure.actual_compactions,
                    case_started.elapsed().as_millis()
                );
                reports.push(failure);
                unload_embedder();
                write_final_report(
                    eval_name,
                    &run_id,
                    &args,
                    &db_path,
                    &dataset_dir,
                    &reports,
                    is_subset,
                    false,
                    started.elapsed().as_secs_f64(),
                )?;
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
    write_final_report(
        eval_name,
        &run_id,
        &args,
        &db_path,
        &dataset_dir,
        &reports,
        is_subset,
        all_passed,
        started.elapsed().as_secs_f64(),
    )?;
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
        optional_api_key(&args.server_api_key),
        optional_provider(&args.server_provider),
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
    let (database, conn) = db::open_fresh_eval_db(&resolve_path(args.db_path.clone())).await?;
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
        let expected_compactions = if manual_compaction || name.contains("one_crossing") {
            1
        } else if name.contains("two_crossings") {
            2
        } else if name.contains("three_crossings") {
            3
        } else {
            anyhow::bail!("Unknown case class in filename: {name}");
        };
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
            expected_compactions,
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
    for (expected_name, expected_turns, expected_compactions) in EXPECTED_CASES {
        let case = cases
            .iter()
            .find(|case| case.name == *expected_name)
            .with_context(|| format!("Missing expected eval case {expected_name}"))?;
        anyhow::ensure!(
            case.turns.len() == *expected_turns,
            "Case {expected_name} has {} turns, expected {expected_turns}",
            case.turns.len()
        );
        anyhow::ensure!(
            case.expected_compactions == *expected_compactions,
            "Case {expected_name} has the wrong expected compaction count"
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
) -> Result<CaseReport> {
    let session_start_ms = case_start_ms(case.ordinal);
    let facts_before = count_facts(conn).await?;
    let personal_memory_before = get_personal_memory(conn, None).await?;
    let session_id =
        db::create_session_at_timestamp(conn, session_start_ms, Some("default")).await?;
    let continuation = fetch_session_continuation(conn, session_id).await?;
    let personal_memory = continuation.personal_memory;
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

    let queue_before = count_queue_items(conn).await?;
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

    let actual_compactions = count_case_compactions(conn, session_id).await?;
    anyhow::ensure!(
        actual_compactions as usize == crossing_turns.len() + manual_compactions as usize,
        "Compaction ledger count {actual_compactions} differs from critical plus manual count {}",
        crossing_turns.len() + manual_compactions as usize
    );
    let incomplete_compactions = count_incomplete_compactions(conn, session_id).await?;
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
    let queue_after_compaction = count_queue_items(conn).await?;
    let max_cycles = ((queue_after_compaction as usize / 16) + 2).clamp(1, MAX_INGESTION_CYCLES);
    let mut ingestion_cycle_reports = Vec::new();
    for cycle in 0..max_cycles {
        let before = count_queue_items(conn).await?;
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
        let after = count_queue_items(conn).await?;
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
    let queue_after = count_queue_items(conn).await?;
    let failed_queue_items = count_failed_queue_items(conn).await?;
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

    let active_personal_before = fetch_active_facts_by_type(conn, "personal").await?.len() as i64;
    let personal_before_consolidation = get_personal_memory(conn, None).await?;
    let prior_memory_document = personal_before_consolidation.content.trim().to_string();
    let prior_memory_anchors = memory_anchors(&prior_memory_document);
    let consolidation_started = Instant::now();
    tokio::time::timeout(
        Duration::from_secs(STAGE_TIMEOUT_SECS),
        consolidate_personal_memory(conn, provider, None, None, Some(&settings.llm), None),
    )
    .await
    .context("Production personal-memory consolidation timed out")?
    .context("Production personal-memory consolidation failed")?;
    let consolidation_latency_ms = consolidation_started.elapsed().as_millis();
    let personal_memory_after = get_personal_memory(conn, None).await?;
    let active_personal_after = fetch_active_facts_by_type(conn, "personal").await?.len() as i64;
    let memory_consolidation_exercised = active_personal_before > 0
        && personal_memory_after.version > personal_before_consolidation.version
        && active_personal_after == 0;
    if active_personal_before > 0 {
        anyhow::ensure!(
            personal_memory_after.version > personal_before_consolidation.version,
            "Personal memory version did not advance after consolidation"
        );
        anyhow::ensure!(
            active_personal_after == 0,
            "{active_personal_after} personal facts remain active after consolidation"
        );
    }

    let prior_memory_full_document_preserved = if prior_memory_document.is_empty() {
        true
    } else if active_personal_before == 0 {
        personal_memory_after
            .content
            .contains(&prior_memory_document)
    } else {
        prior_memory_anchors
            .iter()
            .all(|anchor| personal_memory_after.content.contains(anchor))
    };
    let prior_memory_prefix_preserved = prior_memory_full_document_preserved;
    let facts_after = count_facts(conn).await?;
    let actual_compactions_u32 =
        u32::try_from(actual_compactions).context("Compaction count does not fit in u32")?;
    let classification_match = actual_compactions_u32 == case.expected_compactions;
    Ok(CaseReport {
        case: case.name.clone(),
        expected_compactions: case.expected_compactions,
        actual_compactions: actual_compactions_u32,
        manual_compactions,
        classification_match,
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
        personal_memory_version_after: personal_memory_after.version,
        personal_memory_chars_before: personal_memory_before.content.len(),
        personal_memory_chars_after: personal_memory_after.content.len(),
        personal_memory_injected,
        prior_memory_prefix_preserved,
        prior_memory_anchors,
        prior_memory_full_document_preserved,
        personal_facts_before_consolidation: active_personal_before,
        memory_consolidation_exercised,
        turn_telemetry,
        status: if classification_match
            && budget_invariants_valid
            && ledger_ranges_contiguous
            && final_watermark_valid
            && queue_accounting_complete
            && ingestion_accounting_valid
            && prior_memory_prefix_preserved
            && personal_memory_injected
        {
            "passed".to_string()
        } else {
            "failed_classification".to_string()
        },
        error: None,
    })
}

#[allow(clippy::too_many_arguments)]
fn write_final_report(
    eval_name: &str,
    run_id: &str,
    args: &Args,
    db_path: &Path,
    dataset_dir: &Path,
    reports: &[CaseReport],
    is_subset: bool,
    run_passed: bool,
    total_latency_s: f64,
) -> Result<()> {
    let payload = serde_json::json!({
        "eval": eval_name,
        "status": if reports
            .iter()
            .any(|report| report.status == "failed_setup")
        {
            "failed_setup"
        } else if is_subset {
            if run_passed && reports.iter().all(|report| report.status == "passed") {
                "passed_subset"
            } else {
                "failed_subset"
            }
        } else if run_passed && reports.iter().all(|report| report.status == "passed") {
            "passed"
        } else {
            "failed"
        },
        "complete_matrix": !is_subset,
        "run_passed": run_passed,
        "case_count": reports.len(),
        "memory_pipeline_exercised": reports
            .iter()
            .any(|report| report.memory_consolidation_exercised),
        "ingestion_items_inserted": reports
            .iter()
            .map(|report| report.ingestion_items_inserted)
            .sum::<usize>(),
        "dataset_dir": dataset_dir,
        "server": {
            "url": args.server_url,
            "model": args.server_model,
            "provider": args.server_provider,
        },
        "database": db_path,
        "context_window": args.context_window,
        "max_output_tokens": DEFAULT_LLM_MAX_OUTPUT_TOKENS,
        "auto_compaction": true,
        "retrieval_enabled": false,
        "tools_enabled": false,
        "judge_used": false,
        "cases": reports,
        "total_latency_s": total_latency_s,
    });
    report::write_report(eval_name, run_id, payload)?;
    Ok(())
}

fn memory_anchors(content: &str) -> Vec<String> {
    content
        .lines()
        .map(str::trim)
        .filter(|line| line.len() >= 8)
        .take(5)
        .map(ToOwned::to_owned)
        .collect()
}

fn failed_setup_report(error: &str) -> CaseReport {
    CaseReport {
        case: "<setup>".to_string(),
        expected_compactions: 0,
        actual_compactions: 0,
        manual_compactions: 0,
        classification_match: false,
        session_id: 0,
        session_start_ms: 0,
        input_turns: 0,
        crossing_turns: Vec::new(),
        critical_turns: Vec::new(),
        budget_invariants_valid: false,
        compaction_latencies_ms: Vec::new(),
        compaction_ledger: Vec::new(),
        ledger_ranges_contiguous: false,
        final_watermark_valid: false,
        empty_context_compactions: 0,
        ingestion_cycles: 0,
        ingestion_items_processed: 0,
        stage1_items_processed: 0,
        stage2_items_processed: 0,
        ingestion_items_inserted: 0,
        queue_accounting_complete: false,
        ingestion_accounting_valid: false,
        ingestion_cycle_reports: Vec::new(),
        consolidation_latency_ms: 0,
        queue_before: 0,
        queue_after_compaction: 0,
        queue_after: 0,
        failed_queue_items: 0,
        facts_before: 0,
        facts_after: 0,
        personal_memory_version_before: 0,
        personal_memory_version_after: 0,
        personal_memory_chars_before: 0,
        personal_memory_chars_after: 0,
        personal_memory_injected: false,
        prior_memory_prefix_preserved: false,
        prior_memory_anchors: Vec::new(),
        prior_memory_full_document_preserved: false,
        personal_facts_before_consolidation: 0,
        memory_consolidation_exercised: false,
        turn_telemetry: Vec::new(),
        status: "failed_setup".to_string(),
        error: Some(error.to_string()),
    }
}

fn failed_case_report(case: &EvalCase, error: &str) -> CaseReport {
    CaseReport {
        case: case.name.clone(),
        expected_compactions: case.expected_compactions,
        actual_compactions: 0,
        manual_compactions: 0,
        classification_match: false,
        session_id: 0,
        session_start_ms: 0,
        input_turns: case.turns.len(),
        crossing_turns: Vec::new(),
        critical_turns: Vec::new(),
        budget_invariants_valid: false,
        compaction_latencies_ms: Vec::new(),
        compaction_ledger: Vec::new(),
        ledger_ranges_contiguous: false,
        final_watermark_valid: false,
        empty_context_compactions: 0,
        ingestion_cycles: 0,
        ingestion_items_processed: 0,
        stage1_items_processed: 0,
        stage2_items_processed: 0,
        ingestion_items_inserted: 0,
        queue_accounting_complete: false,
        ingestion_accounting_valid: false,
        ingestion_cycle_reports: Vec::new(),
        consolidation_latency_ms: 0,
        queue_before: 0,
        queue_after_compaction: 0,
        queue_after: 0,
        failed_queue_items: 0,
        facts_before: 0,
        facts_after: 0,
        personal_memory_version_before: 0,
        personal_memory_version_after: 0,
        personal_memory_chars_before: 0,
        personal_memory_chars_after: 0,
        personal_memory_injected: false,
        prior_memory_prefix_preserved: false,
        prior_memory_anchors: Vec::new(),
        prior_memory_full_document_preserved: false,
        personal_facts_before_consolidation: 0,
        memory_consolidation_exercised: false,
        turn_telemetry: Vec::new(),
        status: "failed".to_string(),
        error: Some(error.to_string()),
    }
}

fn resolve_path(path: PathBuf) -> PathBuf {
    if path.is_absolute() {
        path
    } else {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(path)
    }
}

fn optional_api_key(value: &str) -> Option<String> {
    let value = value.trim();
    if value == "env:OPENROUTER_API_KEY" {
        return std::env::var("OPENROUTER_API_KEY")
            .ok()
            .filter(|key| !key.trim().is_empty());
    }
    (!value.is_empty()).then(|| value.to_string())
}

fn optional_provider(value: &str) -> Option<String> {
    let value = value.trim();
    (!value.is_empty()).then(|| value.to_string())
}

fn case_start_ms(case_index: usize) -> i64 {
    let day = NaiveDate::from_ymd_opt(2026, 1, 2)
        .expect("valid eval date")
        .checked_add_signed(ChronoDuration::days(case_index as i64))
        .expect("valid eval date range");
    let hour = 7 + (case_index as u32 * 5) % 12;
    day.and_hms_opt(hour, 0, 0)
        .expect("valid eval time")
        .and_utc()
        .timestamp_millis()
}

async fn fetch_compaction_observations(
    conn: &Connection,
    session_id: i64,
) -> Result<Vec<CompactionObservation>> {
    let mut rows = conn
        .query(
            "SELECT from_turn_id, to_turn_id, trigger_kind, status FROM session_compactions WHERE session_id = ? ORDER BY id ASC;",
            (session_id,),
        )
        .await?;
    let mut observations = Vec::new();
    while let Some(row) = rows.next().await? {
        observations.push(CompactionObservation {
            from_turn: row.get::<i64>(0)? as u32,
            to_turn: row.get::<i64>(1)? as u32,
            trigger_kind: row.get(2)?,
            status: row.get(3)?,
        });
    }
    Ok(observations)
}

fn validate_compaction_ranges(observations: &[CompactionObservation]) -> bool {
    let mut previous_to = 0;
    for observation in observations {
        if !matches!(observation.trigger_kind.as_str(), "critical" | "manual")
            || observation.status != "completed"
            || observation.from_turn != previous_to + 1
        {
            return false;
        }
        previous_to = observation.to_turn;
    }
    true
}

async fn count_facts(conn: &Connection) -> Result<i64> {
    scalar_i64(conn, "SELECT COUNT(*) FROM memory_facts;").await
}

async fn count_queue_items(conn: &Connection) -> Result<i64> {
    scalar_i64(
        conn,
        "SELECT COUNT(*) FROM memory_ingestion_queue WHERE status NOT IN ('completed', 'failed');",
    )
    .await
}

async fn count_failed_queue_items(conn: &Connection) -> Result<i64> {
    scalar_i64(
        conn,
        "SELECT COUNT(*) FROM memory_ingestion_queue WHERE status = 'failed';",
    )
    .await
}

async fn count_case_compactions(conn: &Connection, session_id: i64) -> Result<i64> {
    let mut rows = conn
        .query(
            "SELECT COUNT(*) FROM session_compactions WHERE session_id = ? AND status = 'completed';",
            (session_id,),
        )
        .await?;
    let row = rows.next().await?.context("Scalar query returned no row")?;
    Ok(row.get(0)?)
}

async fn count_incomplete_compactions(conn: &Connection, session_id: i64) -> Result<i64> {
    let mut rows = conn
        .query(
            "SELECT COUNT(*) FROM session_compactions WHERE session_id = ? AND status != 'completed';",
            (session_id,),
        )
        .await?;
    let row = rows.next().await?.context("Scalar query returned no row")?;
    Ok(row.get(0)?)
}

async fn scalar_i64(conn: &Connection, sql: &str) -> Result<i64> {
    let mut rows = conn.query(sql, ()).await?;
    let row = rows.next().await?.context("Scalar query returned no row")?;
    Ok(row.get(0)?)
}
