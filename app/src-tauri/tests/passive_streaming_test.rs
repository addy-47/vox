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

/// Consolidated Passive Streaming Matrix (EN, HI, Silence Guard) with Single Lifecycle & Hard Timeout.
#[test]
fn test_passive_streaming_pipeline() {
    let start_time = Instant::now();
    let max_test_duration = Duration::from_secs(60);

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

    // 1. English Utterance
    {
        let clip_path = get_asset_path("edgetts_01_en_briefing.wav");
        let audio = decode_wav_to_mono_16k(&clip_path).expect("Failed to decode EN WAV");
        let audio_duration_sec = audio.len() as f32 / 16000.0;
        let sub_start = Instant::now();

        stream_audio_to_ring_buffer(&audio, &mut producer);
        stream_silence_frames(&mut producer, VAD_SPEECH_END_FRAMES + 20);
        wait_for_buffer_drain(&producer, 5);

        let mut speech_started = false;
        let mut speech_ended = false;
        let deadline = Instant::now() + Duration::from_secs(5);

        while Instant::now() < deadline && (!speech_started || !speech_ended) {
            if let Ok(event) = vox_event_rx.recv_timeout(Duration::from_millis(50)) {
                match event {
                    VoxEvent::SpeechStart { .. } => speech_started = true,
                    VoxEvent::SpeechEnd { .. } => speech_ended = true,
                    _ => {}
                }
            }
        }

        assert!(speech_started, "Passive streaming did not trigger SpeechStart via VAD (EN)");
        assert!(speech_ended, "Passive streaming did not trigger SpeechEnd via VAD (EN)");

        let transcript = collect_all_final_transcripts(&pipeline_event_rx, 2, Duration::from_secs(20));
        let elapsed = sub_start.elapsed().as_secs_f32();
        let rtf = elapsed / audio_duration_sec;
        let similarity = calculate_similarity(&transcript, EN_GROUND_TRUTH);

        println!("\n=== [Passive Streaming EN] Result ===");
        println!("Ground Truth : {}", EN_GROUND_TRUTH);
        println!("Hypothesis   : {}", transcript);
        println!("Similarity   : {:.4} (Threshold: {:.2})", similarity, MIN_SIMILARITY_THRESHOLD);
        println!("Total Stream : {:.2}s (Audio: {:.2}s, RTF: {:.3}x)", elapsed, audio_duration_sec, rtf);

        assert!(
            similarity >= MIN_SIMILARITY_THRESHOLD,
            "EN passive streaming similarity {:.4} fell below threshold {:.2}",
            similarity,
            MIN_SIMILARITY_THRESHOLD
        );
    }

    // 2. Hindi Utterance
    {
        let clip_path = get_asset_path("edgetts_07_hi_weather.wav");
        let audio = decode_wav_to_mono_16k(&clip_path).expect("Failed to decode HI WAV");
        let audio_duration_sec = audio.len() as f32 / 16000.0;
        let sub_start = Instant::now();

        stream_audio_to_ring_buffer(&audio, &mut producer);
        stream_silence_frames(&mut producer, VAD_SPEECH_END_FRAMES + 20);
        wait_for_buffer_drain(&producer, 5);

        let mut speech_started = false;
        let mut speech_ended = false;
        let deadline = Instant::now() + Duration::from_secs(5);

        while Instant::now() < deadline && (!speech_started || !speech_ended) {
            if let Ok(event) = vox_event_rx.recv_timeout(Duration::from_millis(50)) {
                match event {
                    VoxEvent::SpeechStart { .. } => speech_started = true,
                    VoxEvent::SpeechEnd { .. } => speech_ended = true,
                    _ => {}
                }
            }
        }

        assert!(speech_started, "Passive streaming did not trigger SpeechStart via VAD (HI)");
        assert!(speech_ended, "Passive streaming did not trigger SpeechEnd via VAD (HI)");

        let transcript = collect_all_final_transcripts(&pipeline_event_rx, 2, Duration::from_secs(20));
        let elapsed = sub_start.elapsed().as_secs_f32();
        let rtf = elapsed / audio_duration_sec;
        let similarity = calculate_similarity(&transcript, HI_GROUND_TRUTH);

        println!("\n=== [Passive Streaming HI] Result ===");
        println!("Ground Truth : {}", HI_GROUND_TRUTH);
        println!("Hypothesis   : {}", transcript);
        println!("Similarity   : {:.4} (Threshold: {:.2})", similarity, MIN_SIMILARITY_THRESHOLD);
        println!("Total Stream : {:.2}s (Audio: {:.2}s, RTF: {:.3}x)", elapsed, audio_duration_sec, rtf);

        assert!(
            similarity >= MIN_SIMILARITY_THRESHOLD,
            "HI passive streaming similarity {:.4} fell below threshold {:.2}",
            similarity,
            MIN_SIMILARITY_THRESHOLD
        );

        // Drain any lingering turn events generated during the multi-turn Hindi utterance
        while vox_event_rx.recv_timeout(Duration::from_millis(150)).is_ok() {}
    }

    // 3. Silence-Only Guard
    {
        stream_silence_frames(&mut producer, 100);
        wait_for_buffer_drain(&producer, 5);

        assert_channel_empty_after(&vox_event_rx, Duration::from_millis(300), "vox_event_rx silence");
        assert_channel_empty_after(&pipeline_event_rx, Duration::from_millis(300), "pipeline_event_rx silence");
    }

    // 4. Teardown & Panic Verification
    let _ = vad_cmd_tx.send(VadCommand::Shutdown);
    let _ = stt_tx.send(SttCommand::Shutdown);
    engine_shutdown.store(true, Ordering::Relaxed);
    vad_handle.join().expect("VAD worker panicked during passive streaming shutdown");
    stt_handle.join().expect("STT worker panicked during passive streaming shutdown");

    assert!(
        start_time.elapsed() < max_test_duration,
        "Passive streaming pipeline exceeded hard timeout of 60s"
    );
}
