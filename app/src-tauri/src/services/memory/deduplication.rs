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

/// Returns true if cosine similarity or Jaccard similarity exceeds exact-match thresholds.
pub fn is_exact_duplicate(cosine_sim: f32, jaccard_sim: f32) -> bool {
    cosine_sim >= COSINE_HARD_MATCH_THRESHOLD || jaccard_sim >= JACCARD_EXACT_MATCH_THRESHOLD
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Tests that word casing and punctuation normalization produce exact similarity for identical token sets.
    #[test]
    fn test_jaccard_similarity_normalization() {
        let s1 = "The quick, brown fox jumps!";
        let s2 = "the QUICK brown FOX jumps.";
        let sim = jaccard_similarity(s1, s2);
        assert!((sim - 1.0).abs() < 1e-5);

        let hi1 = "नमस्ते दुनिया।";
        let hi2 = "नमस्ते दुनिया";
        let sim_hi = jaccard_similarity(hi1, hi2);
        assert!((sim_hi - 1.0).abs() < 1e-5);
    }

    /// Tests that completely disjoint token sets yield zero similarity.
    #[test]
    fn test_jaccard_similarity_disjoint() {
        let s1 = "apple banana cherry";
        let s2 = "dog elephant fox";
        let sim = jaccard_similarity(s1, s2);
        assert!((sim - 0.0).abs() < 1e-5);
    }

    /// Tests edge case boundaries with empty strings preventing division by zero.
    #[test]
    fn test_jaccard_similarity_empty_boundaries() {
        assert!((jaccard_similarity("", "") - 1.0).abs() < 1e-5);
        assert!((jaccard_similarity("hello", "") - 0.0).abs() < 1e-5);
        assert!((jaccard_similarity("", "world") - 0.0).abs() < 1e-5);
    }

    /// Tests hard match decision threshold boundaries for cosine and Jaccard metrics.
    #[test]
    fn test_is_exact_duplicate_thresholds() {
        assert!(is_exact_duplicate(0.98, 0.5));
        assert!(is_exact_duplicate(0.99, 0.0));
        assert!(is_exact_duplicate(0.50, 1.0));
        assert!(!is_exact_duplicate(0.97, 0.99));
        assert!(!is_exact_duplicate(0.0, 0.0));
    }
}
