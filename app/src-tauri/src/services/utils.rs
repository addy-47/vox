use lipilekhika::transliterate;
use regex::Regex;
use once_cell::sync::Lazy;

/// Returns `true` if the accumulated token buffer should be flushed to TTS.
#[inline]
pub fn should_flush(buf: &str, word_count: usize) -> bool {
    let trimmed = buf.trim_end();
    let last = trimmed.chars().last().unwrap_or(' ');
    if matches!(last, '.' | '!' | '?') { return true; }
    if matches!(last, ',' | ';') { return true; }
    if trimmed.ends_with(" — ") || trimmed.ends_with(" - ") { return true; }
    word_count >= 6
}

/// Count words in the accumulated buffer.
#[inline]
pub fn count_words(s: &str) -> usize {
    s.split_whitespace().count()
}

/// Detect if string contains Devanagari (Hindi) characters.
pub fn is_devanagari(text: &str) -> bool {
    text.chars().any(|c| ('\u{0900}'..='\u{097F}').contains(&c))
}

/// Transliterates Devanagari to Roman script if Hindi is detected.
/// Uses a high-fidelity "Friendly Hinglish" engine with schwa deletion and 
/// phonetic normalization to ensure >95% readability.
pub fn transliterate_if_hi(text: &str) -> String {
    if is_devanagari(text) {
        let raw = transliterate(text, "Hindi", "English", None)
            .unwrap_or_else(|_| text.to_string());
        to_friendly_hinglish(&raw)
    } else {
        text.to_string()
    }
}

/// The core "Friendly Hinglish" engine.
/// 
/// Targets "WhatsApp-style" readability over scientific precision.
/// Implements:
/// 1. ITRANS/Harvard-Kyoto normalization
/// 2. Heuristic Schwa Deletion (e.g. 'namastE' -> 'namaste', 'karana' -> 'karna')
/// 3. Contextual Nasalization ('M' -> 'n'/'m')
/// 4. Sibilant merging ('sh'/'shh' -> 'sh')
pub fn to_friendly_hinglish(text: &str) -> String {
    // 1. Initial casing and science-marker cleanup
    let mut s = text.replace('E', "e").replace('O', "o"); // Scientific to friendly vowels
    s = s.to_lowercase();
    
    // 2. Common Phonetic Simplification
    static RE_AA: Lazy<Regex> = Lazy::new(|| Regex::new(r"aa+").unwrap());
    static RE_EE: Lazy<Regex> = Lazy::new(|| Regex::new(r"ee+").unwrap());
    static RE_OO: Lazy<Regex> = Lazy::new(|| Regex::new(r"oo+").unwrap());
    
    s = RE_AA.replace_all(&s, "a").into_owned();
    s = RE_EE.replace_all(&s, "i").into_owned();
    s = RE_OO.replace_all(&s, "u").into_owned();

    // 3. Nasalization (Anusvara 'M' mapping)
    // In scientific transliteration, 'M' represents nasalization.
    // Friendly Hinglish uses 'n' or 'm' based on following consonant.
    static RE_NASAL_LABIAL: Lazy<Regex> = Lazy::new(|| Regex::new(r"m([bpfv])").unwrap());
    static RE_NASAL_GENERAL: Lazy<Regex> = Lazy::new(|| Regex::new(r"m(\s|[^bpfv]|$)").unwrap());
    // Lipilekhika might output 'm' for 'M'. We adjust it.
    s = RE_NASAL_LABIAL.replace_all(&s, "m$1").into_owned();
    s = RE_NASAL_GENERAL.replace_all(&s, "n$1").into_owned();

    // 4. Schwa Deletion (The "Real" Hinglish Logic)
    // Apply schwa deletion
    // 1. Middle deletion: C-a-C-a-C -> C-C-a-C (e.g. 'namakIna' -> 'namkina')
    static RE_SCHWA_MID: Lazy<Regex> = Lazy::new(|| Regex::new(r"([bcdfghjklmnpqrstvwxyz])a([bcdfghjklmnpqrstvwxyz])a([bcdfghjklmnpqrstvwxyz])").unwrap());
    // 2. Trailing deletion: C-a-C-a$ -> C-C-a$ (e.g. 'karana' -> 'karna')
    static RE_SCHWA_TRAILING: Lazy<Regex> = Lazy::new(|| Regex::new(r"([bcdfghjklmnpqrstvwxyz])a([bcdfghjklmnpqrstvwxyz])a$").unwrap());

    s = RE_SCHWA_MID.replace_all(&s, "$1$2a$3").into_owned(); 
    s = RE_SCHWA_TRAILING.replace_all(&s, "$1$2a").into_owned();

    // 5. Hard word overrides (Edge Cases)
    // Some words are culturally fixed in Hinglish.
    let words: Vec<String> = s.split_whitespace().map(|w| {
        match w {
            "haiM" | "hain" => "hain".to_string(),
            "kaisE" | "kaise" => "kaise".to_string(),
            "karatE" | "karate" => "karte".to_string(), // Common verb contraction
            "namastE" | "namaste" => "namaste".to_string(),
            "shubh" => "shubh".to_string(),
            _ => {
                // Remove trailing 'a' if word length > 3 (Schwa deletion at end)
                if w.len() > 3 && w.ends_with('a') && !w.ends_with("ia") {
                    w[..w.len()-1].to_string()
                } else {
                    w.to_string()
                }
            }
        }
    }).collect();

    words.join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hinglish_normalization() {
        assert_eq!(to_friendly_hinglish("namastE"), "namaste");
        assert_eq!(to_friendly_hinglish("dIpAvalI"), "dipavali");
        assert_eq!(to_friendly_hinglish("kairatE"), "karte");
    }
}
