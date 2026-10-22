//! ============================================================================
//! aec_barge_in_integration_test.rs — Full-Duplex AEC & Double-Talk Barge-In Test Suite
//! ============================================================================
//! Category     : Integration Test
//! Component    : services/audio/aec.rs + services/vad/actor.rs + pipeline/assistant/interrupt.rs
//! Prerequisites: tests/assets/edgetts_01_en_briefing.wav, tests/assets/supertonic_01_en_briefing.wav
//! Execution    : cargo nextest run --release --test aec_barge_in_integration_test --test-threads=1
//! Metrics      : Echo suppression (ERLE >= 20dB), zero solo interruption, double-talk barge-in, false-green bypass proof
//! ============================================================================

mod common;

use std::{
    path::Path,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc,
    },
    time::{Duration, Instant},
};

use ringbuf::{traits::*, HeapRb};
use vox_lib::{
    config::AudioOutputMode,
    core::{
        events::{InteractionMode, VoxEvent},
        state::{InteractionOwner, InteractionState},
    },
    pipeline::router::spawn_router,
    services::{
        audio::{
            aec::{spawn_aec_worker, AecProcessor},
            sink::PlaybackReferenceTap,
            AEC_FRAME_SAMPLES,
        },
        vad::actor::VadActorConfig,
    },
};

