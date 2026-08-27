//! ============================================================================
//! passive_streaming_test.rs — Passive Streaming Ingestion & VAD -> STT Pipeline Integration Tests
//! ============================================================================
//! Category     : Integration Test
//! Component    : services/pipeline/modular_passive, services/vad, services/stt
//! Prerequisites: ~/.vox/models/stt/nemotron-3.5/
//! Execution    : cargo test --test passive_streaming_test --release -- --nocapture
//! Metrics      : Transcription Fidelity (Levenshtein >= 0.90), Speech Onset/Offset Lifecycle
//! ============================================================================

mod common;

use common::audio::{decode_wav_to_mono_16k, stream_audio_to_ring_buffer, stream_silence_frames, wait_for_buffer_drain};
use common::harness::{assert_channel_empty_after, collect_all_final_transcripts, get_test_app_handle, setup_stt_worker, setup_vad_actor};
use common::paths::get_asset_path;
use common::scoring::calculate_similarity;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use vox_lib::core::events::VoxEvent;
use vox_lib::core::settings::{AudioOutputMode, InteractionMode};
use vox_lib::core::state::{InteractionOwner, VadCommand};
use vox_lib::services::stt::actor::SttCommand;
use vox_lib::services::vad::actor::VadActorConfig;
use vox_lib::services::vad::VAD_SPEECH_END_FRAMES;

const EN_GROUND_TRUTH: &str =
    "Hey Vox, good morning! Can you check my calendar and give me a quick briefing on today's scheduled meetings?";
const HI_GROUND_TRUTH: &str =
    "वॉक्स, आज बाहर का मौसम कैसा है? क्या शाम को बारिश होने की कोई संभावना है?";

const MIN_SIMILARITY_THRESHOLD: f32 = 0.90;

/// Tests Passive streaming audio ingestion pipeline: English first, Hindi second, Silence guard third.
#[test]
fn test_passive_streaming_pipeline() {
    run_passive_streaming_en();
    run_passive_streaming_hi();
    run_passive_streaming_silence_only();
}

/// Subtest: English passive streaming ingestion through SPSC Ring Buffer -> VAD Actor -> STT Worker.
fn run_passive_streaming_en() {
    let clip_path = get_asset_path("edgetts_01_en_briefing.wav");
    let audio = decode_wav_to_mono_16k(&clip_path).expect("Failed to decode EN WAV");
    let audio_duration_sec = audio.len() as f32 / 16000.0;

    let app = get_test_app_handle();
    let (stt_tx, pipeline_event_rx, engine_shutdown, stt_handle) = setup_stt_worker(&app);

    let vad_config = VadActorConfig {
        initial_threshold: 0.3,
        initial_noise_gate: 0.001,
        initial_mode: InteractionMode::Passive,
        initial_audio_mode: AudioOutputMode::Headset,
    };
    let playback_active = Arc::new(AtomicBool::new(false));

    let (vad_cmd_tx, vox_event_rx, mut producer, vad_handle) = setup_vad_actor(
        &app,
        stt_tx.clone(),
        vad_config,
        playback_active,
        InteractionOwner::Assistant,
        engine_shutdown.clone(),
    );

    let start_time = Instant::now();

    // Stream audio in 256-sample chunks (16ms per frame)
    stream_audio_to_ring_buffer(&audio, &mut producer);

    // Stream trailing silence (VAD_SPEECH_END_FRAMES + 20) to trigger final speech offset detection
    stream_silence_frames(&mut producer, VAD_SPEECH_END_FRAMES + 20);

    // Wait until producer buffer is drained by VAD thread
    wait_for_buffer_drain(&producer, 5);

    // Await speech events from VAD
    let mut speech_started = false;
    let mut speech_ended = false;
    let deadline = Instant::now() + Duration::from_secs(5);

    while Instant::now() < deadline && (!speech_started || !speech_ended) {
        if let Ok(event) = vox_event_rx.recv_timeout(Duration::from_millis(50)) {
            match event {
                VoxEvent::SpeechStart { .. } => {
                    speech_started = true;
                }
                VoxEvent::SpeechEnd { .. } => {
                    speech_ended = true;
                }
                _ => {}
            }
        }
    }

    assert!(speech_started, "Passive streaming did not trigger SpeechStart via VAD");
    assert!(speech_ended, "Passive streaming did not trigger SpeechEnd via VAD");

    let transcript = collect_all_final_transcripts(&pipeline_event_rx, 2, Duration::from_secs(25));

    let elapsed = start_time.elapsed().as_secs_f32();
    let rtf = elapsed / audio_duration_sec;
    let similarity = calculate_similarity(&transcript, EN_GROUND_TRUTH);

    println!("\n=== [Passive Streaming EN] Transcription Result ===");
    println!("Ground Truth : {}", EN_GROUND_TRUTH);
    println!("Hypothesis   : {}", transcript);
    println!("Similarity   : {:.4} (Threshold: {:.2})", similarity, MIN_SIMILARITY_THRESHOLD);
    println!("Total Stream : {:.2}s (Audio: {:.2}s, RTF: {:.3}x)", elapsed, audio_duration_sec, rtf);

    assert!(
        similarity >= MIN_SIMILARITY_THRESHOLD,
        "English Passive Streaming transcription similarity {:.4} fell below threshold {:.2}",
        similarity,
        MIN_SIMILARITY_THRESHOLD
    );

    let _ = vad_cmd_tx.send(VadCommand::Shutdown);
    let _ = stt_tx.send(SttCommand::Shutdown);
    engine_shutdown.store(true, Ordering::Relaxed);
    let _ = vad_handle.join();
    let _ = stt_handle.join();
}

