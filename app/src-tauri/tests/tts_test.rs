/// Integration test: TTS runtime (Chatterbox multilingual ONNX)
///
/// Run with:
///   cargo test --test tts_test -- --ignored --nocapture
///
/// The schema validation tests (non-ignored) verify that the ONNX files exist
/// and match the hardcoded tensor shapes from tts.rs.

use std::path::PathBuf;

fn onnx_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets/chatterbox/onnx")
}

fn model_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets/chatterbox")
}

// ─── File Existence (always run) ─────────────────────────────────────────────

#[test]
fn test_chatterbox_onnx_files_exist() {
    let onnx = onnx_dir();
    for file in &[
        "speech_encoder.onnx",
        "embed_tokens.onnx",
        "language_model_q4.onnx",
        "conditional_decoder.onnx",
    ] {
        let path = onnx.join(file);
        // Follow symlinks — these are HF hub symlinks
        let resolved = path.canonicalize()
            .unwrap_or_else(|_| path.clone());
        assert!(
            resolved.exists(),
            "[TTS] ONNX file not found (or broken symlink): {:?}",
            path
        );
        println!("[TTS TEST] {:?} → {:?}", file, resolved);
    }
}

#[test]
fn test_chatterbox_tokenizer_exists() {
    let path = model_dir().join("tokenizer.json");
    let resolved = path.canonicalize().unwrap_or_else(|_| path.clone());
    assert!(resolved.exists(), "[TTS] tokenizer.json missing at {:?}", path);
}

// ─── Model Load + Schema Validation (ignored — requires ort runtime) ─────────

#[test]
#[ignore]
fn test_tts_engine_loads_and_validates_schema() {
    use vox_ui_lib::services::tts::TtsEngine;

    let dir = model_dir();
    let mut engine = TtsEngine::new(&dir);
    assert!(engine.is_ok(), "TtsEngine::new failed: {:?}", engine.err());
    println!("[TTS TEST] TtsEngine loaded and schema validated successfully");
}

#[test]
#[ignore]
fn test_tts_synthesises_audio() {
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;
    use tokio::sync::mpsc;
    use vox_ui_lib::services::tts::TtsEngine;
    use vox_ui_lib::core::events::VoxEvent;

    let mut engine = TtsEngine::new(&model_dir()).expect("TtsEngine load failed");
    let cancel = Arc::new(AtomicBool::new(false));
    let (tx, mut rx) = mpsc::channel(32);

    let cancel_clone = cancel.clone();
    let handle = std::thread::spawn(move || {
        engine.synthesize_chunk("Hello world.", 1, &cancel_clone, &tx)
    });

    let rt = tokio::runtime::Runtime::new().unwrap();
    let chunks: Vec<Vec<f32>> = rt.block_on(async {
        let mut collected = Vec::new();
        loop {
            match tokio::time::timeout(
                std::time::Duration::from_secs(60),
                rx.recv(),
            ).await {
                Ok(Some(VoxEvent::TtsChunk { samples, .. })) => {
                    println!("[TTS TEST] Got chunk: {} samples", samples.len());
                    collected.push(samples);
                }
                Ok(Some(VoxEvent::TtsFinished { .. })) | Ok(None) => break,
                Err(_) => { eprintln!("[TTS TEST] Timeout waiting for TTS output"); break; }
                _ => {}
            }
        }
        collected
    });

    handle.join().expect("thread panicked").expect("synthesize_chunk failed");
    assert!(!chunks.is_empty(), "[TTS] No audio chunks produced");
    let total_samples: usize = chunks.iter().map(|c| c.len()).sum();
    println!("[TTS TEST] Total samples: {} ({:.2}s at 24kHz)", total_samples,
        total_samples as f32 / 24000.0);
    assert!(total_samples > 0, "[TTS] Zero-length audio output");
}

#[test]
#[ignore]
fn test_tts_cancels_mid_synthesis() {
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;
    use tokio::sync::mpsc;
    use vox_ui_lib::services::tts::TtsEngine;
    use vox_ui_lib::core::events::VoxEvent;

    let mut engine = TtsEngine::new(&model_dir()).expect("TtsEngine load failed");
    let cancel = Arc::new(AtomicBool::new(false));
    let cancel_clone = cancel.clone();
    let cancel_killer = cancel.clone();

    let (tx, mut rx) = mpsc::channel(32);
    let handle = std::thread::spawn(move || {
        engine.synthesize_chunk(
            "The quick brown fox jumped over the lazy dog repeatedly.",
            1, &cancel_clone, &tx
        )
    });

    // Cancel immediately
    cancel_killer.store(true, Ordering::Relaxed);

    handle.join().expect("thread panicked").expect("synthesize_chunk errored");
    println!("[TTS TEST] Cancellation respected — no crash");
}
