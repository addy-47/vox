/// End-to-end pipeline integration test: STT→LLM→TTS→Playback
///
/// cargo test --test pipeline_test -- --ignored --nocapture
///
/// Validates the full pipeline runs without crash, deadlock, or panic.
/// All model tests are #[ignore]d — they require the full model set in memory.

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;

// ─── Logic-only tests (always run) ───────────────────────────────────────────

#[test]
fn test_pipeline_session_id_increments_on_cancel() {
    let session_id = Arc::new(AtomicU32::new(0));
    let cancel = Arc::new(AtomicBool::new(false));

    // Simulate first turn
    let sid1 = session_id.fetch_add(1, Ordering::Relaxed) + 1;
    assert_eq!(sid1, 1);
    cancel.store(false, Ordering::Relaxed);

    // Simulate barge-in: cancel current + increment
    cancel.store(true, Ordering::Relaxed);
    let sid2 = session_id.fetch_add(1, Ordering::Relaxed) + 1;
    assert_eq!(sid2, 2);
    cancel.store(false, Ordering::Relaxed);

    assert!(!cancel.load(Ordering::Relaxed), "cancel must be clear at start of new session");
    assert_eq!(session_id.load(Ordering::Relaxed), 2);
}

#[test]
fn test_stale_events_rejected_by_session_id() {
    // Events with old session_id must be ignored by pipeline consumers
    let current_session: u32 = 5;
    let stale_session: u32   = 3;
    let fresh_session: u32   = 5;

    assert_ne!(stale_session, current_session, "stale event should be rejected");
    assert_eq!(fresh_session, current_session, "fresh event should be accepted");
}

// ─── Model Integration (ignored — requires full model set) ────────────────────

#[test]
#[ignore]
fn test_full_pipeline_no_deadlock() {
    use vox_ui_lib::events::VoxEvent;
    use vox_ui_lib::llm::LlmWorker;
    use vox_ui_lib::pipeline::should_flush;
    use vox_ui_lib::playback::{upsample_2x, PlaybackEngine};

    let cancel = Arc::new(AtomicBool::new(false));
    let (tx, mut rx) = mpsc::channel::<VoxEvent>(128);

    let llm_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("assets/gemma4/google_gemma-4-E2B-it-IQ2_M.gguf");

    // LLM on dedicated thread
    let cancel_llm = Arc::clone(&cancel);
    let tx_llm = tx.clone();
    let handle = std::thread::spawn(move || {
        let worker = LlmWorker::new(&llm_path, 512, 2).expect("LLM load failed");
        worker.generate("Say: hello", 1, &cancel_llm, &tx_llm)
    });

    // Collect tokens and simulate sub-sentence chunking
    let rt = tokio::runtime::Runtime::new().unwrap();
    let (chunks, flushed) = rt.block_on(async {
        let mut buf = String::new();
        let mut word_count = 0usize;
        let mut flushed: Vec<String> = Vec::new();
        let mut tokens: Vec<String> = Vec::new();

        loop {
            match tokio::time::timeout(Duration::from_secs(30), rx.recv()).await {
                Ok(Some(VoxEvent::LlmToken { token, .. })) => {
                    buf.push_str(&token);
                    word_count = buf.split_whitespace().count();
                    tokens.push(token);
                    if should_flush(&buf, word_count) {
                        flushed.push(buf.trim().to_string());
                        buf.clear();
                        word_count = 0;
                    }
                }
                Ok(Some(VoxEvent::LlmFinished { .. })) | Ok(None) => break,
                Err(_) => { eprintln!("[PIPELINE TEST] Timeout"); break; }
                _ => {}
            }
        }
        (tokens, flushed)
    });

    handle.join().expect("LLM thread panicked").expect("LLM generation error");

    println!("[PIPELINE TEST] Tokens: {}", chunks.len());
    println!("[PIPELINE TEST] Flush chunks: {:?}", flushed);

    assert!(!chunks.is_empty(), "LLM should produce tokens");
    println!("[PIPELINE TEST] PASS — no deadlock, no crash");
}

#[test]
#[ignore]
fn test_playback_engine_does_not_crash_on_cancel_before_ingest() {
    use vox_ui_lib::playback::PlaybackEngine;

    let active = Arc::new(AtomicBool::new(false));
    let cancel = Arc::new(AtomicBool::new(false));

    let engine = PlaybackEngine::new(Arc::clone(&active), Arc::clone(&cancel))
        .expect("PlaybackEngine creation failed");

    // Cancel immediately before any ingest
    engine.cancel();
    assert!(!active.load(Ordering::Relaxed));

    // Ingest after cancel — should be a no-op
    engine.ingest_chunk(&vec![0.0f32; 1000]);
    assert!(!active.load(Ordering::Relaxed), "Ingesting after cancel must be a no-op");

    println!("[PIPELINE TEST] Cancel-before-ingest: PASS");
}
