/// VAD Unit Tests
/// Verified with real audio assets to ensure detection accuracy.
/// Run with: cargo test --test vad_test -- --nocapture

use std::path::PathBuf;
use vox_ui_lib::services::vad::VadEngine;
use ringbuf::traits::*;

/// Path to the VAD model.
fn vad_model_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets/ten_vad.onnx")
}

/// Path to a known speech file.
fn speech_wav_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets/qwen3-asr/test_wavs/fast1.wav")
}

/// Load a mono 16kHz WAV into a Vec<f32> sample buffer.
fn load_wav(path: &std::path::Path) -> Vec<f32> {
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
fn test_vad_engine_init() {
    let path = vad_model_path();
    assert!(path.exists(), "VAD model missing at {:?}", path);

    let engine = VadEngine::new(&path);
    assert!(engine.is_ok(), "VadEngine::new failed: {:?}", engine.err());
    println!("[PASS] VAD engine initialized");
}

#[test]
fn test_vad_speech_detection() {
    let path = vad_model_path();
    let wav_path = speech_wav_path();
    
    if !path.exists() || !wav_path.exists() {
        eprintln!("[SKIP] Assets missing: VAD={:?}, WAV={:?}", path.exists(), wav_path.exists());
        return;
    }

    let mut engine = VadEngine::new(&path).expect("init vad");
    let audio = load_wav(&wav_path);
    
    let mut speech_detected = false;
    // Process first 2 seconds in 10ms chunks
    for chunk in audio[..audio.len().min(16000 * 2)].chunks(160) {
        if chunk.len() == 160 {
            if engine.predict(chunk) {
                speech_detected = true;
                break;
            }
        }
    }
    
    assert!(speech_detected, "VAD failed to detect speech in a known speech file (fast1.wav)");
    println!("[PASS] VAD speech detection verified on real audio");
}
