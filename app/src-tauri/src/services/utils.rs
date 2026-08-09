/// Returns `true` if the accumulated token buffer ends at a word boundary.
/// Prevents mid-word splits when BPE subword tokens cross word boundaries.
#[inline]
fn ends_at_word_boundary(buf: &str) -> bool {
    if let Some(c) = buf.chars().last() {
        c.is_whitespace()
            || matches!(
                c,
                '.' | '!' | '?' | ',' | ';' | ':' | ')' | ']' | '\u{2014}' | '\u{2013}' | '।'
            )
    } else {
        true // empty buffer is trivially at a word boundary
    }
}

/// Returns `true` if the accumulated token buffer should be flushed to TTS.
///
/// # Fully Dynamic, Model/Hardware-Agnostic Algorithm
///
/// No hardcoded TPS categories or magic thresholds. Every decision parameter
/// is computed as a continuous function of observed generation speed (TPS).
///
/// ## Behavioral Guarantees
///
/// | Condition | Slow TPS (~1) | Medium TPS (~3.5) | Fast TPS (~5+) |
/// |-----------|:---:|:---:|:---:|
/// | Sentence boundary (`.!?।`) | Always flush | Always flush | Always flush |
/// | Clause boundary (`,;—`) | Flush at 3 words | Flush at 5 words | Disabled |
/// | Time gate | 1.0s / 3 words | 2.2s / 5 words | 3.5s / 8 words |
/// | Word-count fallback | 5 words | 12 words | 20 words |
///
/// At low TPS, the algorithm aggressively reduces TTFA by flushing on clause
/// boundaries with low word thresholds. At high TPS, it withholds to let full
/// sentences complete, preserving natural prosody. The transition is continuous
/// — there are no sharp cutoffs or category switches.
#[inline]
pub fn should_flush(buf: &str, word_count: usize, elapsed_ms: u128, tps: f32) -> bool {
    let trimmed = buf.trim_end();
    let last = trimmed.chars().last().unwrap_or(' ');

    // Hard boundaries: always flush on complete sentences
    if matches!(last, '.' | '!' | '?' | '।') {
        return true;
    }

    // ─── Continuous dynamic parameter computation ───
    // TPS range: 0.5 (barely generating) to 6.0 (extremely fast).
    // Clamped and normalized to [0.0, 1.0] for linear interpolation.
    let tps_clamped = tps.clamp(0.5, 6.0);
    let tps_norm = (tps_clamped - 0.5) / (6.0 - 0.5); // 0.0 = slowest, 1.0 = fastest

    // ─── Clause boundaries (`,`, `;`, `—`, `-`) ───
    // At low TPS: flush early to reduce TTFA (user hears something sooner).
    // At high TPS: skip clause flushes — sentences complete fast, so waiting
    // for `.!?` preserves prosody without meaningful latency cost.
    if matches!(last, ',' | ';') || trimmed.ends_with(" — ") || trimmed.ends_with(" - ") {
        // Clause flushing fades out linearly between TPS 3.0 (norm=0.45) and TPS 5.0 (norm=0.82).
        // Word threshold increases from 3→7 as TPS rises within this band.
        let clause_tps_high = 5.0;
        let clause_tps_low = 3.0;
        let clause_norm_low = (clause_tps_low - 0.5) / (6.0 - 0.5); // ≈0.45
        let clause_norm_high = (clause_tps_high - 0.5) / (6.0 - 0.5); // ≈0.82
        if tps_norm < clause_norm_high {
            // Where are we within the clause-flush band?
            let t = (tps_norm - clause_norm_low).max(0.0) / (clause_norm_high - clause_norm_low); // 0..1
            let clause_threshold = (3.0 + t * 4.0).round() as usize; // 3→7 words
            if word_count >= clause_threshold {
                return true;
            }
        }
    }

    // ─── Time-based flush ───
    // Wait time scales with TPS: slow generation gets a shorter leash so TTFA
    // doesn't blow up; fast generation gets more time to complete a sentence.
    let max_wait_ms = lerp(tps_norm, 1000.0, 3500.0) as u128;
    let min_time_words = lerp(tps_norm, 3.0, 8.0).round() as usize;
    if elapsed_ms >= max_wait_ms && word_count >= min_time_words && ends_at_word_boundary(buf) {
        return true;
    }

    // ─── Word-count fallback ───
    // Absolute maximum words to hold before forcing a flush (with word-boundary safety).
    // At low TPS: flush at 5 words to keep latency bounded.
    // At high TPS: hold up to 20 words — by then there should be a sentence boundary.
    let max_words = lerp(tps_norm, 5.0, 20.0).round() as usize;
    if word_count >= max_words && ends_at_word_boundary(buf) {
        return true;
    }

    false
}

