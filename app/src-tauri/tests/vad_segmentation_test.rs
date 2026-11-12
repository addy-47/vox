//! ============================================================================
//! vad_segmentation_test.rs — VAD segmentation seam
//! ============================================================================
//! Category  : Integration (Seam 2 — VAD segmentation → STT command stream)
//! Component : services/vad/{actor,segmenter}.rs + services/stt/actor.rs + router
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
        defaults::{
            DEFAULT_VAD_SILENCE_DURATION_MS, DEFAULT_VAD_SPEECH_ONSET_MS,
            DEFAULT_VAD_THRESHOLD, DEFAULT_VAD_PTT_NOISE_GATE,
        },
        events::{InteractionMode, PipelineMode, VoxEvent},
        state::{AppState, InteractionOwner, InteractionState},
    },
    pipeline::router::spawn_router,
    services::{
        stt::{
            actor::{spawn_stt_worker, SttActorChannels, SttActorHandles, SttCommand},
            EmbeddedSttProvider, SttProvider,
        },
        vad::{actor::VadActorConfig, VadCommand},
    },
};

/// Spawns a real Nemotron STT worker with a partial sink that records emitted partials.
/// The recording exists so tests can assert that streaming partials flowed.
type PartialLog = Arc<std::sync::Mutex<Vec<(u32, String)>>>;

struct RecordingStt {
    stt_tx: mpsc::Sender<SttCommand>,
    pipeline_event_rx: mpsc::Receiver<VoxEvent>,
    partial_log: PartialLog,
    engine_shutdown: Arc<AtomicBool>,
    join: std::thread::JoinHandle<()>,
}

fn setup_stt_with_partial_recording() -> RecordingStt {
    let nemotron_dir = common::paths::get_nemotron_model_dir();
    let provider = Box::new(
        EmbeddedSttProvider::new(&nemotron_dir, "nemotron", 2)
            .expect("Failed to instantiate EmbeddedSttProvider with Nemotron"),
    ) as Box<dyn SttProvider>;

    let (stt_tx, stt_rx) = mpsc::channel::<SttCommand>();
    let (pipeline_event_tx, pipeline_event_rx) = mpsc::channel::<VoxEvent>();
    let partials = Arc::new(std::sync::Mutex::new(Vec::<(u32, String)>::new()));

    let partials_for_emitter = Arc::clone(&partials);
    let partial_emitter: Option<Arc<dyn Fn(u32, String) + Send + Sync>> =
        Some(Arc::new(move |turn_id: u32, text: String| {
            if let Ok(mut guard) = partials_for_emitter.lock() {
                guard.push((turn_id, text));
            }
        }));

    let cancel_flag = Arc::new(AtomicBool::new(false));
    let engine_shutdown = Arc::new(AtomicBool::new(false));
    let channels = SttActorChannels {
        rx: stt_rx,
        pipeline_event_tx: Some(pipeline_event_tx),
        partial_emitter,
    };
    let handles = SttActorHandles {
        cancel_flag,
        engine_shutdown: Arc::clone(&engine_shutdown),
    };
    let join = spawn_stt_worker(channels, provider, handles).expect("Failed to spawn STT worker");

    RecordingStt {
        stt_tx,
        pipeline_event_rx,
        partial_log: partials,
        engine_shutdown,
        join,
    }
}

/// VAD config identical to production defaults (never overridden in tests).
fn production_vad_config() -> VadActorConfig {
    VadActorConfig {
        initial_threshold: DEFAULT_VAD_THRESHOLD,
        initial_noise_gate: DEFAULT_VAD_PTT_NOISE_GATE,
        initial_silence_duration_ms: DEFAULT_VAD_SILENCE_DURATION_MS,
        initial_speech_onset_ms: DEFAULT_VAD_SPEECH_ONSET_MS,
        initial_mode: InteractionMode::Passive,
        initial_audio_mode: AudioOutputMode::Headset,
    }
}

