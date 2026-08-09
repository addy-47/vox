//! ============================================================================
//! vox_realtime_bench.rs — Realtime Voice Pipeline End-to-End Latency Benchmark
//! ============================================================================
//! Category     : Benchmark
//! Component    : Realtime S2S & Voice Provider Pipeline (`vox_lib::services::realtime`)
//! Prerequisites: Active network or local provider credentials
//! Execution    : cargo test --bench vox_realtime_bench
//! ============================================================================

use clap::Parser;
use hound::WavReader;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::sync::mpsc;

use vox_lib::core::events::VoxEvent;
use vox_lib::core::settings::{
    DeepgramVoiceAgentConfig, GeminiRealtimeConfig, InteractionMode, VoxSettings,
};
use vox_lib::services::realtime::{
    providers::deepgram_live::DeepgramVoiceAgentProvider,
    providers::gemini_live::GeminiLiveProvider, RealtimeVoiceProvider,
};

#[derive(Parser)]
#[command(
    name = "vox_realtime_bench",
    about = "Vox Realtime S2S Cloud Speech-to-Speech benchmark"
)]
struct Cli {
    #[arg(
        long,
        help = "Path to input Hindi WAV file. If omitted, resolves temp/data/test_hi.wav"
    )]
    hindi_wav: Option<String>,

    #[arg(
        long,
        help = "Path to input English WAV file. If omitted, picks first WAV in temp/data/benchmark_clips/"
    )]
    english_wav: Option<String>,

    #[arg(
        short,
        long,
        default_value = "passive",
        help = "Interaction mode: 'passive' (server-side VAD) or 'ptt' (push-to-talk)"
    )]
    mode: String,

    #[arg(short, long, help = "Trigger barge-in interruption test")]
    barge_in: bool,

    #[arg(
        short,
        long,
        default_value = "gemini",
        help = "S2S Provider to benchmark: 'gemini' or 'deepgram'"
    )]
    provider: String,
}

fn load_api_key(provider: &str) -> Option<String> {
    let env_var = if provider == "deepgram" {
        "DEEPGRAM_API_KEY"
    } else {
        "GEMINI_API_KEY"
    };
    if let Ok(key) = std::env::var(env_var) {
        return Some(key);
    }
    let paths = [
        "temp/.env",
        "../temp/.env",
        "../../temp/.env",
        "/home/addy/projects/apps/vox/temp/.env",
    ];
    for path in &paths {
        if let Ok(content) = fs::read_to_string(path) {
            for line in content.lines() {
                if line.starts_with(&format!("{}=", env_var)) {
                    let parts: Vec<&str> = line.splitn(2, '=').collect();
                    if parts.len() == 2 {
                        let val = parts[1].trim().trim_matches('"').trim_matches('\'');
                        return Some(val.to_string());
                    }
                }
            }
        }
    }
    None
}

fn resolve_hindi_wav(cli_wav: Option<&str>) -> PathBuf {
    if let Some(p) = cli_wav {
        return PathBuf::from(p);
    }
    let paths = [
        "temp/data/test_hi.wav",
        "../temp/data/test_hi.wav",
        "../../temp/data/test_hi.wav",
        "/home/addy/projects/apps/vox/temp/data/test_hi.wav",
    ];
    for p in &paths {
        if Path::new(p).exists() {
            return PathBuf::from(p);
        }
    }
    PathBuf::from("../../temp/data/test_hi.wav") // Fallback
}

fn resolve_english_wav(cli_wav: Option<&str>) -> PathBuf {
    if let Some(p) = cli_wav {
        return PathBuf::from(p);
    }
    let dirs = [
        "temp/data/benchmark_clips",
        "../temp/data/benchmark_clips",
        "../../temp/data/benchmark_clips",
        "/home/addy/projects/apps/vox/temp/data/benchmark_clips",
    ];
    for dir in &dirs {
        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|s| s.to_str()) == Some("wav") {
                    return path;
                }
            }
        }
    }
    PathBuf::from("../../temp/data/benchmark_clips/hiacc_children_test_CH24036.wav")
    // Fallback
}

struct BenchmarkMetrics {
    handshake_duration: Duration,
    ttft_input: Option<Duration>,
    ttft_output: Option<Duration>,
    ttfa: Option<Duration>,
    first_input_text: Option<String>,
    first_output_text: Option<String>,
    audio_chunks_received: usize,
    total_audio_bytes: usize,
    interrupted: bool,
}

