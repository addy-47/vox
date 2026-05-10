/// Integration test: TTS runtime (Kokoro-82M via sherpa-onnx)
///
/// Run with:
///   cargo test --test tts_test -- --ignored --nocapture --test-threads=1
///
/// The schema validation tests (non-ignored) verify that the model files exist.

use std::path::PathBuf;

fn model_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets/kokoro")
}

// ─── File Existence (always run) ─────────────────────────────────────────────

#[test]
fn test_kokoro_assets_exist() {
    let dir = model_dir();
    for file in &[
        "model.onnx",
        "voices.bin",
        "tokens.txt",
    ] {
        let path = dir.join(file);
        assert!(
            path.exists(),
            "[TTS] Asset not found: {:?}",
            path
        );
        println!("[TTS TEST] Found asset: {:?}", path);
    }
    
    let espeak = dir.join("espeak-ng-data");
    assert!(espeak.exists(), "[TTS] espeak-ng-data directory missing at {:?}", espeak);
}

// ─── Model Load (ignored — requires runtime) ─────────────────────────────────

#[test]
#[ignore]
fn test_tts_engine_loads() {
    use vox_lib::services::tts::TtsEngine;

    let dir = model_dir();
    let engine = TtsEngine::new(&dir);
    assert!(engine.is_ok(), "TtsEngine::new failed: {:?}", engine.err());
    println!("[TTS TEST] TtsEngine loaded successfully");
}

#[test]
#[ignore]
fn test_tts_synthesises_audio() {
    use std::sync::atomic::AtomicBool;
    use std::sync::Arc;
    use tokio::sync::mpsc;
    use vox_lib::services::tts::TtsEngine;
    use vox_lib::core::events::VoxEvent;

    let mut engine = TtsEngine::new(&model_dir()).expect("TtsEngine load failed");
    let cancel = Arc::new(AtomicBool::new(false));
    let (tx, mut rx) = mpsc::channel(32);

    let cancel_clone = cancel.clone();
    let handle = std::thread::spawn(move || {
        engine.synthesize_chunk("Hello world, this is Kokoro speaking.", 1, cancel_clone, tx.clone())
    });

    let rt = tokio::runtime::Runtime::new().unwrap();
    let chunks: Vec<Vec<f32>> = rt.block_on(async {
        let mut collected = Vec::new();
        loop {
            match tokio::time::timeout(
                std::time::Duration::from_secs(30),
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
    use vox_lib::services::tts::TtsEngine;


    let mut engine = TtsEngine::new(&model_dir()).expect("TtsEngine load failed");
    let cancel = Arc::new(AtomicBool::new(false));
    let cancel_clone = cancel.clone();
    let cancel_killer = cancel.clone();

    let (tx, mut rx) = mpsc::channel(32);
    
    // Drain the receiver in the background to avoid blocking the sender
    let rx_handle = tokio::runtime::Runtime::new().unwrap().spawn(async move {
        while let Some(_) = rx.recv().await {}
    });

    let handle = std::thread::spawn(move || {
        engine.synthesize_chunk(
            "The quick brown fox jumped over the lazy dog repeatedly and then some more text to make it longer.",
            1, cancel_clone, tx.clone()
        )
    });

    // Wait a tiny bit then cancel
    std::thread::sleep(std::time::Duration::from_millis(50));
    cancel_killer.store(true, Ordering::Relaxed);

    handle.join().expect("thread panicked").expect("synthesize_chunk errored");
    drop(rx_handle);
    println!("[TTS TEST] Cancellation respected — no crash");
}