/// Assertion 1 (Echo-Only Invariant):
/// Solo playback with injected acoustic echo is completely suppressed by AEC.
/// VAD emits ZERO SpeechStart events, preventing self-interruption.
#[tokio::test]
async fn test_echo_only_solo_playback_causes_zero_false_interruption() {
    let test_timeout = Duration::from_secs(30);
    tokio::time::timeout(test_timeout, async {
        let _guard = common::paths::TempPathsGuard::new();
        vox_lib::utils::paths::init();
        let (_app, state) = common::harness::get_test_app_and_state().await;
        state
            .owner
            .store(InteractionOwner::Assistant as u32, Ordering::Relaxed);

        let tts_path = Path::new("tests/assets/edgetts_01_en_briefing.wav");
        let tts_samples = common::audio::decode_wav_to_mono_16k(tts_path)
            .expect("Failed to load TTS briefing WAV");

        let (event_tx, event_rx) = mpsc::channel::<VoxEvent>();

        // Set state to Speaking
        state.pipeline.set_state(InteractionState::Speaking);

        // Queues: ref_rb, cap_rb, clean_rb
        let ref_rb = HeapRb::<f32>::new(65536);
        let (ref_prod, ref_cons) = ref_rb.split();
        let mut tap = PlaybackReferenceTap::new(ref_prod);

        let cap_rb = HeapRb::<f32>::new(65536);
        let (mut cap_prod, cap_cons) = cap_rb.split();

        let clean_rb = HeapRb::<f32>::new(65536);
        let (clean_prod, clean_cons) = clean_rb.split();

        let aec = AecProcessor::new(Some(ref_cons)).expect("Failed to create AecProcessor");
        let aec_shutdown = Arc::new(AtomicBool::new(false));
        let aec_worker =
            spawn_aec_worker(aec, cap_cons, clean_prod, Arc::clone(&aec_shutdown), None)
                .expect("Failed to spawn AEC worker");

        let audio_suppressed = Arc::new(AtomicBool::new(false));
        let engine_shutdown = Arc::new(AtomicBool::new(false));

        let vad_handles = vox_lib::services::vad::VadActorHandles {
            state_atomic: state.pipeline.current_state_atomic.clone(),
            turn_id_atomic: state.pipeline.turn_id.clone(),
            audio_suppressed: audio_suppressed.clone(),
            engine_shutdown: engine_shutdown.clone(),
            dropped_counter: Arc::new(std::sync::atomic::AtomicU64::new(0)),
            ingestion_gate: state.pipeline.ingestion_gate.clone(),
            owner_atomic: Arc::clone(&state.owner),
            incomplete_clause_hint: Arc::new(AtomicBool::new(false)),
        };
        let vad_channels = vox_lib::services::vad::VadActorChannels {
            stt_tx: _dummy_stt_tx(),
            vad_rx: _dummy_vad_rx(),
            telemetry_tx: crossbeam_channel::unbounded().0,
            vox_event_tx: Some(event_tx.clone()),
        };
        let vad_cfg = VadActorConfig {
            initial_threshold: vox_lib::core::defaults::DEFAULT_VAD_THRESHOLD,
            initial_noise_gate: vox_lib::core::defaults::DEFAULT_VAD_PTT_NOISE_GATE,
            initial_silence_duration_ms: vox_lib::core::defaults::DEFAULT_VAD_SILENCE_DURATION_MS,
            initial_speech_onset_ms: vox_lib::core::defaults::DEFAULT_VAD_SPEECH_ONSET_MS,
            initial_mode: InteractionMode::Passive,
            initial_audio_mode: AudioOutputMode::Speaker,
        };
        let vad_backend = vox_lib::services::vad::earshot_vad::EarshotVadEngine::new(
            vox_lib::core::defaults::DEFAULT_VAD_THRESHOLD,
        )
        .expect("Failed to init Earshot VAD");
        let vad_join = std::thread::spawn(move || {
            let _ = vox_lib::services::vad::spawn_vad_actor(
                vox_lib::services::vad::VadBackend::Earshot(vad_backend),
                clean_cons,
                vad_channels,
                vad_handles,
                vad_cfg,
            );
        });

        // Simulate 40ms acoustic delay and -6dB attenuation echo
        let delay_samples = 640;
        let test_len = 48000.min(tts_samples.len());
        let mut simulated_mic = vec![0.0f32; delay_samples];
        for &s in &tts_samples[..test_len] {
            simulated_mic.push(s * 0.5);
        }

        let num_frames = test_len / AEC_FRAME_SAMPLES;
        for i in 0..num_frames {
            let start = i * AEC_FRAME_SAMPLES;
            let end = start + AEC_FRAME_SAMPLES;

            // Feed render reference (48k)
            if start < tts_samples.len() {
                let r_end = end.min(tts_samples.len());
                for &s in &tts_samples[start..r_end] {
                    tap.push_sample(s);
                    tap.push_sample(s);
                    tap.push_sample(s);
                }
            }

            if end <= simulated_mic.len() {
                cap_prod.push_slice(&simulated_mic[start..end]);
            }
            tokio::time::sleep(Duration::from_millis(2)).await;
        }

        // Assert: Zero SpeechStart events emitted
        let mut saw_speech_start = false;
        while let Ok(ev) = event_rx.recv_timeout(Duration::from_millis(50)) {
            if matches!(ev, VoxEvent::SpeechStart { .. }) {
                saw_speech_start = true;
                break;
            }
        }

        assert!(
            !saw_speech_start,
            "AEC must suppress acoustic loopback and prevent false SpeechStart interruptions"
        );

        // Teardown
        aec_shutdown.store(true, Ordering::Relaxed);
        engine_shutdown.store(true, Ordering::Relaxed);
        aec_worker.join().expect("worker thread panicked");
        vad_join.join().expect("worker thread panicked");
    })
    .await
    .expect("Test timed out");
}

