//! ============================================================================
//! benches/common/scoring.rs — Text Normalization & Levenshtein Similarity
//! ============================================================================

/// Normalizes transcript text by lowercasing and stripping punctuation and extra whitespace.
pub fn normalize_text(text: &str) -> String {
    text.chars()
        .filter(|c| {
            !c.is_ascii_punctuation()
                && !matches!(
                    c,
                    '!' | '?' | '.' | ',' | '।' | '-' | ':' | ';' | '"' | '\'' | '`' | '“' | '”'
                )
        })
        .collect::<String>()
        .to_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Computes normalized Levenshtein similarity [0.0, 1.0] between hypothesis and reference.
pub fn levenshtein_similarity(hyp: &str, ref_str: &str) -> f64 {
    let norm_hyp = normalize_text(hyp);
    let norm_ref = normalize_text(ref_str);

    let hyp_chars: Vec<char> = norm_hyp.chars().collect();
    let ref_chars: Vec<char> = norm_ref.chars().collect();

    let len1 = hyp_chars.len();
    let len2 = ref_chars.len();

    if len1 == 0 && len2 == 0 {
        return 1.0;
    }
    if len1 == 0 || len2 == 0 {
        return 0.0;
    }

    let mut dp = vec![vec![0usize; len2 + 1]; len1 + 1];
    for (i, row) in dp.iter_mut().enumerate().take(len1 + 1) {
        row[0] = i;
    }
    if let Some(first_row) = dp.first_mut() {
        for (j, cell) in first_row.iter_mut().enumerate().take(len2 + 1) {
            *cell = j;
        }
    }

    for i in 1..=len1 {
        for j in 1..=len2 {
            let cost = if hyp_chars[i - 1] == ref_chars[j - 1] {
                0
            } else {
                1
            };
            dp[i][j] = (dp[i - 1][j] + 1)
                .min(dp[i][j - 1] + 1)
                .min(dp[i - 1][j - 1] + cost);
        }
    }

    let distance = dp[len1][len2];
    let max_len = len1.max(len2);
    1.0 - (distance as f64 / max_len as f64)
}
