//! ============================================================================
//! modular_ptt_test.rs — Modular PTT Pipeline Integration Tests (Seam 2)
//! ============================================================================
//! Category     : Integration Test
//! Component    : services/pipeline/modular_ptt, services/stt
//! Prerequisites: ~/.vox/models/stt/nemotron-3.5/
//! Execution    : cargo test --test modular_ptt_test --release -- --nocapture
//! Metrics      : Transcription Fidelity (Levenshtein >= 0.90), Buffer Lifecycle
//! ============================================================================

mod common;

use common::audio::decode_wav_to_mono_16k;
use common::harness::{
    assert_channel_empty_after, attach_mock_engine_to_state, collect_all_final_transcripts,
    get_test_app_and_state, setup_stt_worker,
};
use common::paths::get_asset_path;
use common::scoring::calculate_similarity;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};
use vox_lib::core::state::InteractionState;
use vox_lib::services::pipeline::modular::ptt::{
    get_buffer_len, ptt_cancel, ptt_start, ptt_stop, ingest_audio,
};
use vox_lib::services::stt::actor::SttCommand;
use vox_lib::services::vad::VAD_CHUNK_SIZE;

const EN_GROUND_TRUTH: &str =
    "Hey Vox, good morning! Can you check my calendar and give me a quick briefing on today's scheduled meetings?";
const MIN_SIMILARITY_THRESHOLD: f32 = 0.90;

/// Tests audio accumulation during PTT recording and successful dispatch to STT on release with hard timeout.
#[test]
fn test_modular_ptt_audio_accumulation_en() {
    let start_time = Instant::now();
    let max_test_duration = Duration::from_secs(30);

    let clip_path = get_asset_path("edgetts_01_en_briefing.wav");
    let audio = decode_wav_to_mono_16k(&clip_path).expect("Failed to decode EN WAV");
    let audio_duration_sec = audio.len() as f32 / 16000.0;

    let (app, state) = get_test_app_and_state();
    let (stt_tx, pipeline_event_rx, engine_shutdown, stt_handle) = setup_stt_worker(&app);
    attach_mock_engine_to_state(&app, &state, stt_tx.clone());

    // 1. Upstream Trigger: Start PTT recording
    ptt_start(&app, &state).expect("ptt_start failed");
    assert_eq!(state.pipeline.state(), InteractionState::Listening, "Pipeline state should be Listening after ptt_start");

    // 2. Feed audio in standard VAD_CHUNK_SIZE frames (256 samples / 16ms)
    for chunk in audio.chunks(VAD_CHUNK_SIZE) {
        if chunk.len() == VAD_CHUNK_SIZE {
            ingest_audio(chunk, &state);
        } else {
            let mut padded = chunk.to_vec();
            padded.resize(VAD_CHUNK_SIZE, 0.0);
            ingest_audio(&padded, &state);
        }
    }

    assert!(
        get_buffer_len() >= audio.len(),
        "PTT_BUFFER must contain accumulated audio frames"
    );

    // 3. Upstream Trigger: Stop PTT recording
    ptt_stop(&app, &state).expect("ptt_stop failed");
    assert_ne!(state.pipeline.state(), InteractionState::Listening, "Pipeline state must leave Listening after ptt_stop");
    assert_eq!(
        get_buffer_len(),
        0,
        "PTT_BUFFER must be drained upon release"
    );

    // 4. Downstream Evaluation: Collect emitted transcripts
    let transcript = collect_all_final_transcripts(
        &pipeline_event_rx,
        1,
        Duration::from_secs(20),
    );

    let elapsed = start_time.elapsed().as_secs_f32();
    let rtf = elapsed / audio_duration_sec;
    let similarity = calculate_similarity(&transcript, EN_GROUND_TRUTH);

    println!("\n=== [Modular PTT EN] Transcription Result ===");
    println!("Ground Truth : {}", EN_GROUND_TRUTH);
    println!("Hypothesis   : {}", transcript);
    println!("Similarity   : {:.4} (Threshold: {:.2})", similarity, MIN_SIMILARITY_THRESHOLD);
    println!("Total Time   : {:.2}s (Audio: {:.2}s, RTF: {:.3}x)", elapsed, audio_duration_sec, rtf);

    assert!(
        similarity >= MIN_SIMILARITY_THRESHOLD,
        "Modular PTT EN similarity {:.4} fell below threshold {:.2}",
        similarity,
        MIN_SIMILARITY_THRESHOLD
    );

    let _ = stt_tx.send(SttCommand::Shutdown);
    engine_shutdown.store(true, Ordering::Relaxed);
    stt_handle.join().expect("STT worker thread panicked during PTT teardown");

    assert!(
        start_time.elapsed() < max_test_duration,
        "Modular PTT EN test exceeded hard timeout of 30s"
    );
}

