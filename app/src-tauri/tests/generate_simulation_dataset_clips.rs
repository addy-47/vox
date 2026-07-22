use anyhow::{anyhow, Result};
use serde::Deserialize;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::mpsc::channel;
use std::sync::Arc;
use std::time::Duration;

use vox_lib::core::events::VoxEvent;
use vox_lib::services::tts::{TtsEngine, TtsProvider};

#[derive(Debug, Deserialize)]
struct DatasetTurn {
    turn: usize,
    user: String,
    assistant: String,
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
    let home = dirs::home_dir().expect("Could not find home directory");
    let vox_root = home.join(".vox");
    vox_lib::utils::paths::init_with_root(vox_root);

    let super_tts_path = vox_lib::utils::paths::model_dir("tts").join("supertonic-3");
    if !super_tts_path.exists() {
        return Err(anyhow!("Supertonic 3 model path missing at {:?}", super_tts_path));
    }

    println!("[Generator] Loading Supertonic 3 TTS Engine...");
    let tts_engine = TtsEngine::new(&super_tts_path, 0, 12, 1.05)?;

    // We will generate audio clips for all 10 sessions
    let sessions = vec![
        ("tests/dataset_session1.json", "tests/simulation_clips_session1"),
        ("tests/dataset_session2.json", "tests/simulation_clips_session2"),
        ("tests/dataset_session3.json", "tests/simulation_clips_session3"),
        ("tests/dataset_session4.json", "tests/simulation_clips_session4"),
        ("tests/dataset_session5.json", "tests/simulation_clips_session5"),
        ("tests/dataset_session6.json", "tests/simulation_clips_session6"),
        ("tests/dataset_session7.json", "tests/simulation_clips_session7"),
        ("tests/dataset_session8.json", "tests/simulation_clips_session8"),
        ("tests/dataset_session9.json", "tests/simulation_clips_session9"),
        ("tests/dataset_session10.json", "tests/simulation_clips_session10"),
    ];

    for (dpath_str, outdir_str) in sessions {
        let dataset_path = PathBuf::from(dpath_str);
        let alt_dataset_path = PathBuf::from("app/src-tauri").join(dpath_str);
        let target_dataset_path = if dataset_path.exists() {
            dataset_path
        } else {
            alt_dataset_path
        };

        if !target_dataset_path.exists() {
            println!("[Generator] Skipping missing dataset {:?}", target_dataset_path);
            continue;
        }

        let dataset_json = fs::read_to_string(&target_dataset_path)?;
        let turns: Vec<DatasetTurn> = serde_json::from_str(&dataset_json)?;

        let out_dir = PathBuf::from(outdir_str);
        let alt_out_dir = PathBuf::from("app/src-tauri").join(outdir_str);
        let clips_dir = if out_dir.parent().map(|p| p.exists()).unwrap_or(false) {
            out_dir
        } else {
            alt_out_dir
        };
        fs::create_dir_all(&clips_dir)?;

        println!("[Generator] Generating {} audio clips for {:?}...", turns.len(), clips_dir);

        for turn_item in &turns {
            let clip_path = clips_dir.join(format!("clip_{:02}.wav", turn_item.turn));
            print!("  Synthesizing turn {:02} -> {:?}... ", turn_item.turn, clip_path);

            let cancel = Arc::new(AtomicBool::new(false));
            let (tx, rx) = channel();

            tts_engine.synthesize_chunk(&turn_item.user, turn_item.turn as u32, cancel, tx)?;

            let mut accumulated_samples = Vec::new();
            while let Ok(evt) = rx.recv_timeout(Duration::from_millis(5000)) {
                match evt {
                    VoxEvent::TtsChunk { samples, .. } => {
                        accumulated_samples.extend(samples);
                    }
                    VoxEvent::TtsFinished { .. } => break,
                    _ => {}
                }
            }

            if accumulated_samples.is_empty() {
                println!("FAILED (Empty audio)");
            } else {
                write_wav_file(&clip_path, &accumulated_samples, 24000)?;
                println!("DONE ({} samples)", accumulated_samples.len());
            }
        }
    }

    println!("[Generator] Successfully generated all simulation audio clips!");
    Ok(())
}
