//! ============================================================================
//! vops_tts.rs — Standalone Supertonic/Sherpa TTS Audio Synthesis CLI
//! ============================================================================
//! Category     : Utility Tool (Cargo Example)
//! Component    : TTS Engine (`vox_lib::services::tts`)
//! Prerequisites: Local TTS models at `~/.vox/models/tts/`
//! Execution    : cargo run --example vops_tts -- --help
//! ============================================================================

use anyhow::{anyhow, Result};
use clap::Parser;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::mpsc::channel;
use std::sync::Arc;
use std::time::Duration;

use vox_lib::core::events::VoxEvent;
use vox_lib::services::tts::{TtsEngine, TtsProvider};

#[derive(Parser, Debug)]
#[command(name = "vops-tts", about = "Synthesize TTS using Supertonic 3")]
struct Cli {
    /// The text script to synthesize.
    #[arg(long)]
    text: String,

    /// Output WAV file path.
    #[arg(long)]
    output: String,

    /// Speaker ID / voice style ID.
    #[arg(long, default_value_t = 0)]
    voice_sid: i32,

    /// Quality steps / diffusion steps (typically 2-12).
    #[arg(long, default_value_t = 8)]
    quality_steps: u32,

    /// Speed factor (typically 0.7 - 2.0).
    #[arg(long, default_value_t = 1.05)]
    speed: f32,
}

fn write_wav_file(path: &PathBuf, samples: &[f32], sample_rate: u32) -> Result<()> {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer = hound::WavWriter::create(path, spec)?;
    for &sample in samples {
        let i16_val = (sample.clamp(-1.0, 1.0) * 32767.0) as i16;
        writer.write_sample(i16_val)?;
    }
    writer.finalize()?;
    Ok(())
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    let home = dirs::home_dir().expect("Could not find home directory");
    let vox_root = home.join(".vox");
    vox_lib::utils::paths::init_with_root(vox_root);

    let super_tts_path = vox_lib::utils::paths::model_dir("tts").join("supertonic-3");
    if !super_tts_path.exists() {
        return Err(anyhow!("Supertonic 3 model path missing at {:?}", super_tts_path));
    }

    println!("[vops-tts] Loading Supertonic 3 TTS Engine...");
    let tts_engine = TtsEngine::new(&super_tts_path, cli.voice_sid, cli.quality_steps, cli.speed)?;

    println!("[vops-tts] Synthesizing text: {:?}", cli.text);
    let cancel = Arc::new(AtomicBool::new(false));
    let (tx, rx) = channel();

    tts_engine.synthesize_chunk(&cli.text, 0, cancel, tx)?;

    let mut accumulated_samples = Vec::new();
    while let Ok(evt) = rx.recv_timeout(Duration::from_millis(15000)) {
        match evt {
            VoxEvent::TtsChunk { samples, .. } => {
                accumulated_samples.extend(samples);
            }
            VoxEvent::TtsFinished { .. } => break,
            _ => {}
        }
    }

    if accumulated_samples.is_empty() {
        return Err(anyhow!("Synthesis failed: returned empty audio samples"));
    }

    let out_path = PathBuf::from(&cli.output);
    if let Some(parent) = out_path.parent() {
        fs::create_dir_all(parent)?;
    }

    write_wav_file(&out_path, &accumulated_samples, 24000)?;
    println!("[vops-tts] Successfully saved generated audio to {:?}", out_path);

    Ok(())
}
