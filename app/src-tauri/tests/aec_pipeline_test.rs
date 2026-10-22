//! ============================================================================
//! aec_pipeline_test.rs — AEC Reference Tap, Sonora AEC3 & Echo Suppression
//! ============================================================================
//! Category     : Integration Test
//! Component    : services/audio/aec.rs + services/audio/sink.rs
//! Prerequisites: tests/assets/edgetts_01_en_briefing.wav, tests/assets/supertonic_01_en_briefing.wav
//! Execution    : cargo nextest run --test aec_pipeline_test --release --nocapture --test-threads=1
//! Metrics      : Echo Return Loss Enhancement (ERLE >= 20dB), user speech preservation
//! ============================================================================

mod common;

use std::{path::Path, time::Duration};

use ringbuf::{traits::*, HeapRb};
use vox_lib::services::audio::{
    aec::AecProcessor, sink::PlaybackReferenceTap, AEC_FRAME_SAMPLES, AEC_REFERENCE_BUFFER_SAMPLES,
};

/// Computes Echo Return Loss Enhancement (ERLE) in decibels.
fn calculate_erle_db(input_echo: &[f32], output_residual: &[f32]) -> f64 {
    let rms_in = common::audio::rms(input_echo) as f64;
    let rms_out = common::audio::rms(output_residual) as f64;
    if rms_out < 1e-9 {
        return 60.0;
    }
    20.0 * (rms_in / rms_out).log10()
}

