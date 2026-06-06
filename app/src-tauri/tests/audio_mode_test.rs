/// Tests for audio output mode behaviour:
///   - Speaker mode: mic ducked during playback
///   - Headset mode: mic stays active (barge-in)
///   - sub_sentence chunker (Directive 2) logic tested here as a pure function
///
/// cargo test --test audio_mode_test -- --nocapture

// ─── Sub-Sentence Chunker Unit Tests (Directive 2) ───────────────────────────

fn should_flush(buf: &str, word_count: usize) -> bool {
    vox_lib::services::utils::should_flush(buf, word_count, 100, 3.5)
}

#[test]
fn test_flush_on_hard_boundary_period() {
    assert!(should_flush("Hello world.", 2), "Period should flush");
}

#[test]
fn test_flush_on_hard_boundary_exclamation() {
    assert!(should_flush("Great!", 1), "Exclamation should flush");
}

#[test]
fn test_flush_on_hard_boundary_question() {
    assert!(should_flush("What is this?", 3), "Question should flush");
}

#[test]
fn test_flush_on_soft_boundary_comma() {
    assert!(should_flush("Well,", 1), "Comma should flush (soft boundary)");
}

#[test]
fn test_flush_on_soft_boundary_semicolon() {
    assert!(should_flush("First clause;", 2), "Semicolon should flush");
}

#[test]
fn test_flush_on_word_count_limit() {
    // Exactly 6 words, no punctuation — must flush
    let buf = "one two three four five six";
    assert!(should_flush(buf, 6), "6 words without punctuation must flush (Directive 2)");
}

#[test]
fn test_no_flush_below_word_count_with_no_punctuation() {
    let buf = "one two three four five";
    assert!(!should_flush(buf, 5), "5 words without punctuation should NOT flush");
}

#[test]
fn test_no_flush_on_empty_buffer() {
    assert!(!should_flush("", 0), "Empty buffer should never flush");
}

#[test]
fn test_flush_on_em_dash() {
    // Em dash as soft boundary
    let buf = "he said — ";
    // Note: ends_with check includes trailing space
    assert!(should_flush(buf, 2), "Em dash should flush as soft boundary");
}

#[test]
fn test_flush_priority_hard_before_word_limit() {
    // Even 1 word with hard punctuation should flush
    assert!(should_flush("Stop.", 1), "Hard punctuation beats word count — should flush at 1 word");
    assert!(!should_flush("Go", 1), "1 word no punctuation should NOT flush");
}

// ─── AudioOutputMode Equality ─────────────────────────────────────────────────

#[test]
fn test_audio_output_mode_default_is_speaker() {
    use vox_lib::core::settings::AudioOutputMode;
    let mode: AudioOutputMode = Default::default();
    assert_eq!(mode, AudioOutputMode::Speaker, "Default mode must be Speaker");
}

#[test]
fn test_audio_output_mode_equality() {
    use vox_lib::core::settings::AudioOutputMode;
    assert_eq!(AudioOutputMode::Speaker,  AudioOutputMode::Speaker);
    assert_eq!(AudioOutputMode::Headset,  AudioOutputMode::Headset);
    assert_ne!(AudioOutputMode::Speaker,  AudioOutputMode::Headset);
}
