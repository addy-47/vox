//! ============================================================================
//! tts_test.rs — TTS Actor & Speech Synthesis Integration Tests (Seam 4)
//! ============================================================================
//! Category     : Integration Test
//! Component    : services/tts/actor, services/tts/providers
//! Prerequisites: Edge TTS (Online) & Supertonic (~/.vox/models/tts/supertonic/)
//! Execution    : cargo test --test tts_test --release -- --nocapture
//! Metrics      : Synthesis Output Validity, Acoustic Feature & Duration Comparison
//! ============================================================================

mod common;

use common::harness::{assert_channel_empty_after, get_test_app_handle};
use common::paths::{get_asset_path, get_supertonic_model_dir};
use common::scoring::{
    assert_acoustic_within_tolerance, extract_acoustic_features, AcousticTolerances,
};
use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::Duration;
use vox_lib::core::events::VoxEvent;
use vox_lib::core::settings::{TtsActiveProvider, TtsEdgeConfig, VoxSettings};
use vox_lib::services::tts::actor::{cool_down_tts, warm_up_tts, TtsCommand, TtsWarmUpHandles};

const EN_PROMPT: &str =
    "Hey Vox, good morning! Can you check my calendar and give me a quick briefing on today's scheduled meetings?";
const HI_PROMPT: &str =
    "वॉक्स, आज बाहर का मौसम कैसा है? क्या शाम को बारिश होने की कोई संभावना है?";

enum TestTtsConfig {
    EdgeTts { voice: Option<String> },
    Supertonic,
}

/// Helper to set up TTS worker with specified provider configuration.
fn setup_test_tts_worker(
    config: TestTtsConfig,
) -> (
    std::sync::mpsc::Sender<TtsCommand>,
    std::sync::mpsc::Receiver<VoxEvent>,
    Option<std::thread::JoinHandle<()>>,
) {
    let app = get_test_app_handle();
    let mut settings = VoxSettings::default();
    match config {
        TestTtsConfig::EdgeTts { voice } => {
            settings.tts.active = TtsActiveProvider::EdgeTts;
            settings.tts.edge_tts = TtsEdgeConfig { voice };
        }
        TestTtsConfig::Supertonic => {
            settings.tts.active = TtsActiveProvider::Supertonic;
        }
    }

    let super_tts_path = get_supertonic_model_dir();
    let (event_tx, event_rx) = std::sync::mpsc::channel::<VoxEvent>();

    let mut tts_tx: Option<std::sync::mpsc::Sender<TtsCommand>> = None;
    let mut tts_handle: Option<std::thread::JoinHandle<()>> = None;
    let cancel_flag = Arc::new(AtomicBool::new(false));
    let is_loaded = Arc::new(AtomicBool::new(false));
    let is_sleeping = Arc::new(AtomicBool::new(false));

    let handles = TtsWarmUpHandles {
        tts_tx: &mut tts_tx,
        tts_handle: &mut tts_handle,
        cancel_flag,
        is_loaded,
        is_sleeping,
    };

    warm_up_tts(&app, handles, &settings, &super_tts_path, event_tx)
        .expect("Failed to warm up TTS worker");

    (tts_tx.expect("tts_tx initialized"), event_rx, tts_handle)
}

/// Linear interpolation resampler from 24kHz to 16kHz.
fn resample_24k_to_16k(input_24k: &[f32]) -> Vec<f32> {
    if input_24k.is_empty() {
        return Vec::new();
    }
    let ratio = 16000.0 / 24000.0;
    let target_len = (input_24k.len() as f64 * ratio).round() as usize;
    let mut out = Vec::with_capacity(target_len);
    for i in 0..target_len {
        let src_pos = i as f64 / ratio;
        let idx0 = src_pos.floor() as usize;
        let frac = (src_pos - idx0 as f64) as f32;
        let s0 = input_24k[idx0.min(input_24k.len() - 1)];
        let s1 = input_24k[(idx0 + 1).min(input_24k.len() - 1)];
        out.push(s0 + frac * (s1 - s0));
    }
    out
}

/// Helper to collect all synthesized PCM samples from event channel into mono 16kHz audio.
fn collect_all_tts_audio_16k(
    event_rx: &std::sync::mpsc::Receiver<VoxEvent>,
    turn_id_expected: u32,
    timeout: Duration,
) -> Vec<f32> {
    let mut samples_24k = Vec::new();
    let deadline = std::time::Instant::now() + timeout;

    while std::time::Instant::now() < deadline {
        if let Ok(event) = event_rx.recv_timeout(Duration::from_millis(100)) {
            match event {
                VoxEvent::TtsChunk { turn_id, samples } => {
                    assert_eq!(turn_id, turn_id_expected);
                    samples_24k.extend_from_slice(&samples);
                }
                VoxEvent::PlaybackFinished { .. } => {
                    break;
                }
                _ => {}
            }
        }
    }

    resample_24k_to_16k(&samples_24k)
}

