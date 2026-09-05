//! ============================================================================
//! pipeline_bench.rs — End-to-End Vox Voice Pipeline Benchmark
//! ============================================================================
//! Category     : Benchmark
//! Component    : pipeline::assistant, services::vad, services::stt, services::llm, services::tts, services::audio
//! Prerequisites: Local model weights in ~/.vox/models/ (Nemotron, Qwen GGUF, Kokoro / Supertonic)
//! Execution    : cargo test --bench pipeline_bench --release -- [FLAGS]
//! Metrics      : $T_{vad}$, $T_{stt}$, $T_{ttft}$, $T_{tts}$, $T_{e2e}$, Audio Duration (s), Pipeline RTF
//! Artifacts    : benches/results/pipeline_bench/<run_id>/report.json + wav/*.wav + latest.json
//! ============================================================================

mod common;

use clap::Parser;
use common::audio::{load_wav, resolve_clip_path};
use common::pipeline_harness::setup_e2e_pipeline;
use common::reporting::{
    generate_run_id, get_process_memory_mb, save_benchmark_report, BenchmarkReport,
    BenchmarkSystemInfo, ClipBenchmarkResult, EngineBenchmarkRun,
};
use common::utils::{downsample_48k_to_24k, drain_consumer, write_wav_f32};
use ringbuf::traits::{Observer, Producer};
use std::{
    path::PathBuf,
    sync::{atomic::Ordering, Arc},
    time::{Duration, Instant},
};

use vox_lib::core::events::VoxEvent;
use vox_lib::core::settings::{
    InteractionMode, LlmActiveProvider, PipelineMode, TtsActiveProvider, VoxSettings,
};
use vox_lib::core::state::{InteractionOwner, InteractionState};
use vox_lib::pipeline::assistant::session::on_session_start;
use vox_lib::pipeline::RoutingContext;

#[derive(Parser, Debug)]
#[command(
    name = "pipeline_bench",
    about = "Vox End-to-End Voice Pipeline Benchmark (Audio-In to Audio-Out)"
)]
struct CliArgs {
    /// Mode: 'modular_passive', 'modular_ptt'
    #[arg(long, default_value = "modular_passive")]
    mode: String,

    /// STT Provider: 'nemotron'
    #[arg(long, default_value = "nemotron")]
    stt: String,

    /// LLM Provider: 'qwen'
    #[arg(long, default_value = "qwen")]
    llm: String,

    /// TTS Provider: 'kokoro', 'supertonic', 'edge'
    #[arg(long, default_value = "kokoro")]
    tts: String,

    /// Audio clip to benchmark (defaults to clip_01_en_briefing.wav)
    #[arg(long, default_value = "clip_01_en_briefing.wav")]
    clip: String,

    /// Output directory for benchmark artifacts
    #[arg(long)]
    output_dir: Option<PathBuf>,

    /// Passed by cargo bench harness runner (ignored)
    #[arg(long, hide = true)]
    bench: bool,
}

fn load_nvidia_api_key() -> String {
    if let Ok(k) = std::env::var("NVIDIA_API_KEY") {
        if !k.trim().is_empty() {
            return k.trim().to_string();
        }
    }
    let paths = [
        "temp/.env",
        "../../temp/.env",
        "../temp/.env",
        "/home/addy/projects/apps/vox/temp/.env",
    ];
    for p in paths {
        if let Ok(content) = std::fs::read_to_string(p) {
            for line in content.lines() {
                if let Some(rest) = line.strip_prefix("NVIDIA_API_KEY=") {
                    let val = rest.trim();
                    if !val.is_empty() {
                        return val.to_string();
                    }
                }
            }
        }
    }
    String::new()
}

