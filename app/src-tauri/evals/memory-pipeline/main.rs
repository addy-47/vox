//! ============================================================================
//! evals/memory-pipeline/main.rs — Vox Memory Pipeline Evaluation Harness
//! ============================================================================
//! Category     : Evaluation Suite
//! Component    : services/memory (Compaction, Ingestion, Consolidation)
//! Execution    : cargo bench --bench memory_pipeline_eval --release -- [FLAGS]
//! Metrics      : Compaction Coverage, Ingestion Dedup Precision/Recall, Consolidation Semantic Loss
//! Output       : evals/results/memory-pipeline/<run_id>/
//!                  manifest.json, cases/<case>/, eval_vox.db
//! Post-run     : cargo bench --bench memory_pipeline_eval -- --audit <run_id>
//! ============================================================================

#[path = "../common/mod.rs"]
mod common;

mod compaction;
mod consolidation;
mod ingestion;

use std::path::PathBuf;

use anyhow::{anyhow, Result};
use clap::Parser;
use common::{
    datasets::load_eval_case,
    db::EvalDbGuard,
    keys::resolve_api_key,
    llm_client::{create_pipeline_provider, NvidiaJudgeClient, RecordingLlmProvider},
    reporting::{
        create_case_directory, create_run_directory, generate_run_id, markdown_table,
        resolve_output_dir, write_summary_markdown,
    },
    stage_dump::{build_run_manifest, write_json},
};
use compaction::{evaluate_compaction_stage, run_compaction_and_persist};
use consolidation::evaluate_consolidation_stage;
use ingestion::evaluate_ingestion_stage;
use vox_lib::services::memory::ingestion::run_ingestion_cycle;

#[derive(Parser, Debug)]
#[command(
    name = "memory_eval",
    about = "Vox Memory Pipeline Comprehensive Evaluation Harness"
)]
struct CliArgs {
    /// Number of sequential cases to evaluate from sandbox/datasets/eval-sessions/ (1..14)
    #[arg(long, default_value_t = 1)]
    cases: usize,

    /// Target pipeline layer: 'full', 'compaction', 'ingestion', 'consolidation'
    #[arg(long, default_value = "full")]
    layer: String,

    /// Pipeline execution LLM server endpoint URL (Ollama or OpenAI-compatible)
    #[arg(long, default_value = "http://100.67.98.126:11434/v1")]
    pipeline_url: String,

    /// Pipeline execution LLM model slug
    #[arg(long, default_value = "qwen3.5:9b")]
    pipeline_model: String,

    /// Evaluation Judge model slug on OpenRouter / NVIDIA
    #[arg(long, default_value = "google/gemini-2.5-flash")]
    judge_model: String,

    /// Explicit Judge API key (if omitted, falls back to OPENROUTER_API_KEY or NVIDIA_API_KEY in env or temp/.env)
    #[arg(long)]
    judge_key: Option<String>,

    /// Optional path to an existing evaluation database (e.g. from a prior layer 1 run)
    #[arg(long)]
    db: Option<PathBuf>,

    /// Destination directory for evaluation reports
    #[arg(long)]
    output_dir: Option<PathBuf>,

    /// Internal bench flag passed by cargo harness
    #[arg(long, hide = true)]
    bench: bool,

    /// Skip cases that already have artifacts in the run directory.
    #[arg(long, default_value_t = false)]
    resume: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = CliArgs::parse();

    println!("================================================================================");
    println!("Vox Memory Pipeline Evaluation Suite");
    println!("================================================================================");
    println!("  Target Layer     : {}", args.layer);
    println!("  Cases Count      : {}", args.cases);
    println!("  Pipeline Server  : {}", args.pipeline_url);
    println!("  Pipeline Model   : {}", args.pipeline_model);
    println!("  Judge Model      : {}", args.judge_model);
    println!("================================================================================");

    let judge_key = resolve_api_key(args.judge_key.as_deref())?;
    let run_id = generate_run_id();
    let base_dir = resolve_output_dir(args.output_dir.as_deref(), "memory-pipeline");
    let run_dir = create_run_directory(&base_dir, &run_id)?;

