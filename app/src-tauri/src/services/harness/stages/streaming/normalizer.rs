use std::sync::LazyLock;

use num2words::{Currency, Num2Words};
use regex::Regex;

static RE_TAGS: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"</?[a-zA-Z][^>]*>").expect("valid regex"));

static RE_MARKDOWN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?:[*_~`]{1,3}|#{1,6}\s+|^\s*[-*•]\s+|^\s*\d+[\).]\s*)").expect("valid regex")
});

static RE_CURRENCY: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"([$€£])\s*([0-9]+(?:\.[0-9]{1,2})?)").expect("valid regex"));

static RE_TIME: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\b([0-1]?[0-9]|2[0-3]):([0-5][0-9])\b").expect("valid regex"));

static RE_MEASURE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\b([0-9]+(?:\.[0-9]+)?)\s*(?:(GB|MB|KB|TB|ms|sec|fps|km/h|mph|km|cm|mm|kg|GHz|MHz|kHz|Hz)\b|(%))").expect("valid regex")
});

static RE_VERSION: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\bv([0-9]+)\.([0-9]+)(?:\.([0-9]+))?\b").expect("valid regex"));

static RE_ORDINAL: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\b([0-9]+)(?:st|nd|rd|th)\b").expect("valid regex"));

static RE_NUMBER: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\b([0-9]{1,3}(?:,[0-9]{3})+|[0-9]+(?:\.[0-9]+)?)\b").expect("valid regex")
});

static RE_ACRONYM: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\b(API|CLI|CPU|GPU|UI|OS|RAM|SSD|URL|HTML|CSS|JSON|SDK)\b").expect("valid regex")
});

static RE_WHITESPACE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\s+").expect("valid regex"));

/// Contextual text normalizer and speech sanitizer for TTS preparation.
#[derive(Debug, Clone, Copy, Default)]
pub struct TextNormalizer;

impl TextNormalizer {
    /// Normalizes raw assistant text for high-fidelity speech synthesis.
    pub fn normalize_for_speech(text: &str) -> String {
        let without_tags = RE_TAGS.replace_all(text, " ");
        let without_md = RE_MARKDOWN.replace_all(&without_tags, " ");
        let with_currencies = Self::normalize_currencies(&without_md);
        let with_times = Self::normalize_times(&with_currencies);
        let with_measures = Self::normalize_measures(&with_times);
        let with_versions = Self::normalize_versions(&with_measures);
        let with_ordinals = Self::normalize_ordinals(&with_versions);
        let with_numbers = Self::normalize_numbers(&with_ordinals);
        let with_acronyms = Self::normalize_acronyms(&with_numbers);
        RE_WHITESPACE
            .replace_all(&with_acronyms, " ")
            .trim()
            .to_string()
    }

    /// Normalizes currency symbols and amounts into spoken words.
    fn normalize_currencies(text: &str) -> String {
        RE_CURRENCY
            .replace_all(text, |caps: &regex::Captures| {
                let symbol = caps.get(1).map_or("$", |m| m.as_str());
                let amount_str = caps.get(2).map_or("0", |m| m.as_str());
                let currency = match symbol {
                    "€" => Currency::EUR,
                    "£" => Currency::GBP,
                    _ => Currency::DOLLAR,
                };
                if let Ok(val) = amount_str.parse::<f64>() {
                    Num2Words::new(val)
                        .currency(currency)
                        .to_words()
                        .unwrap_or_else(|_| amount_str.to_string())
                } else {
                    amount_str.to_string()
                }
            })
            .into_owned()
    }

    /// Normalizes digital times into conversational speech.
    fn normalize_times(text: &str) -> String {
        RE_TIME
            .replace_all(text, |caps: &regex::Captures| {
                let hour: u32 = caps
                    .get(1)
                    .and_then(|m| m.as_str().parse().ok())
                    .unwrap_or(0);
                let minute: u32 = caps
                    .get(2)
                    .and_then(|m| m.as_str().parse().ok())
                    .unwrap_or(0);
                let hour_words = Num2Words::new(hour as i64)
                    .cardinal()
                    .to_words()
                    .unwrap_or_default();
                if minute == 0 {
                    format!("{} o'clock", hour_words)
                } else if minute < 10 {
                    let min_words = Num2Words::new(minute as i64)
                        .cardinal()
                        .to_words()
                        .unwrap_or_default();
                    format!("{} oh {}", hour_words, min_words)
                } else {
                    let min_words = Num2Words::new(minute as i64)
                        .cardinal()
                        .to_words()
                        .unwrap_or_default();
                    format!("{} {}", hour_words, min_words)
                }
            })
            .into_owned()
    }

    /// Normalizes quantities paired with measurement units into words.
    fn normalize_measures(text: &str) -> String {
        RE_MEASURE
            .replace_all(text, |caps: &regex::Captures| {
                let num_str = caps.get(1).map_or("0", |m| m.as_str());
                let unit_str = caps
                    .get(2)
                    .or_else(|| caps.get(3))
                    .map_or("", |m| m.as_str());
                let spoken_num = Self::verbalize_number_string(num_str);
                let spoken_unit = match unit_str {
                    "GB" => "gigabytes",
                    "MB" => "megabytes",
                    "KB" => "kilobytes",
                    "TB" => "terabytes",
                    "ms" => "milliseconds",
                    "sec" => "seconds",
                    "fps" => "frames per second",
                    "km/h" => "kilometers per hour",
                    "mph" => "miles per hour",
                    "km" => "kilometers",
                    "cm" => "centimeters",
                    "mm" => "millimeters",
                    "kg" => "kilograms",
                    "GHz" => "gigahertz",
                    "MHz" => "megahertz",
                    "kHz" => "kilohertz",
                    "Hz" => "hertz",
                    "%" => "percent",
                    other => other,
                };
                format!("{} {}", spoken_num, spoken_unit)
            })
            .into_owned()
    }

    /// Normalizes version identifiers into spaced words.
    fn normalize_versions(text: &str) -> String {
        RE_VERSION
            .replace_all(text, |caps: &regex::Captures| {
                let major = caps.get(1).map_or("0", |m| m.as_str());
                let minor = caps.get(2).map_or("0", |m| m.as_str());
                let patch = caps.get(3).map(|m| m.as_str());
                let major_w = Self::verbalize_number_string(major);
                let minor_w = Self::verbalize_number_string(minor);
                if let Some(p) = patch {
                    let patch_w = Self::verbalize_number_string(p);
                    format!("version {} point {} point {}", major_w, minor_w, patch_w)
                } else {
                    format!("version {} point {}", major_w, minor_w)
                }
            })
            .into_owned()
    }

    /// Normalizes ordinal number expressions into words.
    fn normalize_ordinals(text: &str) -> String {
        RE_ORDINAL
            .replace_all(text, |caps: &regex::Captures| {
                let num_str = caps.get(1).map_or("0", |m| m.as_str());
                if let Ok(n) = num_str.parse::<i64>() {
                    Num2Words::new(n)
                        .ordinal()
                        .to_words()
                        .unwrap_or_else(|_| num_str.to_string())
                } else {
                    num_str.to_string()
                }
            })
            .into_owned()
    }

    /// Normalizes generic standalone numbers, disambiguating years from cardinals.
    fn normalize_numbers(text: &str) -> String {
        RE_NUMBER
            .replace_all(text, |caps: &regex::Captures| {
                let raw = caps.get(1).map_or("0", |m| m.as_str());
                let cleaned = raw.replace(',', "");
                if cleaned.len() == 4 && !cleaned.contains('.') {
                    if let Ok(year) = cleaned.parse::<i64>() {
                        if (1900..=2099).contains(&year) {
                            if let Ok(w) = Num2Words::new(year).year().to_words() {
                                return w;
                            }
                        }
                    }
                }
                Self::verbalize_number_string(&cleaned)
            })
            .into_owned()
    }

    /// Spaces out technical acronyms into distinct initialisms.
    fn normalize_acronyms(text: &str) -> String {
        RE_ACRONYM
            .replace_all(text, |caps: &regex::Captures| {
                let acr = caps.get(1).map_or("", |m| m.as_str());
                acr.chars()
                    .map(|c| c.to_string())
                    .collect::<Vec<_>>()
                    .join(" ")
            })
            .into_owned()
    }

    /// Helper converting numeric string to cardinal words.
    fn verbalize_number_string(num: &str) -> String {
        if let Ok(n) = num.parse::<i64>() {
            Num2Words::new(n)
                .cardinal()
                .to_words()
                .unwrap_or_else(|_| num.to_string())
        } else if let Ok(f) = num.parse::<f64>() {
            Num2Words::new(f)
                .cardinal()
                .to_words()
                .unwrap_or_else(|_| num.to_string())
        } else {
            num.to_string()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_currencies() {
        let input = "The bill came to $12.50 for lunch.";
        let result = TextNormalizer::normalize_for_speech(input);
        assert_eq!(
            result,
            "The bill came to twelve dollars and fifty cents for lunch."
        );

        let eur = "It costs €42 total.";
        let eur_res = TextNormalizer::normalize_for_speech(eur);
        assert_eq!(eur_res, "It costs forty-two euros total.");
    }

    #[test]
    fn test_normalize_times() {
        let input = "Meeting is at 12:45 today.";
        let result = TextNormalizer::normalize_for_speech(input);
        assert_eq!(result, "Meeting is at twelve forty-five today.");

        let exact = "Set alarm for 7:00.";
        let exact_res = TextNormalizer::normalize_for_speech(exact);
        assert_eq!(exact_res, "Set alarm for seven o'clock.");
    }

    #[test]
    fn test_normalize_measures() {
        let input = "I have 3.5 GB of logs taking 127 ms.";
        let result = TextNormalizer::normalize_for_speech(input);
        assert_eq!(result, "I have three point five gigabytes of logs taking one hundred twenty-seven milliseconds.");

        let perf = "Target 60 fps with 99% uptime.";
        let perf_res = TextNormalizer::normalize_for_speech(perf);
        assert_eq!(
            perf_res,
            "Target sixty frames per second with ninety-nine percent uptime."
        );
    }

    #[test]
    fn test_normalize_years_and_cardinals() {
        let input = "In 2026, I parsed 1247 files.";
        let result = TextNormalizer::normalize_for_speech(input);
        assert_eq!(
            result,
            "In twenty twenty-six, I parsed one thousand two hundred and forty-seven files."
        );

        let comma_num = "Database has 1,234,567 rows.";
        let comma_res = TextNormalizer::normalize_for_speech(comma_num);
        assert_eq!(comma_res, "Database has one million two hundred thirty-four thousand five hundred and sixty-seven rows.");
    }

    #[test]
    fn test_normalize_versions_and_ordinals() {
        let input = "Upgraded to v1.8.2 on our 3rd attempt.";
        let result = TextNormalizer::normalize_for_speech(input);
        assert_eq!(
            result,
            "Upgraded to version one point eight point two on our third attempt."
        );
    }

    #[test]
    fn test_strip_markdown_and_tags() {
        let input = "<invoke name=\"critic\"><guidelines>**Persona Check:**</guidelines></invoke><Vox>Hello world!</Vox>";
        let result = TextNormalizer::normalize_for_speech(input);
        assert_eq!(result, "Persona Check: Hello world!");
    }

    #[test]
    fn test_acronym_initialisms() {
        let input = "Check the CLI and API on Linux OS.";
        let result = TextNormalizer::normalize_for_speech(input);
        assert_eq!(result, "Check the C L I and A P I on Linux O S.");
    }
}
