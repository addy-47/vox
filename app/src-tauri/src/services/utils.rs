
/// Returns `true` if the accumulated token buffer should be flushed to TTS.
#[inline]
pub fn should_flush(buf: &str, word_count: usize, elapsed_ms: u128) -> bool {
    let trimmed = buf.trim_end();
    let last = trimmed.chars().last().unwrap_or(' ');
    if matches!(last, '.' | '!' | '?') { return true; }
    if matches!(last, ',' | ';') { return true; }
    if trimmed.ends_with(" — ") || trimmed.ends_with(" - ") { return true; }
    
    // Time-based Flush Gateway: If we have at least 1-2 words and 800ms have passed, force flush
    if word_count >= 1 && elapsed_ms > 800 { return true; }
    
    // Fallback: If 4 words accumulate without hitting timeout, flush anyway
    word_count >= 4
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
pub fn transliterate_if_hi(text: &str) -> String {
    if !crate::core::settings::VoxSettings::load().asr.transliterate_enabled {
        return text.to_string();
    }

    if !is_devanagari(text) {
        return text.to_string();
    }

    // Determine if the text ends with whitespace or punctuation
    let ends_with_boundary = if let Some(last_char) = text.chars().last() {
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
    transliterate_if_hi(text)
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

    // 1. Soft subslice/containment check to prevent older/smaller overlapping frames from duplicating
    if is_soft_subslice(&p_words, &s_words) {
        return p_clean.to_string();
    }

    // 2. Overlap matching
    let max_overlap = p_words.len().min(s_words.len());
    let mut best_overlap_len = 0;

    // Find the longest overlap where the end of p_words matches the start of s_words
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

    #[test]
    fn test_hinglish_normalization() {
        // Without engine initialized, transliterate_if_hi should fallback to raw word safely
        assert_eq!(transliterate_if_hi("नमस्ते"), "नमस्ते");
        assert_eq!(transliterate_if_hi("hello"), "hello");
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
    }
}

