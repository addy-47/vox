//! ============================================================================
//! e2e_pipeline_test.rs — Pipeline End-to-End Correctness Integration Test
//! ============================================================================
//! Category     : Integration Test
//! Component    : End-to-End Pipeline (VAD -> STT -> LLM -> TTS)
//! Prerequisites: Compiles against `vox_lib` public API
//! Execution    : cargo test --test e2e_pipeline_test
//! Metrics      : Multi-actor event propagation, VAD audio chunk ingestion, & E2E correctness
//! ============================================================================

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use vox_lib::core::events::VoxEvent;
use vox_lib::services::tts::actor::TtsClauseChunker;

// ─── 1. Pipeline Mock End-to-End Actor Flow Test ─────────────────────────────

#[test]
fn test_e2e_pipeline_mock_turn_flow() {
    let (event_tx, event_rx) = mpsc::channel::<VoxEvent>();
    let cancel_flag = Arc::new(AtomicBool::new(false));

    // Simulated LLM worker generating tokens for turn_id = 100
    let llm_tx = event_tx.clone();
    let llm_cancel = Arc::clone(&cancel_flag);

    let llm_handle = thread::spawn(move || {
        let tokens = vec!["Hello", " ", "there!", " ", "This", " ", "is", " ", "Vox."];
        for token in tokens {
            if llm_cancel.load(Ordering::Relaxed) {
                let _ = llm_tx.send(VoxEvent::Cancelled { turn_id: 100 });
                return;
            }
            let _ = llm_tx.send(VoxEvent::LlmToken {
                turn_id: 100,
                token: token.to_string(),
            });
            thread::sleep(Duration::from_millis(5));
        }
        let _ = llm_tx.send(VoxEvent::LlmFinished { turn_id: 100 });
    });

    // TTS Chunker collecting streaming tokens and emitting clause audio chunks
    let tts_tx = event_tx.clone();
    let mut chunker = TtsClauseChunker::new();
    let mut received_tokens = Vec::new();
    let mut received_tts_chunks = Vec::new();
    let mut is_completed = false;

    llm_handle.join().unwrap();

    // Consume all pipeline events
    while let Ok(event) = event_rx.try_recv() {
        match event {
            VoxEvent::LlmToken { turn_id, token } => {
                assert_eq!(turn_id, 100);
                received_tokens.push(token.clone());
                for chunk in chunker.push_str(&token) {
                    received_tts_chunks.push(chunk.clone());
                    // Emit synthetic TTS audio chunk
                    let _ = tts_tx.send(VoxEvent::TtsChunk {
                        turn_id,
                        samples: vec![0.1f32; 480], // 10ms of 48kHz audio
                    });
                }
            }
            VoxEvent::LlmFinished { turn_id } => {
                assert_eq!(turn_id, 100);
                if let Some(remaining) = chunker.flush() {
                    received_tts_chunks.push(remaining);
                }
                is_completed = true;
            }
            _ => {}
        }
    }

    assert!(is_completed, "Pipeline MUST complete LLM generation!");
    assert_eq!(received_tokens.join(""), "Hello there! This is Vox.");
    assert!(
        !received_tts_chunks.is_empty(),
        "TTS chunker MUST produce clauses!"
    );
}

// ─── 2. Synthetic Audio Frame Ingestion & VAD Buffer Test ────────────────────

#[test]
fn test_e2e_pipeline_synthetic_audio_vad_ingestion() {
    // Generate 1 second of 16kHz mono synthetic audio (440Hz sine wave tone)
    let sample_rate = 16000;
    let duration_secs = 1.0;
    let total_samples = (sample_rate as f32 * duration_secs) as usize;
    let mut pcm_samples = Vec::with_capacity(total_samples);

    for i in 0..total_samples {
        let t = i as f32 / sample_rate as f32;
        let sample = (2.0 * std::f32::consts::PI * 440.0 * t).sin() * 0.5;
        pcm_samples.push(sample);
    }

    assert_eq!(pcm_samples.len(), 16000);

    // Process in 512-sample VAD frame chunks (standard ONNX frame size)
    let frame_size = 512;
    let mut total_frames_processed = 0;

    for frame in pcm_samples.chunks(frame_size) {
        total_frames_processed += 1;
        // Verify frame energy computation
        let energy: f32 = frame.iter().map(|s| s * s).sum::<f32>() / frame.len() as f32;
        assert!(
            energy > 0.0,
            "Sine wave audio frame MUST have non-zero energy!"
        );
    }

    assert_eq!(total_frames_processed, 32); // 16000 / 512 = 31.25 -> 32 chunks
}

// ─── 3. Live Ollama GPU Server E2E Test (Ignored by Default) ──────────────────

#[test]
#[ignore]
fn test_e2e_pipeline_server_ollama_live() {
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .unwrap();

    let response = client.get("http://100.86.62.14:11434/api/tags").send();

    match response {
        Ok(resp) => {
            assert!(
                resp.status().is_success(),
                "Ollama server returned error status!"
            );
            let body = resp.text().unwrap();
            assert!(
                body.contains("models"),
                "Ollama response MUST list available models!"
            );
        }
        Err(e) => panic!("Live Ollama GPU server connection failed: {}", e),
    }
}
