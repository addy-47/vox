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