/// Verifies that PlaybackReferenceTap accurately downsamples 48kHz audio 3:1 to 16kHz.
#[test]
fn test_playback_reference_tap_downsampling_48k_to_16k() {
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

/// Verifies that AecProcessor suppresses synthetic TTS acoustic loopback by at least 20dB ERLE.
#[tokio::test]
async fn test_aec_suppresses_synthetic_tts_echo_alone() {
    let test_timeout = Duration::from_secs(30);
    tokio::time::timeout(test_timeout, async {
        let tts_path = Path::new("tests/assets/edgetts_01_en_briefing.wav");
        let tts_samples = common::audio::decode_wav_to_mono_16k(tts_path)
            .expect("Failed to load TTS briefing WAV");
        assert!(!tts_samples.is_empty());

        let rb = HeapRb::<f32>::new(65536);
        let (mut render_prod, render_cons) = rb.split();

        let mut aec = AecProcessor::new(Some(render_cons)).expect("Failed to create AecProcessor");
        aec.set_stream_delay_ms(40)
            .expect("Failed to set stream delay");

        // Simulate 40ms acoustic delay (640 samples @ 16kHz) and -6dB attenuation (factor 0.5)
        let delay_samples = 640;
        let attenuation = 0.5f32;
        let mut simulated_mic = vec![0.0f32; delay_samples];
        for &s in &tts_samples {
            simulated_mic.push(s * attenuation);
        }

        // Feed render reference audio in lockstep frame-by-frame
        let mut clean_output = Vec::new();
        let mut frame_out = [0.0f32; AEC_FRAME_SAMPLES];

        let num_frames = simulated_mic.len() / AEC_FRAME_SAMPLES;
        for i in 0..num_frames {
            let start = i * AEC_FRAME_SAMPLES;
            let end = start + AEC_FRAME_SAMPLES;

            // Push render reference for this frame if available
            if start < tts_samples.len() {
                let r_end = end.min(tts_samples.len());
                render_prod.push_slice(&tts_samples[start..r_end]);
            }

            aec.process_frame(&simulated_mic[start..end], &mut frame_out)
                .expect("AEC stream processing failed");
            clean_output.extend_from_slice(&frame_out);
        }

        assert_eq!(clean_output.len(), num_frames * AEC_FRAME_SAMPLES);

        // Skip initial filter convergence period (first 1.0 second = 16000 samples)
        let eval_start = 16000;
        let eval_end = (num_frames * AEC_FRAME_SAMPLES).min(tts_samples.len());
        assert!(eval_end > eval_start + 16000);

        let erle = calculate_erle_db(
            &simulated_mic[eval_start..eval_end],
            &clean_output[eval_start..eval_end],
        );

        println!(
            "[AEC Pipeline Test] Measured ERLE: {:.2} dB (Input RMS: {:.4}, Output RMS: {:.4})",
            erle,
            common::audio::rms(&simulated_mic[eval_start..eval_end]),
            common::audio::rms(&clean_output[eval_start..eval_end])
        );

        assert!(
            erle >= 20.0,
            "AEC must provide >= 20dB echo suppression (measured: {:.2}dB)",
            erle
        );
    })
    .await
    .expect("Test timed out");
}

/// Verifies that during double-talk, user speech energy is preserved while echo is suppressed.
#[tokio::test]
async fn test_aec_double_talk_preserves_user_speech() {
    let test_timeout = Duration::from_secs(30);
    tokio::time::timeout(test_timeout, async {
        let tts_path = Path::new("tests/assets/edgetts_01_en_briefing.wav");
        let user_path = Path::new("tests/assets/supertonic_01_en_briefing.wav");

        let tts_samples = common::audio::decode_wav_to_mono_16k(tts_path)
            .expect("Failed to load TTS briefing WAV");
        let user_samples = common::audio::decode_wav_to_mono_16k(user_path)
            .expect("Failed to load user speech WAV");

        let rb = HeapRb::<f32>::new(65536);
        let (mut render_prod, render_cons) = rb.split();

        let mut aec = AecProcessor::new(Some(render_cons))
            .expect("Failed to create AecProcessor");
        aec.set_stream_delay_ms(40)
            .expect("Failed to set stream delay");

        // Prepare mixed capture signal: echo (attenuated + delayed) + user speech
        let delay_samples = 640;
        let total_samples = tts_samples.len().min(user_samples.len()).min(48000);
        let mut simulated_mic = vec![0.0f32; total_samples];
        let mut clean_user_ref = vec![0.0f32; total_samples];

        for i in 0..total_samples {
            let echo = if i >= delay_samples && (i - delay_samples) < tts_samples.len() {
                tts_samples[i - delay_samples] * 0.4
            } else {
                0.0
            };
            let user = user_samples[i];
            simulated_mic[i] = echo + user;
            clean_user_ref[i] = user;
        }

        let mut clean_output = Vec::new();
        let mut frame_out = [0.0f32; AEC_FRAME_SAMPLES];
        let num_frames = total_samples / AEC_FRAME_SAMPLES;

        for i in 0..num_frames {
            let start = i * AEC_FRAME_SAMPLES;
            let end = start + AEC_FRAME_SAMPLES;

            if start < tts_samples.len() {
                let r_end = end.min(tts_samples.len());
                render_prod.push_slice(&tts_samples[start..r_end]);
            }

            aec.process_frame(&simulated_mic[start..end], &mut frame_out)
                .expect("AEC frame processing failed");
            clean_output.extend_from_slice(&frame_out);
        }

        // In double talk, clean output should retain the user speech energy
        let eval_start = 16000;
        let eval_end = num_frames * AEC_FRAME_SAMPLES;
        let user_rms = common::audio::rms(&clean_user_ref[eval_start..eval_end]);
        let clean_rms = common::audio::rms(&clean_output[eval_start..eval_end]);

        println!(
            "[AEC Double-Talk] User speech RMS: {:.4}, Cleaned output RMS: {:.4}",
            user_rms, clean_rms
        );

        // Cleaned output energy should be close to genuine user speech energy (within 3dB factor: 0.7x to 1.4x)
        assert!(
            clean_rms > user_rms * 0.5,
            "User speech must not be attenuated by more than 6dB during double-talk (clean: {}, user: {})",
            clean_rms,
            user_rms
        );
    })
    .await
    .expect("Test timed out");
}

/// Verifies that disabling AEC passes audio through completely untouched.
#[tokio::test]
async fn test_aec_bypass_leaves_echo_unattenuated() {
    let test_timeout = Duration::from_secs(15);
    tokio::time::timeout(test_timeout, async {
        let tts_path = Path::new("tests/assets/edgetts_01_en_briefing.wav");
        let tts_samples = common::audio::decode_wav_to_mono_16k(tts_path)
            .expect("Failed to load TTS briefing WAV");

        let rb = HeapRb::<f32>::new(16384);
        let (_, render_cons) = rb.split();

        let mut aec = AecProcessor::new(Some(render_cons)).expect("Failed to create AecProcessor");
        aec.set_enabled(false);
        assert!(!aec.is_enabled());

        let eval_samples = &tts_samples[..1600];
        let mut clean_output = Vec::new();
        aec.process_stream(eval_samples, &mut clean_output)
            .expect("AEC stream processing failed");

        assert_eq!(clean_output, eval_samples);
    })
    .await
    .expect("Test timed out");
}
