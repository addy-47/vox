use std::collections::HashSet;

/// Cosine similarity threshold for Phase 1 hard deduplication merge (>= 0.98 or Jaccard == 1.0).
pub const COSINE_HARD_MATCH_THRESHOLD: f32 = 0.98;
/// Jaccard token set similarity threshold for exact match.
pub const JACCARD_EXACT_MATCH_THRESHOLD: f32 = 1.0;

/// Calculates Jaccard Word-Set Overlap Similarity between two strings.
/// Formula: J(A, B) = |A ∩ B| / |A ∪ B| on alphanumeric lowercased word tokens.
pub fn jaccard_similarity(s1: &str, s2: &str) -> f32 {
    let w1: HashSet<String> = s1
        .to_lowercase()
        .split_whitespace()
        .map(|s| s.replace(|c: char| c.is_ascii_punctuation() || c == '।', ""))
        .filter(|s| !s.is_empty())
        .collect();
    let w2: HashSet<String> = s2
        .to_lowercase()
        .split_whitespace()
        .map(|s| s.replace(|c: char| c.is_ascii_punctuation() || c == '।', ""))
        .filter(|s| !s.is_empty())
        .collect();

    if w1.is_empty() && w2.is_empty() {
        return 1.0;
    }
    if w1.is_empty() || w2.is_empty() {
        return 0.0;
    }

    let intersection = w1.intersection(&w2).count() as f32;
    let union = w1.union(&w2).count() as f32;
    intersection / union
}

pub fn is_exact_duplicate(cosine_sim: f32, jaccard_sim: f32) -> bool {
    cosine_sim >= COSINE_HARD_MATCH_THRESHOLD || jaccard_sim >= JACCARD_EXACT_MATCH_THRESHOLD
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_jaccard_similarity() {
        assert_eq!(jaccard_similarity("hello world", "hello world"), 1.0);
        assert_eq!(jaccard_similarity("hello world", "hello there world"), 2.0 / 3.0);
        assert_eq!(jaccard_similarity("apple", "banana"), 0.0);
    }

    #[test]
    fn test_jaccard_similarity_devanagari_matras() {
        // Verifies Devanagari vowel marks (matras) like 'ॉ', '्', 'ा' are preserved
        assert_eq!(jaccard_similarity("नमस्ते वॉक्स", "नमस्ते वॉक्स"), 1.0);
        assert_eq!(jaccard_similarity("क्या आप", "क्या आप"), 1.0);
        assert_eq!(jaccard_similarity("क्या आप", "क्या आप बता सकते हैं?"), 2.0 / 5.0);
    }

    #[test]
    fn test_exact_duplicate_checks() {
        assert!(is_exact_duplicate(0.99, 0.5));
        assert!(is_exact_duplicate(0.5, 1.0));
        assert!(!is_exact_duplicate(0.97, 0.9));
    }
}
