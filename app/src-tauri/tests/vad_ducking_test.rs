//! ============================================================================
//! vad_ducking_test.rs — VAD Ducking & Playback Suppression Tests (Seam 7)
//! ============================================================================
//! Category     : Integration Test
//! Component    : services/vad/actor, services/stt
//! Prerequisites: ~/.vox/models/vad/silero_vad.onnx
//! Execution    : cargo test --test vad_ducking_test --release -- --nocapture
//! Metrics      : False-trigger resistance, negative assertion on playback ducking
//! ============================================================================

mod common;

use common::audio::{
    decode_wav_to_mono_16k, stream_audio_to_ring_buffer, stream_silence_frames,
    wait_for_buffer_drain,
};
use common::harness::{
    assert_channel_empty_after, get_test_app_handle, setup_stt_worker, setup_vad_actor,
};
use common::paths::get_asset_path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use vox_lib::core::events::VoxEvent;
use vox_lib::core::settings::{AudioOutputMode, InteractionMode};
use vox_lib::core::state::{InteractionOwner, VadCommand};
use vox_lib::services::stt::actor::SttCommand;
use vox_lib::services::vad::actor::VadActorConfig;
use vox_lib::services::vad::VAD_SPEECH_END_FRAMES;

/// Guard (NEGATIVE): When audio_mode is Speaker and playback_active is true, speech frames must be suppressed.
#[test]
fn test_vad_ducking_suppresses_audio_during_playback() {
    let clip_path = get_asset_path("edgetts_01_en_briefing.wav");
    let audio = decode_wav_to_mono_16k(&clip_path).expect("Failed to decode EN WAV");

    let app = get_test_app_handle();
    let (stt_tx, pipeline_event_rx, engine_shutdown, stt_handle) = setup_stt_worker(&app);

    let vad_config = VadActorConfig {
        initial_threshold: 0.3,
        initial_noise_gate: 0.001,
        initial_mode: InteractionMode::Passive,
        initial_audio_mode: AudioOutputMode::Speaker,
    };
    let playback_active = Arc::new(AtomicBool::new(true));

    let (vad_cmd_tx, vox_event_rx, mut producer, vad_handle) = setup_vad_actor(
        &app,
        stt_tx.clone(),
        vad_config,
        playback_active.clone(),
        InteractionOwner::Assistant,
        engine_shutdown.clone(),
    );

    // Stream speech audio while playback is active
    stream_audio_to_ring_buffer(&audio, &mut producer);
    stream_silence_frames(&mut producer, VAD_SPEECH_END_FRAMES + 20);
    wait_for_buffer_drain(&producer, 5);

    // Assert that NO speech events or transcripts were emitted due to ducking suppression
    assert_channel_empty_after(
        &vox_event_rx,
        Duration::from_millis(500),
        "vox_event_rx ducking suppression",
    );
    assert_channel_empty_after(
        &pipeline_event_rx,
        Duration::from_millis(500),
        "pipeline_event_rx ducking suppression",
    );

    let _ = vad_cmd_tx.send(VadCommand::Shutdown);
    let _ = stt_tx.send(SttCommand::Shutdown);
    engine_shutdown.store(true, Ordering::Relaxed);
    let _ = vad_handle.join();
    let _ = stt_handle.join();
}

