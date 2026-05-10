/// Unit and integration tests for the playback module.
///
/// Unit tests (always run): upsample_2x correctness, buffer logic.
/// Integration tests (ignored): CPAL device availability.

// ─── Directive 3: upsample_2x unit tests ─────────────────────────────────────

#[test]
fn test_upsample_2x_doubles_length() {
    use vox_lib::services::playback::upsample_2x;

    let input = vec![0.0f32, 1.0, 0.0, -1.0];
    let out = upsample_2x(&input);
    assert_eq!(out.len(), input.len() * 2, "output must be exactly 2× input length");
}

#[test]
fn test_upsample_2x_preserves_originals() {
    use vox_lib::services::playback::upsample_2x;

    let input = vec![0.2f32, 0.8, -0.4, 0.6];
    let out = upsample_2x(&input);

    // Even indices must match original samples
    for (i, &orig) in input.iter().enumerate() {
        assert_eq!(out[i * 2], orig,
            "original sample at index {} not preserved: expected {} got {}",
            i, orig, out[i * 2]);
    }
}

#[test]
fn test_upsample_2x_midpoints_correct() {
    use vox_lib::services::playback::upsample_2x;

    let input = vec![0.0f32, 1.0, 0.0];
    let out = upsample_2x(&input);
    // [0.0, 0.5, 1.0, 0.5, 0.0, 0.0]
    assert!((out[1] - 0.5).abs() < 1e-6, "midpoint[0→1] should be 0.5, got {}", out[1]);
    assert!((out[3] - 0.5).abs() < 1e-6, "midpoint[1→0] should be 0.5, got {}", out[3]);
    // Last midpoint: extrapolate by repeating last sample
    assert!((out[5] - 0.0).abs() < 1e-6, "last midpoint should be 0.0, got {}", out[5]);
}

#[test]
fn test_upsample_2x_single_sample() {
    use vox_lib::services::playback::upsample_2x;

    let input = vec![0.5f32];
    let out = upsample_2x(&input);
    assert_eq!(out.len(), 2);
    assert_eq!(out[0], 0.5);
    assert_eq!(out[1], 0.5, "single sample: last midpoint should repeat");
}

#[test]
fn test_upsample_2x_empty_input() {
    use vox_lib::services::playback::upsample_2x;

    let out = upsample_2x(&[]);
    assert!(out.is_empty());
}

#[test]
fn test_upsample_2x_silence_stays_silent() {
    use vox_lib::services::playback::upsample_2x;

    let input = vec![0.0f32; 1000];
    let out = upsample_2x(&input);
    assert!(out.iter().all(|&s| s == 0.0), "silence must stay silence after upsampling");
}

// ─── Playback Engine Integration (ignored — needs audio device) ───────────────

#[test]
#[ignore]
fn test_playback_engine_creates_without_error() {
    use std::sync::atomic::AtomicBool;
    use std::sync::Arc;
    use vox_lib::services::playback::PlaybackEngine;

    let active = Arc::new(AtomicBool::new(false));
    let cancel = Arc::new(AtomicBool::new(false));

    let engine = PlaybackEngine::new(Arc::clone(&active), Arc::clone(&cancel));
    assert!(engine.is_ok(), "PlaybackEngine should create: {:?}", engine.err());
}

#[test]
#[ignore]
fn test_playback_jitter_prebuffer_triggers_active() {
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;
    use vox_lib::services::playback::{PlaybackEngine, upsample_2x};

    let active = Arc::new(AtomicBool::new(false));
    let cancel = Arc::new(AtomicBool::new(false));
    let engine = PlaybackEngine::new(Arc::clone(&active), Arc::clone(&cancel))
        .expect("PlaybackEngine creation failed");

    // Push 400ms of 24kHz silence (should exceed 300ms pre-buffer and trigger active)
    let chunk_24khz = vec![0.0f32; 24_000 / 1000 * 400]; // 9_600 samples
    engine.ingest_chunk(&chunk_24khz);

    std::thread::sleep(std::time::Duration::from_millis(50));
    assert!(
        active.load(Ordering::Relaxed),
        "playback_active should be true after pre-buffer threshold is reached"
    );

    engine.cancel();
    assert!(!active.load(Ordering::Relaxed), "cancel should set playback_active to false");
}