/// Subtest: Hindi passive streaming ingestion through SPSC Ring Buffer -> VAD Actor -> STT Worker.
fn run_passive_streaming_hi() {
    let clip_path = get_asset_path("edgetts_07_hi_weather.wav");
    let audio = decode_wav_to_mono_16k(&clip_path).expect("Failed to decode HI WAV");
    let audio_duration_sec = audio.len() as f32 / 16000.0;

    let app = get_test_app_handle();
    let (stt_tx, pipeline_event_rx, engine_shutdown, stt_handle) = setup_stt_worker(&app);

    let vad_config = VadActorConfig {
        initial_threshold: 0.3,
        initial_noise_gate: 0.001,
        initial_mode: InteractionMode::Passive,
        initial_audio_mode: AudioOutputMode::Headset,
    };
    let playback_active = Arc::new(AtomicBool::new(false));

    let (vad_cmd_tx, vox_event_rx, mut producer, vad_handle) = setup_vad_actor(
        &app,
        stt_tx.clone(),
        vad_config,
        playback_active,
        InteractionOwner::Assistant,
        engine_shutdown.clone(),
    );

    let start_time = Instant::now();

    stream_audio_to_ring_buffer(&audio, &mut producer);
    stream_silence_frames(&mut producer, VAD_SPEECH_END_FRAMES + 20);
    wait_for_buffer_drain(&producer, 5);

    let mut speech_started = false;
    let mut speech_ended = false;
    let deadline = Instant::now() + Duration::from_secs(5);

    while Instant::now() < deadline && (!speech_started || !speech_ended) {
        if let Ok(event) = vox_event_rx.recv_timeout(Duration::from_millis(50)) {
            match event {
                VoxEvent::SpeechStart { .. } => {
                    speech_started = true;
                }
                VoxEvent::SpeechEnd { .. } => {
                    speech_ended = true;
                }
                _ => {}
            }
        }
    }

    assert!(speech_started, "Passive streaming did not trigger SpeechStart via VAD");
    assert!(speech_ended, "Passive streaming did not trigger SpeechEnd via VAD");

    let transcript = collect_all_final_transcripts(&pipeline_event_rx, 2, Duration::from_secs(25));

    let elapsed = start_time.elapsed().as_secs_f32();
    let rtf = elapsed / audio_duration_sec;
    let similarity = calculate_similarity(&transcript, HI_GROUND_TRUTH);

    println!("\n=== [Passive Streaming HI] Transcription Result ===");
    println!("Ground Truth : {}", HI_GROUND_TRUTH);
    println!("Hypothesis   : {}", transcript);
    println!("Similarity   : {:.4} (Threshold: {:.2})", similarity, MIN_SIMILARITY_THRESHOLD);
    println!("Total Stream : {:.2}s (Audio: {:.2}s, RTF: {:.3}x)", elapsed, audio_duration_sec, rtf);

    assert!(
        similarity >= MIN_SIMILARITY_THRESHOLD,
        "Hindi Passive Streaming transcription similarity {:.4} fell below threshold {:.2}",
        similarity,
        MIN_SIMILARITY_THRESHOLD
    );

    let _ = vad_cmd_tx.send(VadCommand::Shutdown);
    let _ = stt_tx.send(SttCommand::Shutdown);
    engine_shutdown.store(true, Ordering::Relaxed);
    let _ = vad_handle.join();
    let _ = stt_handle.join();
}

/// Subtest: Silence-only audio stream guard (must NOT trigger SpeechStart or TranscriptFinal).
fn run_passive_streaming_silence_only() {
    let app = get_test_app_handle();
    let (stt_tx, pipeline_event_rx, engine_shutdown, stt_handle) = setup_stt_worker(&app);

    let vad_config = VadActorConfig {
        initial_threshold: 0.3,
        initial_noise_gate: 0.001,
        initial_mode: InteractionMode::Passive,
        initial_audio_mode: AudioOutputMode::Headset,
    };
    let playback_active = Arc::new(AtomicBool::new(false));

    let (vad_cmd_tx, vox_event_rx, mut producer, vad_handle) = setup_vad_actor(
        &app,
        stt_tx.clone(),
        vad_config,
        playback_active,
        InteractionOwner::Assistant,
        engine_shutdown.clone(),
    );

    // Stream 100 frames of pure silence (~1.6 seconds)
    stream_silence_frames(&mut producer, 100);
    wait_for_buffer_drain(&producer, 5);

    // Assert that no speech events or transcripts were emitted
    assert_channel_empty_after(&vox_event_rx, Duration::from_millis(500), "vox_event_rx silence");
    assert_channel_empty_after(&pipeline_event_rx, Duration::from_millis(500), "pipeline_event_rx silence");

    let _ = vad_cmd_tx.send(VadCommand::Shutdown);
    let _ = stt_tx.send(SttCommand::Shutdown);
    engine_shutdown.store(true, Ordering::Relaxed);
    let _ = vad_handle.join();
    let _ = stt_handle.join();
}
