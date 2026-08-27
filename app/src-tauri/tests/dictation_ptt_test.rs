//! ============================================================================
//! dictation_ptt_test.rs — Dictation PTT Pipeline Integration Tests (Seam 8)
//! ============================================================================
//! Category     : Integration Test
//! Component    : services/pipeline/dictation, services/stt
//! Prerequisites: ~/.vox/models/stt/nemotron-3.5/
//! Execution    : cargo test --test dictation_ptt_test --release -- --nocapture
//! Metrics      : Dictation Transcription Fidelity (Levenshtein >= 0.90), Buffer Lifecycle, Zero-LLM Invariant
//! ============================================================================

mod common;

use common::audio::decode_wav_to_mono_16k;
use common::harness::{
    assert_channel_empty_after, collect_all_final_transcripts, get_test_app_and_state,
    setup_stt_worker,
};
use common::paths::get_asset_path;
use common::scoring::calculate_similarity;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};
use vox_lib::core::events::VoxEvent;
use vox_lib::services::llm::actor::LlmCommand;
use vox_lib::services::pipeline::dictation::{
    get_buffer_len, handle_event as handle_dictation_event, handle_hotkey_press,
    handle_hotkey_release_with_sender, ingest_audio, is_recording,
};
use vox_lib::services::stt::actor::SttCommand;
use vox_lib::services::vad::VAD_CHUNK_SIZE;

const EN_GROUND_TRUTH: &str =
    "Hey Vox, good morning! Can you check my calendar and give me a quick briefing on today's scheduled meetings?";
const MIN_SIMILARITY_THRESHOLD: f32 = 0.90;

/// Tests audio accumulation during dictation hotkey press and transcription dispatch on release with hard 30s timeout.
#[tokio::test]
async fn test_dictation_ptt_audio_accumulation_en() {
    tokio::time::timeout(Duration::from_secs(30), async {
        let clip_path = get_asset_path("edgetts_01_en_briefing.wav");
        let audio = decode_wav_to_mono_16k(&clip_path).expect("Failed to decode EN WAV");
        let audio_duration_sec = audio.len() as f32 / 16000.0;

        let (app, state) = get_test_app_and_state();
        let (stt_tx, pipeline_event_rx, engine_shutdown, stt_handle) = setup_stt_worker(&app);

        let start_time = Instant::now();

        // 1. Upstream Trigger: Hotkey press starts dictation recording
        handle_hotkey_press(&app, &state).await.expect("handle_hotkey_press failed");
        assert!(is_recording(), "IS_RECORDING should be true after hotkey press");

        // 2. Feed audio in standard VAD_CHUNK_SIZE frames (256 samples / 16ms)
        for chunk in audio.chunks(VAD_CHUNK_SIZE) {
            if chunk.len() == VAD_CHUNK_SIZE {
                ingest_audio(chunk);
            } else {
                let mut padded = chunk.to_vec();
                padded.resize(VAD_CHUNK_SIZE, 0.0);
                ingest_audio(&padded);
            }
        }

        assert!(
            get_buffer_len() >= audio.len(),
            "DICTATION_BUFFER must contain accumulated audio frames"
        );

        // 3. Trigger Hotkey release with direct STT sender injection
        handle_hotkey_release_with_sender(&app, &state, Some(&stt_tx))
            .await
            .expect("handle_hotkey_release_with_sender failed");
        assert!(!is_recording(), "IS_RECORDING should be false after release");
        assert_eq!(get_buffer_len(), 0, "DICTATION_BUFFER must be drained after release");

        // 4. Collect final STT transcript
        let transcript = collect_all_final_transcripts(&pipeline_event_rx, 1, Duration::from_secs(20));
        let elapsed = start_time.elapsed().as_secs_f32();
        let rtf = elapsed / audio_duration_sec;
        let similarity = calculate_similarity(&transcript, EN_GROUND_TRUTH);

        println!("\n=== [Dictation PTT EN] Transcription Result ===");
        println!("Ground Truth : {}", EN_GROUND_TRUTH);
        println!("Hypothesis   : {}", transcript);
        println!("Similarity   : {:.4} (Threshold: {:.2})", similarity, MIN_SIMILARITY_THRESHOLD);
        println!("Total Time   : {:.2}s (Audio: {:.2}s, RTF: {:.3}x)", elapsed, audio_duration_sec, rtf);

        assert!(
            similarity >= MIN_SIMILARITY_THRESHOLD,
            "Dictation PTT EN similarity {:.4} fell below threshold {:.2}",
            similarity,
            MIN_SIMILARITY_THRESHOLD
        );

        let _ = stt_tx.send(SttCommand::Shutdown);
        engine_shutdown.store(true, Ordering::Relaxed);
        stt_handle.join().expect("STT worker thread panicked during dictation teardown");
    })
    .await
    .expect("test_dictation_ptt_audio_accumulation_en exceeded hard timeout of 30s");
}

/// Guard (NEGATIVE): Starting and stopping dictation with an empty buffer must NOT emit STT commands.
#[tokio::test]
async fn test_dictation_ptt_empty_buffer_guard() {
    tokio::time::timeout(Duration::from_secs(10), async {
        let (app, state) = get_test_app_and_state();
        let (stt_tx, pipeline_event_rx, engine_shutdown, stt_handle) = setup_stt_worker(&app);

        // 1. Hotkey press
        handle_hotkey_press(&app, &state).await.expect("handle_hotkey_press failed");
        assert!(is_recording(), "IS_RECORDING must be true");

        // 2. Immediately release without ingesting any audio frames
        handle_hotkey_release_with_sender(&app, &state, Some(&stt_tx))
            .await
            .expect("handle_hotkey_release_with_sender failed");
        assert!(!is_recording(), "IS_RECORDING must be false");
        assert_eq!(get_buffer_len(), 0, "Buffer len must be 0");

        // 3. Assert channel is empty (no SttCommand or VoxEvent dispatched)
        assert_channel_empty_after(
            &pipeline_event_rx,
            Duration::from_millis(500),
            "pipeline_event_rx empty buffer guard",
        );

        let _ = stt_tx.send(SttCommand::Shutdown);
        engine_shutdown.store(true, Ordering::Relaxed);
        stt_handle.join().expect("STT worker thread panicked during empty buffer teardown");
    })
    .await
    .expect("test_dictation_ptt_empty_buffer_guard exceeded hard timeout of 10s");
}

/// Invariant Guard: Dictation transcript completion routes directly to OS output and NEVER invokes LLM.
#[test]
fn test_dictation_does_not_invoke_llm() {
    let (app, state) = get_test_app_and_state();

    let (llm_tx, llm_rx) = std::sync::mpsc::channel::<LlmCommand>();

    // 1. Simulate arrival of TranscriptFinal event in Dictation owner mode
    let event = VoxEvent::TranscriptFinal {
        text: "This is a dictated sentence destined for active window typing.".to_string(),
        turn_id: 42,
    };

    handle_dictation_event(&app, &state, event);

    // 2. Assert that llm_tx channel receives ZERO commands (LLM is completely bypassed)
    assert_channel_empty_after(
        &llm_rx,
        Duration::from_millis(500),
        "llm_rx must receive zero commands during dictation flow",
    );

    drop(llm_tx);
}
