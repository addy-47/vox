//! ============================================================================
//! tests/common/scoring.rs — Scoring & Acoustic Feature Comparison Helpers
//! ============================================================================

use std::path::Path;

/// Normalizes transcript text by lowercasing and stripping punctuation and extra whitespace.
pub fn normalize_text(text: &str) -> String {
    let sanitized = text.replace("<unk>", "");
    sanitized
        .chars()
        .filter(|c| !c.is_ascii_punctuation())
        .collect::<String>()
        .to_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Computes normalized Levenshtein similarity [0.0, 1.0] between hypothesis and reference.
pub fn calculate_similarity(hyp: &str, ref_str: &str) -> f32 {
    let norm_hyp = normalize_text(hyp);
    let norm_ref = normalize_text(ref_str);

    let hyp_chars: Vec<char> = norm_hyp.chars().collect();
    let ref_chars: Vec<char> = norm_ref.chars().collect();

    let m = hyp_chars.len();
    let n = ref_chars.len();

    if m == 0 && n == 0 {
        return 1.0;
    }
    if m == 0 || n == 0 {
        return 0.0;
    }

    let mut dp = vec![vec![0usize; n + 1]; m + 1];
    for (i, row) in dp.iter_mut().enumerate().take(m + 1) {
        row[0] = i;
    }
    for (j, cell) in dp[0].iter_mut().enumerate().take(n + 1) {
        *cell = j;
    }

    for i in 1..=m {
        for j in 1..=n {
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

    let edit_dist = dp[m][n];
    let max_len = m.max(n);
    1.0 - (edit_dist as f32 / max_len as f32)
}

/// Asserts that transcript similarity meets or exceeds threshold with detailed panic message.
pub fn assert_similarity_above(hyp: &str, reference: &str, threshold: f32, label: &str) {
    let similarity = calculate_similarity(hyp, reference);
    assert!(
        similarity >= threshold,
        "[{}] Similarity {:.4} fell below required threshold {:.2}.\nReference : {}\nHypothesis : {}",
        label,
        similarity,
        threshold,
        reference,
        hyp
    );
}

/// Summary of acoustic properties extracted from a synthesized WAV file.
#[derive(Debug, Clone)]
pub struct AcousticReport {
    pub duration_sec: f32,
    pub mean_rms: f32,
    pub max_amplitude: f32,
    pub non_silent_ratio: f32,
}

/// Acceptable tolerance ranges for acoustic comparison.
#[derive(Debug, Clone)]
pub struct AcousticTolerances {
    pub duration_rel_tol: f32,
    pub mean_rms_rel_tol: f32,
    pub non_silent_ratio_abs_tol: f32,
}

impl Default for AcousticTolerances {
    fn default() -> Self {
        Self {
            duration_rel_tol: 0.30,
            mean_rms_rel_tol: 0.50,
            non_silent_ratio_abs_tol: 0.25,
        }
    }
}

/// Extracts acoustic features from a 16kHz mono WAV file.
pub fn extract_acoustic_features(path: &Path) -> Result<AcousticReport, String> {
    let audio = super::audio::decode_wav_to_mono_16k(path)?;
    if audio.is_empty() {
        return Err("Audio is empty".to_string());
    }

    let duration_sec = audio.len() as f32 / 16000.0;
    let mut sum_sq = 0.0f32;
    let mut max_amp = 0.0f32;
    let mut non_silent_samples = 0usize;
    let silence_threshold = 0.01f32;

    for &s in &audio {
        let abs = s.abs();
        if abs > max_amp {
            max_amp = abs;
        }
        if abs > silence_threshold {
            non_silent_samples += 1;
        }
        sum_sq += s * s;
    }

    let mean_rms = (sum_sq / audio.len() as f32).sqrt();
    let non_silent_ratio = non_silent_samples as f32 / audio.len() as f32;

    Ok(AcousticReport {
        duration_sec,
        mean_rms,
        max_amplitude: max_amp,
        non_silent_ratio,
    })
}

/// Asserts that generated acoustic report matches golden reference within specified tolerances.
pub fn assert_acoustic_within_tolerance(
    gen: &AcousticReport,
    golden: &AcousticReport,
    tolerances: &AcousticTolerances,
    label: &str,
) {
    let dur_diff = (gen.duration_sec - golden.duration_sec).abs() / golden.duration_sec.max(0.001);
    assert!(
        dur_diff <= tolerances.duration_rel_tol,
        "[{}] Duration delta {:.2}% exceeded tolerance {:.2}%. Gen: {:.2}s, Golden: {:.2}s",
        label,
        dur_diff * 100.0,
        tolerances.duration_rel_tol * 100.0,
        gen.duration_sec,
        golden.duration_sec
    );

    let rms_diff = (gen.mean_rms - golden.mean_rms).abs() / golden.mean_rms.max(0.0001);
    assert!(
        rms_diff <= tolerances.mean_rms_rel_tol,
        "[{}] Mean RMS delta {:.2}% exceeded tolerance {:.2}%. Gen: {:.4}, Golden: {:.4}",
        label,
        rms_diff * 100.0,
        tolerances.mean_rms_rel_tol * 100.0,
        gen.mean_rms,
        golden.mean_rms
    );

    let ratio_diff = (gen.non_silent_ratio - golden.non_silent_ratio).abs();
    assert!(
        ratio_diff <= tolerances.non_silent_ratio_abs_tol,
        "[{}] Voiced/non-silent ratio delta {:.2}% exceeded tolerance {:.2}%. Gen: {:.2}%, Golden: {:.2}%",
        label,
        ratio_diff * 100.0,
        tolerances.non_silent_ratio_abs_tol * 100.0,
        gen.non_silent_ratio * 100.0,
        golden.non_silent_ratio * 100.0
    );
}
