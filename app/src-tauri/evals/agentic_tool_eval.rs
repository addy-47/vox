//! ============================================================================
//! agentic_tool_eval.rs — Evaluation Harness for Agentic Tools & Audio Egress
//! ============================================================================
//! Category     : Evaluation
//! Component    : services/harness/ (loop, steps, stages/tools) + services/tts/
//! Execution    : cargo run --release --bin agentic_tool_eval -- --help
//! Output       : evals/results/agentic_tool_eval/<run_id>/
//!                - report.json (telemetry, arguments, scores, timing)
//!                - judge_report.md (detailed Markdown judge evaluation)
//!                - response.wav (synthesized audio output)
//! ============================================================================

#[path = "common/mod.rs"]
mod common;

use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, AtomicU32, AtomicU64, AtomicU8, Ordering},
        mpsc, Arc,
    },
    time::{Duration, Instant},
};

use anyhow::{Context, Result};
use clap::Parser;
use common::{db, judge, report};
use parking_lot::Mutex;
use ringbuf::{traits::*, HeapRb};
use tokio_util::sync::CancellationToken;
use vox_lib::{
    core::{
        events::VoxEvent,
        settings::{LlmActiveProvider, TtsActiveProvider, VoxSettings},
        state::{AppState, InteractionOwner, TelemetryState},
    },
    persistence::{
        facts::{insert_fact, insert_vector, FactRecord},
        sessions::create_session,
        worker::spawn_persistence_worker,
    },
    services::{
        audio::{
            playback::{PlaybackEngine, PlaybackEngineHandles},
            PLAYBACK_BUFFER_SAMPLES,
        },
        harness::{chassis::Harness, TurnExecutionRequest, TurnOutcome},
        llm::{actor::spawn_llm_worker, LlmCommand},
        tts::{
            actor::{spawn_tts_worker, TtsCommand, TtsWorkerHandles},
            factory::create_tts_provider,
        },
    },
};

#[derive(Parser, Debug)]
#[command(
    name = "agentic_tool_eval",
    about = "Agentic tool execution & audio synthesis evaluation harness"
)]
struct Args {
    /// User query to evaluate.
    #[arg(long, default_value = "What graphics card do I have in my rig?")]
    query: String,

    /// Target tool to restrict the agent to ("search_memory", "respond_and_set_title", or "all").
    #[arg(long, default_value = "search_memory")]
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

    /// LLM provider preset ("ollama" or "" for generic OpenAI/Nvidia).
    #[arg(long, default_value = "ollama")]
    server_provider: String,

    /// Context window size.
    #[arg(long, default_value_t = 8192)]
    context_window: u32,

    /// TTS active provider: "edge_tts", "supertonic", "kokoro", "chatterbox", or "chatterbox_remote".
    #[arg(long, default_value = "edge_tts")]
    tts_provider: String,

    /// TTS voice name or ID.
    #[arg(long, default_value = "en-US-EmmaMultilingualNeural")]
    tts_voice: String,

    /// Seed sample facts into Turso DB before evaluation (useful for memory retrieval tool).
    #[arg(long, default_value_t = true)]
    seed_facts: bool,

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
        .context("agentic_tool_eval execution timed out (15m limit)")?
}

