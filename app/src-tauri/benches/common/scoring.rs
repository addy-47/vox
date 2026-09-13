//! ============================================================================
//! benches/common/scoring.rs — Text Normalization & Levenshtein Similarity
//! ============================================================================

/// Normalizes transcript text by lowercasing and stripping punctuation and extra whitespace.
pub fn normalize_text(text: &str) -> String {
    text.chars()
        .filter(|c| {
            !c.is_ascii_punctuation()
                && !matches!(
                    c,
                    '!' | '?' | '.' | ',' | '।' | '-' | ':' | ';' | '"' | '\'' | '`' | '“' | '”'
                )
        })
        .collect::<String>()
        .to_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Computes normalized Levenshtein similarity [0.0, 1.0] between hypothesis and reference.
pub fn levenshtein_similarity(hyp: &str, ref_str: &str) -> f64 {
    let norm_hyp = normalize_text(hyp);
    let norm_ref = normalize_text(ref_str);

    let hyp_chars: Vec<char> = norm_hyp.chars().collect();
    let ref_chars: Vec<char> = norm_ref.chars().collect();

    let len1 = hyp_chars.len();
    let len2 = ref_chars.len();

    if len1 == 0 && len2 == 0 {
        return 1.0;
    }
    if len1 == 0 || len2 == 0 {
        return 0.0;
    }

    let mut dp = vec![vec![0usize; len2 + 1]; len1 + 1];
    for (i, row) in dp.iter_mut().enumerate().take(len1 + 1) {
        row[0] = i;
    }
    if let Some(first_row) = dp.first_mut() {
        for (j, cell) in first_row.iter_mut().enumerate().take(len2 + 1) {
            *cell = j;
        }
    }

    for i in 1..=len1 {
        for j in 1..=len2 {
            let cost = if hyp_chars[i - 1] == ref_chars[j - 1] {
                0
            } else {
                1
            };
            dp[i][j] = (dp[i - 1][j] + 1)
                .min(dp[i][j - 1] + 1)
                .min(dp[i - 1][j - 1] + cost);
        }
    }

    let distance = dp[len1][len2];
    let max_len = len1.max(len2);
    1.0 - (distance as f64 / max_len as f64)
}

/// Returns the labelled ground-truth transcript for a golden test clip.
/// Mirrors `tests/common/mod.rs` ground truths so the bench verifies STT output
/// against the same fixtures the integration suite gates at ≥ 0.90 similarity.
/// Kept in `benches/common/` per §8.3 decoupling (no cross-dependency on tests/).
pub fn ground_truth_for_clip(filename: &str) -> Option<&'static str> {
    Some(match filename {
        "clip_01_en_briefing.wav" => "Hey Vox, good morning! Can you check my calendar and give me a quick briefing on today's scheduled meetings?",
        "clip_02_en_weather.wav" => "Vox, what's the weather like outside right now? Is it going to rain later this afternoon?",
        "clip_03_en_code.wav" => "Can you help me refactor this Rust async function to reduce mutex contention across our background threads?",
        "clip_04_en_summary.wav" => "Hey Vox, summarize the key action items from my design review notes and draft a quick email to the team.",
        "clip_05_en_timer.wav" => "Set a timer for twenty-five minutes for a focused Pomodoro session, and minimize background notifications.",
        "clip_06_hi_greeting.wav" => "हे वॉक्स, नमस्ते! क्या आप मेरा आज का शेड्यूल देखकर बता सकते हैं कि मेरी अगली मीटिंग कब है?",
        "clip_07_hi_weather.wav" => "वॉक्स, आज बाहर का मौसम कैसा है? क्या शाम को बारिश होने की कोई संभावना है?",
        "clip_08_hi_reminder.wav" => "मेरे लिए एक ज़रूरी रिमाइंडर सेट कर दो, शाम को पाँच बजे टीम के साथ प्रोजेक्ट रिव्यू करना है।",
        "clip_09_hi_system_cmd.wav" => "वॉक्स, टर्मिनल खोलिए और हाई परफॉरमेंस मोड ऑन करके लोकल सर्वर शुरू कर दीजिए।",
        "clip_10_hi_qa.wav" => "वॉक्स, मुझे समझाइए कि मशीन लर्निंग में स्पीच-टू-टेक्स्ट मॉडल इतनी तेज़ी से आवाज़ कैसे पहचानते हैं?",
        // Supertonic single-utterance renders share the briefing/weather sentences.
        "supertonic_01_en_briefing.wav" => "Hey Vox, good morning! Can you check my calendar and give me a quick briefing on today's scheduled meetings?",
        "supertonic_07_hi_weather.wav" => "वॉक्स, आज बाहर का मौसम कैसा है? क्या शाम को बारिश होने की कोई संभावना है?",
        _ => return None,
    })
}

/// Language tag for a golden clip (`HI` when the filename carries `_hi_`).
pub fn lang_for_clip(filename: &str) -> &'static str {
    if filename.contains("_hi_") {
        "HI"
    } else {
        "EN"
    }
}
