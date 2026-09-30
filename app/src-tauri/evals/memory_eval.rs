//! ============================================================================
//! memory_eval.rs — Vox Memory Pipeline Comprehensive Evaluation Harness
//! ============================================================================
//! Category     : Evaluation Suite
//! Component    : services/memory (Compaction, Ingestion, Consolidation)
//! Execution    : cargo bench --bench memory_eval --release -- [FLAGS]
//! Metrics      : Compaction Coverage, Ingestion Dedup Precision/Recall, Consolidation Semantic Loss
//! Output       : evals/results/memory_eval/<run_id>/ (raw_llm_traces.json, *.md, eval_vox.db)
//! Post-run     : OpenCode Subagent master synthesis with opencode/space-bunny-free --variant max
//! ============================================================================

mod common;

use std::{
    fs,
    path::PathBuf,
};

use anyhow::{anyhow, Result};
use clap::Parser;
use common::{
    compaction_eval::evaluate_compaction_stage,
    consolidation_eval::evaluate_consolidation_stage,
    datasets::load_eval_case,
    db::EvalDbGuard,
    ingestion_eval::evaluate_ingestion_stage,
    llm_client::{create_pipeline_provider, NvidiaJudgeClient, RecordingLlmProvider},
    reporting::{create_case_directory, create_run_directory, generate_run_id},
};

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

    /// Evaluation Judge model slug on NVIDIA NIM
    #[arg(long, default_value = "nvidia/nemotron-3-super-120b-a12b")]
    judge_model: String,

    /// Explicit NVIDIA API key (if omitted, falls back to NVIDIA_API_KEY env or temp/.env)
    #[arg(long)]
    judge_key: Option<String>,

    /// Destination directory for evaluation reports
    #[arg(long)]
    output_dir: Option<PathBuf>,

    /// Internal bench flag passed by cargo harness
    #[arg(long, hide = true)]
    bench: bool,
}

/// Resolves the NVIDIA API key from CLI args, environment variables, or `temp/.env`.
fn resolve_nvidia_api_key(cli_key: Option<&str>) -> Result<String> {
    if let Some(key) = cli_key {
        if !key.trim().is_empty() {
            return Ok(key.trim().to_string());
        }
    }

    if let Ok(env_key) = std::env::var("NVIDIA_API_KEY") {
        if !env_key.trim().is_empty() {
            return Ok(env_key.trim().to_string());
        }
    }

    let env_paths = [
        PathBuf::from("temp/.env"),
        PathBuf::from("../../temp/.env"),
        PathBuf::from("../../../temp/.env"),
    ];

    for p in &env_paths {
        if p.exists() {
            if let Ok(content) = fs::read_to_string(p) {
                for line in content.lines() {
                    let trimmed = line.trim();
                    if trimmed.starts_with("NVIDIA_API_KEY=") {
                        let key = trimmed.trim_start_matches("NVIDIA_API_KEY=").trim();
                        if !key.is_empty() {
                            return Ok(key.to_string());
                        }
                    }
                }
            }
        }
    }

    Err(anyhow!(
        "NVIDIA API Key not found. Provide --judge-key, set NVIDIA_API_KEY, or populate temp/.env"
    ))
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

    let judge_key = resolve_nvidia_api_key(args.judge_key.as_deref())?;
    let run_id = generate_run_id();

    let base_dir = args
        .output_dir
        .unwrap_or_else(|| PathBuf::from("evals/results/memory_eval"));

    let run_dir = create_run_directory(&base_dir, &run_id)?;
    let db_path = run_dir.join("eval_vox.db");

    println!(">>> Initializing isolated evaluation database at {:?}", db_path);
    let eval_db = EvalDbGuard::new(&db_path).await?;

    println!(">>> Initializing recording pipeline LLM provider ({})", args.pipeline_model);
    let raw_provider = create_pipeline_provider(&args.pipeline_url, &args.pipeline_model, None);
    let recording_provider = RecordingLlmProvider::new(raw_provider, args.pipeline_model.clone());

    println!(">>> Initializing NVIDIA Judge client ({})", args.judge_model);
    let judge = NvidiaJudgeClient::new(judge_key, args.judge_model.clone());

    let target_layer = args.layer.to_lowercase();
    let run_all = target_layer == "full" || target_layer == "all";

    for case_idx in 1..=args.cases {
        println!("\n--------------------------------------------------------------------------------");
        println!("Evaluating Case {:02} / {:02}...", case_idx, args.cases);
        println!("--------------------------------------------------------------------------------");

        let (case_name, turns) = load_eval_case(case_idx)?;
        let case_dir = create_case_directory(&run_dir, case_idx)?;
        let session_id = case_idx as i64 * 1000;

        println!("Loaded case '{}' with {} conversation turns", case_name, turns.len());

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
        println!(">>> Flushed runtime LLM traces to {:?}", case_dir.join("raw_llm_traces.json"));
    }

    println!("\n================================================================================");
    println!("Evaluation Suite Run Complete!");
    println!("================================================================================");
    println!("Run ID           : {}", run_id);
    println!("Run Directory    : {:?}", run_dir);
    println!("Preserved DB     : {:?}", db_path);
    println!("Subagent Audit   : Run './evals/audit_run.sh {}' to generate master synthesis report", run_id);
    println!("================================================================================");

    Ok(())
}
