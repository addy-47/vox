//! ============================================================================
//! dictation_ptt_test.rs — Dictation PTT Pipeline Integration Tests (Seam 8)
//! ============================================================================
//! Category     : Integration Test
//! Component    : pipeline/dictation, services/stt
//! Prerequisites: ~/.vox/models/stt/nemotron-3.5/
//! Execution    : cargo test --test dictation_ptt_test --release -- --nocapture
//! Metrics      : Dictation Transcription Fidelity (Levenshtein >= 0.90), Buffer Lifecycle, Zero-LLM Invariant
//! ============================================================================

mod common;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use common::audio::{decode_wav_to_mono_16k, stream_audio_to_ring_buffer, wait_for_buffer_drain};
use common::harness::{
    assert_channel_empty_after, attach_mock_engine_with_vad_to_state,
    collect_all_final_transcripts, get_test_app_and_state, setup_stt_worker, setup_vad_actor,
};
use common::paths::get_asset_path;
use common::scoring::calculate_similarity;

use vox_lib::core::events::VoxEvent;
use vox_lib::core::settings::{AudioOutputMode, InteractionMode};
use vox_lib::core::state::DictationState;
use vox_lib::pipeline::dictation::{
    handle_event as handle_dictation_event, handle_hotkey_press, handle_hotkey_release,
};
use vox_lib::services::stt::SttCommand;
use vox_lib::services::vad::VadActorConfig;
use vox_lib::services::vad::VadCommand;

const EN_GROUND_TRUTH: &str =
    "Hey Vox, good morning! Can you check my calendar and give me a quick briefing on today's scheduled meetings?";
const MIN_SIMILARITY_THRESHOLD: f32 = 0.90;

/// Positive: Audio accumulation during hotkey press feeds into STT on release and outputs high-fidelity transcript.
#[tokio::test]
async fn test_dictation_ptt_audio_accumulation_en() {
    let start_time = Instant::now();
    let max_test_duration = Duration::from_secs(30);

    let clip_path = get_asset_path("edgetts_01_en_briefing.wav");
    let audio = decode_wav_to_mono_16k(&clip_path).expect("Failed to decode EN WAV");

    let (app, state) = get_test_app_and_state();
    let (stt_tx, pipeline_event_rx, engine_shutdown, stt_handle) = setup_stt_worker(&app);

    let vad_config = VadActorConfig {
        initial_threshold: 0.5,
        initial_noise_gate: 0.001,
        initial_mode: InteractionMode::PTT,
        initial_audio_mode: AudioOutputMode::Speaker,
    };
    let (vad_cmd_tx, _vad_vox_event_rx, mut producer, vad_handle) = setup_vad_actor(
        stt_tx.clone(),
        vad_config,
        Arc::clone(&state.pipeline.current_state_atomic),
        Arc::new(AtomicBool::new(false)),
        engine_shutdown.clone(),
    );

    attach_mock_engine_with_vad_to_state(&app, &state, stt_tx.clone(), vad_cmd_tx.clone());

    // 1. Press hotkey -> State becomes Recording
    handle_hotkey_press(&app, &state)
        .await
        .expect("handle_hotkey_press failed");
    assert_eq!(
        state.pipeline.dictation_state(),
        DictationState::Recording,
        "Dictation state must be Recording upon hotkey press"
    );

    // 2. Stream audio into SPSC ring buffer while hotkey is held
    stream_audio_to_ring_buffer(&audio, &mut producer);
    wait_for_buffer_drain(&producer, 1);

    // 3. Release hotkey -> State transitions out of Recording and dispatches to STT
    handle_hotkey_release(&app, &state)
        .await
        .expect("handle_hotkey_release failed");
    assert_ne!(
        state.pipeline.dictation_state(),
        DictationState::Recording,
        "Dictation state must transition away from Recording upon release"
    );

    // 4. Collect final transcript from STT worker
    let transcript = collect_all_final_transcripts(&pipeline_event_rx, 1, Duration::from_secs(20));
    assert!(
        !transcript.trim().is_empty(),
        "Transcribed text must not be empty"
    );

    let sim = calculate_similarity(&transcript, EN_GROUND_TRUTH);
    println!(
        "\n[Dictation PTT EN Test] Hypothesis: '{}'\nGround Truth: '{}'\nSimilarity: {:.3}",
        transcript, EN_GROUND_TRUTH, sim
    );
    assert!(
        sim >= MIN_SIMILARITY_THRESHOLD,
        "Transcript similarity {:.3} must be >= {:.3}",
        sim,
        MIN_SIMILARITY_THRESHOLD
    );

    // Teardown
    let _ = vad_cmd_tx.send(VadCommand::Shutdown);
    let _ = stt_tx.send(SttCommand::Shutdown);
    engine_shutdown.store(true, Ordering::Relaxed);
    vad_handle
        .join()
        .expect("VAD actor thread panicked during dictation teardown");
    stt_handle
        .join()
        .expect("STT worker thread panicked during dictation teardown");

    assert!(
        start_time.elapsed() < max_test_duration,
        "Dictation PTT EN test exceeded hard timeout of 30s"
    );
}

