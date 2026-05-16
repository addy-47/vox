use clap::Parser;
use std::path::PathBuf;
use vox_lib::services::traits::{SttEngine as _, LlmEngine as _, TtsEngine as _};
use vox_lib::services::stt::qwen_onnx::SttEngine;
use vox_lib::services::llm::gemma_cpp::LlmWorker;
use vox_lib::services::tts::kokoro_piper::TtsEngine;
use vox_lib::utils::bench_reporter::BenchReporter;
use vox_lib::core::events::VoxEvent;
use vox_lib::core::metrics::{PipelineMetrics, MetricField};
use vox_lib::services::utils::transliterate_if_hi;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::mpsc::channel;

#[derive(Parser, Debug)]
#[command(name = "vox-bench", about = "Headless benchmark for Vox pipeline")]
struct Args {
    /// Path to input WAV file (16kHz mono)
    #[arg(short, long)]
    input: String,

    /// Mode: full, stt, llm, tts
    #[arg(short, long, default_value = "full")]
    mode: String,

    /// Custom system prompt
    #[arg(long)]
    prompt: Option<String>,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let mut reporter = BenchReporter::new();
    let mut pipeline_metrics = PipelineMetrics::new();
    
    reporter.metrics.ram_start = BenchReporter::get_memory_snapshot();

    // Initialize Vox paths for model resolution
    let home = dirs::home_dir().expect("Could not find home directory");
    let vox_root = home.join(".vox");
    vox_lib::utils::paths::init_with_root(vox_root);
    
    println!("\x1b[32m[Bench]\x1b[0m Starting run in: {:?}", reporter.run_dir);
    println!("\x1b[32m[Bench]\x1b[0m Models root: {:?}", vox_lib::utils::paths::models_dir());
    println!("\x1b[32m[Bench]\x1b[0m RAM at start: {} MB", reporter.metrics.ram_start.rss_mb);

    let mut current_text = String::new();

    // 1. STT Phase
    if args.mode == "full" || args.mode == "stt" {
        println!("\x1b[34m[Phase 1]\x1b[0m Running STT (Qwen3-ASR)...");
        let model_dir = vox_lib::utils::paths::model_dir("stt").join("qwen3-asr");
        let engine = SttEngine::new(&model_dir)?;
        
        let mut reader = hound::WavReader::open(&args.input)?;
        let samples: Vec<f32> = reader.samples::<i16>()
            .map(|s| s.unwrap() as f32 / 32768.0)
            .collect();
        
        pipeline_metrics.mark(MetricField::SpeechStart);
        let start = std::time::Instant::now();
        current_text = engine.transcribe(&samples)?;
        let elapsed = start.elapsed();
        pipeline_metrics.mark(MetricField::FinalTranscript);
        
        reporter.metrics.stt_latency_ms = elapsed.as_millis() as u32;
        let audio_duration = samples.len() as f32 / 16000.0;
        reporter.metrics.stt_rtf = elapsed.as_secs_f32() / audio_duration;
        
        reporter.write_artifact("stt_transcript.txt", &current_text);
        println!("  - Transcript: \"{}\"", current_text);
        reporter.update_peak_ram();
    } else if args.mode == "llm" || args.mode == "tts" {
        current_text = "नमस्ते, आप कैसे हैं?".to_string(); 
    }

