//! ============================================================================
//! agentic_tool_eval.rs — Generic Evaluation Harness for Agentic Tools & Audio
//! ============================================================================
//! Category     : Evaluation
//! Component    : services/harness/ (loop, steps, stages/tools) + services/tts/
//! Execution    : cargo run --release --bin agentic_tool_eval -- --help
//! Output       : evals/results/agentic_tool_eval/<run_id>/
//!                - report.json (telemetry, arguments, scores, timing)
//!                - judge_report.md (optional Markdown judge evaluation)
//!                - response.wav (synthesized audio output)
//! ============================================================================

#[path = "common/mod.rs"]
mod common;

use std::{
    path::PathBuf,
    sync::{atomic::Ordering, Arc},
    time::{Duration, Instant},
};

use anyhow::{Context, Result};
use clap::Parser;
use common::{audio, db, harness, judge, report};
use parking_lot::Mutex;
use tokio_util::sync::CancellationToken;
use vox_lib::{
    core::{
        settings::{TtsActiveProvider, VoxSettings},
        state::InteractionOwner,
    },
    services::{
        harness::{chassis::Harness, TurnExecutionRequest, TurnOutcome},
        tts::factory::create_tts_provider,
    },
};

#[derive(Parser, Debug)]
#[command(
    name = "agentic_tool_eval",
    about = "Generic agentic tool execution & audio synthesis evaluation harness"
)]
struct Args {
    /// User query to evaluate.
    #[arg(long, default_value = "Hello, what tools do you have available?")]
    query: String,

    /// Target tool to restrict the agent to ("all" or canonical tool name, e.g. "web_search", "search_memory", "respond_and_set_title").
    #[arg(long, default_value = "all")]
    tool: String,

    /// LLM provider endpoint URL (Ollama or OpenAI-compatible).
    #[arg(long, default_value = "http://100.67.98.126:11434/v1")]
    server_url: String,

    /// LLM model name.
    #[arg(long, default_value = "qwen3.5:9b")]
    server_model: String,

    /// Optional LLM API key.
    #[arg(long, default_value = "")]
    server_api_key: String,

    /// LLM provider preset ("ollama" or "" for generic OpenAI/Nvidia/OpenRouter).
    #[arg(long, default_value = "ollama")]
    server_provider: String,

    /// Context window size.
    #[arg(long, default_value_t = 8192)]
    context_window: u32,

    /// TTS active provider: "kokoro", "edge_tts", "supertonic", "chatterbox".
    #[arg(long, default_value = "kokoro")]
    tts_provider: String,

    /// TTS voice name or ID.
    #[arg(long, default_value = "af_sky")]
    tts_voice: String,

    /// Judge chat-completions URL (default = Nvidia API).
    #[arg(long, default_value = "")]
    judge_url: String,

    /// Judge model name.
    #[arg(long, default_value = "nvidia/nemotron-3-super-120b-a12b")]
    judge_model: String,

    /// Skip LLM judge evaluation.
    #[arg(long, default_value_t = false)]
    no_judge: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();
    let args = Args::parse();
    tokio::time::timeout(Duration::from_secs(15 * 60), run(args))
        .await
        .context("agentic_tool_eval execution timed out (15m limit)")??;
    std::process::exit(0);
}

