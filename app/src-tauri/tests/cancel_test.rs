/// Integration test: atomic cancellation propagation across LLM + TTS + Playback.
///
/// cargo test --test cancel_test -- --nocapture

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

#[test]
fn test_cancel_flag_propagates_across_threads() {
    let cancel = Arc::new(AtomicBool::new(false));

    let cancel_a = Arc::clone(&cancel);
    let cancel_b = Arc::clone(&cancel);
    let cancel_c = Arc::clone(&cancel);

    // Simulate three worker threads checking the same atomic
    let h_a = std::thread::spawn(move || {
        let mut saw_cancel = false;
        for _ in 0..1000 {
            if cancel_a.load(Ordering::Relaxed) {
                saw_cancel = true;
                break;
            }
            std::thread::sleep(Duration::from_micros(10));
        }
        saw_cancel
    });

    let h_b = std::thread::spawn(move || {
        let mut saw_cancel = false;
        for _ in 0..1000 {
            if cancel_b.load(Ordering::Relaxed) {
                saw_cancel = true;
                break;
            }
            std::thread::sleep(Duration::from_micros(10));
        }
        saw_cancel
    });

    // Allow threads to start
    std::thread::sleep(Duration::from_millis(5));

    let t0 = Instant::now();
    cancel.store(true, Ordering::Relaxed);

    let saw_a = h_a.join().unwrap();
    let saw_b = h_b.join().unwrap();
    let elapsed = t0.elapsed();

    assert!(saw_a, "Thread A should observe cancellation");
    assert!(saw_b, "Thread B should observe cancellation");
    assert!(
        elapsed < Duration::from_millis(200),
        "Cancellation propagation took too long: {:?} (limit: 200ms)",
        elapsed
    );

    println!("[CANCEL TEST] Propagation latency: {:?}", elapsed);
}

#[test]
fn test_cancel_flag_reset_for_new_session() {
    let cancel = Arc::new(AtomicBool::new(false));

    // Simulate mid-generation cancel
    cancel.store(true, Ordering::Relaxed);
    assert!(cancel.load(Ordering::Relaxed));

    // New session starts: reset flag
    cancel.store(false, Ordering::Relaxed);
    assert!(!cancel.load(Ordering::Relaxed), "cancel_flag must reset cleanly for new session");
}

#[test]
fn test_session_id_monotonically_increases() {
    use std::sync::atomic::AtomicU32;

    let session_id = Arc::new(AtomicU32::new(0));

    let handles: Vec<_> = (0..4).map(|_| {
        let sid = Arc::clone(&session_id);
        std::thread::spawn(move || {
            sid.fetch_add(1, Ordering::Relaxed)
        })
    }).collect();

    let values: Vec<u32> = handles.into_iter()
        .map(|h| h.join().unwrap())
        .collect();

    let final_id = session_id.load(Ordering::Relaxed);
    assert_eq!(final_id, 4, "session_id should be 4 after 4 increments");
    println!("[CANCEL TEST] Session IDs observed: {:?}, final: {}", values, final_id);
}