    // Written before any pipeline work so a crashed run is still auditable.
    let manifest = build_run_manifest(
        "memory-pipeline",
        &run_id,
        &std::env::args().collect::<Vec<_>>(),
        serde_json::json!({
            "layer": args.layer,
            "cases": args.cases,
            "pipeline_url": args.pipeline_url,
            "pipeline_model": args.pipeline_model,
            "judge_model": args.judge_model,
        }),
    )?;
    write_json(&run_dir, common::stage_dump::names::MANIFEST, &manifest)?;

    let (db_path, is_shared_db) = if let Some(ref custom_db) = args.db {
        if !custom_db.exists() {
            return Err(anyhow!(
                "Provided database file {:?} does not exist",
                custom_db
            ));
        }
        (custom_db.clone(), true)
    } else {
        (run_dir.join("eval_vox.db"), false)
    };

    println!(
        ">>> Initializing evaluation database at {:?} (Shared: {})",
        db_path, is_shared_db
    );
    let eval_db = EvalDbGuard::new(&db_path).await?;

    println!(
        ">>> Initializing recording pipeline LLM provider ({})",
        args.pipeline_model
    );
    let raw_provider = create_pipeline_provider(&args.pipeline_url, &args.pipeline_model, None);
    let recording_provider = RecordingLlmProvider::new(raw_provider, args.pipeline_model.clone());

    println!(">>> Initializing Judge client ({})", args.judge_model);
    let judge = NvidiaJudgeClient::new(judge_key, args.judge_model.clone());

    let target_layer = args.layer.to_lowercase();
    let run_all = target_layer == "full" || target_layer == "all";

    for case_idx in 1..=args.cases {
        println!(
            "\n--------------------------------------------------------------------------------"
        );
        println!("Evaluating Case {:02} / {:02}...", case_idx, args.cases);
        println!(
            "--------------------------------------------------------------------------------"
        );

        let (case_name, turns) = load_eval_case(case_idx)?;
        // Case directories are named from the dataset file stem so artifacts stay
        // self-describing when the tree is opened cold.
        let case_stem = case_name
            .strip_suffix(".json")
            .unwrap_or(&case_name)
            .to_string();
        let case_dir = create_case_directory(&run_dir, &case_stem)?;
        let session_id = case_idx as i64 * 1000;

        if args.resume && case_dir.join("raw_llm_traces.json").exists() {
            println!(
                "    [Resume] '{}' already has artifacts; skipping.",
                case_stem
            );
            continue;
        }

        println!(
            "Loaded case '{}' with {} conversation turns",
            case_name,
            turns.len()
        );

        // Stage 1: Compaction
        if run_all || target_layer == "compaction" {
            println!(">>> Running Compaction Stage & Compaction Judge...");
            let compaction_summary = evaluate_compaction_stage(
                &eval_db,
                session_id,
                &case_name,
                &turns,
                &recording_provider,
                &judge,
                &case_dir,
            )
            .await?;
            println!(
                "    [Compaction Done] Extracted {} facts | Report: {:?}",
                compaction_summary.facts_extracted, compaction_summary.report_path
            );
        }

        // Stage 2: Ingestion
        if run_all || target_layer == "ingestion" {
            // Prerequisite fallback if running standalone ingestion on an empty DB
            if target_layer == "ingestion" && !is_shared_db {
                let conn = eval_db.conn()?;
                let mut check_stmt = conn
                    .query(
                        "SELECT COUNT(*) FROM memory_ingestion_queue WHERE status = 'pending'",
                        (),
                    )
                    .await?;
                let pending_count: i64 = if let Some(row) = check_stmt.next().await? {
                    row.get(0)?
                } else {
                    0
                };
                if pending_count == 0 {
                    println!("    [Prerequisite] Seeding memory_ingestion_queue via unjudged compaction for case '{}'...", case_name);
                    run_compaction_and_persist(
                        &eval_db,
                        session_id,
                        &case_name,
                        &turns,
                        &recording_provider,
                    )
                    .await?;
                }
            }

            println!(">>> Running Ingestion Deduplication Cycle & Ingestion Judge...");
            let ingestion_summary =
                evaluate_ingestion_stage(&eval_db, &case_name, &judge, &case_dir).await?;
            println!(
                "    [Ingestion Done] Processed Stg1: {}, Stg2: {} (Inserted: {}) | Total Active Observations: {}",
                ingestion_summary.stage1_processed,
                ingestion_summary.stage2_processed,
                ingestion_summary.stage2_inserted,
                ingestion_summary.total_active_observations
            );
        }

        // Stage 3: Consolidation
        if run_all || target_layer == "consolidation" {
            // Prerequisite fallback if running standalone consolidation on an empty DB
            if target_layer == "consolidation" && !is_shared_db {
                let conn = eval_db.conn()?;
                let mut check_stmt = conn
                    .query("SELECT COUNT(*) FROM observations WHERE status = 'active' AND type = 'personal'", ())
                    .await?;
                let active_count: i64 = if let Some(row) = check_stmt.next().await? {
                    row.get(0)?
                } else {
                    0
                };
                if active_count == 0 {
                    println!("    [Prerequisite] Seeding active observations via unjudged compaction + ingestion for case '{}'...", case_name);
                    run_compaction_and_persist(
                        &eval_db,
                        session_id,
                        &case_name,
                        &turns,
                        &recording_provider,
                    )
                    .await?;
                    run_ingestion_cycle(&conn).await?;
                }
            }

            println!(">>> Running Personal Memory Consolidation & Consolidation Judge...");
            let consolidation_summary = evaluate_consolidation_stage(
                &eval_db,
                &case_name,
                &recording_provider,
                &judge,
                &case_dir,
            )
            .await?;
            println!(
                "    [Consolidation Done] Mode: {} | Memory: v{} -> v{} ({} sections, {} blocks)",
                consolidation_summary.mode,
                consolidation_summary.prior_version,
                consolidation_summary.new_version,
                consolidation_summary.sections_count,
                consolidation_summary.blocks_count
            );
        }

        // Flush runtime traces for this case
        recording_provider.flush_case_traces(&case_dir)?;
        println!(
            ">>> Flushed runtime LLM traces to {:?}",
            case_dir.join("raw_llm_traces.json")
        );
    }