/// Writes mono f32 samples to a 16-bit PCM WAV file.
fn write_temp_wav(path: &Path, samples: &[f32], sample_rate: u32) {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer = hound::WavWriter::create(path, spec).expect("Failed to create temp WAV");
    for &sample in samples {
        let i16_val = (sample.clamp(-1.0, 1.0) * 32767.0) as i16;
        writer.write_sample(i16_val).expect("Failed to write sample");
    }
    writer.finalize().expect("Failed to finalize WAV");
}

/// Consolidated Edge TTS Session Matrix (EN, HI, Empty Guard) with Single Lifecycle & Hard Timeout.
#[ignore]
#[test]
fn test_tts_edge_synthesis_matrix() {
    let start_time = std::time::Instant::now();
    let max_test_duration = Duration::from_secs(30);

    let (tts_tx, event_rx, tts_handle) =
        setup_test_tts_worker(TestTtsConfig::EdgeTts { voice: None });

    // 1. Edge TTS English Generation vs edgetts_01_en_briefing.wav
    {
        let golden_path = get_asset_path("edgetts_01_en_briefing.wav");
        let golden_features = extract_acoustic_features(&golden_path).expect("Failed golden EN");

        tts_tx
            .send(TtsCommand::Generate {
                turn_id: 1,
                text: EN_PROMPT.to_string(),
            })
            .expect("Failed to send TtsCommand EN");

        let audio_16k = collect_all_tts_audio_16k(&event_rx, 1, Duration::from_secs(12));
        assert!(!audio_16k.is_empty(), "Edge TTS EN audio must not be empty");

        let temp_wav = std::env::temp_dir().join("vox_test_edge_en.wav");
        write_temp_wav(&temp_wav, &audio_16k, 16000);
        let gen_features = extract_acoustic_features(&temp_wav).expect("Failed gen EN features");
        let _ = std::fs::remove_file(temp_wav);

        println!("\n=== [Edge TTS EN Acoustic Report] ===");
        println!("Duration : Gen {:.2}s vs Golden {:.2}s", gen_features.duration_sec, golden_features.duration_sec);
        println!("Mean RMS : Gen {:.4} vs Golden {:.4}", gen_features.mean_rms, golden_features.mean_rms);

        assert_acoustic_within_tolerance(
            &gen_features,
            &golden_features,
            &AcousticTolerances {
                duration_rel_tol: 0.35,
                mean_rms_rel_tol: 0.60,
                non_silent_ratio_abs_tol: 0.35,
            },
            "Edge TTS EN",
        );
    }

    // 2. Edge TTS Hindi Generation vs edgetts_07_hi_weather.wav
    {
        let golden_path = get_asset_path("edgetts_07_hi_weather.wav");
        let golden_features = extract_acoustic_features(&golden_path).expect("Failed golden HI");

        tts_tx
            .send(TtsCommand::Generate {
                turn_id: 2,
                text: HI_PROMPT.to_string(),
            })
            .expect("Failed to send TtsCommand HI");

        let audio_16k = collect_all_tts_audio_16k(&event_rx, 2, Duration::from_secs(12));
        assert!(!audio_16k.is_empty(), "Edge TTS HI audio must not be empty");

        let temp_wav = std::env::temp_dir().join("vox_test_edge_hi.wav");
        write_temp_wav(&temp_wav, &audio_16k, 16000);
        let gen_features = extract_acoustic_features(&temp_wav).expect("Failed gen HI features");
        let _ = std::fs::remove_file(temp_wav);

        println!("\n=== [Edge TTS HI Acoustic Report] ===");
        println!("Duration : Gen {:.2}s vs Golden {:.2}s", gen_features.duration_sec, golden_features.duration_sec);
        println!("Mean RMS : Gen {:.4} vs Golden {:.4}", gen_features.mean_rms, golden_features.mean_rms);

        assert_acoustic_within_tolerance(
            &gen_features,
            &golden_features,
            &AcousticTolerances {
                duration_rel_tol: 0.35,
                mean_rms_rel_tol: 0.60,
                non_silent_ratio_abs_tol: 0.35,
            },
            "Edge TTS HI",
        );
    }

    // 3. Empty Text Guard (Negative)
    {
        tts_tx
            .send(TtsCommand::Generate {
                turn_id: 3,
                text: "".to_string(),
            })
            .expect("Failed to send empty TtsCommand");

        assert_channel_empty_after(
            &event_rx,
            Duration::from_millis(500),
            "event_rx empty text guard",
        );
    }

    // 4. Graceful Teardown & Panic Verification
    let mut tx_opt = Some(tts_tx);
    cool_down_tts(&mut tx_opt);
    if let Some(handle) = tts_handle {
        handle.join().expect("Edge TTS worker thread panicked");
    }

    assert!(
        start_time.elapsed() < max_test_duration,
        "Edge TTS Matrix exceeded hard timeout of 30s"
    );
}