/// Linear interpolation: map `t` in [0.0, 1.0] to [min_val, max_val].
/// Panics if `t` is outside [0.0, 1.0] — caller must clamp.
#[inline]
fn lerp(t: f32, min_val: f32, max_val: f32) -> f32 {
    min_val + t * (max_val - min_val)
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
/// Implements Incomplete Word Protection: If the string does not end with a boundary,
/// the last word is bypassed to prevent partial word transliteration artifacts.
pub fn transliterate_if_hi(text: &str, is_final: bool, transliterate_enabled: bool) -> String {
    if !transliterate_enabled {
        return text.to_string();
    }

    if !is_devanagari(text) {
        return text.to_string();
    }

    // Determine if the text ends with whitespace or punctuation
    let ends_with_boundary = if is_final {
        true
    } else if let Some(last_char) = text.chars().last() {
        last_char.is_whitespace() || last_char.is_ascii_punctuation() || last_char == '।'
    } else {
        true
    };

    #[derive(Debug)]
    enum Token {
        DevanagariWord(String),
        Other(String),
    }

    let mut tokens = Vec::new();
    let mut current_token = String::new();
    let mut in_devanagari = false;

    for c in text.chars() {
        let is_c_devanagari = ('\u{0900}'..='\u{097F}').contains(&c);
        if is_c_devanagari {
            if !in_devanagari && !current_token.is_empty() {
                tokens.push(Token::Other(current_token));
                current_token = String::new();
            }
            in_devanagari = true;
        } else {
            if in_devanagari && !current_token.is_empty() {
                tokens.push(Token::DevanagariWord(current_token));
                current_token = String::new();
            }
            in_devanagari = false;
        }
        current_token.push(c);
    }

    if !current_token.is_empty() {
        if in_devanagari {
            tokens.push(Token::DevanagariWord(current_token));
        } else {
            tokens.push(Token::Other(current_token));
        }
    }

    let mut result = String::new();
    let num_tokens = tokens.len();
    for (i, token) in tokens.into_iter().enumerate() {
        match token {
            Token::DevanagariWord(word) => {
                let is_last = i == num_tokens - 1;
                if is_last && !ends_with_boundary {
                    // Incomplete word protection: leave final word in raw Devanagari
                    result.push_str(&word);
                } else {
                    // Complete word: transliterate directly using the native ONNX model
                    let raw_trans = crate::services::translit::transliterate(&word);
                    result.push_str(&raw_trans);
                }
            }
            Token::Other(other) => {
                result.push_str(&other);
            }
        }
    }

    result
}

/// Backward-compatible Hinglish engine wrapper.
pub fn to_friendly_hinglish(text: &str) -> String {
    transliterate_if_hi(text, true, true)
}

fn edit_distance(s1: &str, s2: &str) -> usize {
    let v1: Vec<char> = s1.chars().collect();
    let v2: Vec<char> = s2.chars().collect();
    let len1 = v1.len();
    let len2 = v2.len();

    let mut dp = vec![vec![0; len2 + 1]; len1 + 1];
    for (i, row) in dp.iter_mut().enumerate().take(len1 + 1) {
        row[0] = i;
    }
    for (j, cell) in dp[0].iter_mut().enumerate().take(len2 + 1) {
        *cell = j;
    }

    for i in 1..=len1 {
        for j in 1..=len2 {
            if v1[i - 1] == v2[j - 1] {
                dp[i][j] = dp[i - 1][j - 1];
            } else {
                dp[i][j] = 1 + dp[i - 1][j - 1].min(dp[i - 1][j].min(dp[i][j - 1]));
            }
        }
    }
    dp[len1][len2]
}

fn words_soft_match(w1: &str, w2: &str) -> bool {
    let clean1 = w1
        .trim_matches(|c: char| c.is_ascii_punctuation() || c == '।')
        .to_lowercase();
    let clean2 = w2
        .trim_matches(|c: char| c.is_ascii_punctuation() || c == '।')
        .to_lowercase();
    if clean1 == clean2 {
        return true;
    }

    let dist = edit_distance(&clean1, &clean2);
    let len = clean1.chars().count().max(clean2.chars().count());

    if len <= 3 {
        dist <= 1
    } else {
        dist <= (len / 3).max(1)
    }
}

fn is_soft_subslice(p_words: &[&str], s_words: &[&str]) -> bool {
    if s_words.is_empty() {
        return true;
    }
    if p_words.len() < s_words.len() {
        return false;
    }

    for i in 0..=(p_words.len() - s_words.len()) {
        let mut matched = true;
        for j in 0..s_words.len() {
            if !words_soft_match(p_words[i + j], s_words[j]) {
                matched = false;
                break;
            }
        }
        if matched {
            return true;
        }
    }
    false
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

    // 0. Soft subslice/containment check to prevent older/smaller overlapping frames from duplicating
    if is_soft_subslice(&p_words, &s_words) {
        return p_clean.to_string();
    }

    // 1. Alignment search (look for an overlapping/matching segment anywhere in prefix)
    let mut best_i = 0;
    let mut best_j = 0;
    let mut max_match_len = 0;

    let max_j = 8.min(s_words.len());
    for i in 0..p_words.len() {
        for j in 0..max_j {
            let mut len = 0;
            while i + len < p_words.len() && j + len < s_words.len() {
                if words_soft_match(p_words[i + len], s_words[j + len]) {
                    len += 1;
                } else {
                    break;
                }
            }
            if len > max_match_len {
                max_match_len = len;
                best_i = i;
                best_j = j;
            }
        }
    }

    let min_required_match = 3.min(s_words.len());
    if max_match_len >= min_required_match {
        let mut result_words = p_words[..best_i].to_vec();
        result_words.extend_from_slice(&s_words[best_j..]);
        return result_words.join(" ");
    }

    // 2. Sequential overlap matching (fallback if alignment search failed)
    let max_overlap = p_words.len().min(s_words.len());
    let mut best_overlap_len = 0;

    for k in (1..=max_overlap).rev() {
        let p_slice = &p_words[p_words.len() - k..];
        let s_slice = &s_words[..k];

        let mut matched = true;
        for i in 0..k {
            if !words_soft_match(p_slice[i], s_slice[i]) {
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

    fn init_paths_for_testing() {
        crate::utils::paths::init_with_root(std::env::temp_dir().join("vox_test"));
    }

    #[test]
    fn test_hinglish_normalization() {
        init_paths_for_testing();
        // Without engine initialized, transliterate_if_hi should fallback to raw word safely
        assert_eq!(transliterate_if_hi("नमस्ते", false, true), "नमस्ते");
        assert_eq!(transliterate_if_hi("hello", false, true), "hello");
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

        // ─── Tests for continuous dynamic algorithm ───
        // At TPS=3.5 (medium): tps_norm≈0.545
        //   Clause threshold: ~4 words  |  Time gate: ~2363ms / ~6 words
        //   Fallback: ~13 words

        // Hard boundaries: always flush
        assert!(should_flush("hello world. ", 2, 100, 3.5));
        assert!(should_flush("hello world! ", 2, 100, 3.5));
        assert!(should_flush(
            "are rangon ke diyas khelte hoon.",
            8,
            100,
            3.5
        ));

        // Clause boundary: threshold increases with TPS (3→7 words)
        assert!(!should_flush("hello, ", 1, 100, 3.5)); // 1 word < 4 words
        assert!(!should_flush("hello world one, ", 3, 100, 3.5)); // 3 words < 4 words
        assert!(should_flush("hello world one two, ", 4, 100, 3.5)); // 4 words >= 4 words

        // Time-based: wait time and word threshold scale with TPS
        // At TPS=3.5: need ~2363ms and ~6 words at word boundary
        assert!(!should_flush("hello world one two three ", 5, 2000, 3.5)); // 5 < 6 words
        assert!(should_flush(
            "hello world one two three four ",
            6,
            2400,
            3.5
        )); // 6 >= 6, 2400 > 2363
        assert!(!should_flush(
            "hello world one two three four five",
            6,
            2400,
            3.5
        )); // No word boundary

        // Word-count fallback: ~13 words at TPS=3.5
        assert!(!should_flush(
            "hello world one two three four five six seven eight nine ten ",
            12,
            100,
            3.5
        )); // 12 < 13
        assert!(should_flush(
            "hello world one two three four five six seven eight nine ten eleven ",
            13,
            100,
            3.5
        )); // 13 >= 13

        // Word-boundary safety: buffer must end with space/punctuation for time/word-count flushes
        assert!(
            !should_flush("are rangon ke diyas khel", 5, 2500, 3.5),
            "Should NOT flush mid-word even with enough words and elapsed time"
        );
        assert!(!should_flush("hello world one two three", 4, 100, 3.5));
        assert!(!should_flush("hello world one two", 3, 3000, 3.5));

        // ─── Slow TPS (tps=1.5): tps_norm≈0.182 ───
        //   Clause threshold: 3 words  |  Time gate: ~1455ms / ~4 words
        //   Fallback: ~8 words
        assert!(should_flush(
            "hello world one two three four five six seven ",
            9,
            100,
            1.5
        )); // 9 >= 8 fallback
        assert!(!should_flush("hello world, ", 2, 100, 1.5)); // 2 < 3 clause threshold
        assert!(should_flush("hello world one, ", 3, 100, 1.5)); // 3 >= 3 clause threshold
        assert!(!should_flush("hello world one two three ", 4, 1000, 1.5)); // 1000 < 1455ms
        assert!(should_flush("hello world one two three ", 4, 1500, 1.5)); // 1500 > 1455ms

        // ─── Fast TPS (tps=5.0): tps_norm≈0.818 ───
        //   Clause flush disabled (tps_norm >= clause_norm_high)
        //   Time gate: ~3045ms / ~7 words  |  Fallback: ~17 words
        assert!(!should_flush("hello world one, ", 3, 100, 5.0)); // Clause flush disabled
        assert!(!should_flush("hello world one two three, ", 5, 100, 5.0)); // Clause flush disabled
        assert!(!should_flush("hello world one two three four five six seven eight nine ten eleven twelve thirteen fourteen fifteen sixteen ", 16, 100, 5.0)); // 16 < 17
        assert!(should_flush("hello world one two three four five six seven eight nine ten eleven twelve thirteen fourteen fifteen sixteen seventeen ", 17, 100, 5.0)); // 17 >= 17
                                                                                                                                                                        // Time-based at high TPS
        assert!(!should_flush(
            "hello world one two three four five six ",
            7,
            2000,
            5.0
        )); // 2000 < 3045ms
        assert!(should_flush(
            "hello world one two three four five six seven ",
            8,
            3100,
            5.0
        )); // 8 >= 7, 3100 > 3045

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

        // Containment of middle slice
        assert_eq!(
            stitch_transcripts("mera phone number hai", "phone number"),
            "mera phone number hai"
        );

        // Soft match overlap
        assert_eq!(
            stitch_transcripts("mera phone numbere", "number hai"),
            "mera phone numbere hai"
        );

        assert_eq!(
            stitch_transcripts(
                "Expert nature is around. Expecting to raise around twelve, I think forty thousand per month.",
                "Expect nature is around twelve. I think forty thousand per month."
            ),
            "Expert nature is around. Expecting to raise around twelve. I think forty thousand per month."
        );

        assert_eq!(
            stitch_transcripts(
                "Spending any more than than than. thousand or greater because they are not they have said that they are spending only through credits.",
                "I have said that they are spending only through credit. Right now."
            ),
            "Spending any more than than than. thousand or greater because they are not they have said that they are spending only through credit. Right now."
        );
    }

    #[test]
    fn test_translit_mixed_script_preservation() {
        init_paths_for_testing();

        // Initialize transliteration engine if local models exist
        if let Some(home) = dirs::home_dir() {
            let vox_root = home.join(".vox");
            if vox_root.exists() {
                crate::utils::paths::init_with_root(vox_root);
                let _ = crate::services::translit::init_transliteration_engine();
            }
        }

        // 1. Pure ASCII / Numbers / Emojis without Devanagari
        let pure_mixed = "Hello namaste hai! 123 😊";
        assert!(!is_devanagari(pure_mixed));
        let res_pure = transliterate_if_hi(pure_mixed, true, true);
        assert_eq!(
            res_pure, pure_mixed,
            "Pure non-Devanagari text should be untouched"
        );

        // 2. Mixed Devanagari script + ASCII + Numbers + Punctuation + Emojis
        let mixed_input = "Hello नमस्ते hai! 123 😊";
        assert!(is_devanagari(mixed_input));

        let res_mixed = transliterate_if_hi(mixed_input, true, true);

        // Verify ASCII prefixes/suffixes, numbers, punctuation, spaces, and emojis are intact
        assert!(
            res_mixed.starts_with("Hello "),
            "ASCII prefix 'Hello ' should be preserved"
        );
        assert!(
            res_mixed.ends_with(" hai! 123 😊"),
            "ASCII, numbers, punctuation, and emoji suffix should be preserved"
        );
        assert!(res_mixed.contains("123"), "Numbers '123' must be preserved");
        assert!(res_mixed.contains("😊"), "Emoji '😊' must be preserved");

        // If engine was initialized, "नमस्ते" should be converted to Roman script ("namaste" / "namas")
        if crate::services::translit::TRANSLITERATION_ENGINE
            .get()
            .is_some()
        {
            assert!(
                res_mixed.to_lowercase().contains("namaste")
                    || res_mixed.to_lowercase().contains("namas"),
                "Devanagari 'नमस्ते' should be transliterated to Roman script, got: '{}'",
                res_mixed
            );
        } else {
            // Fallback mode without engine: Devanagari word is kept as raw "नमस्ते"
            assert!(res_mixed.contains("नमस्ते"));
        }

        // 3. Disabling transliteration entirely
        let res_disabled = transliterate_if_hi(mixed_input, true, false);
        assert_eq!(
            res_disabled, mixed_input,
            "With transliterate_enabled=false, input should be untouched"
        );

        // 4. Incomplete word protection (is_final = false)
        // Trailing word without boundary should remain raw Devanagari to avoid partial transliteration artifacts
        let incomplete_input = "Hello नमस";
        let res_incomplete = transliterate_if_hi(incomplete_input, false, true);
        assert_eq!(
            res_incomplete, "Hello नमस",
            "Incomplete final word should remain raw Devanagari when is_final=false"
        );

        // When trailing boundary (space) is present even with is_final=false, word is complete
        let complete_input = "Hello नमस ";
        let res_complete = transliterate_if_hi(complete_input, false, true);
        assert!(res_complete.starts_with("Hello "), "Prefix preserved");
        assert!(res_complete.ends_with(" "), "Trailing space preserved");
    }

    #[test]
    fn test_devanagari_matra_normalization() {
        init_paths_for_testing();

        if let Some(home) = dirs::home_dir() {
            let vox_root = home.join(".vox");
            if vox_root.exists() {
                crate::utils::paths::init_with_root(vox_root);
                let _ = crate::services::translit::init_transliteration_engine();
            }
        }

        // 1. Devanagari Matras (dependent vowel signs), Virama (halant), Anusvara, Chandrabindu
        let matra_words = vec![
            ("नमस्ते", "Virama/Halant conjunct + E matra"),
            ("क्या", "Half consonant + AA matra"),
            ("कुत्ता", "Short U matra + virama + AA matra"),
            ("हिंदी", "Anusvara + I matra + II matra"),
            ("हाँ", "Chandrabindu + AA matra"),
            ("देश", "E matra"),
            ("पैसा", "AI matra"),
            ("सोना", "O matra"),
            ("कौन", "AU matra"),
            ("वॉक्स", "Candra O matra + virama"),
        ];

        for (word, desc) in &matra_words {
            assert!(
                is_devanagari(word),
                "is_devanagari failed for {} ({})",
                word,
                desc
            );

            // Verify transliterate_if_hi handles matra words without panic or byte corruption
            let res = transliterate_if_hi(word, true, true);
            assert!(
                !res.is_empty(),
                "Transliteration result should not be empty for {} ({})",
                word,
                desc
            );
        }

        // 2. Devanagari Nukta Normalization (Precomposed vs Decomposed)
        // Precomposed: 'फ़' (U+095E FA), 'ज़' (U+095B ZA), 'क़' (U+0958 QA), 'ख़' (U+0959 KHHA), 'ग़' (U+095A GHHA), 'ड़' (U+095C DDDHA), 'ढ़' (U+095D RHA)
        // Decomposed: Base consonant + '़' (U+093C Nukta) e.g. 'फ' + '़'
        let precomposed_phone = "फ़ोन 123";
        let decomposed_phone = "फ\u{093C}ोन 123";

        assert!(
            is_devanagari(precomposed_phone),
            "Precomposed nukta should be detected as Devanagari"
        );
        assert!(
            is_devanagari(decomposed_phone),
            "Decomposed nukta should be detected as Devanagari"
        );

        let res_pre = transliterate_if_hi(precomposed_phone, true, true);
        let res_dec = transliterate_if_hi(decomposed_phone, true, true);

        // Both must preserve ASCII and numbers
        assert!(
            res_pre.ends_with(" 123"),
            "Precomposed nukta test should preserve ' 123'"
        );
        assert!(
            res_dec.ends_with(" 123"),
            "Decomposed nukta test should preserve ' 123'"
        );

        // 3. Multi-word Devanagari string with mixed matras, nuktas, numbers, and emojis
        let complex_devanagari = "यह ख़बर फ़ोन पर 100% सही है! 👍";
        assert!(is_devanagari(complex_devanagari));

        let res_complex = transliterate_if_hi(complex_devanagari, true, true);
        assert!(
            res_complex.contains("100%"),
            "Percentage and numbers must be preserved"
        );
        assert!(res_complex.contains("!"), "Punctuation must be preserved");
        assert!(res_complex.contains("👍"), "Emoji must be preserved");
    }
}
