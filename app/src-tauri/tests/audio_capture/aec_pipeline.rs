//! ============================================================================
//! aec_pipeline.rs — AEC reference tap, Sonora AEC3 and echo suppression
//! ============================================================================
//! Category  : Integration (Seam 11 — playback reference → AEC → clean capture)
//! Component : services/audio/aec.rs + services/audio/sink.rs
//! Binary    : audio_capture_aec_test
//! Prereqs   : the test audio assets
//! Run       : cargo nextest run --test audio_capture_aec_test --release --test-threads=1
//! ============================================================================

use std::{
    path::Path,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc,
    },
    time::{Duration, Instant},
};

use ringbuf::{
    traits::{Consumer, Observer, Producer, Split},
    HeapRb,
};
use vox_lib::{
    core::{events::VoxEvent, state::InteractionState},
    services::audio::{
        aec::{spawn_aec_worker, AecProcessor},
        sink::PlaybackReferenceTap,
        AEC_FRAME_SAMPLES, AEC_REFERENCE_BUFFER_SAMPLES,
    },
};

use crate::common;

/// Simulated render→capture hardware skew. Vox `DEFAULT_STREAM_DELAY_MS` (40) is
/// the production default; real PipeWire/ALSA paths measure 120–180 ms.
const PIPELINE_DELAY_SAMPLES: usize = 1920; // 120 ms @ 16 kHz
/// Microphone attenuation applied to the simulated speaker output.
const PIPELINE_ATTENUATION: f32 = 0.5;
/// Convergence window excluded from the ERLE evaluation (frame AEC3 needs to lock on).
const ERLE_EVAL_SKIP_SAMPLES: usize = 64_000; // 4.0 s @ 16 kHz
/// Test body horizon; must exceed WebRTC AEC3's ~6.0 s TransparentMode transition.
const MIN_TEST_SAMPLES_16K: usize = 104_320; // 6.52 s @ 16 kHz

/// PURPOSE:  A 48 kHz reference must reach the AEC at the 16 kHz capture rate.
/// SCENARIO: Push 480 constant 48 kHz samples through `PlaybackReferenceTap`.
/// ASSERTS:  160 samples (3:1) come out and every settled sample keeps the 0.75 value.
/// CLASS:    happy-path
/// REQUIRES: none
/// MUTANT:   Feed the reference at the playback rate.
#[test]
fn playback_reference_tap_downsamples_48k_to_16k() {
    let rb = HeapRb::<f32>::new(AEC_REFERENCE_BUFFER_SAMPLES);
    let (prod, mut cons) = rb.split();
    let mut tap = PlaybackReferenceTap::new(prod);

    // Push 480 samples of constant 0.75f32 at 48kHz
    for _ in 0..480 {
        tap.push_sample(0.75);
    }

    assert_eq!(cons.occupied_len(), 160);
    let mut popped = vec![0.0f32; 160];
    let count = cons.pop_slice(&mut popped);
    assert_eq!(count, 160);
    // Allow initial IIR Butterworth filter step-response transient to settle (~20 samples @ 16kHz = 1.25ms)
    for &sample in &popped[20..] {
        assert!((sample - 0.75).abs() < 1e-5);
    }
}

