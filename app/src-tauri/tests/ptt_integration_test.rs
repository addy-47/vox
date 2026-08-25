use std::sync::atomic::{AtomicBool, Ordering};

#[test]
fn test_ptt_atomic_compare_exchange_double_call_protection() {
    let is_recording = AtomicBool::new(false);

    // 1. First compare_exchange: false -> true should succeed
    let first_start =
        is_recording.compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst);
    assert!(first_start.is_ok());
    assert!(is_recording.load(Ordering::SeqCst));

    // 2. Second compare_exchange while recording: false -> true MUST FAIL
    let second_start =
        is_recording.compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst);
    assert!(
        second_start.is_err(),
        "Concurrent PTT start must fail atomically when already recording!"
    );
    assert!(second_start.unwrap_err());

    // 3. Verify state remains recording
    assert!(is_recording.load(Ordering::SeqCst));

    // 4. Release recording: true -> false
    let stop = is_recording.compare_exchange(true, false, Ordering::SeqCst, Ordering::SeqCst);
    assert!(stop.is_ok());
    assert!(!is_recording.load(Ordering::SeqCst));
}

#[test]
fn test_ptt_buffer_split_off_clears_accumulator() {
    let buffer = parking_lot::Mutex::new(vec![0.1f32, 0.2, 0.3, 0.4]);
    assert_eq!(buffer.lock().len(), 4);

    let collected = buffer.lock().split_off(0);
    assert_eq!(collected.len(), 4);
    assert_eq!(buffer.lock().len(), 0);
}
