/// VAD Unit Tests
/// Tests feature extraction shape, silence detection, and probability floor.
/// Run with: cargo test --test vad_test -- --nocapture

use std::path::PathBuf;
use vox_ui_lib::vad::VadEngine;

fn vad_model_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets/ten_vad.onnx")
}

// ─── Test 1: VAD engine initializes ──────────────────────────────────────────

#[test]
fn test_vad_engine_init() {
    let path = vad_model_path();
    assert!(path.exists(), "VAD model missing at {:?}", path);

    let engine = VadEngine::new(path.to_str().unwrap());
    assert!(engine.is_ok(), "VadEngine::new failed: {:?}", engine.err());
    println!("[PASS] VAD engine initialized");
}

// ─── Test 2: Feature extraction returns correct shape ────────────────────────

#[test]
fn test_vad_feature_shape() {
    let path = vad_model_path();
    if !path.exists() {
        eprintln!("[SKIP] VAD model missing");
        return;
    }

    let engine = VadEngine::new(path.to_str().unwrap()).expect("init vad");

    // 768 samples = one VAD frame at 16kHz
    let audio = vec![0.0f32; 768];
    let features = engine.extract_features_pub(&audio);

    eprintln!("[test] Feature vector len: {}", features.len());
    // Should match model's expected input dimension (41 bins in this specific model)
    assert_eq!(
        features.len(),
        41,
        "Feature vector should be 41 bins, got {}",
        features.len()
    );
    println!("[PASS] Feature shape correct: {} bins", features.len());
}

// ─── Test 3: Silence produces prob << 0.5 (no false positives) ───────────────

#[test]
fn test_vad_silence_low_probability() {
    let path = vad_model_path();
    if !path.exists() {
        eprintln!("[SKIP] VAD model missing");
        return;
    }

    let mut engine = VadEngine::new(path.to_str().unwrap()).expect("init vad");

    // Feed 3 frames of digital silence
    let silence_frame = vec![0.0f32; 768];
    let mut probs = Vec::new();

    for _ in 0..5 {
        if let Ok(prob) = engine.process_frame_pub(&silence_frame) {
            probs.push(prob);
            eprintln!("[test] Silence frame prob: {:.4}", prob);
        }
    }

    // All silence probs must be well below 0.5 threshold
    for &p in &probs {
        assert!(
            p < 0.4,
            "Silence probability {:.4} is too high (expected < 0.4) — false positive risk",
            p
        );
    }
    println!("[PASS] Silence probabilities all < 0.4: {:?}", probs);
}

// ─── Test 4: Very low amplitude noise stays below threshold ──────────────────

#[test]
fn test_vad_low_noise_no_trigger() {
    let path = vad_model_path();
    if !path.exists() {
        eprintln!("[SKIP] VAD model missing");
        return;
    }

    let mut engine = VadEngine::new(path.to_str().unwrap()).expect("init vad");

    // Simulate ambient noise at very low amplitude (~-60dB)
    let noise: Vec<f32> = (0..768)
        .map(|i| (i as f32 * 0.017).sin() * 0.001)
        .collect();

    let mut triggered = false;
    for _ in 0..10 {
        if let Ok(prob) = engine.process_frame_pub(&noise) {
            eprintln!("[test] Low-noise prob: {:.4}", prob);
            if prob > 0.65 {
                triggered = true;
            }
        }
    }

    assert!(!triggered, "Low-amplitude noise triggered VAD speech detection");
    println!("[PASS] Low noise stays below detection threshold");
}