/// Consolidated Supertonic Offline Session Matrix (EN & HI) with Single Lifecycle & Hard Timeout.
#[test]
fn test_tts_supertonic_synthesis_matrix() {
    let start_time = std::time::Instant::now();
    let max_test_duration = Duration::from_secs(30);

    let (tts_tx, event_rx, tts_handle) = setup_test_tts_worker(TestTtsConfig::Supertonic);

    // 1. Supertonic English vs supertonic_01_en_briefing.wav
    {
        let golden_path = get_asset_path("supertonic_01_en_briefing.wav");
        let golden_features = extract_acoustic_features(&golden_path).expect("Failed golden EN");

        tts_tx
            .send(TtsCommand::Generate {
                turn_id: 1,
                text: EN_PROMPT.to_string(),
            })
            .expect("Failed to send Supertonic EN");

        let audio_16k = collect_all_tts_audio_16k(&event_rx, 1, Duration::from_secs(12));
        assert!(!audio_16k.is_empty(), "Supertonic EN audio must not be empty");

        let temp_wav = std::env::temp_dir().join("vox_test_supertonic_en.wav");
        write_temp_wav(&temp_wav, &audio_16k, 16000);
        let gen_features = extract_acoustic_features(&temp_wav).expect("Failed gen EN features");
        let _ = std::fs::remove_file(temp_wav);

        println!("\n=== [Supertonic EN Acoustic Report] ===");
        println!("Duration : Gen {:.2}s vs Golden {:.2}s", gen_features.duration_sec, golden_features.duration_sec);
        println!("Mean RMS : Gen {:.4} vs Golden {:.4}", gen_features.mean_rms, golden_features.mean_rms);

        assert_acoustic_within_tolerance(
            &gen_features,
            &golden_features,
            &AcousticTolerances {
                duration_rel_tol: 0.30,
                mean_rms_rel_tol: 0.50,
                non_silent_ratio_abs_tol: 0.30,
            },
            "Supertonic EN",
        );
    }

    // 2. Supertonic Hindi vs supertonic_07_hi_weather.wav
    {
        let golden_path = get_asset_path("supertonic_07_hi_weather.wav");
        let golden_features = extract_acoustic_features(&golden_path).expect("Failed golden HI");

        tts_tx
            .send(TtsCommand::Generate {
                turn_id: 2,
                text: HI_PROMPT.to_string(),
            })
            .expect("Failed to send Supertonic HI");

        let audio_16k = collect_all_tts_audio_16k(&event_rx, 2, Duration::from_secs(12));
        assert!(!audio_16k.is_empty(), "Supertonic HI audio must not be empty");

        let temp_wav = std::env::temp_dir().join("vox_test_supertonic_hi.wav");
        write_temp_wav(&temp_wav, &audio_16k, 16000);
        let gen_features = extract_acoustic_features(&temp_wav).expect("Failed gen HI features");
        let _ = std::fs::remove_file(temp_wav);

        println!("\n=== [Supertonic HI Acoustic Report] ===");
        println!("Duration : Gen {:.2}s vs Golden {:.2}s", gen_features.duration_sec, golden_features.duration_sec);
        println!("Mean RMS : Gen {:.4} vs Golden {:.4}", gen_features.mean_rms, golden_features.mean_rms);

        assert_acoustic_within_tolerance(
            &gen_features,
            &golden_features,
            &AcousticTolerances {
                duration_rel_tol: 0.30,
                mean_rms_rel_tol: 0.50,
                non_silent_ratio_abs_tol: 0.30,
            },
            "Supertonic HI",
        );
    }

    // 3. Graceful Teardown & Panic Verification
    let mut tx_opt = Some(tts_tx);
    cool_down_tts(&mut tx_opt);
    if let Some(handle) = tts_handle {
        handle.join().expect("Supertonic worker thread panicked");
    }

    assert!(
        start_time.elapsed() < max_test_duration,
        "Supertonic Matrix exceeded hard timeout of 30s"
    );
}
