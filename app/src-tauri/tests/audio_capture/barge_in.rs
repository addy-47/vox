//! ============================================================================
//! barge_in.rs — full duplex AEC and double talk barge-in
//! ============================================================================
//! Category  : Integration (Seam 11 — echo suppression → VAD → barge-in)
//! Component : services/audio/aec.rs + services/vad/actor.rs
//!             + pipeline/assistant/interrupt.rs
//! Binary    : audio_capture_aec_test
//! Prereqs   : the test audio assets
//! Run       : cargo nextest run --test audio_capture_aec_test --release --test-threads=1
//! ============================================================================

use std::{
    path::Path,
    sync::{Arc, atomic::Ordering},
    time::{Duration, Instant},
};

use ringbuf::traits::{Observer, Producer};

use vox_lib::{
    config::AudioOutputMode,
    core::{
        defaults::{
            DEFAULT_VAD_SILENCE_DURATION_MS, DEFAULT_VAD_SPEECH_ONSET_MS,
            DEFAULT_VAD_THRESHOLD, DEFAULT_VAD_PTT_NOISE_GATE,
        },
        events::InteractionMode,
        state::{AppState, InteractionOwner, InteractionState},
    },
    services::{
        audio::{aec::AecCommand, AEC_FRAME_SAMPLES},
        stt::SttCommand,
        vad::actor::VadActorConfig,
    },
};

use crate::common;

/// Horizon comfortably past the ~6.0 s WebRTC AEC3 TransparentMode transition.
const MIN_TEST_SAMPLES_16K: usize = 104_320;

/// A Speaking-state scene with the real capture→AEC→VAD→router chain wired by
/// `AudioSceneRig`. Playback renders through the real PlaybackStreamContext so
/// the AEC reference tap feeds the same path as production.
struct EchoScene {
    _guard: common::paths::TempPathsGuard,
    state: Arc<AppState>,
    rig: common::harness::AudioSceneRig,
    tts_16k: Vec<f32>,
}

impl EchoScene {
    async fn start(tts_path: &str, turn_id: u32) -> Self {
        let guard = common::paths::TempPathsGuard::new();
        vox_lib::utils::paths::init();
        let (app, state) = common::harness::get_test_app_and_state().await;
        state.owner.store(InteractionOwner::Assistant as u32, Ordering::Relaxed);
        state.pipeline.turn_id.store(turn_id, Ordering::Relaxed);
        state.pipeline.pending_synthesis_jobs.store(0, Ordering::Relaxed);
        state.pipeline.set_state(InteractionState::Speaking);

        let tts_16k = common::audio::decode_wav_to_mono_16k(Path::new(tts_path))
            .expect("Failed to load TTS briefing WAV");
        assert!(
            tts_16k.len() >= MIN_TEST_SAMPLES_16K,
            "edgetts_01 must cover >6.5 s @16kHz for the AEC3 TransparentMode horizon"
        );
        let (tts_24k, rate) = common::audio::decode_wav_to_mono_native(Path::new(tts_path))
            .expect("Failed to load native-rate TTS briefing WAV");
        assert_eq!(rate, 24_000, "edgetts_01 is a native 24 kHz asset");

        let vad_cfg = VadActorConfig {
            initial_threshold: DEFAULT_VAD_THRESHOLD,
            initial_noise_gate: DEFAULT_VAD_PTT_NOISE_GATE,
            initial_silence_duration_ms: DEFAULT_VAD_SILENCE_DURATION_MS,
            initial_speech_onset_ms: DEFAULT_VAD_SPEECH_ONSET_MS,
            initial_mode: InteractionMode::Passive,
            initial_audio_mode: AudioOutputMode::Speaker,
        };
        let vad_backend = common::harness::create_test_vad_backend(
            vox_lib::services::vad::VadBackendOption::Earshot,
            DEFAULT_VAD_THRESHOLD,
        );
        let rig = common::harness::AudioSceneRig::new(app.clone(), Arc::clone(&state), vad_cfg, vad_backend);

        // Queue the whole clip into the playback buffer; it drains while the mic is fed.
        rig.engine.ingest_chunk(&tts_24k);

        Self {
            _guard: guard,
            state,
            rig,
            tts_16k,
        }
    }