/// PURPOSE:  The AEC worker must not drop a captured chunk.
/// SCENARIO: Fifty 20 ms chunks are pushed into the capture ring buffer.
/// ASSERTS:  Every chunk comes out of the clean stream with the same sample count and
///           the signal energy survives the passthrough.
/// CLASS:    happy-path
/// REQUIRES: none
/// MUTANT:   Drop a chunk when the buffer overflows.
/// NOTE:     The old "< 15 ms" latency assertion is gone. It measured the wall clock of
///           a shared CI machine, not the AEC. The per-chunk deadline below is the
///           behavioural bound that remains.
#[tokio::test]
async fn aec_worker_passes_every_captured_chunk_through() {
    let test_timeout = Duration::from_secs(15);
    tokio::time::timeout(test_timeout, async {
        let cap_rb = HeapRb::<f32>::new(32768);
        let (mut cap_prod, cap_cons) = cap_rb.split();

        let clean_rb = HeapRb::<f32>::new(32768);
        let (clean_prod, mut clean_cons) = clean_rb.split();

        let shutdown = Arc::new(AtomicBool::new(false));
        let aec = AecProcessor::new(None).expect("Failed to create AecProcessor");

        let handle = spawn_aec_worker(aec, cap_cons, clean_prod, Arc::clone(&shutdown), None)
            .expect("Failed to spawn AEC worker");

        // Simulate CPAL pushing 20ms chunks (320 samples @ 16kHz) for 1 second (50 chunks)
        let chunk_samples = 320;
        let num_chunks = 50;
        let mut total_received = 0;
        let mut output_samples = Vec::with_capacity(chunk_samples * num_chunks);

        for chunk_idx in 0..num_chunks {
            // Generate synthetic tone
            let input_chunk: Vec<f32> = (0..chunk_samples)
                .map(|i| {
                    let t = (chunk_idx * chunk_samples + i) as f32 / 16000.0;
                    (2.0 * std::f32::consts::PI * 440.0 * t).sin() * 0.5
                })
                .collect();

            let pushed = cap_prod.push_slice(&input_chunk);
            assert_eq!(pushed, chunk_samples, "Cap ring buffer overflow on push");

            // Wait for chunk to be processed and emitted to clean_cons
            let mut chunk_out = Vec::new();
            let chunk_deadline = Instant::now() + Duration::from_millis(50);
            while chunk_out.len() < chunk_samples && Instant::now() < chunk_deadline {
                let mut buf = [0.0f32; 160];
                let count = clean_cons.pop_slice(&mut buf);
                if count > 0 {
                    chunk_out.extend_from_slice(&buf[..count]);
                } else {
                    tokio::time::sleep(Duration::from_millis(1)).await;
                }
            }

            assert_eq!(
                chunk_out.len(),
                chunk_samples,
                "Expected {} samples emitted within deadline, got {}",
                chunk_samples,
                chunk_out.len()
            );

            total_received += chunk_out.len();
            output_samples.extend_from_slice(&chunk_out);
        }

        assert_eq!(total_received, chunk_samples * num_chunks);

        // Safe passthrough check: signal energy must be preserved when playback is idle
        let rms = common::audio::rms(&output_samples);
        assert!(
            rms > 0.2,
            "AEC passthrough energy must be preserved when idle (measured RMS: {})",
            rms
        );

        shutdown.store(true, Ordering::Relaxed);
        handle.join().expect("AEC worker thread join failed");
    })
    .await
    .expect("Test timed out");
}

/// PURPOSE:  The bypass must be switchable while the worker runs.
/// SCENARIO: The bypass command is sent to a running worker.
/// ASSERTS:  The worker honours it without a restart.
/// CLASS:    happy-path
/// REQUIRES: none
/// MUTANT:   Ignore the bypass command.
#[tokio::test]
async fn aec_worker_honours_the_dynamic_bypass_command() {
    let test_timeout = Duration::from_secs(15);
    tokio::time::timeout(test_timeout, async {
        let cap_rb = HeapRb::<f32>::new(16384);
        let (mut cap_prod, cap_cons) = cap_rb.split();

        let clean_rb = HeapRb::<f32>::new(16384);
        let (clean_prod, mut clean_cons) = clean_rb.split();

        use vox_lib::services::audio::aec::AecCommand;
        let (cmd_tx, cmd_rx) = mpsc::channel();
        let shutdown = Arc::new(AtomicBool::new(false));
        let aec = AecProcessor::new(None).expect("Failed to create AecProcessor");

        let handle = spawn_aec_worker(
            aec,
            cap_cons,
            clean_prod,
            Arc::clone(&shutdown),
            Some(cmd_rx),
        )
        .expect("Failed to spawn AEC worker");

        // Send bypass command
        cmd_tx
            .send(AecCommand::SetEnabled(false))
            .expect("Failed to send command");
        tokio::time::sleep(Duration::from_millis(10)).await;

        let test_input = vec![0.42f32; 320];
        cap_prod.push_slice(&test_input);

        let mut received = Vec::new();
        let deadline = Instant::now() + Duration::from_millis(100);
        while received.len() < 320 && Instant::now() < deadline {
            let mut buf = [0.0f32; 160];
            let count = clean_cons.pop_slice(&mut buf);
            if count > 0 {
                received.extend_from_slice(&buf[..count]);
            } else {
                tokio::time::sleep(Duration::from_millis(1)).await;
            }
        }

        assert_eq!(received.len(), 320);
        assert_eq!(received, test_input);

        shutdown.store(true, Ordering::Relaxed);
        handle.join().expect("AEC worker thread join failed");
    })
    .await
    .expect("Test timed out");
}

