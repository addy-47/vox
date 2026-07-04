use anyhow::{anyhow, Result};
use serde::Deserialize;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::mpsc::channel;
use std::sync::Arc;
use std::time::Instant;

use vox_lib::core::events::VoxEvent;
use vox_lib::services::tts::providers::TtsProvider;
use vox_lib::services::tts::TtsEngine as SupertonicEngine;

#[derive(Debug, Deserialize)]
struct TurnItem {
    turn: usize,
    user: String,
    assistant: String,
}

fn write_wav(samples: &[f32], sample_rate: u32, path: &std::path::Path) -> Result<(), hound::Error> {
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

fn main() -> Result<()> {
    println!("============================================================");
    println!("  VOX SIMULATION WAV GENERATOR (SUPERTONIC 3 ENGINE)        ");
    println!("============================================================");

    let dataset_path = PathBuf::from("app/src-tauri/tests/dataset.json");
    if !dataset_path.exists() {
        return Err(anyhow!("Dataset file not found at {:?}", dataset_path));
    }

    let dataset_json = fs::read_to_string(&dataset_path)?;
    let turns: Vec<TurnItem> = serde_json::from_str(&dataset_json)?;
    println!("[Generator] Loaded {} turns from dataset.", turns.len());

    let possible_paths = [
        dirs::home_dir().map(|h| h.join(".vox/models/tts/supertonic-3")).unwrap_or_default(),
        PathBuf::from("vox-models/tts/supertonic-3"),
        PathBuf::from("app/src-tauri/vox-models/tts/supertonic-3"),
    ];

    let mut model_path = None;
    for p in &possible_paths {
        if p.exists() && p.join("tts.json").exists() {
            model_path = Some(p.clone());
            break;
        }
    }

    let model_dir = model_path.ok_or_else(|| anyhow!("Supertonic 3 model directory not found in candidate paths"))?;
    println!("[Generator] Loading Supertonic 3 from {:?}", model_dir);

    let start_load = Instant::now();
    let engine = SupertonicEngine::new(&model_dir, 0, 8, 1.05)?;
    println!("[Generator] Supertonic 3 loaded in {}ms.", start_load.elapsed().as_millis());

    let output_dir = PathBuf::from("app/src-tauri/tests/simulation_clips");
    fs::create_dir_all(&output_dir)?;

    for item in &turns {
        let filename = format!("clip_{:02}.wav", item.turn);
        let wav_path = output_dir.join(&filename);

        print!("[Generator] [{:02}/{}] Synthesizing \"{}\" ... ", item.turn, turns.len(), item.user.chars().take(40).collect::<String>());

        let cancel_flag = Arc::new(AtomicBool::new(false));
        let (tx, rx) = channel();

        let start_synth = Instant::now();
        engine.synthesize_chunk(&item.user, item.turn as u32, cancel_flag, tx)?;

        let mut samples = Vec::new();
        let sample_rate = 24000;

        while let Ok(event) = rx.recv() {
            match event {
                VoxEvent::TtsChunk { samples: chunk_samples, .. } => {
                    samples.extend_from_slice(&chunk_samples);
                }
                VoxEvent::TtsFinished { .. } => break,
                _ => {}
            }
        }

        if !samples.is_empty() {
            write_wav(&samples, sample_rate, &wav_path)?;
            println!("DONE ({} samples, {}ms)", samples.len(), start_synth.elapsed().as_millis());
        } else {
            println!("WARNING: Zero samples generated");
        }
    }

    println!("\n============================================================");
    println!(" SUCCESS: Generated all {} WAV clips in {:?}", turns.len(), output_dir);
    println!("============================================================");

    Ok(())
}