/// Guard (NEGATIVE): Release without audio frames should immediately return to Ready without dispatching STT inference.
#[test]
fn test_modular_ptt_empty_buffer_guard() {
    let start_time = Instant::now();
    let max_test_duration = Duration::from_secs(10);

    let (app, state) = get_test_app_and_state();
    let (stt_tx, pipeline_event_rx, engine_shutdown, stt_handle) = setup_stt_worker(&app);
    attach_mock_engine_to_state(&app, &state, stt_tx.clone());

    // 1. Start PTT recording
    ptt_start(&app, &state).expect("ptt_start failed");
    assert_eq!(state.pipeline.state(), InteractionState::Listening, "Pipeline state should be Listening");

    // 2. Stop PTT immediately without feeding audio
    ptt_stop(&app, &state).expect("ptt_stop failed");
    assert_eq!(state.pipeline.state(), InteractionState::Ready, "State should revert to Ready on empty buffer");

    // 3. Assert channel is empty (no SttCommand or VoxEvent dispatched)
    assert_channel_empty_after(
        &pipeline_event_rx,
        Duration::from_millis(500),
        "pipeline_event_rx empty guard",
    );

    let _ = stt_tx.send(SttCommand::Shutdown);
    engine_shutdown.store(true, Ordering::Relaxed);
    stt_handle.join().expect("STT worker thread panicked during empty guard teardown");

    assert!(
        start_time.elapsed() < max_test_duration,
        "Empty buffer guard test exceeded hard timeout of 10s"
    );
}

/// Guard (NEGATIVE): Cancelling PTT recording must clear accumulated audio and discard dispatch.
#[test]
fn test_modular_ptt_cancel_discards_audio() {
    let start_time = Instant::now();
    let max_test_duration = Duration::from_secs(10);

    let clip_path = get_asset_path("edgetts_01_en_briefing.wav");
    let audio = decode_wav_to_mono_16k(&clip_path).expect("Failed to decode EN WAV");

    let (app, state) = get_test_app_and_state();
    let (stt_tx, pipeline_event_rx, engine_shutdown, stt_handle) = setup_stt_worker(&app);
    attach_mock_engine_to_state(&app, &state, stt_tx.clone());

    // 1. Start PTT recording
    ptt_start(&app, &state).expect("ptt_start failed");
    assert_eq!(state.pipeline.state(), InteractionState::Listening, "Pipeline state must be Listening");

    // 2. Ingest audio frames
    for chunk in audio.chunks(VAD_CHUNK_SIZE) {
        ingest_audio(chunk, &state);
    }
    assert!(get_buffer_len() > 0, "PTT_BUFFER should contain audio frames");

    // 3. Cancel PTT recording
    ptt_cancel(&app, &state).expect("ptt_cancel failed");
    assert_eq!(state.pipeline.state(), InteractionState::Ready, "State must be Ready after cancel");
    assert_eq!(get_buffer_len(), 0, "PTT_BUFFER must be cleared on cancel");

    // 4. Assert no transcripts are dispatched downstream
    assert_channel_empty_after(
        &pipeline_event_rx,
        Duration::from_millis(500),
        "pipeline_event_rx cancel guard",
    );

    let _ = stt_tx.send(SttCommand::Shutdown);
    engine_shutdown.store(true, Ordering::Relaxed);
    stt_handle.join().expect("STT worker thread panicked during cancel teardown");

    assert!(
        start_time.elapsed() < max_test_duration,
        "PTT cancel guard test exceeded hard timeout of 10s"
    );
}
