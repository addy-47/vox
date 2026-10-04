use once_cell::sync::Lazy;
use regex::Regex;

/// Regex matching isolated non-lexical acoustic hesitation markers.
// INVARIANT: Must only match unambiguous phonetic markers (um, uh, er, ah, hmm).
// NEVER match ambiguous conversational words (like, you know, so, but).
static NON_LEXICAL_FILLER_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)\b(um+|uh+|er+|ah+|hmm+)\b[,;]?\s*")
        .expect("Failed to compile non-lexical filler regex")
});

static CURRENCY_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)\b((?:(?:zero|one|two|three|four|five|six|seven|eight|nine|ten|eleven|twelve|thirteen|fourteen|fifteen|sixteen|seventeen|eighteen|nineteen|twenty|thirty|forty|fifty|sixty|seventy|eighty|ninety|hundred|thousand|million|billion|trillion|and|a)[,\s\-]+)+)dollars?(?:\s+and\s+((?:(?:zero|one|two|three|four|five|six|seven|eight|nine|ten|eleven|twelve|thirteen|fourteen|fifteen|sixteen|seventeen|eighteen|nineteen|twenty|thirty|forty|fifty|sixty|seventy|eighty|ninety)[,\s\-]*)+)\s+cents?)?\b")
        .expect("Failed to compile currency regex")
});

static UNFORMATTED_DOLLARS_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\$(\d{4,})\b").expect("Failed to compile unformatted dollars regex"));

/// Applies Tier-1 deterministic speech-to-written cleanup and Inverse Text Normalization.
pub fn apply_tier1_refinement(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return String::new();
    }

    let without_fillers = strip_non_lexical_fillers(trimmed);
    let deduplicated = collapse_verbatim_stutters(&without_fillers);
    let currency_preprocessed = normalize_currency(&deduplicated);
    let itn_result = text_processing_rs::normalize_sentence(&currency_preprocessed);
    let currency_formatted = format_unformatted_currency(&itn_result);

    // INVARIANT: Fail-open to raw transcript if normalization produces empty output on non-empty input.
    if currency_formatted.trim().is_empty() && !trimmed.is_empty() {
        log::warn!("[ITN] Normalization yielded empty string, failing open to raw transcript");
        return trimmed.to_string();
    }

    clean_residual_whitespace(&currency_formatted)
}

/// Normalizes spoken currency expressions (e.g., "one thousand eight hundred dollars" -> "$1,800").
fn normalize_currency(text: &str) -> String {
    CURRENCY_RE
        .replace_all(text, |caps: &regex::Captures| {
            let dollars_str = caps.get(1).map(|m| m.as_str().trim()).unwrap_or("");
            let cleaned_dollars = dollars_str.replace(['-', ','], " ");
            let Some(dollars_digits) =
                text_processing_rs::itn::en::cardinal::parse(&cleaned_dollars)
            else {
                return caps.get(0).map(|m| m.as_str()).unwrap_or("").to_string();
            };

            let formatted_dollars = format_number_with_commas(&dollars_digits);

            if let Some(cents_match) = caps.get(2) {
                let cents_str = cents_match.as_str().trim().replace(['-', ','], " ");
                if let Some(cents_digits) = text_processing_rs::itn::en::cardinal::parse(&cents_str)
                {
                    if let Ok(cents_num) = cents_digits.parse::<u32>() {
                        return format!("${}.{:02}", formatted_dollars, cents_num);
                    }
                }
            }

            format!("${}", formatted_dollars)
        })
        .to_string()
}

/// Inserts thousands commas into bare 4+ digit dollar amounts (e.g., "$1800" -> "$1,800").
fn format_unformatted_currency(text: &str) -> String {
    UNFORMATTED_DOLLARS_RE
        .replace_all(text, |caps: &regex::Captures| {
            let digits = caps.get(1).map(|m| m.as_str()).unwrap_or("");
            format!("${}", format_number_with_commas(digits))
        })
        .to_string()
}

/// Formats a pure digit string with standard thousands commas.
fn format_number_with_commas(digits: &str) -> String {
    if !digits.chars().all(|c| c.is_ascii_digit()) {
        return digits.to_string();
    }
    let len = digits.len();
    let mut result = String::with_capacity(len + (len.saturating_sub(1)) / 3);
    for (i, ch) in digits.chars().enumerate() {
        if i > 0 && (len - i).is_multiple_of(3) {
            result.push(',');
        }
        result.push(ch);
    }
    result
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
    let mut s = text.to_string();
    while s.contains("  ") {
        s = s.replace("  ", " ");
    }
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
