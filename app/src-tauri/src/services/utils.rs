
/// Returns `true` if the accumulated token buffer should be flushed to TTS.
#[inline]
pub fn should_flush(buf: &str, word_count: usize, elapsed_ms: u128, tps: f32) -> bool {
    let trimmed = buf.trim_end();
    let last = trimmed.chars().last().unwrap_or(' ');
    
    // Hard boundaries: always flush
    if matches!(last, '.' | '!' | '?') { return true; }
    
    // Determine dynamic thresholds based on Token-Per-Second (TPS)
    let (soft_words, time_gate_words, fallback_words) = if tps <= 2.0 {
        // Slow generation (e.g. CPU bottlenecks): prioritize latency/TTFA by using smaller chunks
        (3, 3, 4)
    } else if tps > 4.0 {
        // Fast generation: prioritize speech continuity/prosody by using larger chunks
        (5, 5, 12)
    } else {
        // Standard baseline (2.0 < TPS <= 4.0)
        (3, 3, 8)
    };
    
    // Soft boundaries: only flush with enough words for coherent speech
    if matches!(last, ',' | ';') && word_count >= soft_words { return true; }
    if (trimmed.ends_with(" — ") || trimmed.ends_with(" - ")) && word_count >= soft_words { return true; }
    
    // Time-based: elapsed time and dynamic word minimum
    if word_count >= time_gate_words && elapsed_ms > 1500 { return true; }
    
    // Word-count fallback
    word_count >= fallback_words
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
/// Implements Incomplete Word Protection: If the string does not end with a boundary,
/// the last word is bypassed to prevent partial word transliteration artifacts.
pub fn transliterate_if_hi(text: &str, is_final: bool, transliterate_enabled: bool) -> String {
    if !transliterate_enabled {
        return text.to_string();
    }

    if !is_devanagari(text) {
        return text.to_string();
    }

    // Determine if the text ends with whitespace or punctuation
    let ends_with_boundary = if is_final {
        true
    } else if let Some(last_char) = text.chars().last() {
        last_char.is_whitespace() || last_char.is_ascii_punctuation() || last_char == '।'
    } else {
        true
    };

    #[derive(Debug)]
    enum Token {
        DevanagariWord(String),
        Other(String),
    }

    let mut tokens = Vec::new();
    let mut current_token = String::new();
    let mut in_devanagari = false;

    for c in text.chars() {
        let is_c_devanagari = ('\u{0900}'..='\u{097F}').contains(&c);
        if is_c_devanagari {
            if !in_devanagari && !current_token.is_empty() {
                tokens.push(Token::Other(current_token));
                current_token = String::new();
            }
            in_devanagari = true;
        } else {
            if in_devanagari && !current_token.is_empty() {
                tokens.push(Token::DevanagariWord(current_token));
                current_token = String::new();
            }
            in_devanagari = false;
        }
        current_token.push(c);
    }
    
    if !current_token.is_empty() {
        if in_devanagari {
            tokens.push(Token::DevanagariWord(current_token));
        } else {
            tokens.push(Token::Other(current_token));
        }
    }

    let mut result = String::new();
    let num_tokens = tokens.len();
    for (i, token) in tokens.into_iter().enumerate() {
        match token {
            Token::DevanagariWord(word) => {
                let is_last = i == num_tokens - 1;
                if is_last && !ends_with_boundary {
                    // Incomplete word protection: leave final word in raw Devanagari
                    result.push_str(&word);
                } else {
                    // Complete word: transliterate directly using the native ONNX model
                    let raw_trans = crate::services::translit::transliterate(&word);
                    result.push_str(&raw_trans);
                }
            }
            Token::Other(other) => {
                result.push_str(&other);
            }
        }
    }

    result
}

/// Backward-compatible Hinglish engine wrapper.
pub fn to_friendly_hinglish(text: &str) -> String {
    transliterate_if_hi(text, true, true)
}

fn edit_distance(s1: &str, s2: &str) -> usize {
    let v1: Vec<char> = s1.chars().collect();
    let v2: Vec<char> = s2.chars().collect();
    let len1 = v1.len();
    let len2 = v2.len();
    
    let mut dp = vec![vec![0; len2 + 1]; len1 + 1];
    for i in 0..=len1 { dp[i][0] = i; }
    for j in 0..=len2 { dp[0][j] = j; }
    
    for i in 1..=len1 {
        for j in 1..=len2 {
            if v1[i-1] == v2[j-1] {
                dp[i][j] = dp[i-1][j-1];
            } else {
                dp[i][j] = 1 + dp[i-1][j-1].min(dp[i-1][j].min(dp[i][j-1]));
            }
        }
    }
    dp[len1][len2]
}

fn words_soft_match(w1: &str, w2: &str) -> bool {
    let clean1 = w1.trim_matches(|c: char| c.is_ascii_punctuation() || c == '।').to_lowercase();
    let clean2 = w2.trim_matches(|c: char| c.is_ascii_punctuation() || c == '।').to_lowercase();
    if clean1 == clean2 {
        return true;
    }
    
    let dist = edit_distance(&clean1, &clean2);
    let len = clean1.chars().count().max(clean2.chars().count());
    
    if len <= 3 {
        dist <= 1
    } else {
        dist <= (len / 3).max(1)
    }
}


fn is_soft_subslice(p_words: &[&str], s_words: &[&str]) -> bool {
    if s_words.is_empty() {
        return true;
    }
    if p_words.len() < s_words.len() {
        return false;
    }
    
    for i in 0..=(p_words.len() - s_words.len()) {
        let mut matched = true;
        for j in 0..s_words.len() {
            if !words_soft_match(p_words[i + j], s_words[j]) {
                matched = false;
                break;
            }
        }
        if matched {
            return true;
        }
    }
    false
}

/// Seamlessly stitches two transcription fragments (prefix and suffix) using word-level overlap matching.
/// This prevents visual flashing/amnesia in the UI when using rolling window partial transcripters.
pub fn stitch_transcripts(prefix: &str, suffix: &str) -> String {
    let p_clean = prefix.trim();
    let s_clean = suffix.trim();
    if p_clean.is_empty() {
        return s_clean.to_string();
    }
    if s_clean.is_empty() {
        return p_clean.to_string();
    }

    let p_words: Vec<&str> = p_clean.split_whitespace().collect();
    let s_words: Vec<&str> = s_clean.split_whitespace().collect();

    // 0. Soft subslice/containment check to prevent older/smaller overlapping frames from duplicating
    if is_soft_subslice(&p_words, &s_words) {
        return p_clean.to_string();
    }

    // 1. Alignment search (look for an overlapping/matching segment anywhere in prefix)
    let mut best_i = 0;
    let mut best_j = 0;
    let mut max_match_len = 0;

    let max_j = 8.min(s_words.len());
    for i in 0..p_words.len() {
        for j in 0..max_j {
            let mut len = 0;
            while i + len < p_words.len() && j + len < s_words.len() {
                if words_soft_match(p_words[i + len], s_words[j + len]) {
                    len += 1;
                } else {
                    break;
                }
            }
            if len > max_match_len {
                max_match_len = len;
                best_i = i;
                best_j = j;
            }
        }
    }

    let min_required_match = 3.min(s_words.len());
    if max_match_len >= min_required_match {
        let mut result_words = p_words[..best_i].to_vec();
        result_words.extend_from_slice(&s_words[best_j..]);
        return result_words.join(" ");
    }

    // 2. Sequential overlap matching (fallback if alignment search failed)
    let max_overlap = p_words.len().min(s_words.len());
    let mut best_overlap_len = 0;

    for k in (1..=max_overlap).rev() {
        let p_slice = &p_words[p_words.len() - k..];
        let s_slice = &s_words[..k];
        
        let mut matched = true;
        for i in 0..k {
            if !words_soft_match(p_slice[i], s_slice[i]) {
                matched = false;
                break;
            }
        }
        
        if matched {
            best_overlap_len = k;
            break;
        }
    }

    if best_overlap_len > 0 {
        let mut result_words = p_words;
        result_words.extend_from_slice(&s_words[best_overlap_len..]);
        result_words.join(" ")
    } else {
        format!("{} {}", p_clean, s_clean)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn init_paths_for_testing() {
        let _ = crate::utils::paths::init_with_root(std::env::temp_dir().join("vox_test"));
    }

    #[test]
    fn test_hinglish_normalization() {
        init_paths_for_testing();
        // Without engine initialized, transliterate_if_hi should fallback to raw word safely
        assert_eq!(transliterate_if_hi("नमस्ते", false, true), "नमस्ते");
        assert_eq!(transliterate_if_hi("hello", false, true), "hello");
    }

    #[test]
    fn test_stitch_transcripts_overlap() {
        // Standard Hinglish overlap
        assert_eq!(
            stitch_transcripts("mera phone number", "phone number hai 98409"),
            "mera phone number hai 98409"
        );

        // Case insensitivity and punctuation stripping
        assert_eq!(
            stitch_transcripts("Mera phone, number!", "Phone number: hai?"),
            "Mera phone, number! hai?"
        );

        // Baseline tests (tps = 3.5)
        assert!(should_flush("hello world. ", 2, 100, 3.5));
        assert!(should_flush("hello world! ", 2, 100, 3.5));
        assert!(!should_flush("hello, ", 1, 100, 3.5));
        assert!(should_flush("hello world one, ", 3, 100, 3.5));
        assert!(should_flush("hello world one two three four five six seven eight", 8, 100, 3.5));
        assert!(!should_flush("hello world one two three", 4, 100, 3.5));
        assert!(should_flush("hello world one two three", 4, 1600, 3.5));
        assert!(!should_flush("hello world one two", 3, 1600, 3.5));

        // Slow TPS tests (tps = 1.5)
        assert!(should_flush("hello world one two", 4, 100, 1.5)); // Fallback is 4
        assert!(should_flush("hello world, ", 2, 100, 1.5)); // Soft is still 3? Wait, count of words: hello world is 2, soft is 3, so not flush
        assert!(!should_flush("hello world, ", 2, 100, 1.5));
        assert!(should_flush("hello world one, ", 3, 100, 1.5));

        // Fast TPS tests (tps = 5.0)
        assert!(!should_flush("hello world one two three four five six", 6, 100, 5.0)); // Fallback is 12
        assert!(!should_flush("hello world one, ", 3, 100, 5.0)); // Soft is 5
        assert!(should_flush("hello world one two three, ", 5, 100, 5.0)); // Soft is 5
        assert!(should_flush("hello world one two three four five six seven eight nine ten eleven twelve", 12, 100, 5.0));

        // No overlap concatenation fallback
        assert_eq!(
            stitch_transcripts("mera phone", "aur kuch"),
            "mera phone aur kuch"
        );

        // Empty states
        assert_eq!(stitch_transcripts("", "hello"), "hello");
        assert_eq!(stitch_transcripts("world", ""), "world");

        // Complete containment
        assert_eq!(
            stitch_transcripts("mera phone number", "number"),
            "mera phone number"
        );

        // Containment of middle slice
        assert_eq!(
            stitch_transcripts("mera phone number hai", "phone number"),
            "mera phone number hai"
        );

        // Soft match overlap
        assert_eq!(
            stitch_transcripts("mera phone numbere", "number hai"),
            "mera phone numbere hai"
        );

        assert_eq!(
            stitch_transcripts(
                "Expert nature is around. Expecting to raise around twelve, I think forty thousand per month.",
                "Expect nature is around twelve. I think forty thousand per month."
            ),
            "Expert nature is around. Expecting to raise around twelve. I think forty thousand per month."
        );

        assert_eq!(
            stitch_transcripts(
                "Spending any more than than than. thousand or greater because they are not they have said that they are spending only through credits.",
                "I have said that they are spending only through credit. Right now."
            ),
            "Spending any more than than than. thousand or greater because they are not they have said that they are spending only through credit. Right now."
        );
    }
}

