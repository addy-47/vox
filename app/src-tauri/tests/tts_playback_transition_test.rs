//! ============================================================================
//! tts_playback_transition_test.rs — TTS Clause Transition & Playback Flush Test
//! ============================================================================
//! Category     : Integration Test
//! Component    : TTS Chunker (`vox_lib::services::tts::TtsClauseChunker`) &
//!                Playback Engine (`vox_lib::services::audio::PlaybackEngine`)
//! Prerequisites: Compiles against `vox_lib` public API
//! Execution    : cargo test --test tts_playback_transition_test
//! Metrics      : Clause boundary chunking, abbreviation preservation, & instant audio buffer flush
//! ============================================================================

use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::Arc;
use vox_lib::services::audio::PlaybackEngine;
use vox_lib::services::tts::TtsClauseChunker;

// ─── 1. TTS Clause Chunker Tests ─────────────────────────────────────────────

#[test]
fn test_tts_clause_chunker_punctuation_splitting() {
    let mut chunker = TtsClauseChunker::new();

    // 1. Push text with clause boundary (comma)
    let chunks_1 = chunker.push_str("Hello there, ");
    assert_eq!(chunks_1.len(), 1);
    assert_eq!(chunks_1[0], "Hello there,");

    // 2. Push text with period and exclamation
    let chunks_2 = chunker.push_str("this is Vox. How are you doing!");
    assert_eq!(chunks_2.len(), 2);
    assert_eq!(chunks_2[0], "this is Vox.");
    assert_eq!(chunks_2[1], "How are you doing!");

    // 3. Verify buffer is clear
    assert!(chunker.is_empty());
}

#[test]
fn test_tts_clause_chunker_abbreviation_and_decimal_protection() {
    let mut chunker = TtsClauseChunker::new();

    // Push abbreviation (Dr.) and decimal (3.14) and version string (v0.8.6)
    let chunks = chunker.push_str("Dr. Smith release v0.8.6 has 3.14 ratio. Done!");

    // "Dr." and "v0.8.6" and "3.14" MUST NOT split prematurely
    assert_eq!(chunks.len(), 2);
    assert_eq!(chunks[0], "Dr. Smith release v0.8.6 has 3.14 ratio.");
    assert_eq!(chunks[1], "Done!");
}

#[test]
fn test_tts_clause_chunker_clear_on_cancel() {
    let mut chunker = TtsClauseChunker::new();

    // Push incomplete fragment (no boundary punctuation)
    let chunks = chunker.push_str("Incomplete LLM response fragment without punctuation");
    assert_eq!(chunks.len(), 0);
    assert!(!chunker.is_empty());

    // User barges in -> purge chunker buffer
    chunker.clear();

    assert!(chunker.is_empty());
    assert_eq!(chunker.buffer(), "");
    assert_eq!(chunker.flush(), None);
}

// ─── 2. Playback Engine Audio Buffer Flush Tests ─────────────────────────────

#[test]
fn test_playback_engine_ingest_and_instant_cancel_flush() {
    let playback_active = Arc::new(AtomicBool::new(false));
    let cancel_flag = Arc::new(AtomicBool::new(false));
    let energy = Arc::new(AtomicU32::new(0));
    let low = Arc::new(AtomicU32::new(0));
    let mid = Arc::new(AtomicU32::new(0));
    let high = Arc::new(AtomicU32::new(0));
    let underruns = Arc::new(AtomicU64::new(0));
    let speaking = Arc::new(AtomicBool::new(false));

    // Initialize PlaybackEngine without starting CPAL audio device (may fallback if no audio device)
    let engine_res = PlaybackEngine::new(
        playback_active.clone(),
        cancel_flag.clone(),
        energy,
        low,
        mid,
        high,
        underruns,
        speaking,
    );

    if let Ok(engine) = engine_res {
        // 1. Ingest 24kHz audio chunk (1000 samples)
        let sample_chunk = vec![0.1f32; 1000];
        engine.ingest_chunk(&sample_chunk);

        // 2. Assert buffer contains upsampled samples (1000 24kHz -> 2000 48kHz)
        assert_eq!(engine.buffer_len(), 2000);
        assert_eq!(engine.total_samples_ingested(), 2000);

        // 3. Start playback
        engine.start_playback();
        assert!(playback_active.load(Ordering::Relaxed));

        // 4. Trigger barge-in / cancellation
        engine.cancel();

        // 5. Assert instant audio buffer flush (0ms audio spillover)
        assert_eq!(
            engine.buffer_len(),
            0,
            "Cancellation MUST flush playback audio buffer to 0 samples!"
        );
        assert!(
            cancel_flag.load(Ordering::Relaxed),
            "Cancellation MUST set cancel_flag to true!"
        );
        assert!(
            !playback_active.load(Ordering::Relaxed),
            "Cancellation MUST set playback_active to false!"
        );
    } else {
        println!("CPAL device not available in headless test environment; skipping CPAL stream assertions.");
    }
}
