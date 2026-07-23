use std::collections::HashSet;

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

/// Evaluates Phase 1 O(1) exact merge condition (v5 §5.3 Phase 1).
/// Returns true if cosine similarity is >= 0.9999 or Jaccard similarity is 1.0.
pub fn is_exact_duplicate(cosine_sim: f32, jaccard_sim: f32) -> bool {
    cosine_sim >= 0.9999 || jaccard_sim >= 1.0
}

/// Evaluates Phase 1 Cosine Hard Match check (v5 §5.3 Step 1C).
pub fn is_cosine_hard_match(cosine_sim: f32) -> bool {
    cosine_sim > 0.999
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
        assert!(is_exact_duplicate(0.9999, 0.5));
        assert!(is_exact_duplicate(0.5, 1.0));
        assert!(!is_exact_duplicate(0.98, 0.9));
    }
}