async fn run(args: Args) -> Result<()> {
    vox_lib::utils::paths::init();
    let started = Instant::now();
    let eval_name = "agentic_tool_eval";
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));

    let api_key = if !args.server_api_key.is_empty() {
        Some(args.server_api_key.clone())
    } else {
        std::env::var("OPENROUTER_API_KEY")
            .ok()
            .or_else(|| std::env::var("NVIDIA_API_KEY").ok())
    };

    let eval_cfg = harness::EvalConfig {
        eval_name: eval_name.to_string(),
        server_url: args.server_url.clone(),
        server_model: args.server_model.clone(),
        server_api_key: api_key,
        server_provider: if args.server_provider.is_empty() {
            None
        } else {
            Some(args.server_provider.clone())
        },
        context_window: args.context_window,
    };

    let ctx =
        harness::setup_isolated_eval_context(&eval_cfg, &manifest_dir.join("evals/results"))
            .await?;

    println!("=== Agentic Tool Evaluation: Run {} ===", ctx.run_id);
    println!("Query       : {}", args.query);
    println!("Target Tool : {}", args.tool);
    println!("LLM Model   : {} @ {}", args.server_model, args.server_url);
    println!("TTS Provider: {} ({})", args.tts_provider, args.tts_voice);
    println!("Output Dir  : {}", ctx.results_dir.display());

    // Setup TTS Provider
    let tts_settings = {
        let mut s = VoxSettings::default();
        match args.tts_provider.as_str() {
            "kokoro" => {
                s.tts.active = TtsActiveProvider::Kokoro;
            }
            "edge_tts" => {
                s.tts.active = TtsActiveProvider::EdgeTts;
                s.tts.edge_tts.voice = Some(args.tts_voice.clone());
            }
            _ => {
                s.tts.active = TtsActiveProvider::Kokoro;
            }
        }
        s
    };

    let model_path = vox_lib::utils::paths::model_dir("");
    let tts_provider = create_tts_provider(&tts_settings, &model_path, None)
        .map_err(|e| anyhow::anyhow!(e))
        .context("Failed to initialize TTS provider")?;

    let audio_handles = audio::setup_audio_capture(tts_provider, ctx.event_tx.clone());

    let system_prompt = ctx.state.resolve_base_prompt();
    let mut harness = Harness::new_modular(
        Some(ctx.session_id),
        system_prompt,
        None,
        &ctx.state.settings.read().unwrap(),
        ctx.llm_tx.clone(),
        true,
    );

    // If a specific tool is targeted, isolate only that tool in the harness registry.
    if args.tool != "all" {
        let mut filtered_registry = vox_lib::services::harness::ToolRegistry::new();
        if let Some(tool_def) = harness.tool_registry().get(&args.tool) {
            filtered_registry.register(tool_def);
            harness.set_tool_registry(filtered_registry);
        } else {
            anyhow::bail!(
                "Requested tool '{}' is not registered in ToolRegistry. Available: web_search, search_memory, respond_and_set_title, set_title, or 'all'",
                args.tool
            );
        }
    }

    // Warm up the embedding model so cold-start ONNX loading doesn't distort latency
    let _ = vox_lib::services::memory::ml::embedder::ensure_embedder_loaded(true);

    let harness_arc = Arc::new(Mutex::new(Some(harness)));

    let turn_id = 1;
    let routing_ctx = vox_lib::pipeline::RoutingContext::from_app_state(&ctx.state);
    let req = TurnExecutionRequest {
        turn_id,
        query: args.query.clone(),
        cancel: CancellationToken::new(),
        app: ctx.tauri_app.clone(),
        app_state: Arc::clone(&ctx.state),
        routing_ctx,
        owner: InteractionOwner::Assistant,
        tts_tx: Some(audio_handles.tts_tx.clone()),
        pipeline_tx: Some(ctx.event_tx.clone()),
        llm_tx: Some(ctx.llm_tx.clone()),
        provider: None,
        accumulator: Arc::clone(&ctx.state.pipeline_accumulator),
        pending_synthesis_jobs: Arc::clone(&audio_handles.pending_synthesis_jobs),
        db: Arc::clone(&ctx.db),
    };

    println!("\nExecuting harness cognitive turn loop...");
    let turn_start = Instant::now();
    let outcome = Harness::execute_turn(&harness_arc, req).await;
    let turn_latency_s = turn_start.elapsed().as_secs_f64();

    // Wait for all pending TTS jobs to finish
    let tts_drain_deadline = Instant::now() + Duration::from_secs(15);
    while audio_handles.pending_synthesis_jobs.load(Ordering::Relaxed) > 0
        && Instant::now() < tts_drain_deadline
    {
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    tokio::time::sleep(Duration::from_millis(300)).await;
    audio_handles.cancel_flag.store(true, Ordering::Relaxed);

    // Save audio to response.wav
    let audio_samples = audio_handles.captured_samples.lock().clone();
    let wav_path = ctx.results_dir.join("response.wav");
    let wav_duration_s = audio::save_wav_file(&wav_path, &audio_samples, 48000)?;
    if !audio_samples.is_empty() {
        println!(
            "Synthesized audio saved: {} ({:.2}s, {} samples)",
            wav_path.display(),
            wav_duration_s,
            audio_samples.len()
        );
    } else {
        println!("Warning: No audio samples were captured during TTS synthesis.");
    }

    // Inspect database tool call records
    let ledger_deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let mut probe = ctx
            .conn
            .query(
                "SELECT COUNT(*) FROM session_tool_calls WHERE session_id = ? AND turn_id = ?;",
                (ctx.session_id, turn_id),
            )
            .await?;
        let landed = matches!(probe.next().await?, Some(row) if row.get::<i64>(0).unwrap_or(0) > 0);
        if landed || Instant::now() >= ledger_deadline {
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    let mut tool_rows = ctx
        .conn
        .query(
            "SELECT tool_name, id, arguments, result, is_error, duration_ms FROM session_tool_calls WHERE session_id = ? AND turn_id = ? ORDER BY created_at ASC;",
            (ctx.session_id, turn_id),
        )
        .await?;

    let mut executed_tools = Vec::new();
    while let Ok(Some(row)) = tool_rows.next().await {
        let tool_name: String = row.get(0).unwrap_or_default();
        let tool_call_id: String = row.get(1).unwrap_or_default();
        let arguments: String = row.get(2).unwrap_or_default();
        let result: String = row.get(3).unwrap_or_default();
        let is_error_int: i64 = row.get(4).unwrap_or(0);
        let is_error = is_error_int != 0;
        let duration_ms: i64 = row.get(5).unwrap_or(0);

        executed_tools.push(serde_json::json!({
            "tool_name": tool_name,
            "tool_call_id": tool_call_id,
            "arguments": arguments,
            "result": result,
            "is_error": is_error,
            "duration_ms": duration_ms,
        }));
    }

    let final_assistant_text = match &outcome {
        TurnOutcome::Completed {
            assistant_response, ..
        } => assistant_response.clone(),
        TurnOutcome::Cancelled { .. } => "[CANCELLED]".to_string(),
        TurnOutcome::Error { message, .. } => format!("[ERROR: {}]", message),
        TurnOutcome::DuplicateIgnored { .. } => "[DUPLICATE]".to_string(),
    };

    println!("\nTurn Outcome: {:?}", outcome);
    println!("Final Text  : {}", final_assistant_text);
    println!("Tool Calls  : {} executed", executed_tools.len());

    // Optional LLM Judge
    let judge_out = if args.no_judge {
        None
    } else {
        let api_key = std::env::var("NVIDIA_API_KEY").unwrap_or_default();
        if let Ok(prompt_template) = judge::load_prompt("judge_tool_eval.md") {
            let user_content = format!(
                "QUERY:\n{}\n\nTARGET_TOOL:\n{}\n\nTOOL_CALL:\n{}\n\nSPOKEN_TEXT:\n{}\n\nTTS_TELEMETRY:\nSamples: {}, Duration: {:.2}s, WAV: {}\n",
                args.query,
                args.tool,
                serde_json::to_string_pretty(&executed_tools)?,
                final_assistant_text,
                audio_samples.len(),
                wav_duration_s,
                wav_path.display()
            );

            match tokio::time::timeout(
                Duration::from_secs(300),
                judge::run_judge(
                    &args.judge_url,
                    &api_key,
                    &args.judge_model,
                    &prompt_template,
                    &user_content,
                    4000,
                ),
            )
            .await
            {
                Ok(Ok(j)) => Some(j),
                _ => None,
            }
        } else {
            None
        }
    };

    let total_latency_s = started.elapsed().as_secs_f64();
    let report_payload = serde_json::json!({
        "eval": "agentic_tool_eval",
        "inputs": {
            "query": args.query,
            "target_tool": args.tool,
            "server_model": args.server_model,
            "server_url": args.server_url,
            "tts_provider": args.tts_provider,
            "tts_voice": args.tts_voice,
        },
        "execution": {
            "turn_latency_s": turn_latency_s,
            "total_latency_s": total_latency_s,
            "final_response": final_assistant_text,
            "executed_tools": executed_tools,
            "audio": {
                "wav_file": wav_path.to_string_lossy(),
                "duration_s": wav_duration_s,
                "samples_count": audio_samples.len(),
            },
        },
        "judge": judge_out.as_ref().map(|j| serde_json::json!({
            "verdict": j.verdict.as_str(),
            "latency_s": j.latency_s,
            "report_markdown": j.report_markdown,
        })),
    });

    db::checkpoint_source_db(&ctx.results_dir.join("eval.db")).await?;
    let written = report::write_report(eval_name, &ctx.run_id, report_payload)?;

    println!("\n============================================================");
    println!(
        "Evaluation Completed in {:.2}s (turn latency: {:.2}s)",
        total_latency_s, turn_latency_s
    );
    println!("Report JSON: {}", written.join("report.json").display());
    println!("============================================================");

    Ok(())
}
