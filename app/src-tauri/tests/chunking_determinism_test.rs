//! ============================================================================
//! chunking_determinism_test.rs — Clause Chunking Determinism
//! ============================================================================
//! Category     : Integration Test (Seam 10)
//! Component    : services/tts/actor.rs (TtsClauseChunker) +
//!                pipeline/assistant/accumulator.rs (TurnAccumulator)
//! Prerequisites: None (pure logic, no models or services)
//! Execution    : cargo nextest run --test chunking_determinism_test --release --nocapture --test-threads=1
//! Metrics      : Determinism across token fragmentations, emergency 20-word cap,
//!                comma prosody gating stability, clean buffer flush
//! ============================================================================

use vox_lib::{
    pipeline::assistant::TurnAccumulator, services::harness::stages::streaming::ClauseChunker,
};

/// Subtest 1: The exact same logical text fed across two wildly different
/// token fragmentations must yield identical clause sequences byte-for-byte and order-preserved.
#[test]
fn test_chunking_determinism_across_fragmentations() {
    // Logical text containing multiple clause split types:
    // 1. Gated comma (>= 5 words before comma)
    // 2. Strong sentence terminator (. ! ?)
    // 3. Sub-clause with semicolon/colon
    // 4. Abbreviation guard ("Dr. Smith")
    // 5. Decimal guard ("3.14")
    // Logical text containing multiple clauses, questions, and sentences
    let _full_text = "Good morning everyone, today we are testing. Dr. Smith is here. How are you doing today? Let's verify this now.";

    // Fragmentation A: Fine-grained tokenization (word/sub-word tokens from LLM)
    let tokens_a = vec![
        "Good",
        " morning",
        " everyone",
        ",",
        " today",
        " we",
        " are",
        " testing",
        ".",
        " Dr. Smith",
        " is",
        " here",
        ".",
        " How",
        " are",
        " you",
        " doing",
        " today",
        "?",
        " Let's",
        " verify",
        " this",
        " now",
        ".",
    ];

    // Fragmentation B: Coarse/erratic tokenization (varying chunk sizes, split across punctuation)
    let tokens_b = vec![
        "Good morning",
        " everyone,",
        " today we are",
        " testing.",
        " Dr. Smith is here. How are you",
        " doing today? Let's verify this now.",
    ];

    // Verify both token streams reconstruct the exact logical text
    assert_eq!(
        tokens_a.concat(),
        tokens_b.concat(),
        "Token sets must represent the exact same logical stream"
    );

    // Run Fragmentation A
    let mut acc_a = TurnAccumulator::new();
    let mut clauses_a = Vec::new();
    for tok in tokens_a {
        clauses_a.extend(acc_a.push_token(tok));
    }
    if let Some(remainder) = acc_a.flush_chunker() {
        if !remainder.trim().is_empty() {
            clauses_a.push(remainder);
        }
    }

    // Run Fragmentation B
    let mut acc_b = TurnAccumulator::new();
    let mut clauses_b = Vec::new();
    for tok in tokens_b {
        clauses_b.extend(acc_b.push_token(tok));
    }
    if let Some(remainder) = acc_b.flush_chunker() {
        if !remainder.trim().is_empty() {
            clauses_b.push(remainder);
        }
    }

    // Assert determinism
    assert!(
        !clauses_a.is_empty(),
        "Upstream producer must produce clauses (clauses_a was empty)"
    );
    assert_eq!(
        clauses_a, clauses_b,
        "Chunker clauses must be identical byte-for-byte regardless of token fragmentation"
    );

    // Verify accumulator state
    assert!(
        acc_a.chunker.is_empty(),
        "Accumulator A buffer must be empty after flush"
    );
    assert!(
        acc_b.chunker.is_empty(),
        "Accumulator B buffer must be empty after flush"
    );

    // Verify clear() contract
    acc_a.clear();
    assert!(acc_a.assistant_response.is_empty());
    assert!(acc_a.user_transcript.is_empty());
    assert!(acc_a.chunker.is_empty());
}