fn spawn_vad(
    stt_tx: mpsc::Sender<SttCommand>,
    vox_event_tx: mpsc::Sender<VoxEvent>,
    state_atomic: Arc<AtomicU32>,
    turn_id_atomic: Arc<AtomicU32>,
    audio_suppressed: Arc<AtomicBool>,
    ingestion_gate: Arc<AtomicBool>,
    vad_shutdown: Arc<AtomicBool>,
) -> (
    mpsc::Sender<VadCommand>,
    common::harness::RbProducer,
    std::thread::JoinHandle<()>,
) {
    let vad_backend = common::harness::create_test_vad_backend(
        vox_lib::services::vad::VadBackendOption::Earshot,
        DEFAULT_VAD_THRESHOLD,
    );
    let vad_config = production_vad_config();
    common::harness::setup_vad_actor(
        vad_backend,
        stt_tx,
        vad_config,
        state_atomic,
        turn_id_atomic,
        audio_suppressed,
        ingestion_gate,
        vad_shutdown,
        vox_event_tx,
    )
}

/// Sets the interaction to Passive + Realtime so the router never dispatches
/// stale modular LLM work; VAD/STT pathways are unaffected.
fn realtime_passive(state: &AppState) {
    let mut settings = state.settings.write().unwrap();
    settings.interaction.mode = InteractionMode::Passive;
    settings.interaction.pipeline_mode = PipelineMode::Realtime;
}


