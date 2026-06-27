use std::fs;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::mpsc::channel;
use std::sync::Arc;
use std::time::Instant;

use clap::{Parser, Subcommand};
use vox_lib::core::events::VoxEvent;

fn write_wav(
    samples: &[f32],
    sample_rate: u32,
    path: &std::path::Path,
) -> Result<(), hound::Error> {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer = hound::WavWriter::create(path, spec)?;
    let peak = samples.iter().map(|&s| s.abs()).fold(0.0f32, f32::max);
    let scale = if peak > 1.0 { 1.0 / peak } else { 1.0 };
    for &s in samples {
        let s16 = (s * scale * i16::MAX as f32).clamp(i16::MIN as f32, i16::MAX as f32) as i16;
        writer.write_sample(s16)?;
    }
    writer.finalize()?;
    Ok(())
}

#[derive(Parser)]
#[command(name = "tts-bench", about = "TTS benchmarking tool")]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,

    /// Reference audio file path for Chatterbox voice cloning
    #[arg(long, global = true)]
    tts_voice: Option<String>,
}

#[derive(Subcommand)]
enum Command {
    /// Run the Supertonic 3 benchmark suite
    Bench,
    /// Run the multi-engine comparison framework
    Compare,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    match cli.command.unwrap_or(Command::Bench) {
        Command::Bench => run_bench(),
        Command::Compare => run_compare(cli.tts_voice.as_deref()),
    }
}

fn run_supertonic_bench(
    models_dir: &std::path::Path,
    prompts: &[Prompt],
    audio_out_dir: &std::path::Path,
) -> anyhow::Result<()> {
    use vox_lib::services::tts::providers::TtsProvider as _;
    use vox_lib::services::tts::TtsEngine as SupertonicEngine;
    use vox_lib::utils::bench_reporter::BenchReporter;

    println!("\x1b[32m[TTS-Bench]\x1b[0m Loading Supertonic 3...");
    let super_model_path = models_dir.join("tts/supertonic-3");

    let snap_init = BenchReporter::get_memory_snapshot();
    let load_start = Instant::now();

    let engine = match SupertonicEngine::new(&super_model_path, 0, 8, 1.05) {
        Ok(e) => e,
        Err(e) => {
            println!(
                "\x1b[31m[TTS-Bench] Failed to load Supertonic: {}\x1b[0m",
                e
            );
            return Err(e);
        }
    };

    let load_time = load_start.elapsed().as_millis();
    let snap_loaded = BenchReporter::get_memory_snapshot();
    let ram = snap_loaded.rss_mb.saturating_sub(snap_init.rss_mb);
    println!(
        "\x1b[32m[TTS-Bench]\x1b[0m Supertonic loaded in {}ms. RAM usage: {}MB",
        load_time, ram
    );

    for (i, prompt) in prompts.iter().enumerate() {
        println!(
            "[{:?}/{:?}] Running prompt (Supertonic): {:?}",
            i + 1,
            prompts.len(),
            prompt.text.chars().take(60).collect::<String>()
        );

        let (tx, rx) = channel::<VoxEvent>();
        let cancel = Arc::new(AtomicBool::new(false));
        let cancel_clone = cancel.clone();

        let turn_id = i as u32;

        let start = Instant::now();
        let mut ttfa_ms: Option<u128> = None;
        let mut total_samples = 0;
        let mut rtf = 0.0f32;
        let mut accumulated_samples = Vec::new();

        std::thread::scope(|s| {
            s.spawn(|| {
                let _ = engine.synthesize_chunk(prompt.text, turn_id, cancel_clone, tx);
            });
        });

        while let Ok(event) = rx.recv() {
            match event {
                VoxEvent::TtsChunk { samples, .. } => {
                    if ttfa_ms.is_none() {
                        ttfa_ms = Some(start.elapsed().as_millis());
                    }
                    total_samples += samples.len();
                    accumulated_samples.extend_from_slice(&samples);
                }
                VoxEvent::TtsFinished {
                    rtf: finished_rtf, ..
                } => {
                    rtf = finished_rtf;
                }
                _ => {}
            }
        }
        let inference_ms = start.elapsed().as_millis();
        let audio_duration_s = total_samples as f32 / 24000.0;

        let wav_name = format!("{:02}_supertonic.wav", i + 1);
        let wav_path = audio_out_dir.join(wav_name);
        if let Err(e) = write_wav(&accumulated_samples, 24000, &wav_path) {
            println!(
                "\x1b[31m[TTS-Bench] Failed to save WAV file {:?}: {}\x1b[0m",
                wav_path, e
            );
        }

        println!(
            "  TTFA: {}ms, RTF: {:.3}, Audio: {:.2}s, Inference: {}ms",
            ttfa_ms.unwrap_or(0),
            rtf,
            audio_duration_s,
            inference_ms,
        );
    }

    println!("\x1b[32m[TTS-Bench]\x1b[0m Supertonic benchmark complete.");
    Ok(())
}

struct Prompt {
    text: &'static str,
}