/// Simulates a microphone that hears the playback after `delay_samples` of
/// render→capture skew (PipeWire/ALSA), at `attenuation` relative amplitude.
fn synthetic_mic_echo(tts16: &[f32], delay_samples: usize, attenuation: f32) -> Vec<f32> {
    let mut mic = vec![0.0f32; delay_samples];
    mic.extend(tts16.iter().map(|s| s * attenuation));
    mic
}

/// PURPOSE:  The AEC must remove the assistant's own echo even when hardware
///           latency is 120 ms (far above the 40 ms production default).
/// SCENARIO: Real playback sink renders into PlaybackReferenceTap; AEC must
///           adapt its delay estimate and suppress the echo. Runs past the
///           ~6.0 s WebRTC AEC3 TransparentMode horizon.
/// ASSERTS:  Tail-window ERLE >= 20 dB.
/// CLASS:    happy-path (physical skew)
/// REQUIRES: the test audio assets
/// MUTANT:   Pin the delay adaptation (set_stream_delay_ms) or stop before 6 s.
/// NOTE:     Deliberately does NOT call set_stream_delay_ms or calibrate inputs
///           to the default — this tests the dynamic adaptation path.
#[tokio::test]
async fn aec_suppresses_the_synthetic_tts_echo() {
    let test_timeout = Duration::from_secs(60);
    tokio::time::timeout(test_timeout, async {
        let _guard = common::paths::TempPathsGuard::new();
        vox_lib::utils::paths::init();
        let (_app, state) = common::harness::get_test_app_and_state().await;
        state.pipeline.turn_id.store(1101, Ordering::Relaxed);
        state.pipeline.pending_synthesis_jobs.store(0, Ordering::Relaxed);
        state.pipeline.set_state(InteractionState::Speaking);

        let tts_path = Path::new("tests/assets/edgetts_01_en_briefing.wav");
        let tts16 = common::audio::decode_wav_to_mono_16k(tts_path)
            .expect("Failed to load TTS briefing WAV");
        assert!(
            tts16.len() >= MIN_TEST_SAMPLES_16K,
            "edgetts_01 must cover >6.5 s @16kHz for the AEC3 TransparentMode horizon"
        );
        let (tts24, rate) = common::audio::decode_wav_to_mono_native(tts_path)
            .expect("Failed to load native-rate TTS briefing WAV");
        assert_eq!(rate, 24_000, "edgetts_01 is a native 24 kHz asset");

        let (event_tx, _event_rx) = mpsc::channel::<VoxEvent>();
        let (playback_engine, mut sink_ctx, ref_cons) =
            common::harness::create_headless_playback_with_aec_tap(&state, event_tx);

        // PLAYBACK_BUFFER_SAMPLES is 30 s — the full clip fits in one ingest.
        playback_engine.ingest_chunk(&tts24);

        let total_mic = (tts16.len() / AEC_FRAME_SAMPLES) * AEC_FRAME_SAMPLES;
        let cap_rb = HeapRb::<f32>::new(MIN_TEST_SAMPLES_16K + 4096);
        let (mut cap_prod, cap_cons) = cap_rb.split();

        let clean_rb = HeapRb::<f32>::new(MIN_TEST_SAMPLES_16K + 4096);
        let (clean_prod, mut clean_cons) = clean_rb.split();

        let shutdown = Arc::new(AtomicBool::new(false));
        let aec = AecProcessor::new(Some(ref_cons)).expect("Failed to create AecProcessor");
        let join_handle =
            spawn_aec_worker(aec, cap_cons, clean_prod, Arc::clone(&shutdown), None)
                .expect("Failed to spawn AEC worker");

        let mic = synthetic_mic_echo(&tts16[..total_mic], PIPELINE_DELAY_SAMPLES, PIPELINE_ATTENUATION);
        let mut clean_output = Vec::with_capacity(mic.len());
        let mut stereo_out = [0.0f32; 960]; // 10 ms @ 48 kHz stereo

        for frame in 0..(total_mic / AEC_FRAME_SAMPLES) {
            let start = frame * AEC_FRAME_SAMPLES;
            let end = start + AEC_FRAME_SAMPLES;
            let pushed = cap_prod.push_slice(&mic[start..end]);
            assert_eq!(pushed, AEC_FRAME_SAMPLES, "Cap ring buffer overflow on push");

            sink_ctx.process_output_buffer(&mut stereo_out);

            let deadline = Instant::now() + Duration::from_millis(20);
            while clean_output.len() < end && Instant::now() < deadline {
                let mut buf = [0.0f32; 160];
                let count = clean_cons.pop_slice(&mut buf);
                if count > 0 {
                    clean_output.extend_from_slice(&buf[..count]);
                } else {
                    tokio::time::sleep(Duration::from_millis(1)).await;
                }
            }
        }

        // Final drain of any buffered clean frames
        let drain_deadline = Instant::now() + Duration::from_secs(2);
        while clean_output.len() < total_mic && Instant::now() < drain_deadline {
            sink_ctx.process_output_buffer(&mut stereo_out);
            let mut buf = [0.0f32; 160];
            let count = clean_cons.pop_slice(&mut buf);
            if count > 0 {
                clean_output.extend_from_slice(&buf[..count]);
            } else {
                tokio::time::sleep(Duration::from_millis(1)).await;
            }
        }

        shutdown.store(true, Ordering::Relaxed);
        join_handle.join().expect("AEC worker thread join failed");

        assert_eq!(clean_output.len(), total_mic, "AEC did not emit a full clean stream");

        let eval_start = ERLE_EVAL_SKIP_SAMPLES;
        let eval_end = clean_output.len();
        let erle = common::audio::erle_db(&mic[eval_start..eval_end], &clean_output[eval_start..eval_end]);

        for (i, (m, c)) in mic
            .chunks_exact(1_600)
            .zip(clean_output.chunks_exact(1_600))
            .enumerate()
        {
            eprintln!(
                "[AEC Window] 100ms {} ERLE {:.2} dB in_rms {:.4} out_rms {:.4}",
                i,
                common::audio::erle_db(m, c),
                common::audio::rms(m),
                common::audio::rms(c)
            );
        }

        println!(
            "[AEC Pipeline Test] Measured ERLE (120ms PipeWire skew, >6.5s): {:.2} dB (Input RMS: {:.4}, Output RMS: {:.4})",
            erle,
            common::audio::rms(&mic[eval_start..eval_end]),
            common::audio::rms(&clean_output[eval_start..eval_end])
        );

        assert!(
            erle >= 20.0,
            "AEC must achieve >= 20 dB ERLE under 120 ms PipeWire render-capture skew after the 6.0 s AEC3 horizon (measured: {:.2} dB)",
            erle
        );
    })
    .await
    .expect("Test timed out");
}
