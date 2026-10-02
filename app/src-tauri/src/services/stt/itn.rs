use once_cell::sync::Lazy;
use regex::Regex;

/// Regex matching isolated non-lexical acoustic hesitation markers.
// INVARIANT: Must only match unambiguous phonetic markers (um, uh, er, ah, hmm).
// NEVER match ambiguous conversational words (like, you know, so, but).
static NON_LEXICAL_FILLER_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)\b(um+|uh+|er+|ah+|hmm+)\b[,;]?\s*")
        .expect("Failed to compile non-lexical filler regex")
});

/// Applies Tier-1 deterministic speech-to-written cleanup and Inverse Text Normalization.
pub fn apply_tier1_refinement(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return String::new();
    }

    let without_fillers = strip_non_lexical_fillers(trimmed);
    let deduplicated = collapse_verbatim_stutters(&without_fillers);
    let itn_result = text_processing_rs::normalize(&deduplicated);

    // INVARIANT: Fail-open to raw transcript if normalization produces empty output on non-empty input.
    if itn_result.trim().is_empty() && !trimmed.is_empty() {
        log::warn!("[ITN] Normalization yielded empty string, failing open to raw transcript");
        return trimmed.to_string();
    }

    clean_residual_whitespace(&itn_result)
}

/// Strips unambiguous phonetic hesitation markers from the text.
fn strip_non_lexical_fillers(text: &str) -> String {
    NON_LEXICAL_FILLER_RE.replace_all(text, "").to_string()
}

/// Collapses verbatim consecutive identical word tokens case-insensitively.
fn collapse_verbatim_stutters(text: &str) -> String {
    let mut words = text.split_whitespace().peekable();
    let mut result = Vec::new();

    while let Some(word) = words.next() {
        let clean_word = word.trim_matches(|c: char| c.is_ascii_punctuation());
        if let Some(&next_word) = words.peek() {
            let next_clean = next_word.trim_matches(|c: char| c.is_ascii_punctuation());
            if !clean_word.is_empty() && clean_word.eq_ignore_ascii_case(next_clean) {
                continue;
            }
        }
        result.push(word);
    }

    result.join(" ")
}

/// Normalizes spacing around punctuation introduced by token edits.
fn clean_residual_whitespace(text: &str) -> String {
    let mut s = text.replace("  ", " ");
    s = s.replace(" ,", ",");
    s = s.replace(" .", ".");
    s = s.replace(" ?", "?");
    s = s.replace(" !", "!");
    s.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_itn_numbers_and_currency() {
        assert_eq!(apply_tier1_refinement("twenty five"), "25");
        assert_eq!(
            apply_tier1_refinement("one thousand eight hundred dollars"),
            "$1,800"
        );
        assert_eq!(
            apply_tier1_refinement("Set a timer for twenty five minutes."),
            "Set a timer for 25 minutes."
        );
    }

    #[test]
    fn test_filler_removal() {
        assert_eq!(
            apply_tier1_refinement("Hey Vox, um, what's the weather like?"),
            "Hey Vox, what's the weather like?"
        );
        assert_eq!(
            apply_tier1_refinement("I want, uh, twenty five dollars."),
            "I want, $25."
        );
    }

    #[test]
    fn test_stutter_deduplication() {
        assert_eq!(
            apply_tier1_refinement("Can you help me with the the function?"),
            "Can you help me with the function?"
        );
    }
}