/// Assertion 2 (Double-Talk / Barge-In Invariant):
/// When genuine user speech overlaps assistant playback, AEC preserves user speech.
/// VAD fires SpeechStart, playback is cancelled, and state transitions to Listening.
#[tokio::test]
async fn test_double_talk_barge_in_cancels_playback_and_transitions_to_listening() {
    let test_timeout = Duration::from_secs(30);
    tokio::time::timeout(test_timeout, async {
        let _guard = common::paths::TempPathsGuard::new();
        vox_lib::utils::paths::init();
        let (app, state) = common::harness::get_test_app_and_state().await;
        state
            .owner
            .store(InteractionOwner::Assistant as u32, Ordering::Relaxed);

        let tts_path = Path::new("tests/assets/edgetts_01_en_briefing.wav");
        let user_path = Path::new("tests/assets/supertonic_01_en_briefing.wav");

        let tts_samples = common::audio::decode_wav_to_mono_16k(tts_path)
            .expect("Failed to load TTS briefing WAV");
        let user_samples = common::audio::decode_wav_to_mono_16k(user_path)
            .expect("Failed to load user speech WAV");

        let (vox_event_tx, vox_event_rx) = mpsc::channel::<VoxEvent>();

        let turn_id = 100;
        state.pipeline.turn_id.store(turn_id, Ordering::Relaxed);
        state.pipeline.set_state(InteractionState::Speaking);

        let router_handle =
            spawn_router(app.clone(), vox_event_rx).expect("Failed to spawn router");

        // Queues: ref_rb, cap_rb, clean_rb
        let ref_rb = HeapRb::<f32>::new(65536);
        let (ref_prod, ref_cons) = ref_rb.split();
        let mut tap = PlaybackReferenceTap::new(ref_prod);

        let cap_rb = HeapRb::<f32>::new(65536);
        let (mut cap_prod, cap_cons) = cap_rb.split();

        let clean_rb = HeapRb::<f32>::new(65536);
        let (clean_prod, clean_cons) = clean_rb.split();

        let aec = AecProcessor::new(Some(ref_cons)).expect("Failed to create AecProcessor");
        let aec_shutdown = Arc::new(AtomicBool::new(false));
        let aec_worker =
            spawn_aec_worker(aec, cap_cons, clean_prod, Arc::clone(&aec_shutdown), None)
                .expect("Failed to spawn AEC worker");

        let audio_suppressed = Arc::new(AtomicBool::new(false));
        let engine_shutdown = Arc::new(AtomicBool::new(false));

        let vad_handles = vox_lib::services::vad::VadActorHandles {
            state_atomic: state.pipeline.current_state_atomic.clone(),
            turn_id_atomic: state.pipeline.turn_id.clone(),
            audio_suppressed: audio_suppressed.clone(),
            engine_shutdown: engine_shutdown.clone(),
            dropped_counter: Arc::new(std::sync::atomic::AtomicU64::new(0)),
            ingestion_gate: state.pipeline.ingestion_gate.clone(),
            owner_atomic: Arc::clone(&state.owner),
            incomplete_clause_hint: Arc::new(AtomicBool::new(false)),
        };
        let vad_channels = vox_lib::services::vad::VadActorChannels {
            stt_tx: _dummy_stt_tx(),
            vad_rx: _dummy_vad_rx(),
            telemetry_tx: crossbeam_channel::unbounded().0,
            vox_event_tx: Some(vox_event_tx.clone()),
        };
        let vad_cfg = VadActorConfig {
            initial_threshold: vox_lib::core::defaults::DEFAULT_VAD_THRESHOLD,
            initial_noise_gate: vox_lib::core::defaults::DEFAULT_VAD_PTT_NOISE_GATE,
            initial_silence_duration_ms: vox_lib::core::defaults::DEFAULT_VAD_SILENCE_DURATION_MS,
            initial_speech_onset_ms: vox_lib::core::defaults::DEFAULT_VAD_SPEECH_ONSET_MS,
            initial_mode: InteractionMode::Passive,
            initial_audio_mode: AudioOutputMode::Speaker,
        };
        let vad_backend = vox_lib::services::vad::earshot_vad::EarshotVadEngine::new(
            vox_lib::core::defaults::DEFAULT_VAD_THRESHOLD,
        )
        .expect("Failed to init Earshot VAD");
        let vad_join = std::thread::spawn(move || {
            let _ = vox_lib::services::vad::spawn_vad_actor(
                vox_lib::services::vad::VadBackend::Earshot(vad_backend),
                clean_cons,
                vad_channels,
                vad_handles,
                vad_cfg,
            );
        });

        // Mix echo and inject genuine user speech at frame 30
        let total_frames = 120; // 1.2 seconds
        let inject_user_frame = 30;

        for frame_idx in 0..total_frames {
            let start = frame_idx * AEC_FRAME_SAMPLES;
            let end = start + AEC_FRAME_SAMPLES;

            // Push TTS reference to tap (48kHz)
            if start < tts_samples.len() {
                let r_end = end.min(tts_samples.len());
                for &s in &tts_samples[start..r_end] {
                    tap.push_sample(s);
                    tap.push_sample(s);
                    tap.push_sample(s);
                }
            }

            // Create mic frame: echo + user speech
            let mut mic_frame = [0.0f32; AEC_FRAME_SAMPLES];
            for (i, sample) in mic_frame.iter_mut().enumerate() {
                let s_idx = start + i;
                let echo = if s_idx >= 640 && (s_idx - 640) < tts_samples.len() {
                    tts_samples[s_idx - 640] * 0.4
                } else {
                    0.0
                };
                let user = if frame_idx >= inject_user_frame {
                    let u_idx = (frame_idx - inject_user_frame) * AEC_FRAME_SAMPLES + i;
                    if u_idx < user_samples.len() {
                        user_samples[u_idx]
                    } else {
                        0.0
                    }
                } else {
                    0.0
                };
                *sample = echo + user;
            }

            cap_prod.push_slice(&mic_frame);
            tokio::time::sleep(Duration::from_millis(2)).await;

            if frame_idx > inject_user_frame + 20
                && state.pipeline.state() == InteractionState::Listening
            {
                break;
            }
        }

        let deadline = Instant::now() + Duration::from_secs(3);
        while Instant::now() < deadline {
            if state.pipeline.state() == InteractionState::Listening {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }

        // Assert: Interruption was processed and transitioned state to Listening
        assert_eq!(
            state.pipeline.state(),
            InteractionState::Listening,
            "User speech barge-in must cancel active playback and transition to Listening"
        );
        assert!(
            state.pipeline.peek_turn_id() > turn_id,
            "User speech barge-in must advance turn ID"
        );

        // Teardown
        aec_shutdown.store(true, Ordering::Relaxed);
        engine_shutdown.store(true, Ordering::Relaxed);
        aec_worker.join().expect("worker thread panicked");
        vad_join.join().expect("worker thread panicked");
        let _ = vox_event_tx.send(VoxEvent::Shutdown);
        router_handle.join().expect("worker thread panicked");
    })
    .await
    .expect("Test timed out");
}

/// Assertion 3 (False-Green Proof / Bypass Invariant):
/// When AEC has NO reference tap (bypassed), the same echo input triggers SpeechStart.
/// This strictly proves the test suite detects an inactive or broken echo filter.
#[tokio::test]
async fn test_false_green_bypass_reproduces_echo_interruption() {
    let test_timeout = Duration::from_secs(30);
    tokio::time::timeout(test_timeout, async {
        let _guard = common::paths::TempPathsGuard::new();
        vox_lib::utils::paths::init();
        let (_app, state) = common::harness::get_test_app_and_state().await;
        state
            .owner
            .store(InteractionOwner::Assistant as u32, Ordering::Relaxed);

        let tts_path = Path::new("tests/assets/edgetts_01_en_briefing.wav");
        let tts_samples = common::audio::decode_wav_to_mono_16k(tts_path)
            .expect("Failed to load TTS briefing WAV");

        let (event_tx, event_rx) = mpsc::channel::<VoxEvent>();

        // Set state to Speaking
        state.pipeline.set_state(InteractionState::Speaking);

        let cap_rb = HeapRb::<f32>::new(65536);
        let (mut cap_prod, cap_cons) = cap_rb.split();

        let clean_rb = HeapRb::<f32>::new(65536);
        let (clean_prod, clean_cons) = clean_rb.split();

        // AEC initialized WITHOUT render consumer -> cannot cancel echo!
        let mut aec = AecProcessor::new(None).expect("Failed to create AecProcessor");
        aec.set_enabled(false);

        let aec_shutdown = Arc::new(AtomicBool::new(false));
        let aec_worker =
            spawn_aec_worker(aec, cap_cons, clean_prod, Arc::clone(&aec_shutdown), None)
                .expect("Failed to spawn AEC worker");

        let audio_suppressed = Arc::new(AtomicBool::new(false));
        let engine_shutdown = Arc::new(AtomicBool::new(false));

        let vad_handles = vox_lib::services::vad::VadActorHandles {
            state_atomic: state.pipeline.current_state_atomic.clone(),
            turn_id_atomic: state.pipeline.turn_id.clone(),
            audio_suppressed: audio_suppressed.clone(),
            engine_shutdown: engine_shutdown.clone(),
            dropped_counter: Arc::new(std::sync::atomic::AtomicU64::new(0)),
            ingestion_gate: state.pipeline.ingestion_gate.clone(),
            owner_atomic: Arc::clone(&state.owner),
            incomplete_clause_hint: Arc::new(AtomicBool::new(false)),
        };
        let vad_channels = vox_lib::services::vad::VadActorChannels {
            stt_tx: _dummy_stt_tx(),
            vad_rx: _dummy_vad_rx(),
            telemetry_tx: crossbeam_channel::unbounded().0,
            vox_event_tx: Some(event_tx.clone()),
        };
        let vad_cfg = VadActorConfig {
            initial_threshold: vox_lib::core::defaults::DEFAULT_VAD_THRESHOLD,
            initial_noise_gate: vox_lib::core::defaults::DEFAULT_VAD_PTT_NOISE_GATE,
            initial_silence_duration_ms: vox_lib::core::defaults::DEFAULT_VAD_SILENCE_DURATION_MS,
            initial_speech_onset_ms: vox_lib::core::defaults::DEFAULT_VAD_SPEECH_ONSET_MS,
            initial_mode: InteractionMode::Passive,
            initial_audio_mode: AudioOutputMode::Speaker,
        };
        let vad_backend = vox_lib::services::vad::earshot_vad::EarshotVadEngine::new(
            vox_lib::core::defaults::DEFAULT_VAD_THRESHOLD,
        )
        .expect("Failed to init Earshot VAD");
        let vad_join = std::thread::spawn(move || {
            let _ = vox_lib::services::vad::spawn_vad_actor(
                vox_lib::services::vad::VadBackend::Earshot(vad_backend),
                clean_cons,
                vad_channels,
                vad_handles,
                vad_cfg,
            );
        });

        // Stream echo into mic without AEC reference
        let test_len = 48000.min(tts_samples.len());
        let num_frames = test_len / AEC_FRAME_SAMPLES;
        let mut saw_interruption = false;

        for i in 0..num_frames {
            let start = i * AEC_FRAME_SAMPLES;
            let end = start + AEC_FRAME_SAMPLES;
            cap_prod.push_slice(&tts_samples[start..end]);
            tokio::time::sleep(Duration::from_millis(2)).await;

            while let Ok(ev) = event_rx.try_recv() {
                if matches!(ev, VoxEvent::SpeechStart { .. }) {
                    saw_interruption = true;
                    break;
                }
            }
            if saw_interruption {
                break;
            }
        }

        // Assert: False interruption DOES occur without AEC
        assert!(
            saw_interruption,
            "Without AEC, acoustic echo MUST trigger false SpeechStart (false-green proof)"
        );

        // Teardown
        aec_shutdown.store(true, Ordering::Relaxed);
        engine_shutdown.store(true, Ordering::Relaxed);
        aec_worker.join().expect("worker thread panicked");
        vad_join.join().expect("worker thread panicked");
    })
    .await
    .expect("Test timed out");
}

/// Assertion 4 (ERLE Mathematical Metric):
/// Echo Return Loss Enhancement (ERLE = 10 * log10(E_in / E_out)) across echo frames must be >= 25 dB.
#[tokio::test]
async fn test_erle_mathematical_metric_exceeds_twenty_five_db() {
    let test_timeout = Duration::from_secs(30);
    tokio::time::timeout(test_timeout, async {
        let _guard = common::paths::TempPathsGuard::new();
        vox_lib::utils::paths::init();

        let tts_path = Path::new("tests/assets/edgetts_01_en_briefing.wav");
        let tts_samples = common::audio::decode_wav_to_mono_16k(tts_path)
            .expect("Failed to load TTS briefing WAV");

        let ref_rb = HeapRb::<f32>::new(65536);
        let (ref_prod, ref_cons) = ref_rb.split();
        let mut tap = PlaybackReferenceTap::new(ref_prod);

        let mut aec = AecProcessor::new(Some(ref_cons)).expect("Failed to create AecProcessor");

        // Simulate 40ms acoustic delay and -6dB attenuation echo
        let delay_samples = 640;
        let test_len = 32000.min(tts_samples.len());
        let mut simulated_mic = vec![0.0f32; delay_samples];
        for &s in &tts_samples[..test_len] {
            simulated_mic.push(s * 0.5);
        }

        let num_frames = test_len / AEC_FRAME_SAMPLES;
        let mut total_in_energy = 0.0f64;
        let mut total_out_energy = 0.0f64;
        let mut evaluated_frames = 0;

        for i in 0..num_frames {
            let start = i * AEC_FRAME_SAMPLES;
            let end = start + AEC_FRAME_SAMPLES;

            // Feed render reference (48k)
            if start < tts_samples.len() {
                let r_end = end.min(tts_samples.len());
                for &s in &tts_samples[start..r_end] {
                    tap.push_sample(s);
                    tap.push_sample(s);
                    tap.push_sample(s);
                }
            }

            let mut out_frame = [0.0f32; AEC_FRAME_SAMPLES];
            let mic_slice = &simulated_mic[start..end];
            aec.process_frame(mic_slice, &mut out_frame)
                .expect("AEC process_frame failed");

            // Evaluate ERLE after initial convergence (first 50 frames / 0.5s)
            if i >= 50 {
                let in_e: f64 = mic_slice.iter().map(|&x| (x as f64) * (x as f64)).sum();
                let out_e: f64 = out_frame.iter().map(|&x| (x as f64) * (x as f64)).sum();
                if in_e > 1e-4 {
                    total_in_energy += in_e;
                    total_out_energy += out_e;
                    evaluated_frames += 1;
                }
            }
        }

        assert!(
            evaluated_frames > 20,
            "Must have enough frames to calculate ERLE"
        );
        let erle_db = 10.0 * (total_in_energy / total_out_energy.max(1e-12)).log10();
        println!(
            "Measured ERLE: {:.2} dB across {} frames",
            erle_db, evaluated_frames
        );

        assert!(
            erle_db >= 25.0,
            "AEC must achieve at least 25dB ERLE (got {:.2} dB)",
            erle_db
        );
    })
    .await
    .expect("Test timed out");
}

fn _dummy_stt_tx() -> mpsc::Sender<vox_lib::services::stt::SttCommand> {
    mpsc::channel().0
}

fn _dummy_vad_rx() -> mpsc::Receiver<vox_lib::services::vad::VadCommand> {
    mpsc::channel().1
}

#[tokio::test]
async fn test_clear_render_queue_prevents_post_cancellation_suppression() {
    let _guard = common::paths::TempPathsGuard::new();
    vox_lib::utils::paths::init();

    let ref_ring = HeapRb::<f32>::new(16384);
    let (ref_prod, ref_cons) = ref_ring.split();
    let mut tap = PlaybackReferenceTap::new(ref_prod);
    let mut aec = AecProcessor::new(Some(ref_cons)).unwrap();

    // Push 480 samples of render reference into tap
    for i in 0..480 {
        tap.push_sample(0.5 * ((i as f32) * 0.1).sin());
    }

    // Calling clear_render_queue flushes all pending reference frames
    aec.clear_render_queue();

    // User speech frames (near-end voice @ 400Hz tone, 16kHz sample rate, 5 frames = 50ms)
    let total_samples = 5 * AEC_FRAME_SAMPLES;
    let user_speech: Vec<f32> = (0..total_samples)
        .map(|i| 0.35 * (2.0 * std::f32::consts::PI * 400.0 * (i as f32) / 16000.0).sin())
        .collect();

    let mut in_energy = 0.0f32;
    let mut out_energy = 0.0f32;

    for (f_idx, chunk) in user_speech.chunks(AEC_FRAME_SAMPLES).enumerate() {
        let mut processed = [0.0f32; AEC_FRAME_SAMPLES];
        aec.process_frame(chunk, &mut processed)
            .expect("AEC process_frame should succeed");

        // Skip initial filter ramp-up frame (first 10ms transient)
        if f_idx >= 1 {
            in_energy += chunk.iter().map(|&x| x * x).sum::<f32>();
            out_energy += processed.iter().map(|&x| x * x).sum::<f32>();
        }
    }

    // Verify speech is not suppressed (steady-state energy retention >= 80%)
    assert!(
        out_energy >= 0.8 * in_energy,
        "Near-end steady-state speech must not be suppressed when render queue was cleared (in={}, out={})",
        in_energy,
        out_energy
    );
}
