//! ============================================================================
//! audio_capture_aec_test.rs — Live Mic Capture AEC Ingestion & Passthrough
//! ============================================================================
//! Category     : Integration Test
//! Component    : services/audio/aec.rs + services/audio/device.rs
//! Prerequisites: tests/assets/edgetts_01_en_briefing.wav
//! Execution    : cargo nextest run --test audio_capture_aec_test --release --nocapture --test-threads=1
//! Metrics      : Zero frame drops, bounded latency (<15ms processing overhead), safe passthrough
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
use vox_lib::services::audio::{
    aec::{spawn_aec_worker, AecCommand, AecProcessor},
    AEC_FRAME_SAMPLES,
};

/// Verifies that the AEC worker processes live incoming microphone chunks with <15ms latency and zero frame drops.
#[tokio::test]
async fn test_aec_worker_live_capture_bounded_latency_and_zero_drops() {
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
        let mut max_latency = Duration::ZERO;
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

            let start_time = Instant::now();
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

            let elapsed = start_time.elapsed();
            if elapsed > max_latency {
                max_latency = elapsed;
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

        println!(
            "[AEC Ingestion Test] Processed {} samples across {} chunks. Max latency: {:.2}ms",
            total_received,
            num_chunks,
            max_latency.as_secs_f64() * 1000.0
        );

        assert_eq!(total_received, chunk_samples * num_chunks);
        assert!(
            max_latency < Duration::from_millis(15),
            "Processing latency must be < 15ms (measured: {:.2}ms)",
            max_latency.as_secs_f64() * 1000.0
        );

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

/// Verifies that the AEC worker suppresses echo when render audio is fed concurrently with capture.
#[tokio::test]
async fn test_aec_worker_realtime_echo_suppression() {
    let test_timeout = Duration::from_secs(30);
    tokio::time::timeout(test_timeout, async {
        let tts_path = Path::new("tests/assets/edgetts_01_en_briefing.wav");
        let tts_samples = common::audio::decode_wav_to_mono_16k(tts_path)
            .expect("Failed to load TTS briefing WAV");

        let ref_rb = HeapRb::<f32>::new(65536);
        let (mut ref_prod, ref_cons) = ref_rb.split();

        let cap_rb = HeapRb::<f32>::new(65536);
        let (mut cap_prod, cap_cons) = cap_rb.split();

        let clean_rb = HeapRb::<f32>::new(65536);
        let (clean_prod, mut clean_cons) = clean_rb.split();

        let shutdown = Arc::new(AtomicBool::new(false));
        let aec = AecProcessor::new(Some(ref_cons)).expect("Failed to create AecProcessor");

        let handle = spawn_aec_worker(aec, cap_cons, clean_prod, Arc::clone(&shutdown), None)
            .expect("Failed to spawn AEC worker");

        // Simulate 40ms echo (640 samples @ 16kHz) and -6dB attenuation (0.5x)
        let delay_samples = 640;
        let test_len = 48000; // 3 seconds
        let mut simulated_mic = vec![0.0f32; delay_samples];
        for &s in &tts_samples[..test_len] {
            simulated_mic.push(s * 0.5);
        }

        let num_frames = simulated_mic.len() / AEC_FRAME_SAMPLES;
        let mut clean_output = Vec::with_capacity(num_frames * AEC_FRAME_SAMPLES);

        for i in 0..num_frames {
            let start = i * AEC_FRAME_SAMPLES;
            let end = start + AEC_FRAME_SAMPLES;

            if start < tts_samples.len() {
                let r_end = end.min(tts_samples.len());
                ref_prod.push_slice(&tts_samples[start..r_end]);
            }

            cap_prod.push_slice(&simulated_mic[start..end]);

            // Drain clean_cons
            let deadline = Instant::now() + Duration::from_millis(50);
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

        assert_eq!(clean_output.len(), num_frames * AEC_FRAME_SAMPLES);

        // Evaluate ERLE after initial filter convergence (1.0 second = 16000 samples)
        let eval_start = 16000;
        let eval_end = (num_frames * AEC_FRAME_SAMPLES).min(tts_samples.len());
        let rms_in = common::audio::rms(&simulated_mic[eval_start..eval_end]);
        let rms_out = common::audio::rms(&clean_output[eval_start..eval_end]);
        let erle = 20.0 * (rms_in as f64 / rms_out as f64).log10();

        println!(
            "[AEC Worker Test] Real-time loopback ERLE: {:.2} dB (Input RMS: {:.4}, Output RMS: {:.4})",
            erle, rms_in, rms_out
        );

        assert!(
            erle >= 20.0,
            "Worker loop must achieve >= 20dB ERLE (measured: {:.2}dB)",
            erle
        );

        shutdown.store(true, Ordering::Relaxed);
        handle.join().expect("AEC worker thread join failed");
    })
    .await
    .expect("Test timed out");
}

/// Verifies dynamic runtime bypass toggle via AecCommand channel.
#[tokio::test]
async fn test_aec_worker_dynamic_command_channel_bypass() {
    let test_timeout = Duration::from_secs(15);
    tokio::time::timeout(test_timeout, async {
        let cap_rb = HeapRb::<f32>::new(16384);
        let (mut cap_prod, cap_cons) = cap_rb.split();

        let clean_rb = HeapRb::<f32>::new(16384);
        let (clean_prod, mut clean_cons) = clean_rb.split();

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