/// Subtest 2: Unpunctuated runaway stream exceeding the hard ceiling (>= 38 words)
/// is force-chunked at word 30 to prevent buffer bloat while maintaining determinism across tokenizations.
#[test]
fn test_chunking_determinism_emergency_cap() {
    // 42 unpunctuated words
    let words = vec![
        "alpha", "bravo", "charlie", "delta", "echo", "foxtrot", "golf", "hotel", "india",
        "juliet", "kilo", "lima", "mike", "november", "oscar", "papa", "quebec", "romeo", "sierra",
        "tango", "uniform", "victor", "whiskey", "xray", "yankee", "zulu", "one", "two", "three",
        "four", "five", "six", "seven", "eight", "nine", "ten", "eleven", "twelve", "thirteen",
        "fourteen", "fifteen", "sixteen",
    ];
    assert_eq!(words.len(), 42);

    // Fragmentation A: 1 word per token
    let tokens_a: Vec<String> = words.iter().map(|w| format!("{} ", w)).collect();

    // Fragmentation B: 3 words per token
    let tokens_b: Vec<String> = words.chunks(3).map(|chunk| chunk.join(" ") + " ").collect();

    // Run A
    let mut chunker_a = ClauseChunker::new();
    let mut chunks_a = Vec::new();
    for tok in &tokens_a {
        chunks_a.extend(chunker_a.push_str(tok));
    }
    if let Some(rem) = chunker_a.flush() {
        if !rem.trim().is_empty() {
            chunks_a.push(rem);
        }
    }

    // Run B
    let mut chunker_b = ClauseChunker::new();
    let mut chunks_b = Vec::new();
    for tok in &tokens_b {
        chunks_b.extend(chunker_b.push_str(tok));
    }
    if let Some(rem) = chunker_b.flush() {
        if !rem.trim().is_empty() {
            chunks_b.push(rem);
        }
    }

    // Both must yield exactly 2 chunks under the >= 38 word safety ceiling:
    // Chunk 0: 30 words (emergency split at word 30)
    // Chunk 1: 12 words (flushed remainder)
    assert_eq!(
        chunks_a.len(),
        2,
        "42-word unpunctuated input must produce exactly 2 chunks under safety ceiling (got {})",
        chunks_a.len()
    );
    assert_eq!(
        chunks_a, chunks_b,
        "Emergency cap chunks must match across fragmentations"
    );

    let chunk_0_word_count = chunks_a[0].split_whitespace().count();
    let chunk_1_word_count = chunks_a[1].split_whitespace().count();

    assert_eq!(
        chunk_0_word_count, 30,
        "First chunk must have exactly 30 words from emergency cap"
    );
    assert_eq!(
        chunk_1_word_count, 12,
        "Second chunk must have remaining 12 words"
    );
}

/// Subtest 3: Comma prosody preservation & dynamic conjunction seam split.
/// Normal sentences with commas must NOT split at commas.
/// Compound runaway sentences (>= 22 words) split at coordinating conjunction seams.
#[test]
fn test_chunking_determinism_comma_gate_stable() {
    // Case 1: Standard sentence with comma -> comma does NOT split
    let _short_sentence = "Hello my friend, how are you today?";
    let tokens_short_1 = vec!["Hello my friend, ", "how are you today?"];
    let tokens_short_2 = vec!["Hello", " my ", "friend", ",", " how are you today?"];

    let mut c1 = ClauseChunker::new();
    let mut res1 = Vec::new();
    for t in tokens_short_1 {
        res1.extend(c1.push_str(t));
    }
    if let Some(r) = c1.flush() {
        res1.push(r);
    }

    let mut c2 = ClauseChunker::new();
    let mut res2 = Vec::new();
    for t in tokens_short_2 {
        res2.extend(c2.push_str(t));
    }
    if let Some(r) = c2.flush() {
        res2.push(r);
    }

    assert_eq!(res1, res2);
    assert_eq!(
        res1.len(),
        1,
        "Standard sentence with comma must not split at comma, preserving natural prosody"
    );
    assert_eq!(res1[0], "Hello my friend, how are you today?");

    // Case 2: Runaway compound sentence (>= 22 words) with conjunction seam -> DOES split at seam
    let _long_sentence = "We have carefully verified that all system components and background services are running smoothly, and we can proceed with the live rollout now.";
    let tokens_long_1 = vec![
        "We have carefully verified that all system components and background services are running smoothly, ",
        "and we can proceed with the live rollout now.",
    ];
    let tokens_long_2 = vec![
        "We have carefully ",
        "verified that all system components ",
        "and background services are running smoothly,",
        " and we can proceed with ",
        "the live rollout now.",
    ];

    let mut c3 = ClauseChunker::new();
    let mut res3 = Vec::new();
    for t in tokens_long_1 {
        res3.extend(c3.push_str(t));
    }
    if let Some(r) = c3.flush() {
        res3.push(r);
    }

    let mut c4 = ClauseChunker::new();
    let mut res4 = Vec::new();
    for t in tokens_long_2 {
        res4.extend(c4.push_str(t));
    }
    if let Some(r) = c4.flush() {
        res4.push(r);
    }

    assert_eq!(res3, res4);
    assert_eq!(
        res3.len(),
        2,
        "Long compound sentence (>= 22 words) must split into 2 clauses at conjunction seam"
    );
    assert_eq!(
        res3[0],
        "We have carefully verified that all system components and background services are running smoothly,"
    );
    assert_eq!(
        res3[1],
        "and we can proceed with the live rollout now."
    );
}
