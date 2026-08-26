/// Returns true if the accumulated token buffer ends at a word boundary or whitespace.
#[inline]
fn ends_at_word_boundary(buf: &str) -> bool {
    if let Some(c) = buf.chars().last() {
        c.is_whitespace()
            || matches!(
                c,
                '.' | '!' | '?' | ',' | ';' | ':' | ')' | ']' | '\u{2014}' | '\u{2013}' | '।'
            )
    } else {
        true
    }
}

/// Linear interpolation mapping `t` in [0.0, 1.0] to [min_val, max_val].
#[inline]
fn lerp(t: f32, min_val: f32, max_val: f32) -> f32 {
    min_val + t * (max_val - min_val)
}

/// Evaluates if the accumulated token buffer should be flushed to the TTS synthesizer.
#[inline]
pub fn should_flush(buf: &str, word_count: usize, elapsed_ms: u128, tps: f32) -> bool {
    let trimmed = buf.trim_end();
    let last = trimmed.chars().last().unwrap_or(' ');

    if matches!(last, '.' | '!' | '?' | '।') {
        return true;
    }

    let tps_clamped = tps.clamp(0.5, 6.0);
    let tps_norm = (tps_clamped - 0.5) / (6.0 - 0.5);

    if matches!(last, ',' | ';' | '—' | '–') || trimmed.ends_with(" —") || trimmed.ends_with(" -") {
        let clause_tps_high = 5.0;
        let clause_tps_low = 3.0;
        let clause_norm_low = (clause_tps_low - 0.5) / (6.0 - 0.5);
        let clause_norm_high = (clause_tps_high - 0.5) / (6.0 - 0.5);
        if tps_norm < clause_norm_high {
            let t = (tps_norm - clause_norm_low).max(0.0) / (clause_norm_high - clause_norm_low);
            let clause_threshold = (3.0 + t * 4.0).round() as usize;
            if word_count >= clause_threshold {
                return true;
            }
        }
    }

    let max_wait_ms = lerp(tps_norm, 1000.0, 3500.0) as u128;
    let min_time_words = lerp(tps_norm, 3.0, 8.0).round() as usize;
    if elapsed_ms >= max_wait_ms && word_count >= min_time_words && ends_at_word_boundary(buf) {
        return true;
    }

    let max_words = lerp(tps_norm, 5.0, 20.0).round() as usize;
    if word_count >= max_words && ends_at_word_boundary(buf) {
        return true;
    }

    false
}

/// Counts whitespace-separated words in a text string.
#[inline]
pub fn count_words(s: &str) -> usize {
    s.split_whitespace().count()
}

/// Returns true if the string contains any Devanagari Unicode characters (U+0900..=U+097F).
pub fn is_devanagari(text: &str) -> bool {
    text.chars().any(|c| ('\u{0900}'..='\u{097F}').contains(&c))
}

#[derive(Debug)]
enum ScriptToken {
    Devanagari(String),
    Other(String),
}

/// Partitions a text string into contiguous Devanagari and non-Devanagari character slices.
fn tokenize_devanagari_slices(text: &str) -> Vec<ScriptToken> {
    let mut tokens = Vec::new();
    let mut current_token = String::new();
    let mut in_devanagari = false;

    for c in text.chars() {
        let is_c_devanagari = ('\u{0900}'..='\u{097F}').contains(&c);
        if is_c_devanagari {
            if !in_devanagari && !current_token.is_empty() {
                tokens.push(ScriptToken::Other(current_token));
                current_token = String::new();
            }
            in_devanagari = true;
        } else {
            if in_devanagari && !current_token.is_empty() {
                tokens.push(ScriptToken::Devanagari(current_token));
                current_token = String::new();
            }
            in_devanagari = false;
        }
        current_token.push(c);
    }

    if !current_token.is_empty() {
        if in_devanagari {
            tokens.push(ScriptToken::Devanagari(current_token));
        } else {
            tokens.push(ScriptToken::Other(current_token));
        }
    }

    tokens
}

/// Transliterates Devanagari Hindi text to Roman script with trailing incomplete word protection.
pub fn transliterate_if_hi(text: &str, is_final: bool, transliterate_enabled: bool) -> String {
    if !transliterate_enabled || !is_devanagari(text) {
        return text.to_string();
    }

    let ends_with_boundary = if is_final {
        true
    } else if let Some(last_char) = text.chars().last() {
        last_char.is_whitespace() || last_char.is_ascii_punctuation() || last_char == '।'
    } else {
        true
    };

    let tokens = tokenize_devanagari_slices(text);
    let mut result = String::new();
    let num_tokens = tokens.len();

    for (i, token) in tokens.into_iter().enumerate() {
        match token {
            ScriptToken::Devanagari(word) => {
                let is_last = i == num_tokens - 1;
                if is_last && !ends_with_boundary {
                    result.push_str(&word);
                } else {
                    let raw_trans = crate::services::translit::transliterate(&word);
                    result.push_str(&raw_trans);
                }
            }
            ScriptToken::Other(other) => {
                result.push_str(&other);
            }
        }
    }

    result
}

/// Converts Hindi text to friendly phonetic Hinglish using full final transliteration.
pub fn to_friendly_hinglish(text: &str) -> String {
    transliterate_if_hi(text, true, true)
}