    // Run-level summary is intentionally a manifest plus an index of per-case
    // reports. The per-stage judges own their own verdicts; duplicating them here
    // would create a second source of truth that can silently drift.
    let case_rows: Vec<String> = std::fs::read_dir(&run_dir)
        .map_err(|e| anyhow!("Failed to list run directory {:?}: {}", run_dir, e))?
        .flatten()
        .filter(|e| e.path().is_dir())
        .map(|e| {
            let name = e.file_name().to_string_lossy().to_string();
            let reports = std::fs::read_dir(e.path())
                .map(|rd| {
                    rd.flatten()
                        .filter(|f| f.path().extension().is_some_and(|x| x == "md"))
                        .count()
                })
                .unwrap_or(0);
            format!("{} | {} | `{}`", name, reports, name)
        })
        .collect();

    let preamble = format!(
        "Run ID `{}` · git `{}`{} · layer `{}` · {} case(s).\n\nPer-stage judges write their verdicts into each case directory. \
         This file indexes them; it does not restate them. Audit with:\n\n```\ncargo bench --bench memory_pipeline_eval --release -- --audit {}\n```",
        run_id,
        manifest["git"]["sha"],
        if manifest["git"]["dirty"] == serde_json::json!(true) { " (dirty)" } else { "" },
        args.layer,
        case_rows.len(),
        run_id,
    );

    write_summary_markdown(
        &run_dir,
        "Memory Pipeline Evaluation Summary",
        &preamble,
        &[(
            "Cases".to_string(),
            markdown_table(&["case", "reports", "dir"], &case_rows),
        )],
    )?;

    println!("\n================================================================================");
    println!("Evaluation Suite Run Complete!");
    println!("================================================================================");
    println!("Run ID           : {}", run_id);
    println!("Run Directory    : {:?}", run_dir);
    println!("Preserved DB     : {:?}", db_path);
    println!("QA Prompt       : evals/common/qa_prompts.yaml (id: memory_pipeline)");
    println!("================================================================================");

    Ok(())
}
