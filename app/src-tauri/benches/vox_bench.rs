//! ============================================================================
//! vox_bench.rs — Production-Parity Async Voice Pipeline Benchmark
//! ============================================================================
//! Category     : Benchmark (Audio Pipeline Benchmark Target)
//! Component    : Audio Pipeline (VAD -> STT -> LLM -> TTS)
//! Prerequisites: 16kHz mono WAV input file
//! Execution    : cargo test --bench vox_bench
//! ============================================================================

use clap::Parser;
use std::sync::atomic::AtomicBool;
use std::sync::mpsc::channel;
use std::sync::Arc;
use std::time::Duration;

use vox_lib::core::events::VoxEvent;
use vox_lib::core::metrics::{MetricField, PipelineMetrics};
use vox_lib::core::settings::SttProviderConfig;
use vox_lib::core::state::InteractionOwner;
use vox_lib::services::llm::{EmbeddedProvider, LlmProvider, OpenAiCompatProvider};
use vox_lib::services::stt::providers::{create_stt_provider, SttProvider};
use vox_lib::services::utils::{count_words, is_devanagari, should_flush, transliterate_if_hi};
use vox_lib::services::vad::ten_onnx::VadEngine;
use vox_lib::services::vad::VadEngine as _;
use vox_lib::utils::bench_reporter::BenchReporter;

#[derive(Parser, Debug)]
#[command(
    name = "vox-bench",
    about = "Production-parity async benchmark for Vox"
)]
struct Args {
    /// Path to input WAV file (16kHz mono)
    #[arg(short, long, default_value = "test-clips/short_en.wav")]
    input: String,

    /// System prompt (overrides constants)
    #[arg(short, long)]
    prompt: Option<String>,

    /// Number of concurrent turns to simulate (1 for now)
    #[arg(short, long, default_value = "1")]
    turns: usize,

    /// Name of the LLM GGUF model file inside the gemma4 directory (defaults to E2B)
    #[arg(short, long)]
    llm: Option<String>,

    /// STT engine to use: nemotron (default) or qwen
    #[arg(short, long, default_value = "nemotron")]
    asr: String,

    /// Prefix for output run directory (e.g. 'q6' → outputs/q6_run_20260609_...)
    #[arg(short, long)]
    output: Option<String>,

    /// LLM provider: embedded (default) or openai_compat
    #[arg(long, default_value = "embedded")]
    llm_provider: String,

    /// Remote LLM base URL for openai_compat provider
    #[arg(long, default_value = "http://100.86.62.14:11434")]
    llm_url: String,

    /// Remote LLM model name for openai_compat provider
    #[arg(long, default_value = "llama3.1:8b-instruct-q4_K_M")]
    llm_model: String,

    /// TTS engine: edge_tts (default), supertonic, or chatterbox
    #[arg(long, default_value = "edge_tts")]
    tts: String,

    /// Reference audio file path, voice ID, or voice name for Chatterbox voice cloning
    #[arg(long, alias = "voice")]
    tts_voice: Option<String>,
}

enum BenchCommand {
    SttPartial(u32, Vec<f32>),
    SttFinal(u32, Vec<f32>),
    Llm(String, String),
    Tts(String, u32),
    Shutdown,
}

