use lipilekhika::transliterate;

/// Returns `true` if the accumulated token buffer should be flushed to TTS.
///
/// Flush conditions (in priority order):
///   1. Hard boundaries: `.` `!` `?`
///   2. Soft boundaries: `,` `;` ` — ` ` - `
///   3. Word count limit: ≥ 6 words accumulated without any boundary
///
/// This guarantees Time-to-First-Audio ≤ ~500ms regardless of LLM sentence length.
#[inline]
pub fn should_flush(buf: &str, word_count: usize) -> bool {
    let trimmed = buf.trim_end();
    let last = trimmed.chars().last().unwrap_or(' ');

    // Hard boundaries — always flush
    if matches!(last, '.' | '!' | '?') {
        return true;
    }

    // Soft boundaries — flush to begin audio early
    if matches!(last, ',' | ';') {
        return true;
    }
    if trimmed.ends_with(" — ") || trimmed.ends_with(" - ") {
        return true;
    }

    // Word count gate — prevent long-sentence lag
    word_count >= 6
}

/// Count words in the accumulated buffer.
#[inline]
pub fn count_words(s: &str) -> usize {
    s.split_whitespace().count()
}

/// Detect if string contains Devanagari (Hindi) characters.
pub fn is_devanagari(text: &str) -> bool {
    text.chars().any(|c| ('\u{0900}'..='\u{097F}').contains(&c))
}

/// Transliterates Devanagari to Roman script if Hindi is detected.
/// Used strictly for UI display to provide a "Hinglish" experience.
pub fn transliterate_if_hi(text: &str) -> String {
    if is_devanagari(text) {
        // Transliterate from Devanagari to Latin (English/Roman)
        transliterate(text, "Hindi", "English", None).unwrap_or_else(|_| text.to_string())
    } else {
        text.to_string()
    }
}
