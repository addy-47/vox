//! ============================================================================
//! llm_bench.rs — Realtime Embedded LLM Inference & TTFT Benchmark
//! ============================================================================
//! Category     : Benchmark
//! Component    : services::llm (EmbeddedProvider, LlmWorker)
//! Prerequisites: Local GGUF models in ~/.vox/models/llm/
//! Execution    : cargo bench --bench llm_bench -- [FLAGS]
//! Metrics      : TTFT (ms), Generation Latency (ms), Tokens/sec, Memory RSS (MB)
//! ============================================================================

mod common;

use std::{path::PathBuf, sync::Arc};

use clap::Parser;
use common::{
    llm_harness::{benchmark_llm_provider, LlmBenchmarkParams, LlmBenchmarkReport},
    reporting::{generate_run_id, save_json_report, BenchmarkSystemInfo},
};
use vox_lib::{
    core::defaults::{DEFAULT_LLM_CONTEXT_WINDOW, DEFAULT_SYSTEM_PROMPT_MODULAR},
    services::llm::{embedded::EmbeddedProvider, QWEN_MODEL_DIR, QWEN_MODEL_FILE},
};

#[derive(Parser, Debug)]
#[command(
    name = "llm_bench",
    about = "Vox Embedded LLM Inference & Multi-Turn Benchmark Harness"
)]
struct CliArgs {
    /// Model to benchmark: 'qwen' or 'all'
    #[arg(long, default_value = "qwen")]
    model: String,

    /// Sampling temperature (default 0.7)
    #[arg(long, default_value_t = 0.7)]
    temperature: f32,

    /// Nucleus sampling top_p (default 0.8)
    #[arg(long, default_value_t = 0.8)]
    top_p: f32,

    /// Top_k sampling threshold (default 20)
    #[arg(long, default_value_t = 20)]
    top_k: u32,

    /// Maximum generation output tokens per turn (default 128)
    #[arg(long, default_value_t = 128)]
    max_tokens: u32,

    /// Number of multi-turn conversation steps to evaluate (default 3)
    #[arg(long, default_value_t = 3)]
    turns: usize,

    /// Whether to perform session start prefill warmup before Turn 1
    #[arg(long, default_value_t = true)]
    warmup: bool,

    /// Sampling seed
    #[arg(long, default_value_t = 42)]
    seed: u64,

    /// Destination directory for benchmark JSON reports (defaults to benches/results/llm_bench/)
    #[arg(long)]
    output_dir: Option<PathBuf>,

    /// Passed by cargo bench harness runner (ignored)
    #[arg(long, hide = true)]
    bench: bool,
}

const BENCH_SYSTEM_PROMPT: &str = DEFAULT_SYSTEM_PROMPT_MODULAR;

fn resolve_model_path(rel_dir: &str, filename: &str) -> PathBuf {
    let home = dirs::home_dir().expect("Unable to resolve user home directory");
    home.join(".vox/models").join(rel_dir).join(filename)
}

fn main() {
    let args = CliArgs::parse();

    println!("================================================================================");
    println!("Vox Embedded LLM Inference & Multi-Turn Benchmark Harness");
    println!("================================================================================");

    let params = LlmBenchmarkParams {
        temperature: args.temperature,
        top_p: args.top_p,
        top_k: args.top_k,
        max_output_tokens: args.max_tokens,
        warmup: args.warmup,
        turns: args.turns,
        seed: args.seed,
    };

    println!("Configuration:");
    println!("  Target Model     : {}", args.model);
    println!("  Temperature      : {}", params.temperature);
    println!("  Top-P            : {}", params.top_p);
    println!("  Top-K            : {}", params.top_k);
    println!("  Max Output Tokens: {}", params.max_output_tokens);
    println!("  Multi-turn Steps : {}", params.turns);
    println!("  Warmup Enabled   : {}", params.warmup);
    println!("================================================================================");

    let qwen_path = resolve_model_path(QWEN_MODEL_DIR, QWEN_MODEL_FILE);
    let mut run_results = Vec::new();

    if args.model == "qwen" || args.model == "all" {
        println!("\n>>> Benchmarking Qwen Embedded LLM (Qwen3.5-0.8B Q4_K_M)...");
        if !qwen_path.exists() {
            eprintln!("[ERROR] Qwen model not found at {:?}", qwen_path);
            std::process::exit(1);
        }

        let provider = match EmbeddedProvider::new(&qwen_path, DEFAULT_LLM_CONTEXT_WINDOW, 4) {
            Ok(p) => Arc::new(p),
            Err(e) => {
                eprintln!("[ERROR] Failed to load EmbeddedProvider: {:?}", e);
                std::process::exit(1);
            }
        };

        match benchmark_llm_provider(
            provider,
            "Qwen3.5-0.8B",
            &qwen_path,
            BENCH_SYSTEM_PROMPT,
            &params,
        ) {
            Ok(res) => run_results.push(res),
            Err(e) => {
                eprintln!("[ERROR] Benchmark failed: {}", e);
                std::process::exit(1);
            }
        }
    }

    println!("\n================================================================================");
    println!("Benchmark Execution Summary");
    println!("================================================================================");
    for run in &run_results {
        println!(
            "Model: {} | RSS: ~{} MB | Warmup: {} ({:.1}ms) | Avg TTFT: {:.1}ms | Avg Speed: {:.1} tok/s",
            run.model_name,
            run.memory_rss_mb,
            run.warmup_performed,
            run.warmup_duration_ms,
            run.avg_ttft_ms,
            run.avg_tokens_per_sec,
        );
    }

    let run_id = generate_run_id();
    let report = LlmBenchmarkReport {
        run_id: run_id.clone(),
        timestamp_utc: chrono::Utc::now().to_rfc3339(),
        benchmark_name: "llm_bench".to_string(),
        system_info: BenchmarkSystemInfo::default(),
        params,
        runs: run_results,
    };

    let base_output_dir = args
        .output_dir
        .unwrap_or_else(|| PathBuf::from("benches/results/llm_bench"));

    match save_json_report(&base_output_dir, &run_id, &report) {
        Ok(path) => {
            println!(
                "================================================================================"
            );
            println!("Benchmark Result Artifact Successfully Saved!");
            println!("Run ID   : {}", run_id);
            println!("Report   : {:?}", path);
            println!("Latest   : {:?}", base_output_dir.join("latest.json"));
            println!(
                "================================================================================"
            );
        }
        Err(e) => {
            eprintln!("[ERROR] Failed to save benchmark report: {}", e);
            std::process::exit(1);
        }
    }
}