/// PURPOSE:  Real speech must produce exactly one final transcript per segment under standard silence timing.
/// SCENARIO: EN and HI clips with natural intra-sentence pauses at the
///           production silence-duration default (400ms), plus a composed clip with a
///           250 ms intra-utterance pause within the 400 ms silence window.
/// ASSERTS:  One TranscriptFinal each; similarity >= 0.90 vs ground truth; HI transliterates to ASCII.
/// CLASS:    happy-path (speech)
/// REQUIRES: VOX_MODELS_DIR with Earshot + Nemotron.
/// MUTANT:   Lower silence duration frames below 250ms so natural pauses split utterances.
/// NOTE: Scenarios share one VAD/STT worker (re-warming corrupts the ONNX allocator).
#[test]
fn passive_streaming_segments_speech_and_suppresses_silence() {
    let test_deadline = Instant::now() + Duration::from_secs(90);

    let (_guard, app, state) = common::harness::setup_isolated_app_state_sync();
    state.owner.store(InteractionOwner::Assistant as u32, Ordering::Relaxed);
    realtime_passive(&state);
    state.pipeline.set_state(InteractionState::Ready);

    // STT worker with recording partial emitter
    let RecordingStt {
        stt_tx,
        pipeline_event_rx,
        partial_log,
        engine_shutdown: stt_shutdown,
        join: stt_join,
    } = setup_stt_with_partial_recording();

    let audio_suppressed = Arc::new(AtomicBool::new(false));
    let ingestion_gate = Arc::new(AtomicBool::new(true));
    let vad_shutdown = Arc::new(AtomicBool::new(false));
    let state_atomic = state.pipeline.current_state_atomic.clone();
    let turn_id_atomic = state.pipeline.turn_id.clone();

    // ---------- Subtest A: English clip — natural 0.4 s pause, one utterance ----------
    {
        state.pipeline.set_state(InteractionState::Ready);
        state.pipeline.turn_id.store(1, Ordering::Relaxed);

        let (vox_event_tx, vox_event_rx) = mpsc::channel::<VoxEvent>();
        let router_join = spawn_router(app.clone(), vox_event_rx).expect("router");
        let (vad_cmd_tx, mut producer, vad_join) = spawn_vad(
            stt_tx.clone(),
            vox_event_tx,
            state_atomic.clone(),
            turn_id_atomic.clone(),
            Arc::clone(&audio_suppressed),
            Arc::clone(&ingestion_gate),
            Arc::clone(&vad_shutdown),
        );

        let clip_path = common::paths::get_asset_path(common::ASSET_SUPERTONIC_01_EN_FILENAME);
        let audio = common::audio::decode_wav_to_mono_16k(&clip_path)
            .expect("Failed to decode supertonic_01_en_briefing.wav");

        let (seen_bits, watcher_join) = common::harness::spawn_state_watcher(&state, Duration::from_secs(20));
        common::audio::stream_audio_to_ring_buffer(&audio, &mut producer);
        common::audio::stream_silence_frames(&mut producer, 60);
        common::audio::wait_for_buffer_drain(&producer, 5);
        watcher_join.join().expect("state watcher panicked");

        assert!(
            seen_bits.load(Ordering::SeqCst) & 0b001 != 0,
            "VAD SpeechStart must reach the router and switch Ready -> Listening"
        );

        let transcript = common::harness::collect_all_final_transcripts(
            &pipeline_event_rx,
            1,
            Duration::from_secs(30),
        );
        assert!(!transcript.is_empty(), "Transcript was empty for EN clip");

        common::scoring::assert_similarity_above(
            &transcript,
            common::ASSET_SUPERTONIC_01_EN_GROUND_TRUTH,
            0.90,
            "Passive Streaming EN supertonic_01",
        );
        assert_eq!(
            state.pipeline.state(),
            InteractionState::Thinking,
            "router must land on Thinking after TranscriptFinal (Realtime pending)"
        );

        // End of EN turn: Finish the drain, then join VAD before re-arming for HI clip.
        let _ = vad_cmd_tx.send(VadCommand::Shutdown);
        vad_shutdown.store(true, Ordering::SeqCst);
        vad_join.join().expect("VAD join failed");
        let _ = stt_tx.send(SttCommand::ResetStream);
        let _ = router_join;
        state.pipeline.set_state(InteractionState::Ready);
    }

    // ---------- Subtest B: Hindi clip — Devanagari + transliteration ----------
    {
        state.pipeline.set_state(InteractionState::Ready);
        state.pipeline.turn_id.store(2, Ordering::Relaxed);
        vad_shutdown.store(false, Ordering::SeqCst);

        let (vox_event_tx, vox_event_rx) = mpsc::channel::<VoxEvent>();
        let _router_join = spawn_router(app.clone(), vox_event_rx).expect("router");
        let (vad_cmd_tx, mut producer, vad_join) = spawn_vad(
            stt_tx.clone(),
            vox_event_tx,
            state_atomic.clone(),
            turn_id_atomic.clone(),
            Arc::clone(&audio_suppressed),
            Arc::clone(&ingestion_gate),
            Arc::clone(&vad_shutdown),
        );

        let clip_path = common::paths::get_asset_path(common::ASSET_SUPERTONIC_07_HI_FILENAME);
        let audio = common::audio::decode_wav_to_mono_16k(&clip_path)
            .expect("Failed to decode supertonic_07_hi_weather.wav");

        common::audio::stream_audio_to_ring_buffer(&audio, &mut producer);
        common::audio::stream_silence_frames(&mut producer, 60);
        common::audio::wait_for_buffer_drain(&producer, 5);

        let transcript = common::harness::collect_all_final_transcripts(
            &pipeline_event_rx,
            1,
            Duration::from_secs(30),
        );
        assert!(!transcript.is_empty(), "Transcript was empty for HI clip");

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

        let roman = vox_lib::services::translit::transliterate_if_hi(&transcript, true, true);
        assert!(!roman.is_empty(), "Transliterated HI transcript is empty");
        assert!(
            roman.is_ascii(),
            "Transliterated HI transcript is not ASCII-only: {:?}",
            roman
        );

        let _ = vad_cmd_tx.send(VadCommand::Shutdown);
        vad_shutdown.store(true, Ordering::SeqCst);
        vad_join.join().expect("VAD join failed");
        let _ = stt_tx.send(SttCommand::ResetStream);
        state.pipeline.set_state(InteractionState::Ready);
    }

    // ---------- Subtest C: 250 ms pause within standard 400 ms silence duration ----------
    // Standard silence duration (DEFAULT_VAD_SILENCE_DURATION_MS = 400 ms / 25 frames)
    // must absorb a composed 250 ms pause and keep the utterance unified as one segment.
    {
        const SPLIT_SAMPLES: usize = 9_600; // 0.6 s of speech
        const GAP_SAMPLES: usize = 4_000;   // 250 ms pause (< 400 ms default silence threshold)

        state.pipeline.set_state(InteractionState::Ready);
        state.pipeline.turn_id.store(3, Ordering::Relaxed);
        vad_shutdown.store(false, Ordering::SeqCst);

        let (vox_event_tx, vox_event_rx) = mpsc::channel::<VoxEvent>();
        let _router_join = spawn_router(app.clone(), vox_event_rx).expect("router");
        let (vad_cmd_tx, mut producer, vad_join) = spawn_vad(
            stt_tx.clone(),
            vox_event_tx,
            state_atomic.clone(),
            turn_id_atomic.clone(),
            Arc::clone(&audio_suppressed),
            Arc::clone(&ingestion_gate),
            Arc::clone(&vad_shutdown),
        );

        let clip_path = common::paths::get_asset_path(common::ASSET_SUPERTONIC_01_EN_FILENAME);
        let audio = common::audio::decode_wav_to_mono_16k(&clip_path)
            .expect("Failed to decode supertonic_01_en_briefing.wav");
        let composed = common::audio::insert_silence(&audio, SPLIT_SAMPLES, GAP_SAMPLES);

        common::audio::stream_audio_to_ring_buffer(&composed, &mut producer);
        common::audio::stream_silence_frames(&mut producer, 60);
        common::audio::wait_for_buffer_drain(&producer, 5);

        let transcript = common::harness::collect_all_final_transcripts(
            &pipeline_event_rx,
            1,
            Duration::from_secs(30),
        );
        assert!(
            !transcript.is_empty(),
            "composed clip must yield a transcript (250 ms pause absorbed by standard silence duration)"
        );

        // Natural pauses inside standard silence duration keep ground truth similarity intact.
        common::scoring::assert_similarity_above(
            &transcript,
            common::ASSET_SUPERTONIC_01_EN_GROUND_TRUTH,
            0.90,
            "Passive Streaming EN supertonic_01 with 250 ms mid-utterance pause",
        );

        let partial_count = partial_log.lock().map(|guard| guard.len()).unwrap_or(0);
        assert!(
            partial_count > 0,
            "Nemotron streaming partials must reach the partial_emitter"
        );

        let _ = vad_cmd_tx.send(VadCommand::Shutdown);
        vad_shutdown.store(true, Ordering::SeqCst);
        vad_join.join().expect("VAD join failed");
        let _ = stt_tx.send(SttCommand::ResetStream);
        state.pipeline.set_state(InteractionState::Ready);
    }

    // ---------- Subtest D: Branch Point 2 / Scenario 2B — sub-300 ms burst ----------
    {
        // 4160 voiced samples (260 ms) lifted from the clip's high-energy "Hey Vox" burst —
        // below the 4800-sample (300 ms) minimum utterance, but loud enough to confirm onset.
        const BURST_START: usize = 9_200; // 0.575 s @ 16 kHz — first strongly voiced window
        const BURST_END: usize = 13_360; // 0.835 s — 260 ms of voiced energy

        state.pipeline.set_state(InteractionState::Ready);
        state.pipeline.turn_id.store(4, Ordering::Relaxed);
        vad_shutdown.store(false, Ordering::SeqCst);

        let (vox_event_tx, vox_event_rx) = mpsc::channel::<VoxEvent>();
        let _router_join = spawn_router(app.clone(), vox_event_rx).expect("router");
        let (vad_cmd_tx, mut producer, vad_join) = spawn_vad(
            stt_tx.clone(),
            vox_event_tx,
            state_atomic.clone(),
            turn_id_atomic.clone(),
            Arc::clone(&audio_suppressed),
            Arc::clone(&ingestion_gate),
            Arc::clone(&vad_shutdown),
        );

        let clip_path = common::paths::get_asset_path(common::ASSET_SUPERTONIC_01_EN_FILENAME);
        let audio = common::audio::decode_wav_to_mono_16k(&clip_path)
            .expect("Failed to decode supertonic_01_en_briefing.wav");

        let (seen_bits, watcher_join) = common::harness::spawn_state_watcher(&state, Duration::from_secs(25));
        common::audio::stream_audio_to_ring_buffer(&audio[BURST_START..BURST_END], &mut producer);
        common::audio::stream_silence_frames(&mut producer, 60);
        common::audio::wait_for_buffer_drain(&producer, 5);
        watcher_join.join().expect("state watcher panicked");

        // The burst confirms SpeechStart (onset). Thinking -> Ready completes within a
        // single router iteration after the fallback empty TranscriptFinal, so polling
        // cannot reliably sample the intermediate Thinking state. The provable contract:
        // Listening is entered, then the pipeline recovers to Ready without deadlock.
        let bits = seen_bits.load(Ordering::SeqCst);
        assert!(
            bits & 0b001 != 0,
            "sub-threshold burst must still trigger SpeechStart/Listening"
        );
        assert_eq!(
            state.pipeline.state(),
            InteractionState::Ready,
            "fallback empty TranscriptFinal must recover Thinking -> Ready (no deadlock)"
        );

        let _ = vad_cmd_tx.send(VadCommand::Shutdown);
        vad_shutdown.store(true, Ordering::SeqCst);
        vad_join.join().expect("VAD join failed");
        let _ = stt_tx.send(SttCommand::ResetStream);
    }

    // ---------- Subtest E: Silence guard ----------
    {
        assert!(Instant::now() < test_deadline, "Test exceeded overall deadline");

        state.pipeline.set_state(InteractionState::Ready);
        state.pipeline.turn_id.store(5, Ordering::Relaxed);
        vad_shutdown.store(false, Ordering::SeqCst);

        let (vox_event_tx, vox_event_rx) = mpsc::channel::<VoxEvent>();
        let _router_join = spawn_router(app.clone(), vox_event_rx).expect("router");
        let (vad_cmd_tx, mut producer, vad_join) = spawn_vad(
            stt_tx.clone(),
            vox_event_tx,
            state_atomic.clone(),
            turn_id_atomic.clone(),
            Arc::clone(&audio_suppressed),
            Arc::clone(&ingestion_gate),
            Arc::clone(&vad_shutdown),
        );

        common::audio::stream_silence_frames(&mut producer, 100);
        common::audio::wait_for_buffer_drain(&producer, 5);

        common::harness::vad_quiescence_barrier(&vad_cmd_tx);
        common::harness::assert_channel_empty(
            &pipeline_event_rx,
            "Silence guard — pipeline_event_rx must be empty after silence streaming",
        );
        assert_eq!(
            state.pipeline.state(),
            InteractionState::Ready,
            "silence must not open a turn"
        );

        let _ = vad_cmd_tx.send(VadCommand::Shutdown);
        vad_shutdown.store(true, Ordering::SeqCst);
        vad_join.join().expect("VAD join failed");
    }

    // ---------- Teardown ----------
    assert!(Instant::now() < test_deadline, "Test exceeded overall deadline");

    let _ = stt_tx.send(SttCommand::Shutdown);
    stt_shutdown.store(true, Ordering::SeqCst);
    stt_join.join().expect("STT worker panicked");

    common::harness::assert_channel_empty(
        &pipeline_event_rx,
        "pipeline_event_rx must be empty after the STT worker joined",
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

    let vad_config = production_vad_config();

    let silero_backend = common::harness::create_test_vad_backend(
        vox_lib::services::vad::VadBackendOption::SileroVad,
        vad_config.initial_threshold,
    );
    let (stt_tx, _stt_rx) = mpsc::channel();

    let (vox_event_tx, vox_event_rx) = mpsc::channel::<VoxEvent>();
    let (vad_cmd_tx, mut producer, vad_join) = common::harness::setup_vad_actor(
        silero_backend,
        stt_tx,
        vad_config,
        state_atomic,
        turn_id_atomic,
        audio_suppressed,
        ingestion_gate,
        vad_shutdown.clone(),
        vox_event_tx,
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
    common::harness::vad_quiescence_barrier(&vad_cmd_tx);
    common::harness::assert_channel_empty(
        &vox_event_rx,
        "Silero boundary — vox_event_rx must be empty before silence guard",
    );
    common::audio::stream_silence_frames(&mut producer, 80);
    common::audio::wait_for_buffer_drain(&producer, 5);
    common::harness::vad_quiescence_barrier(&vad_cmd_tx);

    let _ = vad_cmd_tx.send(VadCommand::Shutdown);
    vad_shutdown.store(true, Ordering::SeqCst);
    vad_join.join().expect("Silero VAD actor panicked");

    common::harness::assert_channel_empty(
        &vox_event_rx,
        "Silero silence guard — vox_event_rx must remain empty for silence frames",
    );
}

/// PURPOSE:  SmartTurnDetector must gracefully degrade to inactive without errors when model is absent.
/// SCENARIO: Construct SmartTurnDetector with a non-existent path and evaluate predict_eou.
/// ASSERTS:  is_loaded is false, predict_eou returns Ok(None).
/// CLASS:    contract
/// REQUIRES: none
/// MUTANT:   Return Err on absent model path in SmartTurnDetector::new.
#[test]
fn smart_turn_detector_gracefully_degrades_when_model_absent() {
    let fake_path = std::path::PathBuf::from("/non/existent/path/smart_turn.onnx");
    let detector = vox_lib::services::vad::turn_detector::SmartTurnDetector::new(&fake_path)
        .expect("SmartTurnDetector::new must succeed with graceful fallback on missing model");
    assert!(!detector.is_loaded(), "Detector must report not loaded when file is missing");
    let dummy_audio = vec![0.0f32; 16000];
    let result = detector
        .predict_eou(&dummy_audio)
        .expect("predict_eou must succeed with None when model is absent");
    assert!(result.is_none(), "predict_eou must return None for dormant detector");
}
