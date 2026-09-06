//! ============================================================================
//! passive_streaming_test.rs — Passive Streaming Audio Pipeline Integration Test
//! ============================================================================
//! Category     : Integration Test
//! Component    : services/vad/actor.rs + services/stt/actor.rs
//! Prerequisites: Local Nemotron STT + Earshot VAD weights in ~/.vox/models/
//! Execution    : cargo nextest run --test passive_streaming_test --release --nocapture --test-threads=1
//! Metrics      : Latency, Levenshtein transcript similarity (>= 0.90)
//! ============================================================================

mod common;

use std::{
    sync::{
        atomic::{AtomicBool, AtomicU32, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};

use vox_lib::{
    core::{
        events::VoxEvent,
        settings::{AudioOutputMode, InteractionMode},
        state::InteractionState,
    },
    services::{
        stt::actor::SttCommand,
        vad::{actor::VadActorConfig, VadCommand},
    },
};

#[test]
fn test_passive_streaming_matrix() {
    let test_deadline = Instant::now() + Duration::from_secs(60);

    vox_lib::utils::paths::init();
    let app = common::harness::get_test_app_handle();

    // 1. Setup production STT worker with local Nemotron
    let (stt_tx, pipeline_event_rx, stt_shutdown, stt_join) =
        common::harness::setup_stt_worker(&app);

    // 2. Setup production VAD actor in ContinuousSegmentation (Passive) mode
    let state_atomic = Arc::new(AtomicU32::new(InteractionState::Ready as u32));
    let turn_id_atomic = Arc::new(AtomicU32::new(1));
    let audio_suppressed = Arc::new(AtomicBool::new(false));
    let ingestion_gate = Arc::new(AtomicBool::new(true));
    let vad_shutdown = Arc::new(AtomicBool::new(false));

    let vad_config = VadActorConfig {
        initial_threshold: vox_lib::core::defaults::DEFAULT_VAD_THRESHOLD,
        initial_noise_gate: vox_lib::core::defaults::DEFAULT_VAD_PTT_NOISE_GATE,
        // The test audio clip supertonic_01_en_briefing.wav has a 0.4s (400ms) intra-sentence pause
        // between greeting and question. We explicitly configure 800ms here so VAD tests single-turn
        // transcription correctness on this specific legacy test clip without premature segmentation.
        initial_silence_duration_ms: 800,
        initial_speech_onset_ms: vox_lib::core::defaults::DEFAULT_VAD_SPEECH_ONSET_MS,
        initial_mode: InteractionMode::Passive,
        initial_audio_mode: AudioOutputMode::Headset,
    };

    let (vad_cmd_tx, vox_event_rx, mut producer, vad_join) = common::harness::setup_vad_actor(
        stt_tx.clone(),
        vad_config,
        state_atomic.clone(),
        turn_id_atomic.clone(),
        audio_suppressed.clone(),
        ingestion_gate.clone(),
        vad_shutdown.clone(),
    );

    // --- Subtest 1: English Speech Clip (supertonic_01_en_briefing.wav) ---
    // Single-utterance clip by construction: max intra-sentence pause 0.4s,
    // below the 800ms production silence default, so VAD deterministically
    // segments exactly ONE utterance regardless of threshold presets.
    {
        let clip_path = common::paths::get_asset_path(common::ASSET_SUPERTONIC_01_EN_FILENAME);
        let audio = common::audio::decode_wav_to_mono_16k(&clip_path)
            .expect("Failed to decode supertonic_01_en_briefing.wav");

        // Stream audio in VAD_CHUNK_SIZE chunks
        common::audio::stream_audio_to_ring_buffer(&audio, &mut producer);

        // Feed trailing silence frames to trigger speech offset detection
        common::audio::stream_silence_frames(&mut producer, 60);
        common::audio::wait_for_buffer_drain(&producer, 5);

        // Await SpeechStart and SpeechEnd
        let mut saw_speech_start = false;
        let mut saw_speech_end = false;
        let speech_deadline = Instant::now() + Duration::from_secs(10);
        while Instant::now() < speech_deadline && (!saw_speech_start || !saw_speech_end) {
            if let Ok(ev) = vox_event_rx.recv_timeout(Duration::from_millis(100)) {
                match ev {
                    VoxEvent::SpeechStart => saw_speech_start = true,
                    VoxEvent::SpeechEnd => saw_speech_end = true,
                    _ => {}
                }
            }
        }
        assert!(
            saw_speech_start,
            "Did not receive VoxEvent::SpeechStart for EN clip"
        );
        assert!(
            saw_speech_end,
            "Did not receive VoxEvent::SpeechEnd for EN clip"
        );

        // Await the single TranscriptFinal for the one segmented utterance.
        let transcript = common::harness::collect_all_final_transcripts(
            &pipeline_event_rx,
            1,
            Duration::from_secs(15),
        );
        assert!(!transcript.is_empty(), "Transcript was empty for EN clip");

        common::scoring::assert_similarity_above(
            &transcript,
            common::ASSET_SUPERTONIC_01_EN_GROUND_TRUTH,
            0.90,
            "Passive Streaming EN supertonic_01",
        );
    }

    // --- Subtest 2: Hindi Speech Clip (supertonic_07_hi_weather.wav) ---
    // Single-utterance clip by construction: max intra-sentence pause 0.3s,
    // below the 800ms production silence default — exactly one utterance.
    {
        // Drain any lingering events from Subtest 1
        while vox_event_rx.try_recv().is_ok() {}
        while pipeline_event_rx.try_recv().is_ok() {}

        let clip_path = common::paths::get_asset_path(common::ASSET_SUPERTONIC_07_HI_FILENAME);
        let audio = common::audio::decode_wav_to_mono_16k(&clip_path)
            .expect("Failed to decode supertonic_07_hi_weather.wav");

        common::audio::stream_audio_to_ring_buffer(&audio, &mut producer);
        common::audio::stream_silence_frames(&mut producer, 60);
        common::audio::wait_for_buffer_drain(&producer, 5);

        let mut saw_speech_start = false;
        let mut saw_speech_end = false;
        let speech_deadline = Instant::now() + Duration::from_secs(10);
        while Instant::now() < speech_deadline && (!saw_speech_start || !saw_speech_end) {
            if let Ok(ev) = vox_event_rx.recv_timeout(Duration::from_millis(100)) {
                match ev {
                    VoxEvent::SpeechStart => saw_speech_start = true,
                    VoxEvent::SpeechEnd => saw_speech_end = true,
                    _ => {}
                }
            }
        }
        assert!(
            saw_speech_start,
            "Did not receive VoxEvent::SpeechStart for HI clip"
        );
        assert!(
            saw_speech_end,
            "Did not receive VoxEvent::SpeechEnd for HI clip"
        );

        let transcript = common::harness::collect_all_final_transcripts(
            &pipeline_event_rx,
            1,
            Duration::from_secs(30),
        );
        assert!(!transcript.is_empty(), "Transcript was empty for HI clip");

        // Raw STT hypothesis must carry Devanagari script for Hindi speech.
        assert!(
            !transcript.is_ascii(),
            "HI hypothesis contains no Devanagari script: {:?}",
            transcript
        );

        common::scoring::assert_similarity_above(
            &transcript,
            common::ASSET_SUPERTONIC_07_HI_GROUND_TRUTH,
            0.90,
            "Passive Streaming HI supertonic_07",
        );

        // Spec: transliterate_if_hi engages on Devanagari final transcripts —
        // enabled final call must yield non-empty ASCII-only Roman script.
        let roman = vox_lib::services::translit::transliterate_if_hi(&transcript, true, true);
        assert!(!roman.is_empty(), "Transliterated HI transcript is empty");
        assert!(
            roman.is_ascii(),
            "Transliterated HI transcript is not ASCII-only: {:?}",
            roman
        );
    }

    // --- Subtest 3: Silence Only Guard (Negative assertion) ---
    {
        // Drain any lingering events
        while vox_event_rx.try_recv().is_ok() {}
        while pipeline_event_rx.try_recv().is_ok() {}

        // Stream 100 silence frames
        common::audio::stream_silence_frames(&mut producer, 100);
        common::audio::wait_for_buffer_drain(&producer, 5);

        // Assert no speech events triggered
        common::harness::assert_channel_empty_after(
            &vox_event_rx,
            Duration::from_millis(500),
            "Passive Streaming Silence Guard — vox_event_rx must remain empty",
        );
        common::harness::assert_channel_empty_after(
            &pipeline_event_rx,
            Duration::from_millis(200),
            "Passive Streaming Silence Guard — pipeline_event_rx must remain empty",
        );
    }

    // --- Teardown: Graceful Shutdown & Join ---
    assert!(
        Instant::now() < test_deadline,
        "Test exceeded overall 60-second deadline"
    );

    let _ = vad_cmd_tx.send(VadCommand::Shutdown);
    let _ = stt_tx.send(SttCommand::Shutdown);
    vad_shutdown.store(true, Ordering::SeqCst);
    stt_shutdown.store(true, Ordering::SeqCst);

    vad_join
        .join()
        .expect("VAD worker thread panicked during test execution");
    stt_join
        .join()
        .expect("STT worker thread panicked during test execution");
}