    /// Streams mic samples in 10ms frames; the playback sink is pumped one
    /// 480-frame stereo slice per 10 ms so the reference tap stays in step.
    fn feed_mixed(&mut self, samples: &[f32]) {
        let frame = AEC_FRAME_SAMPLES;
        let mut stereo_out = [0.0f32; 960];
        let mut off = 0;
        while off < samples.len() {
            let chunk = &samples[off..(off + frame).min(samples.len())];
            let mut padded = chunk.to_vec();
            padded.resize(frame, 0.0);
            while self.rig.cap_prod.vacant_len() < frame {
                std::thread::sleep(Duration::from_millis(1));
            }
            self.rig.cap_prod.push_slice(&padded);
            self.rig.sink.process_output_buffer(&mut stereo_out);
            off += frame;
        }
    }

    /// Pumps the sink until the buffered playback drains and PlaybackFinished
    /// is emitted (turn disarmed, no pending synthesis jobs).
    fn drain_sink(&mut self, max_frames: usize) -> Vec<f32> {
        common::harness::pump_playback_sink_until_finished(&mut self.rig.sink, max_frames)
    }

    fn interrupt_aec_via_engine_channel(&self, command: AecCommand) {
        if let Ok(guard) = self.state.engine.try_lock() {
            if let Some(ref e) = *guard {
                if let Some(ref tx) = e.aec_tx {
                    tx.send(command).expect("AEC worker must accept commands");
                    return;
                }
            }
        }
        panic!("attached engine must hold an AecCommand sender");
    }
}

/// PURPOSE:  The assistant's own echo must never interrupt the assistant.
/// SCENARIO: Only the playback reference reaches the capture, at 120 ms hardware skew,
///           for the full duration of a >6.5 s utterance.
/// ASSERTS:  No SpeechStart fires (turn_id is never incremented and the machine never
///           reaches Listening), and no STT commands are emitted.
/// CLASS:    suppression
/// REQUIRES: the test audio assets
/// MUTANT:   Let the echo reach the VAD unattenuated.
#[tokio::test]
#[ignore = "PROD-BUG: AEC-120MS — AEC does not suppress 120 ms PipeWire render→capture skew; see docs/tests/test-findings.md F-001"]
async fn solo_playback_over_six_seconds_never_interrupts_with_pipewire_delay() {
    let test_timeout = Duration::from_secs(60);
    tokio::time::timeout(test_timeout, async {
        let mut scene = EchoScene::start("tests/assets/edgetts_01_en_briefing.wav", 1201).await;

        let mic = common::harness::scene_mic(&scene.tts_16k, None);
        scene.feed_mixed(&mic);
        // Let the remaining playback tail flush and settle before judging suppression.
        let trailing_silence = vec![0.0f32; 32_000];
        scene.feed_mixed(&trailing_silence);
        let _ = scene.drain_sink(3_000);
        // Give the router a moment to process PlaybackFinished if it was emitted.
        tokio::time::sleep(Duration::from_millis(300)).await;

        assert_eq!(
            scene.state.pipeline.turn_id.load(Ordering::Relaxed),
            1201,
            "echo must not increment the turn identifier"
        );
        assert_ne!(
            scene.state.pipeline.state(),
            InteractionState::Listening,
            "echo alone must not trigger barge-in"
        );
        common::harness::assert_channel_empty(&scene.rig.stt_rx, "echo-only playback must not emit STT commands");
    })
    .await
    .expect("Test timed out");
}