fn run_bench() -> anyhow::Result<()> {
    let home = dirs::home_dir().expect("Could not find home directory");
    let vox_root = home.join(".vox");
    vox_lib::utils::paths::init_with_root(vox_root);
    let models_dir = vox_lib::utils::paths::get().models.clone();

    let audio_out_dir = PathBuf::from("/home/addy/projects/apps/vox/docs/benchmarks/audio_outputs");
    fs::create_dir_all(&audio_out_dir).ok();

    println!("\x1b[32m[TTS-Bench]\x1b[0m Starting TTS Benchmark Suite (Supertonic 3)...");

    let prompts = vec![
        Prompt {
            text: "I'm just a large language model, I don't have personal experiences or emotions, but I can tell you about some popular festive activities that people enjoy: Decorating the house with lights, garlands, and festive decorations. Attending family gatherings and parties. Singing festive songs and watching holiday movies. Cooking traditional holiday dishes. Exchanging gifts with loved ones. Participating in festive events and activities.",
        },
        Prompt {
            text: "नमस्ते, मैं एक बड़ा भाषा मॉडल हूँ। मुझे आपकी मदद करने में खुशी होगी। मैं विभिन्न विषयों पर जानकारी प्रदान कर सकता हूँ और सवालों के जवाब दे सकता हूँ। कृपया बेझिझक पूछें!",
        },
    ];

    // Run Supertonic benchmark (handles both EN and HI)
    run_supertonic_bench(&models_dir, &prompts, &audio_out_dir)?;

    Ok(())
}

fn run_compare(tts_voice: Option<&str>) -> anyhow::Result<()> {
    use vox_lib::services::tts::providers::TtsProvider;

    fn run_tts(name: &str, text: &str, engine: &mut dyn TtsProvider) -> anyhow::Result<()> {
        let (tx, rx) = channel::<VoxEvent>();
        let cancel = Arc::new(AtomicBool::new(false));

        let start = Instant::now();
        let mut ttfa_ms: Option<u128> = None;
        let mut total_samples = 0;
        let mut accumulated = Vec::new();

        engine.synthesize_chunk(text, 0, cancel, tx)?;

        while let Ok(event) = rx.recv() {
            match event {
                VoxEvent::TtsChunk { samples, .. } => {
                    if ttfa_ms.is_none() {
                        ttfa_ms = Some(start.elapsed().as_millis());
                    }
                    total_samples += samples.len();
                    accumulated.extend_from_slice(&samples);
                }
                VoxEvent::TtsFinished { rtf: _rtf, .. } => {
                    let elapsed = start.elapsed();
                    let audio_dur = total_samples as f32 / 24000.0;
                    println!("\n\x1b[36m=== {} ===\x1b[0m", name);
                    println!("  Load time:        skipped (pre-loaded)");
                    println!("  TTFA:             {} ms", ttfa_ms.unwrap_or(0));
                    println!("  Inference time:   {:.2} s", elapsed.as_secs_f32());
                    println!("  Audio duration:   {:.2} s", audio_dur);
                    println!(
                        "  RTF:              {:.3}",
                        elapsed.as_secs_f32() / audio_dur
                    );
                    println!("  Samples:          {}", total_samples);

                    let out_dir =
                        PathBuf::from("/home/addy/projects/apps/vox/docs/benchmarks/audio_outputs");
                    fs::create_dir_all(&out_dir).ok();
                    let wav_name = format!("compare_{}.wav", name.to_lowercase().replace(' ', "_"));
                    let wav_path = out_dir.join(wav_name);
                    write_wav(&accumulated, 24000, &wav_path)?;
                    println!("  WAV:              {:?}", wav_path);
                }
                _ => {}
            }
        }

        Ok(())
    }

    let home = dirs::home_dir().expect("Could not find home directory");
    let vox_root = home.join(".vox");
    vox_lib::utils::paths::init_with_root(vox_root);
    let models_dir = vox_lib::utils::paths::get().models.clone();

    let text = "I'm just a large language model, I don't have personal experiences or emotions, but I can tell you about some popular festive activities that people enjoy: Decorating the house with lights, garlands, and festive decorations. Attending family gatherings and parties. Singing festive songs and watching holiday movies. Cooking traditional holiday dishes. Exchanging gifts with loved ones. Participating in festive events and activities.";

    println!("\x1b[32m[TTS-Compare]\x1b[0m Text: {:.60}...", text);
    println!(
        "\x1b[32m[TTS-Compare]\x1b[0m Length: {} chars\n",
        text.len()
    );

    println!("\x1b[32m[TTS-Compare]\x1b[0m Loading Supertonic 3...");
    let super_model_path = models_dir.join("tts/supertonic-3");
    let mut supertonic =
        vox_lib::services::tts::TtsEngine::new(&super_model_path, 0, 8, 1.05)?;
    run_tts("Supertonic 3", text, &mut supertonic)?;

    println!("\x1b[32m[TTS-Compare]\x1b[0m Loading Chatterbox...");
    let cb_model_path = models_dir.join("tts/chatterbox");
    let mut chatterbox =
        vox_lib::services::tts::ChatterboxEngine::new(&cb_model_path, "en", 10, 1.0, tts_voice)?;
    run_tts("Chatterbox", text, &mut chatterbox)?;

    println!("\n\x1b[32m[TTS-Compare]\x1b[0m Done!");
    Ok(())
}
