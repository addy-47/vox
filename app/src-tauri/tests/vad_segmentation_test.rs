//! ============================================================================
//! vad_segmentation_test.rs — VAD segmentation seam
//! ============================================================================
//! Category  : Integration (Seam 2 — VAD segmentation → STT command stream)
//! Component : services/vad/{actor,segmenter}.rs + services/stt/actor.rs
//! Binary    : vad_segmentation_test
//! Prereqs   : Nemotron STT + Earshot VAD (VOX_MODELS_DIR)
//! Run       : cargo nextest run --test vad_segmentation_test --release --test-threads=1
//! ============================================================================

mod common;

use std::{
    sync::{
        atomic::{AtomicBool, AtomicU32, Ordering},
        mpsc, Arc,
    },
    time::{Duration, Instant},
};

use vox_lib::{
    config::AudioOutputMode,
    core::{
        events::{InteractionMode, VoxEvent},
        state::InteractionState,
    },
    services::{
        stt::actor::SttCommand,
        vad::{actor::VadActorConfig, VadCommand},
    },
};

/// PURPOSE:  Real speech must produce exactly one final transcript, and real silence
///           must produce none — the two ends of the segmentation contract.
/// SCENARIO: Earshot VAD + Nemotron STT run in Passive mode. One English clip, one Hindi
///           clip, then 100 silent frames.
/// ASSERTS:  Each clip yields a transcript at Levenshtein >= 0.90 against ground truth;
///           Hindi transliterates to non-empty ASCII; after both producers are joined the
///           event channels are empty.
/// CLASS:    happy-path (speech) + suppression (silence)
/// REQUIRES: VOX_MODELS_DIR with Earshot + Nemotron weights; ~4 s.
/// MUTANT:   Remove the `in_speech` guard in `process_continuous_segmentation`, so silence
///           enters the speech branch.
///
/// NOTE: the three scenarios share one VAD/STT worker because re-warming Sherpa/ONNX in one
/// process corrupts memory (testing-style-guide.md §13). Scenarios remain order-independent.
#[test]
fn passive_streaming_segments_speech_and_suppresses_silence() {
    let test_deadline = Instant::now() + Duration::from_secs(60);

    let _guard = common::paths::TempPathsGuard::new();
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

    let vad_backend = common::harness::create_test_vad_backend(
        vox_lib::services::vad::VadBackendOption::Earshot,
        vad_config.initial_threshold,
    );
    let (vad_cmd_tx, vox_event_rx, mut producer, vad_join) = common::harness::setup_vad_actor(
        vad_backend,
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
                    VoxEvent::SpeechStart { .. } => saw_speech_start = true,
                    VoxEvent::SpeechEnd { .. } => saw_speech_end = true,
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
        // Subtest 1 consumed its own events while asserting. The channels are therefore
        // empty at this boundary; anything present would belong to no scenario.
        common::harness::assert_channel_empty(
            &vox_event_rx,
            "Subtest 1 boundary — vox_event_rx must be empty",
        );
        common::harness::assert_channel_empty(
            &pipeline_event_rx,
            "Subtest 1 boundary — pipeline_event_rx must be empty",
        );

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
                    VoxEvent::SpeechStart { .. } => saw_speech_start = true,
                    VoxEvent::SpeechEnd { .. } => saw_speech_end = true,
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
        // Subtest 2 consumed its own events while asserting. The channels are empty here.
        // The negative assertion runs after both producers are shut down and joined,
        // so emptiness there proves the silence path emitted nothing.
        common::harness::assert_channel_empty(
            &vox_event_rx,
            "Silence guard boundary — vox_event_rx must be empty",
        );
        common::harness::assert_channel_empty(
            &pipeline_event_rx,
            "Silence guard boundary — pipeline_event_rx must be empty",
        );

        // Stream 100 silence frames
        common::audio::stream_silence_frames(&mut producer, 100);
        common::audio::wait_for_buffer_drain(&producer, 5);
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

    // Quiescence is now proven: both producers are joined and cannot emit again.
    common::harness::assert_channel_empty(
        &vox_event_rx,
        "Passive Streaming Silence Guard — vox_event_rx must be empty after the producers joined",
    );
    common::harness::assert_channel_empty(
        &pipeline_event_rx,
        "Passive Streaming Silence Guard — pipeline_event_rx must be empty after the producers joined",
    );
}

/// PURPOSE:  Silero ONNX VAD engine must classify chunk-level speech and trigger SpeechStart/SpeechEnd
///           in the continuous segmentation pipeline.
/// SCENARIO: Stream real audio clip into ring buffer with SileroVadEngine active, follow with silence frames.
/// ASSERTS:  SpeechStart and SpeechEnd events fire, and pure silence produces zero speech events.
/// CLASS:    happy-path (speech) + suppression (silence)
/// REQUIRES: VOX_MODELS_DIR with silero_vad.onnx
/// MUTANT:   Invert prediction return in SileroVadEngine::predict.
#[test]
fn silero_vad_segments_speech_and_triggers_auto_silence_stop() {
    let _guard = common::paths::TempPathsGuard::new();

    let state_atomic = Arc::new(AtomicU32::new(InteractionState::Ready as u32));
    let turn_id_atomic = Arc::new(AtomicU32::new(1));
    let audio_suppressed = Arc::new(AtomicBool::new(false));
    let ingestion_gate = Arc::new(AtomicBool::new(true));
    let vad_shutdown = Arc::new(AtomicBool::new(false));

    let vad_config = VadActorConfig {
        initial_threshold: vox_lib::core::defaults::DEFAULT_VAD_THRESHOLD,
        initial_noise_gate: vox_lib::core::defaults::DEFAULT_VAD_PTT_NOISE_GATE,
        initial_silence_duration_ms: 800,
        initial_speech_onset_ms: vox_lib::core::defaults::DEFAULT_VAD_SPEECH_ONSET_MS,
        initial_mode: InteractionMode::Passive,
        initial_audio_mode: AudioOutputMode::Headset,
    };

    let silero_backend = common::harness::create_test_vad_backend(
        vox_lib::services::vad::VadBackendOption::SileroVad,
        vad_config.initial_threshold,
    );
    let (stt_tx, _stt_rx) = mpsc::channel();

    let (vad_cmd_tx, vox_event_rx, mut producer, vad_join) = common::harness::setup_vad_actor(
        silero_backend,
        stt_tx,
        vad_config,
        state_atomic,
        turn_id_atomic,
        audio_suppressed,
        ingestion_gate,
        vad_shutdown.clone(),
    );

    // 1. Stream English speech clip
    let clip_path = common::paths::get_asset_path(common::ASSET_SUPERTONIC_01_EN_FILENAME);
    let audio = common::audio::decode_wav_to_mono_16k(&clip_path)
        .expect("Failed to decode supertonic_01_en_briefing.wav");

    common::audio::stream_audio_to_ring_buffer(&audio, &mut producer);
    common::audio::stream_silence_frames(&mut producer, 60);
    common::audio::wait_for_buffer_drain(&producer, 5);

    let mut saw_speech_start = false;
    let mut saw_speech_end = false;
    let speech_deadline = Instant::now() + Duration::from_secs(10);
    while Instant::now() < speech_deadline && (!saw_speech_start || !saw_speech_end) {
        if let Ok(ev) = vox_event_rx.recv_timeout(Duration::from_millis(100)) {
            match ev {
                VoxEvent::SpeechStart { .. } => saw_speech_start = true,
                VoxEvent::SpeechEnd { .. } => saw_speech_end = true,
                _ => {}
            }
        }
    }

    assert!(saw_speech_start, "Silero VAD did not emit SpeechStart");
    assert!(saw_speech_end, "Silero VAD did not emit SpeechEnd");

    // 2. Stream silence only and verify negative emission
    common::harness::assert_channel_empty(
        &vox_event_rx,
        "Silero boundary — vox_event_rx must be empty before silence guard",
    );
    common::audio::stream_silence_frames(&mut producer, 80);
    common::audio::wait_for_buffer_drain(&producer, 5);

    let _ = vad_cmd_tx.send(VadCommand::Shutdown);
    vad_shutdown.store(true, Ordering::SeqCst);
    vad_join.join().expect("Silero VAD actor panicked");

    common::harness::assert_channel_empty(
        &vox_event_rx,
        "Silero silence guard — vox_event_rx must remain empty for silence frames",
    );
}
