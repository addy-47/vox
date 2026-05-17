/// VAD Unit Tests
/// Covers both Earshot (default) and TenVAD (legacy) backends.
/// Run with: cargo test --test vad_test -- --nocapture

use std::path::PathBuf;
use vox_lib::services::vad::{VadBackend, ten_onnx::VadEngine as TenVadEngine, earshot_vad::EarshotVadEngine};
use vox_lib::services::traits::VadEngine as VadEngineTrait;

// ── Shared Helpers ────────────────────────────────────────────────────────────

/// Path to the TenVAD ONNX model (only needed for TenVAD tests).
fn tenvad_model_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets/ten_vad.onnx")
}

/// Path to a known speech WAV file (16kHz mono preferred).
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

// ── Earshot Tests (default backend, no model file required) ──────────────────

#[test]
fn test_earshot_engine_init() {
    let engine = EarshotVadEngine::new(0.5);
    assert!(engine.is_ok(), "EarshotVadEngine::new failed: {:?}", engine.err());
    println!("[PASS] Earshot VAD engine initialized (no model file, pure Rust).");
}

#[test]
fn test_earshot_threshold_hot_update() {
    let mut engine = EarshotVadEngine::new(0.5).expect("init earshot");
    // Hot threshold update must not fail or panic
    engine.update_threshold(0.7);
    engine.update_threshold(0.3);
    println!("[PASS] Earshot threshold hot-update (free f32 write, no reload).");
}

#[test]
fn test_earshot_flush_reset() {
    let mut engine = EarshotVadEngine::new(0.5).expect("init earshot");
    // flush() calls detector.reset() — must not panic
    engine.flush();
    println!("[PASS] Earshot flush/reset OK.");
}

#[test]
fn test_earshot_predict_silent_frame() {
    let mut engine = EarshotVadEngine::new(0.5).expect("init earshot");
    // A silent frame (all zeros) must not be classified as speech.
    let silent = vec![0.0f32; 256];
    let result = engine.predict(&silent);
    assert!(!result, "Earshot falsely detected speech in a silent frame.");
    println!("[PASS] Earshot: silent frame correctly classified as silence.");
}

#[test]
fn test_earshot_speech_detection() {
    let wav_path = speech_wav_path();
    if !wav_path.exists() {
        eprintln!("[SKIP] Speech WAV asset missing at {:?}", wav_path);
        return;
    }

    let mut engine = EarshotVadEngine::new(0.5).expect("init earshot");
    let audio = load_wav(&wav_path);

    let mut speech_detected = false;
    // Process first 2 seconds in 16ms chunks (256 samples at 16kHz)
    for chunk in audio[..audio.len().min(16000 * 2)].chunks(256) {
        if chunk.len() == 256 {
            if engine.predict(chunk) {
                speech_detected = true;
                break;
            }
        }
    }

    assert!(speech_detected, "Earshot failed to detect speech in a known speech file (fast1.wav)");
    println!("[PASS] Earshot speech detection verified on real audio.");
}

#[test]
fn test_earshot_via_vad_backend_enum() {
    let engine = EarshotVadEngine::new(0.5).expect("init earshot");
    let mut backend = VadBackend::Earshot(engine);

    // Silent frame via unified dispatch
    let silent = vec![0.0f32; 256];
    assert!(!backend.predict(&silent), "VadBackend(Earshot): silent frame detected as speech.");

    // Threshold hot-update via VadBackend
    backend.update_threshold(0.3).expect("update_threshold should not fail for Earshot");
    backend.flush();

    println!("[PASS] VadBackend(Earshot) enum dispatch verified.");
}

// ── TenVAD Tests (legacy backend, requires model file) ───────────────────────

#[test]
fn test_tenvad_engine_init() {
    let path = tenvad_model_path();
    if !path.exists() {
        eprintln!("[SKIP] TenVAD model missing at {:?}", path);
        return;
    }

    let engine = TenVadEngine::new(&path, 0.45);
    assert!(engine.is_ok(), "TenVadEngine::new failed: {:?}", engine.err());
    println!("[PASS] TenVAD engine initialized.");
}

#[test]
fn test_tenvad_speech_detection() {
    let path = tenvad_model_path();
    let wav_path = speech_wav_path();

    if !path.exists() || !wav_path.exists() {
        eprintln!("[SKIP] Assets missing: TenVAD={:?}, WAV={:?}", path.exists(), wav_path.exists());
        return;
    }

    let mut engine = TenVadEngine::new(&path, 0.45).expect("init tenvad");
    let audio = load_wav(&wav_path);

    let mut speech_detected = false;
    // TenVAD also uses 256-sample chunks
    for chunk in audio[..audio.len().min(16000 * 2)].chunks(256) {
        if chunk.len() == 256 {
            if engine.predict(chunk) {
                speech_detected = true;
                break;
            }
        }
    }

    assert!(speech_detected, "TenVAD failed to detect speech in a known speech file (fast1.wav)");
    println!("[PASS] TenVAD speech detection verified on real audio.");
}