/// Computes character Levenshtein edit distance between two strings.
fn edit_distance(s1: &str, s2: &str) -> usize {
    let v1: Vec<char> = s1.chars().collect();
    let v2: Vec<char> = s2.chars().collect();
    let len1 = v1.len();
    let len2 = v2.len();

    let mut dp = vec![vec![0; len2 + 1]; len1 + 1];
    for (i, row) in dp.iter_mut().enumerate().take(len1 + 1) {
        row[0] = i;
    }
    for (j, cell) in dp[0].iter_mut().enumerate().take(len2 + 1) {
        *cell = j;
    }

    for i in 1..=len1 {
        for j in 1..=len2 {
            if v1[i - 1] == v2[j - 1] {
                dp[i][j] = dp[i - 1][j - 1];
            } else {
                dp[i][j] = 1 + dp[i - 1][j - 1].min(dp[i - 1][j].min(dp[i][j - 1]));
            }
        }
    }
    dp[len1][len2]
}

/// Returns true if two words match exactly or within normalized Levenshtein threshold.
fn words_soft_match(w1: &str, w2: &str) -> bool {
    let clean1 = w1
        .trim_matches(|c: char| c.is_ascii_punctuation() || c == '।')
        .to_lowercase();
    let clean2 = w2
        .trim_matches(|c: char| c.is_ascii_punctuation() || c == '।')
        .to_lowercase();
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

/// Returns true if suffix words are entirely contained as a soft subslice within prefix words.
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

/// Searches for the longest matching overlapping segment between prefix and suffix word vectors.
fn find_alignment_match(p_words: &[&str], s_words: &[&str]) -> Option<(usize, usize)> {
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
        Some((best_i, best_j))
    } else {
        None
    }
}

/// Computes sequential overlap length between trailing prefix words and leading suffix words.
fn find_sequential_overlap(p_words: &[&str], s_words: &[&str]) -> usize {
    let max_overlap = p_words.len().min(s_words.len());

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
            return k;
        }
    }

    0
}

/// Stitches prefix and suffix transcription streams using soft word-level alignment matching.
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

    if is_soft_subslice(&p_words, &s_words) {
        return p_clean.to_string();
    }

    if let Some((best_i, best_j)) = find_alignment_match(&p_words, &s_words) {
        let mut result_words = p_words[..best_i].to_vec();
        result_words.extend_from_slice(&s_words[best_j..]);
        return result_words.join(" ");
    }

    let best_overlap_len = find_sequential_overlap(&p_words, &s_words);
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

    /// Tests immediate flushing on terminal sentence punctuation regardless of elapsed time.
    #[test]
    fn test_should_flush_sentence_terminals() {
        assert!(should_flush("Hello world.", 2, 0, 1.0));
        assert!(should_flush("This is a test!", 4, 0, 1.0));
        assert!(should_flush("Is this working?", 3, 0, 1.0));
        assert!(should_flush("यह एक परीक्षण है।", 4, 0, 1.0));
        assert!(should_flush("Ending with dot.  ", 3, 0, 1.0));
    }

    /// Tests dynamic clause punctuation scaling and threshold cutoff at high TPS.
    #[test]
    fn test_should_flush_clause_scaling() {
        assert!(should_flush("one, two, three,", 3, 50, 0.5));
        assert!(should_flush("one; two; three;", 3, 50, 3.0));
        assert!(should_flush("one — two — three — ", 3, 50, 3.0));
        assert!(!should_flush("one, two,", 2, 50, 0.5));
        assert!(!should_flush("one, two, three,", 3, 50, 6.0));
    }

    /// Tests timeout starvation triggering strictly at word boundaries and rejecting mid-word tokens.
    #[test]
    fn test_should_flush_timeout_and_word_boundaries() {
        assert!(should_flush("the quick brown ", 3, 1500, 0.5));
        assert!(!should_flush("the quick bro", 3, 1500, 0.5));
        assert!(!should_flush("the quick brown ", 3, 500, 0.5));
    }

    /// Tests TPS extreme boundary clamping without arithmetic overflow or panics.
    #[test]
    fn test_should_flush_tps_clamping_boundaries() {
        assert!(should_flush("terminal punctuation.", 2, 0, -100.0));
        assert!(should_flush("terminal punctuation.", 2, 0, 100.0));
        assert!(!should_flush("no punctuation incomplete", 3, 50, -5.0));
        assert!(!should_flush("no punctuation incomplete", 3, 50, 50.0));
    }

    /// Tests empty string handling for prefix and suffix transcript stitching.
    #[test]
    fn test_stitch_transcripts_empty_inputs() {
        assert_eq!(stitch_transcripts("", "hello world"), "hello world");
        assert_eq!(stitch_transcripts("hello world", ""), "hello world");
        assert_eq!(stitch_transcripts("   ", "hello world"), "hello world");
        assert_eq!(stitch_transcripts("hello world", "   "), "hello world");
    }

    /// Tests soft subslice containment returning prefix text unmodified.
    #[test]
    fn test_stitch_transcripts_subslice_containment() {
        let prefix = "The quick brown fox jumps over the lazy dog";
        let suffix = "brown fox jumps";
        assert_eq!(stitch_transcripts(prefix, suffix), prefix);
    }

    /// Tests multi-word alignment matching and sequential overlap stitching.
    #[test]
    fn test_stitch_transcripts_overlap_alignment() {
        let p1 = "The quick brown fox jumps over the lazy dog";
        let s1 = "jumps over the lazy dog and runs into the woods";
        assert_eq!(
            stitch_transcripts(p1, s1),
            "The quick brown fox jumps over the lazy dog and runs into the woods"
        );

        let p2 = "hello world there";
        let s2 = "there friend";
        assert_eq!(stitch_transcripts(p2, s2), "hello world there friend");
    }

    /// Tests disjoint non-overlapping transcripts concatenation and case/punctuation normalization.
    #[test]
    fn test_stitch_transcripts_disjoint_and_variations() {
        assert_eq!(
            stitch_transcripts("hello world", "apple banana"),
            "hello world apple banana"
        );
        assert_eq!(
            stitch_transcripts("Hello, World!", "world! How are you?"),
            "Hello, World! How are you?"
        );
    }
}