/// PURPOSE:  Real user speech during Speaking must interrupt the assistant.
/// SCENARIO: After 2.5 s of clean reference, the mic hears double talk (user speech
///           over the speaker's playback).
/// ASSERTS:  Speaking -> Listening; turn_id increments; interrupt raises cancel_flag;
///           playback buffer is flushed by the discard path; the user's speech is
///           flushed to STT as StreamChunks carrying the NEW turn_id.
/// CLASS:    happy-path (barge-in) + interrupt wiring
/// REQUIRES: the test audio assets
/// MUTANT:   Drop SpeechStart during Speaking (bracket arms never reached).
#[tokio::test]
#[ignore = "PROD-BUG: AEC-120MS — residual echo still triggers VAD onset; see docs/tests/test-findings.md F-001"]
async fn double_talk_barge_in_cancels_the_playback_and_opens_listening() {
    let test_timeout = Duration::from_secs(60);
    tokio::time::timeout(test_timeout, async {
        const TURN: u32 = 1202;
        let mut scene = EchoScene::start("tests/assets/edgetts_01_en_briefing.wav", TURN).await;

        // Phase A: pure echo for 2.5 s — AEC locks onto the render reference.
        let phase_a = common::harness::scene_mic(&scene.tts_16k[..40_000], None);
        scene.feed_mixed(&phase_a);

        // Phase B: double talk — the same window again, but the mic now carries
        // real user speech concurrently with the speaker's echo.
        let user = common::audio::decode_wav_to_mono_16k(Path::new("tests/assets/supertonic_01_en_briefing.wav"))
            .expect("Failed to load user speech WAV");
        let phase_b = common::harness::scene_mic(&scene.tts_16k[..40_000], Some(&user[..40_000]));
        scene.feed_mixed(&phase_b);

        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline {
            if scene.state.pipeline.state() == InteractionState::Listening {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        assert_eq!(
            scene.state.pipeline.state(),
            InteractionState::Listening,
            "double-talk during Speaking must barge in and switch to Listening"
        );
        assert_eq!(
            scene.state.pipeline.turn_id.load(Ordering::Relaxed),
            TURN + 1,
            "barge-in must open a new turn"
        );
        assert!(
            scene.state.pipeline.pending_synthesis_jobs.load(Ordering::Relaxed) == 0,
            "interrupt must reset pending synthesis jobs"
        );

        // The discard path in PlaybackStreamContext empties the playback buffer.
        for _ in 0..100 {
            scene.rig.sink.process_output_buffer(&mut [0.0f32; 960]);
        }
        assert_eq!(
            scene.rig.sink.consumer.occupied_len(),
            0,
            "interrupt must flush the playback buffer via the discard path"
        );

        // The user's speech must reach STT with the NEW turn_id.
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut saw_stream_chunk = false;
        while Instant::now() < deadline && !saw_stream_chunk {
            match scene.rig.stt_rx.recv_timeout(Duration::from_millis(100)) {
                Ok(SttCommand::StreamChunk { turn_id, .. }) => {
                    if turn_id == TURN + 1 {
                        saw_stream_chunk = true;
                    }
                }
                Ok(_) => {}
                Err(_) => {}
            }
        }
        assert!(
            saw_stream_chunk,
            "barge-in must flush pre-roll + body StreamChunks to STT with the new turn_id"
        );
    })
    .await
    .expect("Test timed out");
}

/// PURPOSE:  With AEC bypassed, the same echo must interrupt the assistant.
/// SCENARIO: The production AecCommand::SetEnabled(false) is delivered to the worker.
/// ASSERTS:  The echo-only mic triggers the SpeechStart interrupt path.
/// CLASS:    happy-path (positive control)
/// REQUIRES: the test audio assets
/// MUTANT:   Disable the bypass command handling.
#[tokio::test]
async fn bypass_reproduces_the_echo_interruption() {
    let test_timeout = Duration::from_secs(45);
    tokio::time::timeout(test_timeout, async {
        const TURN: u32 = 1203;
        let mut scene = EchoScene::start("tests/assets/edgetts_01_en_briefing.wav", TURN).await;

        scene.interrupt_aec_via_engine_channel(AecCommand::SetEnabled(false));
        tokio::time::sleep(Duration::from_millis(50)).await;

        let mic = common::harness::scene_mic(&scene.tts_16k[..40_000], None);
        scene.feed_mixed(&mic);

        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline {
            if scene.state.pipeline.turn_id.load(Ordering::Relaxed) == TURN + 1 {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        assert_eq!(
            scene.state.pipeline.turn_id.load(Ordering::Relaxed),
            TURN + 1,
            "unsuppressed echo must reach the VAD and interrupt the assistant"
        );
    })
    .await
    .expect("Test timed out");
}
