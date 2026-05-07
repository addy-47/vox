/// Batch STT Performance and Accuracy Test
/// Processes all WAV files in assets/qwen3-asr/test_wavs/ and saves results to a file.
/// Run with: cargo test --test batch_stt_test -- --nocapture

use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};
use vox_ui_lib::services::stt::SttEngine;

/// Resolve path to model dir relative to the cargo manifest.
fn model_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets/qwen3-asr")
}

/// Resolve path to the test wavs directory.
fn test_wavs_dir() -> PathBuf {
    model_dir().join("test_wavs")
}

/// Load a mono 16kHz WAV into a Vec<f32> sample buffer.
fn load_wav(path: &Path) -> Vec<f32> {
    let mut reader = hound::WavReader::open(path)
        .unwrap_or_else(|e| panic!("Failed to open {:?}: {}", path, e));

    let spec = reader.spec();
    let samples: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Float => reader
            .samples::<f32>()
            .map(|s| s.expect("read sample"))
            .collect(),
        hound::SampleFormat::Int => reader
            .samples::<i16>()
            .map(|s| s.expect("read sample") as f32 / 32768.0)
            .collect(),
    };

    // Convert stereo → mono if needed
    let mono: Vec<f32> = if spec.channels == 2 {
        samples.chunks(2).map(|c| (c[0] + c[1]) / 2.0).collect()
    } else {
        samples
    };

    // Resample to 16kHz if source rate differs
    if spec.sample_rate != 16000 {
        let ratio = 16000.0 / spec.sample_rate as f32;
        let new_len = (mono.len() as f32 * ratio) as usize;
        (0..new_len)
            .map(|i| {
                let src = i as f32 / ratio;
                let idx = src as usize;
                let frac = src - idx as f32;
                let a = mono.get(idx).copied().unwrap_or(0.0);
                let b = mono.get(idx + 1).copied().unwrap_or(0.0);
                a * (1.0 - frac) + b * frac
            })
            .collect()
    } else {
        mono
    }
}

#[test]
fn test_stt_batch_processing() {
    let _ = env_logger::builder().is_test(true).filter_level(log::LevelFilter::Info).try_init();
    
    let dir = test_wavs_dir();
    assert!(dir.exists(), "Test WAVs directory missing: {:?}", dir);

    let engine = SttEngine::new(&model_dir()).expect("Failed to initialize STT Engine");
    
    let mut output_file = File::create(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/transcription_results.txt"))
        .expect("Failed to create results file");

    writeln!(output_file, "Vox STT Batch Transcription Results").unwrap();
    writeln!(output_file, "========================================").unwrap();
    writeln!(output_file, "Model: Qwen3-ASR").unwrap();
    writeln!(output_file, "Date: {}\n", chrono::Local::now()).unwrap();

    let mut entries = std::fs::read_dir(dir).unwrap()
        .map(|res| res.map(|e| e.path()))
        .collect::<Result<Vec<_>, std::io::Error>>().unwrap();
    
    entries.sort();

    for path in entries {
        if path.extension().and_then(|s| s.to_str()) == Some("wav") {
            let filename = path.file_name().unwrap().to_string_lossy().to_string();
            println!("[BATCH] Processing {}...", filename);
            
            let audio = load_wav(&path);
            let start = std::time::Instant::now();
            let result = engine.transcribe(&audio);
            let duration = start.elapsed();

            match result {
                Ok(text) => {
                    writeln!(output_file, "File: {}", filename).unwrap();
                    writeln!(output_file, "Time: {:?}", duration).unwrap();
                    writeln!(output_file, "Result: {}", text).unwrap();
                    writeln!(output_file, "------------------------------------------").unwrap();
                    println!("[BATCH] Done in {:?}", duration);
                }
                Err(e) => {
                    writeln!(output_file, "File: {}", filename).unwrap();
                    writeln!(output_file, "Error: {:?}", e).unwrap();
                    writeln!(output_file, "------------------------------------------").unwrap();
                    eprintln!("[BATCH] Error processing {}: {:?}", filename, e);
                }
            }
        }
    }
    
    println!("[BATCH] Batch processing complete. Results saved to tests/transcription_results.txt");
}
