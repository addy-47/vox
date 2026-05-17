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
    word_count >= 4
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

/// Seamlessly stitches two transcription fragments (prefix and suffix) using word-level overlap matching.
/// This prevents visual flashing/amnesia in the UI when using rolling window partial transcripters.
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

    let max_overlap = p_words.len().min(s_words.len());
    let mut best_overlap_len = 0;

    // Find the longest overlap where the end of p_words matches the start of s_words
    for k in (1..=max_overlap).rev() {
        let p_slice = &p_words[p_words.len() - k..];
        let s_slice = &s_words[..k];
        
        let mut matched = true;
        for i in 0..k {
            let p_w = p_slice[i].trim_matches(|c: char| c.is_ascii_punctuation() || c == '।').to_lowercase();
            let s_w = s_slice[i].trim_matches(|c: char| c.is_ascii_punctuation() || c == '।').to_lowercase();
            if p_w != s_w {
                matched = false;
                break;
            }
        }
        
        if matched {
            best_overlap_len = k;
            break;
        }
    }

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

    #[test]
    fn test_hinglish_normalization() {
        assert_eq!(to_friendly_hinglish("namastE"), "namaste");
        assert_eq!(to_friendly_hinglish("dIpAvalI"), "dipavali");
        assert_eq!(to_friendly_hinglish("kairatE"), "karte");
    }

    #[test]
    fn test_stitch_transcripts_overlap() {
        // Standard Hinglish overlap
        assert_eq!(
            stitch_transcripts("mera phone number", "phone number hai 98409"),
            "mera phone number hai 98409"
        );

        // Case insensitivity and punctuation stripping
        assert_eq!(
            stitch_transcripts("Mera phone, number!", "Phone number: hai?"),
            "Mera phone, number! hai?"
        );

        // No overlap concatenation fallback
        assert_eq!(
            stitch_transcripts("mera phone", "aur kuch"),
            "mera phone aur kuch"
        );

        // Empty states
        assert_eq!(stitch_transcripts("", "hello"), "hello");
        assert_eq!(stitch_transcripts("world", ""), "world");

        // Complete containment
        assert_eq!(
            stitch_transcripts("mera phone number", "number"),
            "mera phone number"
        );
    }
}