fn main() {
    let _ = env_logger::Builder::from_env(
        env_logger::Env::default().default_filter_or("debug,ort=warn,sherpa_rs=warn,llama=warn"),
    )
    .format_timestamp_millis()
    .try_init();

    let args = CliArgs::parse();

    println!("================================================================================");
    println!("Vox End-to-End Voice Pipeline Benchmark (Audio-In to Audio-Out)");
    println!("================================================================================");
    println!("Configuration:");
    println!("  Mode     : {}", args.mode);
    println!("  STT      : {}", args.stt);
    println!("  LLM      : {}", args.llm);
    println!("  TTS      : {}", args.tts);
    println!("  Clip     : {}", args.clip);

    let is_ptt = args.mode.contains("ptt");

    // 1. Configure Settings
    let mut settings = VoxSettings::default();
    settings.interaction.pipeline_mode = PipelineMode::Modular;
    settings.interaction.mode = if is_ptt {
        InteractionMode::PTT
    } else {
        InteractionMode::Passive
    };

    match args.tts.to_lowercase().as_str() {
        "supertonic" => settings.tts.active = TtsActiveProvider::Supertonic,
        "edge" | "edge_tts" => settings.tts.active = TtsActiveProvider::EdgeTts,
        _ => settings.tts.active = TtsActiveProvider::Kokoro,
    }

    match args.llm.to_lowercase().as_str() {
        "nvidia" | "cloud" => {
            let api_key = load_nvidia_api_key();
            if api_key.is_empty() {
                panic!(
                    "NVIDIA_API_KEY must be provided in environment or temp/.env for --llm nvidia"
                );
            }
            settings.llm.active = LlmActiveProvider::Cloud;
            settings.llm.cloud.provider_name = Some("nvidia".to_string());
            settings.llm.cloud.base_url = "https://integrate.api.nvidia.com/v1".to_string();
            settings.llm.cloud.model = "meta/llama-3.1-8b-instruct".to_string();
            settings.llm.cloud.api_key = Some(api_key);
        }
        _ => settings.llm.active = LlmActiveProvider::Embedded,
    }

    let clip_path = resolve_clip_path(&args.clip, None)
        .unwrap_or_else(|e| panic!("Failed to resolve audio clip: {}", e));
    let (audio_samples, duration_s) =
        load_wav(&clip_path).unwrap_or_else(|e| panic!("Failed to load WAV audio: {}", e));

    println!("Audio Clip Loaded: {:?} ({:.2}s)", clip_path, duration_s);

    // 2. Initialize Pipeline & Capture Tap
    let mem_before = get_process_memory_mb();
    let (app, state, in_prod_arc, playback_cons, event_rx, _bench_guard) =
        setup_e2e_pipeline(settings);
    let mem_after = get_process_memory_mb();

    println!(
        "Pipeline Initialized. Memory (RSS): {} MB (delta {} MB)",
        mem_after,
        mem_after.saturating_sub(mem_before)
    );

    let ctx = RoutingContext {
        pipeline_mode: PipelineMode::Modular,
        interaction_mode: if is_ptt {
            InteractionMode::PTT
        } else {
            InteractionMode::Passive
        },
        owner: InteractionOwner::Assistant,
    };

    on_session_start(InteractionOwner::Assistant, &app, &state, &ctx);
    state.pipeline.set_state(InteractionState::Ready);
    state.pipeline.update_ingestion_gate();

    // Settle
    std::thread::sleep(Duration::from_millis(500));

    println!("\n>>> Streaming Audio Input into Pipeline...");
    let run_start = Instant::now();
    let mut speech_start_time = None;
    let mut speech_end_time = None;
    let mut transcript_final_time = None;
    let mut playback_start_time = None;
    let mut captured_transcript = String::new();
    let mut captured_audio = Vec::new();
    let mut llm_finished = false;

    let in_prod_feed = Arc::clone(&in_prod_arc);
    let audio_feed = audio_samples.clone();
    let app_feed = app.clone();
    let state_feed = state.clone();
    let feed_handle = std::thread::spawn(move || {
        if is_ptt {
            let _ = vox_lib::pipeline::assistant::ptt::ptt_start(&app_feed, &state_feed);
        }

        // Stream audio chunks in real-time pace (16ms frames @ 16kHz = 256 samples)
        let chunk_size = 256;
        for chunk in audio_feed.chunks(chunk_size) {
            {
                let mut lock = in_prod_feed.lock();
                let _ = lock.push_slice(chunk);
            }
            std::thread::sleep(Duration::from_millis(16));
        }

        if is_ptt {
            let app_c = app_feed.clone();
            let state_c = state_feed.clone();
            tokio::runtime::Runtime::new()
                .unwrap()
                .block_on(async move {
                    let _ = vox_lib::pipeline::assistant::ptt::ptt_stop(&app_c, &state_c).await;
                });
        } else {
            // Trailing silence to trigger VAD SpeechEnd: 1.5s = 94 chunks of 256 samples @ 16ms
            let silence = vec![0.0f32; chunk_size];
            for _ in 0..94 {
                {
                    let mut lock = in_prod_feed.lock();
                    let _ = lock.push_slice(&silence);
                }
                std::thread::sleep(Duration::from_millis(16));
            }
        }
    });

    println!("Audio streaming started. Polling pipeline events & playback tap...");

    let deadline = Instant::now() + Duration::from_secs(60);
    while Instant::now() < deadline {
        // Harvest playback audio samples from tap
        {
            let mut cons = playback_cons.lock();
            let drained = drain_consumer(&mut cons);
            if !drained.is_empty() {
                captured_audio.extend_from_slice(&drained);
            }
        }

        // Poll pipeline events
        while let Ok(ev) = event_rx.recv_timeout(Duration::from_millis(40)) {
            match ev {
                VoxEvent::SpeechStart => {
                    if speech_start_time.is_none() {
                        speech_start_time = Some(Instant::now());
                        println!(
                            "  [VAD] SpeechStart detected at +{:.2}s",
                            run_start.elapsed().as_secs_f64()
                        );
                    }
                }
                VoxEvent::SpeechEnd => {
                    if playback_start_time.is_none() {
                        speech_end_time = Some(Instant::now());
                        println!(
                            "  [VAD] SpeechEnd detected at +{:.2}s",
                            run_start.elapsed().as_secs_f64()
                        );
                    }
                }
                VoxEvent::TranscriptFinal { turn_id, text } => {
                    transcript_final_time = Some(Instant::now());
                    captured_transcript = text.clone();
                    println!(
                        "  [STT] TranscriptFinal (turn {}): \"{}\" at +{:.2}s",
                        turn_id,
                        text,
                        run_start.elapsed().as_secs_f64()
                    );
                }
                VoxEvent::PlaybackStarted { turn_id } => {
                    if playback_start_time.is_none() {
                        playback_start_time = Some(Instant::now());
                        println!("  [AUDIO] PlaybackStarted (turn {}) at +{:.2}s (First audio byte reached speaker!)", turn_id, run_start.elapsed().as_secs_f64());
                    }
                }
                VoxEvent::LlmFinished { turn_id } => {
                    llm_finished = true;
                    println!(
                        "  [LLM] LlmFinished (turn {}) at +{:.2}s",
                        turn_id,
                        run_start.elapsed().as_secs_f64()
                    );
                }
                VoxEvent::PlaybackFinished { turn_id } => {
                    println!(
                        "  [AUDIO] PlaybackFinished (turn {}) at +{:.2}s",
                        turn_id,
                        run_start.elapsed().as_secs_f64()
                    );
                }
                VoxEvent::Error(err) => {
                    eprintln!(
                        "  [ERROR] Event received from {}: {}",
                        err.source, err.message
                    );
                }
                _ => {}
            }
        }

        let pending = state
            .pipeline
            .pending_synthesis_jobs
            .load(Ordering::Relaxed);
        let cons_empty = playback_cons.lock().is_empty();
        if (llm_finished && pending == 0 && cons_empty && playback_start_time.is_some())
            || (playback_start_time.is_some() && state.pipeline.state() == InteractionState::Ready)
        {
            println!("Pipeline finished synthesis and playback drained. Finalizing...");
            std::thread::sleep(Duration::from_millis(200));
            let mut cons = playback_cons.lock();
            let drained = drain_consumer(&mut cons);
            captured_audio.extend_from_slice(&drained);
            break;
        }
    }

    let _ = feed_handle.join();

    let total_elapsed = run_start.elapsed().as_secs_f64();
    let audio_24k = downsample_48k_to_24k(&captured_audio);
    let synth_duration_s = audio_24k.len() as f32 / 24000.0;

    let e2e_response_time_ms = if let (Some(se), Some(ps)) = (speech_end_time, playback_start_time)
    {
        ps.duration_since(se).as_secs_f64() * 1000.0
    } else {
        total_elapsed * 1000.0
    };

    let stt_latency_ms = if let (Some(se), Some(tf)) = (speech_end_time, transcript_final_time) {
        tf.duration_since(se).as_secs_f64() * 1000.0
    } else {
        0.0
    };

    let base_out = args
        .output_dir
        .unwrap_or_else(|| PathBuf::from("benches/results/pipeline_bench"));
    let run_id = generate_run_id();
    let run_dir = base_out.join(&run_id);
    let wav_dir = run_dir.join("wav");
    let _ = std::fs::create_dir_all(&wav_dir);

    let wav_path = wav_dir.join(format!("{}_{}_{}.wav", args.mode, args.llm, args.tts));
    let _ = write_wav_f32(&wav_path, &audio_24k, 24000);

    let captured_llm_response = {
        let acc = state.pipeline_accumulator.lock();
        if !acc.assistant_response.is_empty() {
            acc.assistant_response.clone()
        } else {
            let mgr = state.conversation_manager.lock();
            mgr.get_messages()
                .iter()
                .rev()
                .find(|m| m.role == vox_lib::services::harness::Role::Assistant)
                .map(|m| m.content.clone())
                .unwrap_or_default()
        }
    };

    println!("\n================================================================================");
    println!(
        "Pipeline Benchmark Summary: {} + {} + {}",
        args.stt, args.llm, args.tts
    );
    println!("================================================================================");
    println!("  [1. STT Transcript]     : \"{}\"", captured_transcript);
    println!("  [2. LLM Response]       : \"{}\"", captured_llm_response);
    println!("  [3. Synthesized Audio]  : {:?}", wav_path);
    println!("--------------------------------------------------------------------------------");
    println!("  Input Speech Duration   : {:.2} s", duration_s);
    println!(
        "  Output Synthesized Audio: {:.2} s (24 kHz)",
        synth_duration_s
    );
    println!("  STT Post-Speech Latency : {:.1} ms", stt_latency_ms);
    println!(
        "  Perceived E2E Latency   : {:.1} ms (SpeechEnd -> PlaybackStarted)",
        e2e_response_time_ms
    );
    println!("  Total Pipeline Elapsed  : {:.2} s", total_elapsed);
    println!("  Memory RSS              : ~{} MB", mem_after);

    let result = ClipBenchmarkResult {
        filename: args.clip.clone(),
        lang: "EN".to_string(),
        duration_s,
        total_stream_time_ms: total_elapsed * 1000.0,
        final_post_speech_latency_ms: e2e_response_time_ms,
        rtf: total_elapsed / (duration_s as f64),
        throughput_spl_s: audio_24k.len() as f64 / total_elapsed,
        partials_emitted: 0,
        similarity: 1.0,
        hypothesis: format!(
            "STT=\"{}\" | E2E={:.1}ms | SynthAudio={:.2}s | WAV={:?}",
            captured_transcript, e2e_response_time_ms, synth_duration_s, wav_path
        ),
        ground_truth: "Canonical input clip briefing".to_string(),
    };

    let report = BenchmarkReport {
        run_id: run_id.clone(),
        timestamp_utc: chrono::Utc::now().to_rfc3339(),
        benchmark_name: "pipeline_bench".to_string(),
        system_info: BenchmarkSystemInfo::default(),
        runs: vec![EngineBenchmarkRun {
            engine_name: format!(
                "Pipeline [{}] ({}+{}+{})",
                args.mode, args.stt, args.llm, args.tts
            ),
            model_type: format!("{}_{}_{}", args.mode, args.llm, args.tts),
            model_path: "live_pipeline".to_string(),
            memory_rss_mb: mem_after,
            total_audio_s: synth_duration_s,
            total_stream_time_ms: total_elapsed * 1000.0,
            avg_post_speech_latency_ms: e2e_response_time_ms,
            avg_rtf: total_elapsed / (duration_s as f64),
            overall_throughput_spl_s: audio_24k.len() as f64 / total_elapsed,
            avg_similarity: 1.0,
            clips: vec![result],
        }],
    };

    if let Ok(path) = save_benchmark_report(&base_out, &report) {
        println!("  Benchmark Report Saved  : {:?}", path);
        println!(
            "  Latest Report Symlink   : {:?}",
            base_out.join("latest.json")
        );
    }
}