/// Tests that once playback finishes (playback_active=false), speech detection immediately resumes.
#[test]
fn test_vad_ducking_resumes_after_playback() {
    let clip_path = get_asset_path("edgetts_01_en_briefing.wav");
    let audio = decode_wav_to_mono_16k(&clip_path).expect("Failed to decode EN WAV");

    let app = get_test_app_handle();
    let (stt_tx, _pipeline_event_rx, engine_shutdown, stt_handle) = setup_stt_worker(&app);

    let vad_config = VadActorConfig {
        initial_threshold: 0.3,
        initial_noise_gate: 0.001,
        initial_mode: InteractionMode::Passive,
        initial_audio_mode: AudioOutputMode::Speaker,
    };
    let playback_active = Arc::new(AtomicBool::new(true));

    let (vad_cmd_tx, vox_event_rx, mut producer, vad_handle) = setup_vad_actor(
        &app,
        stt_tx.clone(),
        vad_config,
        playback_active.clone(),
        InteractionOwner::Assistant,
        engine_shutdown.clone(),
    );

    // 1. First burst: playback is active -> suppressed
    stream_audio_to_ring_buffer(&audio, &mut producer);
    stream_silence_frames(&mut producer, VAD_SPEECH_END_FRAMES + 10);
    wait_for_buffer_drain(&producer, 5);

    assert_channel_empty_after(
        &vox_event_rx,
        Duration::from_millis(300),
        "vox_event_rx must remain empty during active playback",
    );

    // 2. Playback ends -> unsuppress
    playback_active.store(false, Ordering::Relaxed);

    // 3. Second burst: audio streamed after playback ends must trigger SpeechStart
    stream_audio_to_ring_buffer(&audio, &mut producer);
    stream_silence_frames(&mut producer, VAD_SPEECH_END_FRAMES + 20);
    wait_for_buffer_drain(&producer, 5);

    let mut speech_started = false;
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    while std::time::Instant::now() < deadline && !speech_started {
        if let Ok(event) = vox_event_rx.recv_timeout(Duration::from_millis(50)) {
            if matches!(event, VoxEvent::SpeechStart { .. }) {
                speech_started = true;
            }
        }
    }

    assert!(speech_started, "SpeechStart must fire after playback finishes");

    let _ = vad_cmd_tx.send(VadCommand::Shutdown);
    let _ = stt_tx.send(SttCommand::Shutdown);
    engine_shutdown.store(true, Ordering::Relaxed);
    let _ = vad_handle.join();
    let _ = stt_handle.join();
}

/// Tests that in Headset mode, playback does not suppress VAD (barge-in is always active).
#[test]
fn test_vad_headset_mode_no_suppression_during_playback() {
    let clip_path = get_asset_path("edgetts_01_en_briefing.wav");
    let audio = decode_wav_to_mono_16k(&clip_path).expect("Failed to decode EN WAV");

    let app = get_test_app_handle();
    let (stt_tx, _pipeline_event_rx, engine_shutdown, stt_handle) = setup_stt_worker(&app);

    let vad_config = VadActorConfig {
        initial_threshold: 0.3,
        initial_noise_gate: 0.001,
        initial_mode: InteractionMode::Passive,
        initial_audio_mode: AudioOutputMode::Headset,
    };
    let playback_active = Arc::new(AtomicBool::new(true));

    let (vad_cmd_tx, vox_event_rx, mut producer, vad_handle) = setup_vad_actor(
        &app,
        stt_tx.clone(),
        vad_config,
        playback_active.clone(),
        InteractionOwner::Assistant,
        engine_shutdown.clone(),
    );

    // Stream speech audio in Headset mode with playback_active=true
    stream_audio_to_ring_buffer(&audio, &mut producer);
    stream_silence_frames(&mut producer, VAD_SPEECH_END_FRAMES + 20);
    wait_for_buffer_drain(&producer, 5);

    let mut speech_started = false;
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    while std::time::Instant::now() < deadline && !speech_started {
        if let Ok(event) = vox_event_rx.recv_timeout(Duration::from_millis(50)) {
            if matches!(event, VoxEvent::SpeechStart { .. }) {
                speech_started = true;
            }
        }
    }

    assert!(speech_started, "Headset mode must allow speech detection even during active playback");

    let _ = vad_cmd_tx.send(VadCommand::Shutdown);
    let _ = stt_tx.send(SttCommand::Shutdown);
    engine_shutdown.store(true, Ordering::Relaxed);
    let _ = vad_handle.join();
    let _ = stt_handle.join();
}
