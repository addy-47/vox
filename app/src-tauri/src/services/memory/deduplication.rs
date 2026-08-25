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
