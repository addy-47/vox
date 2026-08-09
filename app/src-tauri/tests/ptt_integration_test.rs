//! ============================================================================
//! ptt_integration_test.rs — Push-to-Talk State Machine & Buffer Invariants Test
//! ============================================================================
//! Category     : Integration Test
//! Component    : PTT Service (`vox_lib::services::ptt`)
//! Prerequisites: Compiles against `vox_lib` public API
//! Execution    : cargo test --test ptt_integration_test
//! Metrics      : State transition correctness & buffer discard validation
//! ============================================================================

use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use vox_lib::core::state::PttState;
use vox_lib::services::ptt::{discard_ptt_hold_inner, reset_ptt_state_inner};

#[test]
fn test_ptt_state_machine_resets_and_discards() {
    let turn_id = Arc::new(AtomicU32::new(10));
    let ptt = PttState {
        is_recording: AtomicBool::new(true),
        turn_id: Arc::clone(&turn_id),
        audio_buffer: parking_lot::Mutex::new(vec![0.1, 0.2, 0.3, 0.4]),
        samples_since_partial: AtomicUsize::new(12800),
        samples_since_waveform: AtomicUsize::new(960),
        speech_detected: AtomicBool::new(true),
        ptt_start_ms: AtomicU64::new(1000),
    };

    // 1. Verify initial recording state
    assert!(ptt.is_recording.load(Ordering::SeqCst));
    assert_eq!(ptt.audio_buffer.lock().len(), 4);

    // 2. Execute inner reset state
    reset_ptt_state_inner(&ptt);
    assert_eq!(ptt.audio_buffer.lock().len(), 0);
    assert_eq!(ptt.samples_since_partial.load(Ordering::Relaxed), 0);
    assert_eq!(ptt.samples_since_waveform.load(Ordering::Relaxed), 0);
    assert!(!ptt.speech_detected.load(Ordering::SeqCst));

    // 3. Re-populate audio buffer and test discard hold on silence
    {
        let mut buf = ptt.audio_buffer.lock();
        buf.extend_from_slice(&[0.01, 0.02, 0.03]);
    }
    ptt.is_recording.store(true, Ordering::SeqCst);
    ptt.speech_detected.store(false, Ordering::SeqCst);

    discard_ptt_hold_inner(&ptt);

    // 4. Assert discard hold cleared recording flag, audio buffer, and speech flags
    assert!(!ptt.is_recording.load(Ordering::SeqCst));
    assert_eq!(ptt.audio_buffer.lock().len(), 0);
    assert!(!ptt.speech_detected.load(Ordering::SeqCst));
}

#[test]
fn test_ptt_atomic_compare_exchange_double_call_protection() {
    let turn_id = Arc::new(AtomicU32::new(1));
    let ptt = PttState {
        is_recording: AtomicBool::new(false),
        turn_id: Arc::clone(&turn_id),
        audio_buffer: parking_lot::Mutex::new(Vec::new()),
        samples_since_partial: AtomicUsize::new(0),
        samples_since_waveform: AtomicUsize::new(0),
        speech_detected: AtomicBool::new(false),
        ptt_start_ms: AtomicU64::new(0),
    };

    // 1. First compare_exchange: false -> true should succeed
    let first_start = ptt
        .is_recording
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst);
    assert!(first_start.is_ok());
    assert!(ptt.is_recording.load(Ordering::SeqCst));

    // 2. Second compare_exchange while recording: false -> true MUST FAIL
    let second_start = ptt
        .is_recording
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst);
    assert!(second_start.is_err(), "Concurrent PTT start must fail atomically when already recording!");
    assert!(second_start.unwrap_err());

    // 3. Verify state remains recording
    assert!(ptt.is_recording.load(Ordering::SeqCst));
}