fn main() -> anyhow::Result<()> {
    // CPU governor check: abort if not 'performance' (results would be misleading)
    #[cfg(target_os = "linux")]
    if let Some(governor) = vox_lib::utils::check_cpu_governor() {
        if governor != "performance" {
            eprintln!(
                "\x1b[33m[WARNING]\x1b[0m CPU governor is '{}', not 'performance'.\n\
                 Running benchmarks with a non-performance governor might produce slightly slower results.\n\
                 Switch to performance governor if you want official production numbers:\n\
                 \x1b[33m  echo performance | sudo tee /sys/devices/system/cpu/cpu*/cpufreq/scaling_governor\x1b[0m",
                governor
            );
            // std::process::exit(1);
        }
    }
    let args = Args::parse();
    let reporter = BenchReporter::new_with_prefix(args.output.as_deref());
    let metrics = Arc::new(std::sync::Mutex::new(PipelineMetrics::new()));

    // 1. Setup paths & Hardware init simulation
    let home = dirs::home_dir().expect("Could not find home directory");
    let vox_root = home.join(".vox");
    vox_lib::utils::paths::init_with_root(vox_root);

    println!("\x1b[32m[Bench]\x1b[0m Starting Production-Parity Run...");
    println!("\x1b[32m[Bench]\x1b[0m Run dir: {:?}", reporter.run_dir);

    // 2. Initialize Channels (Production-like Actor pattern)
    let (event_tx, event_rx) = channel::<VoxEvent>();
    let (stt_tx, stt_rx) = channel::<BenchCommand>();
    let (llm_tx, llm_rx) = channel::<BenchCommand>();
    let (tts_tx, tts_rx) = channel::<BenchCommand>();

    let cancel_flag = Arc::new(AtomicBool::new(false));
    // Default prompt for KV cache warmup (English — no text to detect language from yet)
    let system_prompt = args.prompt.unwrap_or_else(|| {
        vox_lib::core::constants::SYSTEM_PROMPT_MODULAR
            .replace("<lang>", "English")
            .replace("<script>", "Latin")
    });

    // 3. Load Models Sequentially (to avoid ONNX environment conflicts and improve memory tracking)
    println!("\x1b[32m[Bench]\x1b[0m Loading STT ({})...", args.asr);
    let stt_path = if args.asr == "nemotron" {
        vox_lib::utils::paths::get()
            .models
            .join(vox_lib::services::stt::MODEL_DIR_STT_NEMOTRON)
    } else {
        vox_lib::utils::paths::get()
            .models
            .join(vox_lib::services::stt::MODEL_DIR_STT_QWEN)
    };

    let snap_1 = BenchReporter::get_memory_snapshot();
    let stt_provider_config = SttProviderConfig::Embedded {
        model_type: args.asr.clone(),
    };
    let stt_provider: Box<dyn SttProvider> =
        create_stt_provider(&stt_provider_config, &stt_path).expect("Failed to load STT provider");
    let snap_2 = BenchReporter::get_memory_snapshot();
    let stt_mem_mb = snap_2.rss_mb.saturating_sub(snap_1.rss_mb);
    metrics.lock().unwrap().stt_mem_mb = stt_mem_mb;

    let snap_3 = BenchReporter::get_memory_snapshot();
    let llm_engine: Box<dyn LlmProvider> = if args.llm_provider == "openai_compat" {
        println!(
            "\x1b[32m[Bench]\x1b[0m Using OpenAiCompat provider at {} with model {}...",
            args.llm_url, args.llm_model
        );
        Box::new(OpenAiCompatProvider::new(
            &args.llm_url,
            &args.llm_model,
            None,
            None,
        ))
    } else {
        let llm_filename = if let Some(custom) = args.llm.clone() {
            custom
        } else {
            let candidate_paths = [
                "qwen/qwen-3.5-0.8b-q4_k_m.gguf",
                "llama/llama-3.2-1b-q4_k_m.gguf",
                "llama/Llama-3.2-1B-Instruct-Q4_K_M.gguf",
                "gemma4/gemma-4-e2b-q4_k_m.gguf",
            ];
            let mut chosen = "qwen/qwen-3.5-0.8b-q4_k_m.gguf".to_string();
            for cand in &candidate_paths {
                if vox_lib::utils::paths::model_dir("llm").join(cand).exists() {
                    chosen = cand.to_string();
                    break;
                }
            }
            chosen
        };
        println!(
            "\x1b[32m[Bench]\x1b[0m Loading Embedded LLM ({})...",
            llm_filename
        );
        let total_cores = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(4);
        let llm_threads = (total_cores.saturating_sub(2)).clamp(2, 4) as u32;
        println!(
            "\x1b[32m[Bench]\x1b[0m System cores: {}, LLM threads: {}",
            total_cores, llm_threads
        );
        let llm_path = vox_lib::utils::paths::model_dir("llm").join(&llm_filename);
        Box::new(EmbeddedProvider::new(&llm_path, 2048, llm_threads).expect("Failed to load LLM"))
    };
    let snap_4 = BenchReporter::get_memory_snapshot();
    let llm_mem_mb = snap_4.rss_mb.saturating_sub(snap_3.rss_mb);
    metrics.lock().unwrap().llm_mem_mb = llm_mem_mb;

    println!("\x1b[32m[Bench]\x1b[0m Warming up LLM provider...");
    let warmup_cancel = Arc::new(AtomicBool::new(false));
    let (warmup_tx, _) = channel();
    let warmup_start = std::time::Instant::now();

    let bench_rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("Failed to create tokio runtime for bench");

    let warmup_policy = vox_lib::services::llm::policy::GenerationPolicy::from_settings(
        &vox_lib::core::settings::LlmSettings::default(),
    );
    let warmup_req = warmup_policy.build_request(
        vox_lib::services::llm::types::GenerationPurpose::Conversation,
        vox_lib::services::llm::types::ConversationInput {
            messages: vec![vox_lib::services::memory::ChatMessage {
                role: vox_lib::services::memory::Role::System,
                content: system_prompt.clone(),
                timestamp_ms: 0,
            }],
        },
    );

    let _ = bench_rt.block_on(llm_engine.generate(warmup_req, 0, &warmup_cancel, &warmup_tx));
    println!(
        "\x1b[32m[Bench]\x1b[0m LLM provider warmed up in {:?}",
        warmup_start.elapsed()
    );

    let snap_5 = BenchReporter::get_memory_snapshot();
    let tts_engine: Box<dyn vox_lib::services::tts::providers::TtsProvider> =
        if args.tts == "chatterbox_remote" {
            println!("\x1b[32m[Bench]\x1b[0m Loading TTS (ChatterboxRemote)...");
            Box::new(
                vox_lib::services::tts::ChatterboxRemoteProvider::new(
                    "http://100.86.62.14:7860",
                    "en",
                    10,
                    1.0,
                    "/home/hypr4/.vox",
                )
                .expect("Failed to load ChatterboxRemote TTS"),
            )
        } else if args.tts == "chatterbox" {
            println!("\x1b[32m[Bench]\x1b[0m Loading TTS (Chatterbox)...");
            let cb_path = vox_lib::utils::paths::model_dir("tts").join("chatterbox");
            let resolved_voice = args.tts_voice.as_ref().map(|v| resolve_voice_path(v));
            Box::new(
                vox_lib::services::tts::ChatterboxEngine::new(
                    &cb_path,
                    "en",
                    10,
                    1.0,
                    resolved_voice.as_deref(),
                )
                .expect("Failed to load Chatterbox TTS"),
            )
        } else if args.tts == "edge_tts" || args.tts == "edge-tts" || args.tts == "edge" {
            println!("\x1b[32m[Bench]\x1b[0m Loading TTS (Edge TTS)...");
            Box::new(vox_lib::services::tts::EdgeTtsProvider::new(
                args.tts_voice.as_deref(),
            ))
        } else {
            println!("\x1b[32m[Bench]\x1b[0m Loading TTS (Supertonic 3)...");
            let super_tts_path = vox_lib::utils::paths::model_dir("tts").join("supertonic-3");
            Box::new(
                vox_lib::services::tts::TtsEngine::new(&super_tts_path, 0, 12, 1.05)
                    .expect("Failed to load TTS"),
            )
        };
    let snap_6 = BenchReporter::get_memory_snapshot();
    let tts_mem_mb = snap_6.rss_mb.saturating_sub(snap_5.rss_mb);
    metrics.lock().unwrap().tts_mem_mb = tts_mem_mb;

    println!("\x1b[32m[Bench]\x1b[0m Loading Transliteration Engine (Native RNN)...");
    vox_lib::services::translit::init_transliteration_engine()
        .expect("Failed to load Transliteration Engine");

    // 4. Spawn Dedicated Worker Threads

    // STT Worker
    let stt_event_tx = event_tx.clone();
    let stt_handle = std::thread::spawn(move || {
        let provider = stt_provider; // Move initialized provider
        let mut last_transcript = String::new();

        while let Ok(cmd) = stt_rx.recv() {
            match cmd {
                BenchCommand::SttPartial(tid, samples) => {
                    match provider.transcribe_chunk(&samples, false) {
                        Ok(text) => {
                            if !text.is_empty() && text != last_transcript {
                                let _ = stt_event_tx.send(VoxEvent::TranscriptPartial {
                                    turn_id: tid,
                                    owner: InteractionOwner::MainWindow,
                                    text: text.clone(),
                                });
                                last_transcript = text;
                            }
                        }
                        Err(e) => eprintln!("[BenchSTT] Partial error: {}", e),
                    }
                }
                BenchCommand::SttFinal(tid, samples) => {
                    let text = match provider.transcribe_chunk(&samples, true) {
                        Ok(t) => t,
                        Err(e) => {
                            eprintln!("[BenchSTT] Final error: {}", e);
                            String::new()
                        }
                    };
                    let _ = provider.reset_state();

                    if !text.is_empty() {
                        let _ = stt_event_tx.send(VoxEvent::TranscriptFinal {
                            turn_id: tid,
                            owner: InteractionOwner::MainWindow,
                            text,
                        });
                    }

                    last_transcript.clear();
                }
                BenchCommand::Shutdown => break,
                _ => {}
            }
        }
    });

    // LLM Worker
    let llm_event_tx = event_tx.clone();
    let llm_cancel = Arc::clone(&cancel_flag);
    let llm_handle = std::thread::spawn(move || {
        let engine = llm_engine; // Move initialized engine
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("Failed to create tokio runtime for LLM bench worker");

        while let Ok(cmd) = llm_rx.recv() {
            match cmd {
                BenchCommand::Llm(text, prompt) => {
                    println!("\x1b[34m[LLM]\x1b[0m Starting generation for: \"{}\"", text);
                    let policy = vox_lib::services::llm::policy::GenerationPolicy::from_settings(
                        &vox_lib::core::settings::LlmSettings::default(),
                    );
                    let req = policy.build_request(
                        vox_lib::services::llm::types::GenerationPurpose::Conversation,
                        vox_lib::services::llm::types::ConversationInput {
                            messages: vec![
                                vox_lib::services::memory::ChatMessage {
                                    role: vox_lib::services::memory::Role::System,
                                    content: prompt,
                                    timestamp_ms: 0,
                                },
                                vox_lib::services::memory::ChatMessage {
                                    role: vox_lib::services::memory::Role::User,
                                    content: text,
                                    timestamp_ms: 0,
                                },
                            ],
                        },
                    );
                    let _ = rt.block_on(engine.generate(req, 1, &llm_cancel, &llm_event_tx));
                }
                BenchCommand::Shutdown => break,
                _ => {}
            }
        }
    });

    // TTS Worker
    let tts_event_tx = event_tx.clone();
    let tts_cancel = Arc::clone(&cancel_flag);
    let tts_handle = std::thread::spawn(move || {
        let engine = tts_engine; // Move initialized engine
        while let Ok(cmd) = tts_rx.recv() {
            match cmd {
                BenchCommand::Tts(text, turn_id) => {
                    println!("\x1b[35m[TTS]\x1b[0m Synthesizing chunk: \"{}\"", text);
                    let _ = engine.synthesize_chunk(
                        &text,
                        turn_id,
                        Arc::clone(&tts_cancel),
                        tts_event_tx.clone(),
                    );
                }
                BenchCommand::Shutdown => break,
                _ => {}
            }
        }
    });

    // 4. Start Streaming Ingestion (Production simulation)
    let input_path = std::path::Path::new(&args.input);
    let resolved_input = if input_path.exists() {
        input_path.to_path_buf()
    } else {
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(&args.input)
    };

    let mut reader = hound::WavReader::open(&resolved_input)
        .unwrap_or_else(|e| panic!("Failed to open WAV file {:?}: {}", resolved_input, e));
    let spec = reader.spec();
    println!(
        "\x1b[32m[Bench]\x1b[0m Ingesting WAV {:?} ({} channels, {} Hz, {:?})...",
        resolved_input, spec.channels, spec.sample_rate, spec.sample_format
    );

    let raw_samples: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Float => reader.samples::<f32>().filter_map(|s| s.ok()).collect(),
        hound::SampleFormat::Int => match spec.bits_per_sample {
            16 => reader
                .samples::<i16>()
                .filter_map(|s| s.ok())
                .map(|s| s as f32 / 32768.0)
                .collect(),
            32 => reader
                .samples::<i32>()
                .filter_map(|s| s.ok())
                .map(|s| s as f32 / 2147483648.0)
                .collect(),
            _ => {
                let max_val = (2u64.pow(spec.bits_per_sample as u32) / 2 - 1) as f64;
                reader
                    .samples::<i32>()
                    .filter_map(|s| s.ok())
                    .map(|s| (s as f64 / max_val) as f32)
                    .collect()
            }
        },
    };

    let all_samples: Vec<f32> = if spec.channels > 1 {
        raw_samples
            .chunks(spec.channels as usize)
            .map(|chunk| chunk.iter().sum::<f32>() / chunk.len() as f32)
            .collect()
    } else {
        raw_samples
    };

    let vad_path = vox_lib::utils::paths::model_dir("vad").join("ten_vad.onnx");
    let mut vad = VadEngine::new(&vad_path, 0.5).expect("Failed to load VAD");
    let mut utterance_buf = Vec::new();
    let mut in_speech = false;
    let mut turn_id = 0;

    let input_duration = all_samples.len() as f64 / 16000.0;
    let max_amp = all_samples.iter().map(|s| s.abs()).fold(0.0f32, f32::max);
    println!(
        "\x1b[32m[Bench]\x1b[0m Loaded {} audio samples ({:.2}s duration). Max amplitude: {:.4}",
        all_samples.len(),
        input_duration,
        max_amp
    );

    // Spawn memory tracker for PEAK RSS (total process)
    let mem_cancel = Arc::clone(&cancel_flag);
    let mem_handle = std::thread::spawn(move || {
        let mut max_rss = 0;
        while !mem_cancel.load(std::sync::atomic::Ordering::Relaxed) {
            let snap = BenchReporter::get_memory_snapshot();
            if snap.rss_mb > max_rss {
                max_rss = snap.rss_mb;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        max_rss
    });

    metrics.lock().unwrap().mark(MetricField::SpeechStart);

    // Simulate 20ms chunks (320 samples at 16kHz)
    for chunk in all_samples.chunks(320) {
        if chunk.len() < 320 {
            break;
        }

        let detected = vad.predict(chunk);
        if detected {
            if !in_speech {
                in_speech = true;
                turn_id += 1;
                utterance_buf.clear();
            }
            utterance_buf.extend_from_slice(chunk);

            // Periodically send partials (every 500ms)
            if utterance_buf.len() % 8000 == 0 {
                stt_tx.send(BenchCommand::SttPartial(turn_id, utterance_buf.clone()))?;
            }
        } else if in_speech {
            in_speech = false;
            stt_tx.send(BenchCommand::SttFinal(turn_id, utterance_buf.clone()))?;
            utterance_buf.clear();
        }
    }
    // Final flush if still in speech
    if in_speech {
        stt_tx.send(BenchCommand::SttFinal(turn_id, utterance_buf.clone()))?;
    }

    // 5. Production Pipeline Orchestration (Event Loop)
    let mut assistant_text = String::new();
    let mut token_buf = String::new();
    let mut tts_samples = Vec::new();
    let mut final_transcript = String::new();
    let mut tts_finished_count = 0;
    let mut tts_started_count = 0;
    let mut tokens_generated = 0;
    let mut llm_done = false;
    let mut last_tts_flush = std::time::Instant::now();
    let mut first_token_time: Option<std::time::Instant> = None;

    while !llm_done || tts_finished_count < tts_started_count {
        match event_rx.recv_timeout(Duration::from_secs(120)) {
            Ok(event) => match event {
                VoxEvent::TranscriptPartial { text, .. } => {
                    println!("\x1b[30m[STT]\x1b[0m Partial: \"{}\"", text);
                }
                VoxEvent::TranscriptFinal { text, .. } => {
                    final_transcript = text.clone();
                    let mut m_lock = metrics.lock().unwrap();
                    m_lock.mark(MetricField::FinalTranscript);
                    m_lock.mark(MetricField::LlmStart);
                    m_lock.input_len_chars = text.len();
                    println!("\x1b[34m[STT]\x1b[0m Final: \"{}\"", text);

                    // Language detection: route to Hindi or English prompt (mirrors real pipeline)
                    let base_prompt = if is_devanagari(&text) {
                        vox_lib::core::constants::SYSTEM_PROMPT_MODULAR
                            .replace("<lang>", "Hindi")
                            .replace("<script>", "Devanagari")
                    } else {
                        vox_lib::core::constants::SYSTEM_PROMPT_MODULAR
                            .replace("<lang>", "English")
                            .replace("<script>", "Latin")
                    };
                    // Inject expression tag instructions (Supertonic supports <laugh>, <breath>, <sigh>)
                    let prompt = format!(
                        "{} You may use <laugh>, <breath>, <sigh> tags for expressive speech.",
                        base_prompt
                    );

                    llm_tx.send(BenchCommand::Llm(text, prompt))?;
                    first_token_time = None;
                    tokens_generated = 0;
                }
                VoxEvent::LlmToken { token, .. } => {
                    let mut m_lock = metrics.lock().unwrap();
                    if m_lock.first_token.is_none() {
                        m_lock.mark(MetricField::FirstToken);
                    }
                    print!("{}", token);
                    use std::io::Write;
                    std::io::stdout().flush().unwrap();

                    assistant_text.push_str(&token);
                    token_buf.push_str(&token);
                    tokens_generated += 1;

                    let first_time = first_token_time.get_or_insert_with(std::time::Instant::now);
                    let elapsed_secs = first_time.elapsed().as_secs_f32();
                    let tps = if elapsed_secs > 0.5 {
                        tokens_generated as f32 / elapsed_secs
                    } else {
                        3.5
                    };

                    let wc = count_words(&token_buf);
                    let elapsed_ms = last_tts_flush.elapsed().as_millis();
                    if should_flush(&token_buf, wc, elapsed_ms, tps) {
                        let chunk = token_buf.trim().to_string();
                        if !chunk.is_empty() {
                            m_lock.mark(MetricField::TtsStart);
                            tts_started_count += 1;
                            tts_tx.send(BenchCommand::Tts(chunk, 1))?;
                            token_buf.clear();
                            last_tts_flush = std::time::Instant::now();
                        }
                    }
                }
                VoxEvent::LlmFinished { .. } => {
                    println!("\n\x1b[34m[LLM]\x1b[0m Response complete.");
                    let mut m_lock = metrics.lock().unwrap();
                    m_lock.mark(MetricField::LlmEnd);
                    m_lock.output_len_chars = assistant_text.len();
                    m_lock.tokens_generated = tokens_generated;

                    let remainder = token_buf.trim().to_string();
                    if !remainder.is_empty() {
                        m_lock.mark(MetricField::TtsStart);
                        tts_started_count += 1;
                        tts_tx.send(BenchCommand::Tts(remainder, 1))?;
                        last_tts_flush = std::time::Instant::now();
                    }
                    llm_done = true;
                }
                VoxEvent::TtsChunk { samples, .. } => {
                    let mut m_lock = metrics.lock().unwrap();
                    if m_lock.first_audio.is_none() {
                        m_lock.mark(MetricField::FirstAudio);
                        m_lock.mark(MetricField::PlaybackStart);
                        println!("\x1b[35m[TTS]\x1b[0m First audio generated!");
                    }
                    tts_samples.extend(samples);
                }
                VoxEvent::TtsFinished { .. } => {
                    tts_finished_count += 1;
                    println!(
                        "\x1b[35m[TTS]\x1b[0m Chunk {}/{} complete.",
                        tts_finished_count, tts_started_count
                    );
                    if tts_finished_count == tts_started_count && llm_done {
                        metrics.lock().unwrap().mark(MetricField::TtsEnd);
                    }
                }
                VoxEvent::Error { message, .. } => {
                    println!("\x1b[31m[Pipeline Error]\x1b[0m {}", message);
                }
                _ => {}
            },
            Err(e) => {
                println!(
                    "\x1b[31m[Bench Event Loop]\x1b[0m recv_timeout error: {:?}",
                    e
                );
                break;
            }
        }
    }

    // 6. Artifact Collection & Formal Reporting
    stt_tx.send(BenchCommand::Shutdown)?;
    llm_tx.send(BenchCommand::Shutdown)?;
    tts_tx.send(BenchCommand::Shutdown)?;

    let _ = stt_handle.join();
    let _ = llm_handle.join();
    let _ = tts_handle.join();

    cancel_flag.store(true, std::sync::atomic::Ordering::Relaxed);
    let peak_rss_mb = mem_handle.join().unwrap_or(0);

    let mut m = metrics.lock().unwrap();
    m.mark(MetricField::PlaybackFinish);

    let output_duration = tts_samples.len() as f64 / 24000.0;
    let mut report = m.latency_report(
        input_duration,
        output_duration,
        vox_lib::core::settings::PipelineMode::Modular,
        false,
    );

    if let Some(obj) = report.as_object_mut() {
        if let Some(perf) = obj.get_mut("memory_mb").and_then(|v| v.as_object_mut()) {
            perf.insert(
                "peak_process_rss_mb".to_string(),
                serde_json::json!(peak_rss_mb),
            );
        }
    }

    // Write detailed artifacts
    reporter.write_artifact("stt_transcript.txt", &final_transcript);
    reporter.write_artifact("llm_response.txt", &assistant_text);
    reporter.write_artifact(
        "transliteration.txt",
        &format!(
            "STT: {}\nLLM: {}",
            transliterate_if_hi(&final_transcript, true, true),
            transliterate_if_hi(&assistant_text, true, true)
        ),
    );

    // Export result audio
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: 24000,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let wav_path = reporter.run_dir.join("output_tts.wav");
    let mut writer = hound::WavWriter::create(&wav_path, spec)?;
    for &sample in &tts_samples {
        writer.write_sample((sample * 32767.0) as i16)?;
    }
    writer.finalize()?;

    // Final Memory & Latency report
    println!(
        "\n\x1b[32m[Bench]\x1b[0m {}",
        report["summary"].as_str().unwrap_or("")
    );
    println!(
        "\x1b[32m[Bench]\x1b[0m STT RAM: {}MB | LLM RAM: {}MB | TTS RAM: {}MB",
        m.stt_mem_mb, m.llm_mem_mb, m.tts_mem_mb
    );
    println!("\x1b[32m[Bench]\x1b[0m Peak Memory RSS: {}MB", peak_rss_mb);

    reporter.save_report(report);
    println!(
        "\x1b[32m[Bench]\x1b[0m All artifacts saved to: {:?}",
        reporter.run_dir
    );

    Ok(())
}

fn resolve_voice_path(voice_arg: &str) -> String {
    if std::path::Path::new(voice_arg).exists() {
        return voice_arg.to_string();
    }
    // Try to open SQLite DB and find matching voice by ID or name
    let db_path = vox_lib::utils::paths::db_path();
    if let Ok(rt) = tokio::runtime::Runtime::new() {
        let res = rt.block_on(async {
            let conn = vox_lib::persistence::db::VoxDb::open_readonly(&db_path)
                .await
                .ok()?;
            let mut rows = conn
                .query(
                    "SELECT voice_dir, wav_path FROM voices WHERE id = ? OR name = ?",
                    (voice_arg.to_string(), voice_arg.to_string()),
                )
                .await
                .ok()?;
            if let Some(row) = rows.next().await.ok()? {
                let voice_dir: Option<String> = row.get(0).ok();
                let wav_path: Option<String> = row.get(1).ok();
                voice_dir.or(wav_path)
            } else {
                None
            }
        });
        if let Some(path) = res {
            if std::path::Path::new(&path).exists() {
                println!(
                    "\x1b[32m[Bench]\x1b[0m Resolved voice '{}' to {:?}",
                    voice_arg, path
                );
                return path;
            }
        }
    }
    // Fallback: search in ~/.vox/voices/
    let home = dirs::home_dir().unwrap_or_default();
    let voices_dir = home.join(".vox").join("voices").join(voice_arg);
    if voices_dir.join("baked").exists() {
        return voices_dir.join("baked").to_string_lossy().into_owned();
    }
    if voices_dir.join("source.wav").exists() {
        return voices_dir.join("source.wav").to_string_lossy().into_owned();
    }

    voice_arg.to_string()
}