/// Guard (NEGATIVE): Release without streaming audio must transition to Idle without dispatching STT inference.
#[tokio::test]
async fn test_dictation_ptt_empty_buffer_guard() {
    let start_time = Instant::now();
    let max_test_duration = Duration::from_secs(10);

    let (app, state) = get_test_app_and_state();
    let (stt_tx, pipeline_event_rx, engine_shutdown, stt_handle) = setup_stt_worker(&app);

    let vad_config = VadActorConfig {
        initial_threshold: 0.5,
        initial_noise_gate: 0.001,
        initial_mode: InteractionMode::PTT,
        initial_audio_mode: AudioOutputMode::Speaker,
    };
    let (vad_cmd_tx, _vad_vox_event_rx, _producer, vad_handle) = setup_vad_actor(
        stt_tx.clone(),
        vad_config,
        Arc::clone(&state.pipeline.current_state_atomic),
        Arc::new(AtomicBool::new(false)),
        engine_shutdown.clone(),
    );

    attach_mock_engine_with_vad_to_state(&app, &state, stt_tx.clone(), vad_cmd_tx.clone());

    // 1. Press hotkey -> State becomes Recording
    handle_hotkey_press(&app, &state)
        .await
        .expect("handle_hotkey_press failed");
    assert_eq!(
        state.pipeline.dictation_state(),
        DictationState::Recording,
        "Dictation state must be Recording upon hotkey press"
    );

    // 2. Release immediately without feeding any audio frames
    handle_hotkey_release(&app, &state)
        .await
        .expect("handle_hotkey_release failed");

    // 3. Assert DictationState reverted to Idle
    assert_eq!(
        state.pipeline.dictation_state(),
        DictationState::Idle,
        "Dictation state must revert to Idle on empty hold"
    );

    // 4. Assert zero downstream events were dispatched to STT
    assert_channel_empty_after(
        &pipeline_event_rx,
        Duration::from_millis(500),
        "pipeline_event_rx empty guard",
    );

    // Teardown
    let _ = vad_cmd_tx.send(VadCommand::Shutdown);
    let _ = stt_tx.send(SttCommand::Shutdown);
    engine_shutdown.store(true, Ordering::Relaxed);
    vad_handle
        .join()
        .expect("VAD actor thread panicked during empty guard teardown");
    stt_handle
        .join()
        .expect("STT worker thread panicked during empty guard teardown");

    assert!(
        start_time.elapsed() < max_test_duration,
        "Empty buffer guard test exceeded hard timeout of 10s"
    );
}

/// Invariant (NEGATIVE): Final transcript event in dictation mode must NOT dispatch commands to the LLM engine.
#[test]
fn test_dictation_does_not_invoke_llm() {
    let start_time = Instant::now();
    let max_test_duration = Duration::from_secs(10);

    let (app, state) = get_test_app_and_state();

    let (llm_tx, llm_rx) = std::sync::mpsc::channel::<vox_lib::services::llm::actor::LlmCommand>();
    let (stt_tx, _) = std::sync::mpsc::channel();
    let (vad_tx, _) = std::sync::mpsc::channel();
    let (pipeline_tx, _) = std::sync::mpsc::channel();
    let (telemetry_tx, _) = crossbeam_channel::unbounded();

    let playback_engine = Arc::new(
        vox_lib::services::audio::PlaybackEngine::new(
            Arc::new(std::sync::atomic::AtomicBool::new(false)),
            Arc::clone(&state.pipeline.current_state_atomic),
            vox_lib::services::audio::playback::PlaybackTelemetryHandles {
                energy: Arc::new(std::sync::atomic::AtomicU32::new(0)),
                low: Arc::new(std::sync::atomic::AtomicU32::new(0)),
                mid: Arc::new(std::sync::atomic::AtomicU32::new(0)),
                high: Arc::new(std::sync::atomic::AtomicU32::new(0)),
                underruns: Arc::new(std::sync::atomic::AtomicU64::new(0)),
            },
        )
        .expect("Failed to create mock PlaybackEngine"),
    );

    let engine = vox_lib::core::state::VoxEngine {
        audio_stream: vox_lib::services::audio::AudioStream::mock(),
        stt_tx,
        vad_tx,
        llm_tx: Some(llm_tx),
        tts_tx: None,
        telemetry_tx,
        pipeline_tx,
        playback_engine,
        stt_handle: None,
        vad_handle: None,
        llm_handle: None,
        tts_handle: None,
        orchestrator_handle: None,
    };
    if let Ok(mut guard) = state.engine.try_lock() {
        *guard = Some(engine);
    } else {
        *state.engine.blocking_lock() = Some(engine);
    }

    // Dispatch TranscriptFinal to the dictation handler
    handle_dictation_event(
        &app,
        &state,
        VoxEvent::TranscriptFinal {
            turn_id: 1,
            text: "Insert this text directly into active application".to_string(),
        },
    );

    // Verify LLM channel is untouched
    assert_channel_empty_after(
        &llm_rx,
        Duration::from_millis(500),
        "llm_rx must receive zero commands during dictation flow",
    );

    // Verify dictation state settled to Idle
    assert_eq!(
        state.pipeline.dictation_state(),
        DictationState::Idle,
        "Dictation state must be Idle after final transcript processing"
    );

    assert!(
        start_time.elapsed() < max_test_duration,
        "Dictation zero-LLM invariant test exceeded hard timeout of 10s"
    );
}