    // 2. LLM Phase
    let mut assistant_text = String::new();
    if args.mode == "full" || args.mode == "llm" {
        println!("\x1b[34m[Phase 2]\x1b[0m Running LLM (Gemma 4)...");
        let model_path = vox_lib::utils::paths::model_dir("llm")
            .join("gemma4")
            .join("google_gemma-4-E2B-it-Q4_K_M.gguf");
        let engine = LlmWorker::new(&model_path, 2048, 4)?;
        
        let (tx, rx) = channel();
        let cancel_flag = Arc::new(AtomicBool::new(false));
        let system_prompt = args.prompt.unwrap_or_else(|| "You are Vox. Always reply in Hindi using Devanagari script.".to_string());
        
        pipeline_metrics.mark(MetricField::LlmStart);
        let start = std::time::Instant::now();
        let mut ttft: Option<u32> = None;
        let mut tokens = 0;
        
        let engine_clone = engine; 
        let text_clone = current_text.clone();
        
        let gen_thread = std::thread::spawn(move || {
            engine_clone.generate(&text_clone, &system_prompt, 0, &cancel_flag, &tx)
        });

        while let Ok(event) = rx.recv() {
            match event {
                VoxEvent::LlmToken { token, .. } => {
                    if ttft.is_none() {
                        pipeline_metrics.mark(MetricField::FirstToken);
                        ttft = Some(start.elapsed().as_millis() as u32);
                    }
                    assistant_text.push_str(&token);
                    tokens += 1;
                }
                VoxEvent::LlmFinished { .. } => break,
                VoxEvent::Error { message, .. } => return Err(anyhow::anyhow!("LLM Error: {}", message)),
                _ => {}
            }
        }
        gen_thread.join().unwrap()?;
        
        let elapsed = start.elapsed().as_secs_f32();
        reporter.metrics.llm_ttft_ms = ttft.unwrap_or(0);
        reporter.metrics.llm_tps = tokens as f32 / elapsed;
        
        reporter.write_artifact("llm_response.txt", &assistant_text);
        println!("  - Response: \"{}\"", assistant_text);
        reporter.update_peak_ram();
    } else if args.mode == "tts" {
        assistant_text = "मैं ठीक हूँ, धन्यवाद।".to_string();
    }

    // 3. TTS Phase
    if args.mode == "full" || args.mode == "tts" {
        println!("\x1b[34m[Phase 3]\x1b[0m Running TTS (Kokoro/Piper)...");
        let en_model_dir = vox_lib::utils::paths::model_dir("tts").join("kokoro");
        let hi_model_path = vox_lib::utils::paths::model_dir("tts").join("piper_hi").join("hi_IN-priyamvada-medium.onnx");
        
        let mut engine = TtsEngine::new(&en_model_dir, &hi_model_path)?;
        
        let (tx, rx) = channel();
        let cancel_flag = Arc::new(AtomicBool::new(false));
        
        pipeline_metrics.mark(MetricField::TtsStart);
        let start = std::time::Instant::now();
        let mut all_samples = Vec::new();
        
        engine.synthesize_chunk(&assistant_text, 1, 0, cancel_flag, tx)?;
        
        while let Ok(event) = rx.recv() {
            match event {
                VoxEvent::TtsChunk { samples, .. } => {
                    pipeline_metrics.mark(MetricField::FirstAudio);
                    all_samples.extend(samples);
                }
                VoxEvent::TtsFinished { .. } => break,
                _ => {}
            }
        }
        
        let elapsed = start.elapsed();
        reporter.metrics.tts_latency_ms = elapsed.as_millis() as u32;
        let audio_duration = all_samples.len() as f32 / 24000.0;
        reporter.metrics.tts_rtf = elapsed.as_secs_f32() / audio_duration;
        
        // Save WAV
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate: 24000,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let wav_path = reporter.run_dir.join("output.wav");
        let mut writer = hound::WavWriter::create(&wav_path, spec)?;
        for &sample in &all_samples {
            writer.write_sample((sample * 32767.0) as i16)?;
        }
        writer.finalize()?;

        println!("  - Audio generated: {:.2}s", audio_duration);
        reporter.update_peak_ram();
    }

    // 4. Post-processing: Transliteration & Report
    let stt_roman = transliterate_if_hi(&current_text);
    let llm_roman = transliterate_if_hi(&assistant_text);
    reporter.write_artifact("transliteration.txt", &format!("STT: {}\nLLM: {}", stt_roman, llm_roman));

    reporter.metrics.ram_end = BenchReporter::get_memory_snapshot();
    reporter.save_report(pipeline_metrics.latency_report());

    println!("\x1b[32m[Bench]\x1b[0m Run complete. Results in: {:?}", reporter.run_dir);
    println!("\x1b[32m[Bench]\x1b[0m RAM Peak: {} MB (Virt: {} MB)", reporter.metrics.ram_peak.rss_mb, reporter.metrics.ram_peak.virt_mb);
    
    Ok(())
}