async fn run(args: Args) -> Result<()> {
    vox_lib::utils::paths::init();
    let started = Instant::now();
    let eval_name = "agentic_tool_eval";
    let run_id = report::new_run_id();

    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    for candidate in &[
        manifest_dir.join("../../temp/.env"),
        manifest_dir.join("../temp/.env"),
        manifest_dir.join("temp/.env"),
    ] {
        if let Ok(content) = std::fs::read_to_string(candidate) {
            for line in content.lines() {
                let trimmed = line.trim();
                if trimmed.is_empty() || trimmed.starts_with('#') {
                    continue;
                }
                if let Some((k, v)) = trimmed.split_once('=') {
                    let k = k.trim();
                    let v = v.trim().trim_matches('"');
                    if std::env::var(k).is_err() && !k.is_empty() {
                        std::env::set_var(k, v);
                    }
                }
            }
            break;
        }
    }

    let results_dir = manifest_dir
        .join("evals/results")
        .join(eval_name)
        .join(&run_id);
    tokio::fs::create_dir_all(&results_dir).await?;

    println!("=== Agentic Tool Evaluation: Run {} ===", run_id);
    println!("Query       : {}", args.query);
    println!("Target Tool : {}", args.tool);
    println!("LLM Model   : {} @ {}", args.server_model, args.server_url);
    println!("TTS Provider: {} ({})", args.tts_provider, args.tts_voice);
    println!("Output Dir  : {}", results_dir.display());

    // 1. Setup isolated database
    let db_path = results_dir.join("eval.db");
    let (db, conn) = db::open_fresh_eval_db(&db_path).await?;
    let db_arc = Arc::new(db);

    // 2. Create session in DB
    let session_id = create_session(&conn, None).await?;

    // 3. Seed memory facts if requested
    if args.seed_facts {
        conn.execute(
            "INSERT INTO session_compactions (id, session_id, trigger_kind, from_turn_id, to_turn_id, compaction_output, status, created_at)
             VALUES (1, ?, 'periodic', 1, 1, 'Initial summary', 'completed', 1700000000)",
            (session_id,),
        )
        .await?;

        let fact_record = FactRecord {
            id: "fact_gpu_rig_01".to_string(),
            session_id: Some(session_id),
            compaction_id: 1,
            fact_type: "hardware".to_string(),
            text: "User possesses an NVIDIA RTX 4090 GPU with 24GB VRAM.".to_string(),
            status: "active".to_string(),
            created_at: 1700000000,
            updated_at: 1700000000,
        };
        insert_fact(&conn, &fact_record).await?;
        let dummy_vector = vec![0.05f32; 384];
        insert_vector(
            &conn,
            &fact_record.id,
            "hardware",
            "active",
            None,
            &dummy_vector,
        )
        .await?;
        println!(
            "Seeded episodic memory fact [ID: {}]: {}",
            fact_record.id, fact_record.text
        );
    }

    println!("[DEBUG] Configuring VoxSettings...");
    let mut settings = VoxSettings::default();
    settings.llm.active = LlmActiveProvider::Server;
    settings.llm.server.base_url = args.server_url.clone();
    settings.llm.server.model = args.server_model.clone();
    settings.llm.server.api_key = if args.server_api_key.is_empty() {
        None
    } else {
        Some(args.server_api_key.clone())
    };
    settings.llm.server.provider_name = if args.server_provider.is_empty() {
        None
    } else {
        Some(args.server_provider.clone())
    };
    settings.llm.context_window = args.context_window;
    if args.tool == "respond_and_set_title" || args.tool == "set_title" {
        settings.personal_memory.context_retrieval_enabled = false;
    } else {
        settings.personal_memory.context_retrieval_enabled = true;
    }

    // TTS Provider selection
    match args.tts_provider.as_str() {
        "edge_tts" => {
            settings.tts.active = TtsActiveProvider::EdgeTts;
            settings.tts.edge_tts.voice = Some(args.tts_voice.clone());
        }
        "supertonic" => {
            settings.tts.active = TtsActiveProvider::Supertonic;
        }
        "kokoro" => {
            settings.tts.active = TtsActiveProvider::Kokoro;
        }
        "chatterbox" => {
            settings.tts.active = TtsActiveProvider::Chatterbox;
        }
        "chatterbox_remote" => {
            settings.tts.active = TtsActiveProvider::ChatterboxRemote;
        }
        other => {
            anyhow::bail!("Unsupported TTS provider: {}", other);
        }
    }

    println!("[DEBUG] Creating mock AppHandle & TelemetryState...");
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

    println!("[DEBUG] Initializing AppState...");
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

    println!("[DEBUG] Spawning persistence worker (tool-call ledger)...");
    let persist_tx = spawn_persistence_worker(
        Arc::clone(&db_arc),
        Arc::clone(&state.telemetry.is_db_healthy),
        Arc::clone(&state.telemetry.latest_persistence_rate),
        Arc::clone(&state.telemetry.is_private_mode),
    );
    *state.persist_tx.lock() = Some(persist_tx);

    println!("[DEBUG] Spawning LLM Provider & Worker thread...");
    let (llm_tx, llm_rx) = mpsc::channel::<LlmCommand>();
    let llm_provider = vox_lib::services::llm::actor::create_llm_provider_from_llm_settings(
        &settings.llm,
        &vox_lib::utils::paths::model_dir(""),
    )
    .map_err(|e| anyhow::anyhow!(e))
    .context("Failed to create LLM provider")?;
    let llm_provider_arc: Arc<dyn vox_lib::services::llm::LlmProvider> = llm_provider.into();
    let _llm_handle = std::thread::Builder::new()
        .name("vox-llm-worker".to_string())
        .spawn(move || {
            spawn_llm_worker(llm_rx, llm_provider_arc);
        })
        .context("Failed to spawn LLM worker thread")?;

    println!("[DEBUG] Spawning TTS Worker & RingBuffer...");
    let (tts_tx, tts_rx) = mpsc::channel::<TtsCommand>();
    let (event_tx, _event_rx) = mpsc::channel::<VoxEvent>();
    let pending_synthesis_jobs = Arc::new(AtomicU32::new(0));
    let cancel_flag = Arc::new(AtomicBool::new(false));

    let playback_rb = HeapRb::<f32>::new(PLAYBACK_BUFFER_SAMPLES);
    let (pb_prod, mut pb_cons) = playback_rb.split();
    let pb_handles = PlaybackEngineHandles {
        cancel_flag: Arc::clone(&cancel_flag),
        state_atomic: Arc::clone(&state.pipeline.current_state_atomic),
        current_turn_id: Arc::new(AtomicU32::new(1)),
        pending_synthesis_jobs: Arc::clone(&pending_synthesis_jobs),
        event_tx: event_tx.clone(),
        playback_intent: Arc::new(AtomicU8::new(0)),
        is_playback_muted: Arc::new(AtomicBool::new(false)),
        turn_metrics: None,
    };
    let playback_engine = PlaybackEngine::from_parts(
        pb_prod,
        pb_handles,
        Arc::new(AtomicBool::new(false)),
        Arc::new(AtomicBool::new(false)),
        None,
    );
    let playback_arc = Arc::new(playback_engine);

    let captured_samples: Arc<Mutex<Vec<f32>>> = Arc::new(Mutex::new(Vec::new()));
    let captured_clone = Arc::clone(&captured_samples);
    let playback_cancel = Arc::clone(&cancel_flag);
    std::thread::spawn(move || {
        let mut buffer = [0.0f32; 1024];
        while !playback_cancel.load(Ordering::Relaxed) {
            let read = pb_cons.pop_slice(&mut buffer);
            if read > 0 {
                captured_clone.lock().extend_from_slice(&buffer[..read]);
            } else {
                std::thread::sleep(Duration::from_millis(10));
            }
        }
    });

    println!("[DEBUG] Creating TTS provider ({}) ...", args.tts_provider);
    let supertonic_path =
        vox_lib::utils::paths::model_dir(vox_lib::services::tts::SUPERTONIC_MODEL_DIR);
    let tts_provider = create_tts_provider(&settings, &supertonic_path, None)
        .map_err(|e| anyhow::anyhow!(e))
        .context("Failed to create TTS provider")?;

    let tts_handles = TtsWorkerHandles {
        playback: Arc::clone(&playback_arc),
        event_tx: event_tx.clone(),
        cancel_flag: Arc::clone(&cancel_flag),
        pending_synthesis_jobs: Some(Arc::clone(&pending_synthesis_jobs)),
        telemetry_rtf: None,
        turn_metrics: None,
    };
    let _tts_worker = std::thread::Builder::new()
        .name("vox-tts-worker".to_string())
        .spawn(move || {
            spawn_tts_worker(tts_rx, tts_provider, tts_handles);
        })
        .context("Failed to spawn TTS worker thread")?;

    println!("[DEBUG] Creating Harness::new_modular...");
    let system_prompt = state.resolve_base_prompt();
    let harness = Harness::new_modular(
        Some(session_id),
        system_prompt,
        None,
        &settings,
        llm_tx.clone(),
        true, // supports_tools = true
    );

    let harness_arc = Arc::new(Mutex::new(Some(harness)));

    println!("[DEBUG] Preparing TurnExecutionRequest...");
    let routing_ctx = vox_lib::pipeline::RoutingContext::from_app_state(&state);
    let turn_id = 1;

    let req = TurnExecutionRequest {
        turn_id,
        query: args.query.clone(),
        cancel: CancellationToken::new(),
        app: tauri_app,
        app_state: Arc::clone(&state),
        routing_ctx,
        owner: InteractionOwner::Assistant,
        tts_tx: Some(tts_tx.clone()),
        pipeline_tx: Some(event_tx),
        llm_tx: Some(llm_tx),
        provider: None,
        accumulator: Arc::clone(&state.pipeline_accumulator),
        pending_synthesis_jobs: Arc::clone(&pending_synthesis_jobs),
        db: Arc::clone(&db_arc),
    };

    println!("\nExecuting harness reentrant cognitive turn loop...");
    let turn_start = Instant::now();
    let outcome = Harness::execute_turn(&harness_arc, req).await;
    let turn_latency_s = turn_start.elapsed().as_secs_f64();
    println!("[DEBUG] Turn outcome received: {:?}", outcome);

    // Wait for all pending TTS audio synthesis jobs to finish
    let tts_drain_deadline = Instant::now() + Duration::from_secs(15);
    while pending_synthesis_jobs.load(Ordering::Relaxed) > 0 && Instant::now() < tts_drain_deadline
    {
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    tokio::time::sleep(Duration::from_millis(300)).await; // allow consumer drain
    cancel_flag.store(true, Ordering::Relaxed);

    // 9. Capture Spoken Audio to WAV
    let audio_samples = captured_samples.lock().clone();
    let wav_path = results_dir.join("response.wav");
    let mut wav_duration_s = 0.0f64;
    if !audio_samples.is_empty() {
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate: 48000,
            bits_per_sample: 32,
            sample_format: hound::SampleFormat::Float,
        };
        let mut writer = hound::WavWriter::create(&wav_path, spec)?;
        for sample in &audio_samples {
            writer.write_sample(*sample)?;
        }
        writer.finalize()?;
        wav_duration_s = audio_samples.len() as f64 / 48000.0;
        println!(
            "Synthesized audio saved: {} ({:.2}s, {} samples)",
            wav_path.display(),
            wav_duration_s,
            audio_samples.len()
        );
    } else {
        println!("Warning: No audio samples were captured during TTS synthesis.");
    }

    // 10. Inspect Database Tool Calls & Outcome
    // Drain the async persistence worker: poll until the tool-call ledger row
    // lands (or 10s), otherwise the query below races the worker thread.
    let ledger_deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let mut probe = conn
            .query(
                "SELECT COUNT(*) FROM session_tool_calls WHERE session_id = ?;",
                (session_id,),
            )
            .await?;
        let landed = matches!(probe.next().await?, Some(row) if row.get::<i64>(0).unwrap_or(0) > 0);
        if landed || Instant::now() >= ledger_deadline {
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    let mut tool_rows = conn
        .query(
            "SELECT tool_name, id, arguments, result, is_error, duration_ms FROM session_tool_calls WHERE session_id = ? ORDER BY created_at ASC;",
            (session_id,),
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

    // 11. Run LLM Judge Evaluation (Markdown Report)
    let judge_out = if args.no_judge {
        None
    } else {
        let api_key = std::env::var("NVIDIA_API_KEY").unwrap_or_default();
        let prompt_template = judge::load_prompt("judge_tool_eval.md")?;
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

        Some(
            tokio::time::timeout(
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
            .context("Judge evaluation timed out")?
            .context("Judge evaluation call failed")?,
        )
    };

    // 12. Write Telemetry & Markdown Reports
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
            "judge_model": args.judge_model,
        },
        "execution": {
            "turn_latency_s": turn_latency_s,
            "total_latency_s": total_latency_s,
            "executed_tools": executed_tools,
            "final_response": final_assistant_text,
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

    db::checkpoint_source_db(&db_path).await?;
    let written = report::write_report(eval_name, &run_id, report_payload)?;

    // Write full markdown judge report directly to run dir
    if let Some(j) = judge_out.as_ref() {
        let judge_md_path = written.join("judge_report.md");
        tokio::fs::write(&judge_md_path, &j.report_markdown).await?;
        println!("\n=== LLM Judge Verdict: {} ===", j.verdict.as_str());
        println!("Judge Report: {}", judge_md_path.display());
    }

    println!("\nReport JSON : {}", written.join("report.json").display());
    println!("Total Time  : {:.2}s", total_latency_s);

    Ok(())
}
