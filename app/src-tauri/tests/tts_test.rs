//! ============================================================================
//! tts_test.rs — TTS Actor & Speech Synthesis Integration Tests (Seam 4)
//! ============================================================================
//! Category     : Integration Test
//! Component    : services/tts/actor, services/tts/providers
//! Prerequisites: Edge TTS (Online) or Supertonic (~/.vox/models/tts/supertonic/)
//! Execution    : cargo test --test tts_test --release -- --nocapture
//! Metrics      : Synthesis Output Validity, TtsChunk Emission, Lifecycle
//! ============================================================================

mod common;

use common::harness::{assert_channel_empty_after, get_test_app_handle};
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::Duration;
use vox_lib::core::events::VoxEvent;
use vox_lib::core::settings::{TtsActiveProvider, TtsEdgeConfig, VoxSettings};
use vox_lib::services::tts::actor::{cool_down_tts, warm_up_tts, TtsCommand, TtsWarmUpHandles};
use vox_lib::services::tts::SUPERTONIC_MODEL_DIR;

const TEST_SYNTHESIS_PROMPT: &str = "Hello world, this is a test of the speech synthesis engine.";

#[allow(dead_code)]
enum TestTtsConfig {
    EdgeTts { voice: Option<String> },
    Supertonic,
}

/// Helper to set up TTS worker with specified provider configuration.
fn setup_test_tts_worker(
    config: TestTtsConfig,
) -> (
    std::sync::mpsc::Sender<TtsCommand>,
    std::sync::mpsc::Receiver<VoxEvent>,
    Option<std::thread::JoinHandle<()>>,
) {
    let app = get_test_app_handle();
    let mut settings = VoxSettings::default();
    match config {
        TestTtsConfig::EdgeTts { voice } => {
            settings.tts.active = TtsActiveProvider::EdgeTts;
            settings.tts.edge_tts = TtsEdgeConfig { voice };
        }
        TestTtsConfig::Supertonic => {
            settings.tts.active = TtsActiveProvider::Supertonic;
        }
    }

    let super_tts_path = PathBuf::from(SUPERTONIC_MODEL_DIR);
    let (event_tx, event_rx) = std::sync::mpsc::channel::<VoxEvent>();

    let mut tts_tx: Option<std::sync::mpsc::Sender<TtsCommand>> = None;
    let mut tts_handle: Option<std::thread::JoinHandle<()>> = None;
    let cancel_flag = Arc::new(AtomicBool::new(false));
    let is_loaded = Arc::new(AtomicBool::new(false));
    let is_sleeping = Arc::new(AtomicBool::new(false));

    let handles = TtsWarmUpHandles {
        tts_tx: &mut tts_tx,
        tts_handle: &mut tts_handle,
        cancel_flag,
        is_loaded,
        is_sleeping,
    };

    warm_up_tts(&app, handles, &settings, &super_tts_path, event_tx)
        .expect("Failed to warm up TTS worker");

    (tts_tx.expect("tts_tx initialized"), event_rx, tts_handle)
}

/// Tests Edge TTS English synthesis emission of audio chunks and lifecycle events.
#[test]
fn test_tts_edge_en_synthesis() {
    let (tts_tx, event_rx, tts_handle) =
        setup_test_tts_worker(TestTtsConfig::EdgeTts { voice: None });

    // 1. Upstream Trigger: Send TtsCommand::Generate
    tts_tx
        .send(TtsCommand::Generate {
            turn_id: 1,
            text: TEST_SYNTHESIS_PROMPT.to_string(),
        })
        .expect("Failed to send TtsCommand");

    // 2. Collect emitted VoxEvents
    let mut total_samples = 0;
    let mut chunk_count = 0;
    let deadline = std::time::Instant::now() + Duration::from_secs(15);

    while std::time::Instant::now() < deadline {
        if let Ok(VoxEvent::TtsChunk { turn_id, samples }) =
            event_rx.recv_timeout(Duration::from_millis(100))
        {
            assert_eq!(turn_id, 1);
            total_samples += samples.len();
            chunk_count += 1;
            if total_samples > 16000 {
                break;
            }
        }
    }

    println!("\n=== [Edge TTS EN] Synthesis Result ===");
    println!("Chunks Received : {}", chunk_count);
    println!("Total Samples   : {} (Duration: {:.2}s @ 24kHz)", total_samples, total_samples as f32 / 24000.0);

    assert!(chunk_count > 0, "TTS Actor must emit at least one TtsChunk");
    assert!(total_samples > 0, "TTS Actor must generate non-empty audio samples");

    let mut tx_opt = Some(tts_tx);
    cool_down_tts(&mut tx_opt);
    if let Some(handle) = tts_handle {
        let _ = handle.join();
    }
}

/// Guard (NEGATIVE): Sending empty text to TTS must NOT emit audio chunks or panic.
#[test]
fn test_tts_empty_text_guard() {
    let (tts_tx, event_rx, tts_handle) =
        setup_test_tts_worker(TestTtsConfig::EdgeTts { voice: None });

    // 1. Send empty text
    tts_tx
        .send(TtsCommand::Generate {
            turn_id: 2,
            text: "".to_string(),
        })
        .expect("Failed to send TtsCommand");

    // 2. Assert no TtsChunks are emitted
    assert_channel_empty_after(
        &event_rx,
        Duration::from_millis(500),
        "event_rx empty text guard",
    );

    let mut tx_opt = Some(tts_tx);
    cool_down_tts(&mut tx_opt);
    if let Some(handle) = tts_handle {
        let _ = handle.join();
    }
}