async fn run_benchmark_for_clip(
    provider_name: &str,
    api_key: &str,
    wav_path: &Path,
    interaction_mode: InteractionMode,
    barge_in: bool,
    system_prompt: String,
    output_wav_path: &Path,
) -> anyhow::Result<BenchmarkMetrics> {
    // Read WAV file samples
    let mut reader = WavReader::open(wav_path)?;
    let _spec = reader.spec();
    let samples: Vec<i16> = reader.samples::<i16>().flatten().collect();

    let provider: Box<dyn RealtimeVoiceProvider> = if provider_name == "deepgram" {
        let config = DeepgramVoiceAgentConfig {
            api_key: api_key.to_string(),
            model: "gpt-4o-mini".to_string(),
            voice: "Aoede".to_string(),
            temperature: 0.7,
            agent_mode: false,
        };
        Box::new(DeepgramVoiceAgentProvider::new(
            config,
            system_prompt,
            std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
        ))
    } else {
        let config = GeminiRealtimeConfig {
            api_key: api_key.to_string(),
            model: "gemini-3.1-flash-live-preview".to_string(),
            voice_name: "Aoede".to_string(),
            language_code: "en-US".to_string(),
            temperature: 0.2,
            enable_web_search: false,
            resume_handle: None,
        };
        Box::new(GeminiLiveProvider::new(
            config,
            system_prompt,
            std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
        ))
    };
    let (playback_tx, mut playback_rx) = mpsc::channel::<Vec<i16>>(100);
    let (event_tx, event_rx) = std::sync::mpsc::channel::<VoxEvent>();

    let connect_start = Instant::now();
    let session = provider.connect(interaction_mode.clone(), playback_tx, event_tx)?;
    let handshake_duration = connect_start.elapsed();

    // Shared metrics tracking structure
    struct SharedMetrics {
        ttft_input: Option<Duration>,
        ttft_output: Option<Duration>,
        ttfa: Option<Duration>,
        first_input_text: Option<String>,
        first_output_text: Option<String>,
        audio_chunks_received: usize,
        total_audio_bytes: usize,
        last_audio_received: Option<Instant>,
        interrupted: bool,
        output_audio: Vec<i16>,
    }

    let metrics = Arc::new(Mutex::new(SharedMetrics {
        ttft_input: None,
        ttft_output: None,
        ttfa: None,
        first_input_text: None,
        first_output_text: None,
        audio_chunks_received: 0,
        total_audio_bytes: 0,
        last_audio_received: None,
        interrupted: false,
        output_audio: Vec::new(),
    }));

    let metrics_clone = metrics.clone();
    let speech_end_tx = Arc::new(Mutex::new(None::<Instant>));
    let speech_end_rx = speech_end_tx.clone();

    // Spawn task to process events from the server
    let event_handler = tokio::spawn(async move {
        tokio::task::spawn_blocking(move || {
            while let Ok(evt) = event_rx.recv() {
                let now = Instant::now();
                let end_time_opt = *speech_end_rx.lock().unwrap();

                match evt {
                    VoxEvent::TranscriptFinal { text, .. } => {
                        println!(
                            "      \x1b[36m[Event] ASR Transcript (User): {:?}\x1b[0m",
                            text
                        );
                        let mut m = metrics_clone.lock().unwrap();
                        if m.first_input_text.is_none() {
                            m.first_input_text = Some(text.clone());
                            if let Some(end_time) = end_time_opt {
                                m.ttft_input = Some(now.duration_since(end_time));
                            }
                        }
                    }
                    VoxEvent::LlmToken { token, .. } => {
                        let mut m = metrics_clone.lock().unwrap();
                        if m.first_output_text.is_none() {
                            println!(
                                "      \x1b[35m[Event] LLM Output Token (Assistant): {:?}\x1b[0m",
                                token
                            );
                            m.first_output_text = Some(token.clone());
                            if let Some(end_time) = end_time_opt {
                                m.ttft_output = Some(now.duration_since(end_time));
                            }
                        } else {
                            if let Some(ref mut text) = m.first_output_text {
                                text.push_str(&token);
                            }
                        }
                    }
                    VoxEvent::Cancelled { .. } => {
                        metrics_clone.lock().unwrap().interrupted = true;
                    }
                    _ => {}
                }
            }
        });
    });

    let metrics_clone2 = metrics.clone();
    let speech_end_rx2 = speech_end_tx.clone();

    // Spawn task to process received audio from the server
    let audio_handler = tokio::spawn(async move {
        while let Some(pcm) = playback_rx.recv().await {
            let now = Instant::now();
            let end_time_opt = *speech_end_rx2.lock().unwrap();
            let mut m = metrics_clone2.lock().unwrap();

            m.audio_chunks_received += 1;
            m.total_audio_bytes += pcm.len() * 2;
            m.last_audio_received = Some(now);
            m.output_audio.extend_from_slice(&pcm);

            if m.ttfa.is_none() {
                if let Some(end_time) = end_time_opt {
                    m.ttfa = Some(now.duration_since(end_time));
                }
            }
        }
    });

    // Stream WAV audio to the session
    let _speech_start = Instant::now();
    if interaction_mode == InteractionMode::PTT {
        session.activity_start()?;
    }

    let chunk_size = 320; // 20ms
    for chunk in samples.chunks(chunk_size) {
        let mut pcm_chunk = chunk.to_vec();
        if pcm_chunk.len() < chunk_size {
            pcm_chunk.resize(chunk_size, 0);
        }
        session.send_audio(&pcm_chunk)?;
        tokio::time::sleep(Duration::from_millis(20)).await;
    }

    let speech_end = Instant::now();
    *speech_end_tx.lock().unwrap() = Some(speech_end);

    if interaction_mode == InteractionMode::PTT {
        session.activity_end()?;
    } else {
        // Continuous VAD: stream silence chunks to let server detect end of speech
        let silence_chunk = vec![0i16; chunk_size];
        for _ in 0..75 {
            // up to 1.5 seconds of silence
            if metrics.lock().unwrap().ttfa.is_some() {
                break;
            }
            if session.send_audio(&silence_chunk).is_err() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    }

    // Handle Barge-In test if requested
    if barge_in {
        let mut first_audio_received = false;
        for _ in 0..100 {
            // 5 seconds timeout
            tokio::time::sleep(Duration::from_millis(50)).await;
            if metrics.lock().unwrap().ttfa.is_some() {
                first_audio_received = true;
                break;
            }
        }

        if first_audio_received {
            tokio::time::sleep(Duration::from_secs(1)).await;
            let _interrupt_time = Instant::now();
            session.cancel()?;

            let mut last_len = metrics.lock().unwrap().audio_chunks_received;
            let mut silence_start = Instant::now();

            for _ in 0..60 {
                tokio::time::sleep(Duration::from_millis(50)).await;
                let current_len = metrics.lock().unwrap().audio_chunks_received;
                if current_len == last_len {
                    if silence_start.elapsed() >= Duration::from_millis(400) {
                        break;
                    }
                } else {
                    last_len = current_len;
                    silence_start = Instant::now();
                }
            }
        }
    } else {
        // Wait for response to finish (up to 6 seconds of silence/no new audio)
        let mut last_len = 0;
        let mut silence_start = Instant::now();
        for _ in 0..60 {
            tokio::time::sleep(Duration::from_millis(100)).await;
            let current_len = metrics.lock().unwrap().audio_chunks_received;
            if current_len > 0 && current_len == last_len {
                if silence_start.elapsed() >= Duration::from_secs(2) {
                    break;
                }
            } else if current_len > last_len {
                last_len = current_len;
                silence_start = Instant::now();
            }
        }
    }

    session.disconnect().ok();
    event_handler.abort();
    audio_handler.abort();

    let m = metrics.lock().unwrap();

    // Save outputs
    if !m.output_audio.is_empty() {
        if let Some(parent) = output_wav_path.parent() {
            fs::create_dir_all(parent).ok();
        }
        let output_spec = hound::WavSpec {
            channels: 1,
            sample_rate: 24000, // Gemini responds in 24kHz audio
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut writer = hound::WavWriter::create(output_wav_path, output_spec)?;
        for &sample in &m.output_audio {
            writer.write_sample(sample)?;
        }
        writer.finalize()?;
        println!(
            "      \x1b[32m[WAV Output] Saved response audio to {:?}\x1b[0m",
            output_wav_path
        );
    }

    Ok(BenchmarkMetrics {
        handshake_duration,
        ttft_input: m.ttft_input,
        ttft_output: m.ttft_output,
        ttfa: m.ttfa,
        first_input_text: m.first_input_text.clone(),
        first_output_text: m.first_output_text.clone(),
        audio_chunks_received: m.audio_chunks_received,
        total_audio_bytes: m.total_audio_bytes,
        interrupted: m.interrupted,
    })
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let home = std::env::var("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."));
    let root = home.join(".vox");
    vox_lib::utils::paths::init_with_root(root.clone());
    let settings = VoxSettings::load();
    let system_prompt = settings.assistant.realtime_prompt.clone();
    let cli = Cli::parse();

    let provider_name = cli.provider.to_lowercase();
    if provider_name != "gemini" && provider_name != "deepgram" {
        eprintln!(
            "\x1b[31mError: Unsupported provider '{}'. Use 'gemini' or 'deepgram'.\x1b[0m",
            provider_name
        );
        std::process::exit(1);
    }

    let api_key = match load_api_key(&provider_name) {
        Some(key) => key,
        None => {
            eprintln!(
                "\x1b[31mError: API key for provider '{}' not found in env or temp/.env\x1b[0m",
                provider_name
            );
            std::process::exit(1);
        }
    };

    let interaction_mode = match cli.mode.to_lowercase().as_str() {
        "passive" => InteractionMode::Passive,
        "ptt" => InteractionMode::PTT,
        _ => {
            eprintln!(
                "\x1b[31mError: Invalid mode. Supported modes are 'passive' and 'ptt'\x1b[0m"
            );
            std::process::exit(1);
        }
    };

    // Resolve Hindi and English wav paths
    let hindi_wav_path = resolve_hindi_wav(cli.hindi_wav.as_deref());
    let english_wav_path = resolve_english_wav(cli.english_wav.as_deref());

    println!("\x1b[34m[Realtime-Bench]\x1b[0m Starting S2S cloud provider benchmark...");
    println!("  Provider:         {}", provider_name);
    println!("  Mode:             {:?}", interaction_mode);
    println!("  Barge-In Test:    {}", cli.barge_in);
    println!("  Realtime Prompt:  {}", system_prompt);
    println!("  Hindi WAV Path:   {:?}", hindi_wav_path);
    println!("  English WAV Path: {:?}", english_wav_path);
    println!();

    // 1. Run Hindi prompt evaluation
    println!("\x1b[34m[Realtime-Bench]\x1b[0m === RUNNING HINDI CLIP EVALUATION ===");
    let out_hindi_path = PathBuf::from(format!("outputs/{}_response_hindi.wav", provider_name));
    let hindi_res = match run_benchmark_for_clip(
        &provider_name,
        &api_key,
        &hindi_wav_path,
        interaction_mode.clone(),
        cli.barge_in,
        system_prompt.clone(),
        &out_hindi_path,
    )
    .await
    {
        Ok(res) => {
            println!("\x1b[32m[Realtime-Bench] Hindi prompt evaluation run completed successfully.\x1b[0m");
            Some(res)
        }
        Err(e) => {
            eprintln!(
                "\x1b[31m[Realtime-Bench] Hindi evaluation failed: {:?}\x1b[0m",
                e
            );
            None
        }
    };
    println!();

    // 2. Run English prompt evaluation
    println!("\x1b[34m[Realtime-Bench]\x1b[0m === RUNNING ENGLISH CLIP EVALUATION ===");
    let out_english_path = PathBuf::from(format!("outputs/{}_response_english.wav", provider_name));
    let english_res = match run_benchmark_for_clip(
        &provider_name,
        &api_key,
        &english_wav_path,
        interaction_mode.clone(),
        cli.barge_in,
        system_prompt.clone(),
        &out_english_path,
    )
    .await
    {
        Ok(res) => {
            println!("\x1b[32m[Realtime-Bench] English prompt evaluation run completed successfully.\x1b[0m");
            Some(res)
        }
        Err(e) => {
            eprintln!(
                "\x1b[31m[Realtime-Bench] English evaluation failed: {:?}\x1b[0m",
                e
            );
            None
        }
    };
    println!();

    // 3. Print side-by-side comparison report
    println!("\n\x1b[1;32m========================= COMPARATIVE S2S BENCHMARK REPORT =========================\x1b[0m");
    println!(
        "  {:<26} | {:<22} | {:<22}",
        "Metric", "Hindi Prompt", "English Prompt"
    );
    println!("  ---------------------------+------------------------+-----------------------");

    let print_row_dur = |label: &str, d_hin: Option<Duration>, d_eng: Option<Duration>| {
        let h_str = d_hin
            .map(|d| format!("{} ms", d.as_millis()))
            .unwrap_or_else(|| "N/A".to_string());
        let e_str = d_eng
            .map(|d| format!("{} ms", d.as_millis()))
            .unwrap_or_else(|| "N/A".to_string());
        println!("  {:<26} | {:<22} | {:<22}", label, h_str, e_str);
    };

    let print_row_val = |label: &str, val_hin: &str, val_eng: &str| {
        println!("  {:<26} | {:<22} | {:<22}", label, val_hin, val_eng);
    };

    let print_row_opt_str = |label: &str, s_hin: Option<&String>, s_eng: Option<&String>| {
        let h_str = s_hin.map(|s| s.trim()).unwrap_or("N/A");
        let e_str = s_eng.map(|s| s.trim()).unwrap_or("N/A");
        let h_trunc = if h_str.chars().count() > 20 {
            format!("{}...", h_str.chars().take(17).collect::<String>())
        } else {
            h_str.to_string()
        };
        let e_trunc = if e_str.chars().count() > 20 {
            format!("{}...", e_str.chars().take(17).collect::<String>())
        } else {
            e_str.to_string()
        };
        println!("  {:<26} | {:<22} | {:<22}", label, h_trunc, e_trunc);
    };

    print_row_dur(
        "Handshake Latency",
        hindi_res.as_ref().map(|r| r.handshake_duration),
        english_res.as_ref().map(|r| r.handshake_duration),
    );
    print_row_dur(
        "TTFT (Input ASR)",
        hindi_res.as_ref().and_then(|r| r.ttft_input),
        english_res.as_ref().and_then(|r| r.ttft_input),
    );
    print_row_dur(
        "TTFT (Output Text)",
        hindi_res.as_ref().and_then(|r| r.ttft_output),
        english_res.as_ref().and_then(|r| r.ttft_output),
    );
    print_row_dur(
        "TTFA / A2A Latency",
        hindi_res.as_ref().and_then(|r| r.ttfa),
        english_res.as_ref().and_then(|r| r.ttfa),
    );
    print_row_val(
        "Audio Chunks Received",
        &hindi_res
            .as_ref()
            .map(|r| r.audio_chunks_received.to_string())
            .unwrap_or_else(|| "N/A".to_string()),
        &english_res
            .as_ref()
            .map(|r| r.audio_chunks_received.to_string())
            .unwrap_or_else(|| "N/A".to_string()),
    );
    print_row_val(
        "Total Audio Bytes Recv",
        &hindi_res
            .as_ref()
            .map(|r| r.total_audio_bytes.to_string())
            .unwrap_or_else(|| "N/A".to_string()),
        &english_res
            .as_ref()
            .map(|r| r.total_audio_bytes.to_string())
            .unwrap_or_else(|| "N/A".to_string()),
    );
    print_row_opt_str(
        "First Input Text Segment",
        hindi_res.as_ref().and_then(|r| r.first_input_text.as_ref()),
        english_res
            .as_ref()
            .and_then(|r| r.first_input_text.as_ref()),
    );
    print_row_opt_str(
        "First Output Text Segment",
        hindi_res
            .as_ref()
            .and_then(|r| r.first_output_text.as_ref()),
        english_res
            .as_ref()
            .and_then(|r| r.first_output_text.as_ref()),
    );
    print_row_val(
        "Server Interrupted",
        &hindi_res
            .as_ref()
            .map(|r| r.interrupted.to_string())
            .unwrap_or_else(|| "N/A".to_string()),
        &english_res
            .as_ref()
            .map(|r| r.interrupted.to_string())
            .unwrap_or_else(|| "N/A".to_string()),
    );
    println!("\x1b[1;32m====================================================================================\x1b[0m\n");

    Ok(())
}
