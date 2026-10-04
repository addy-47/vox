//! ============================================================================
//! evals/common/metrics.rs — Percentile, similarity and stage-timing aggregation
//! ============================================================================
//! Category     : Utility Module
//! Component    : evals harness
//! Prerequisites: see evals/README.md
//! Execution    : cargo bench --bench agentic_tool_eval --release -- --help
//! Metrics      : see summary.md in the run directory
//! ============================================================================

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LatencyStats {
    pub n: usize,
    pub mean_ms: f64,
    pub p50_ms: f64,
    pub p95_ms: f64,
    pub p99_ms: f64,
    pub min_ms: f64,
    pub max_ms: f64,
    pub stddev_ms: f64,
}

impl LatencyStats {
    /// Nearest-rank percentiles: with a 24-query corpus, interpolated percentiles
    /// would report values that were never observed.
    pub fn from_samples(samples: &[f64]) -> Self {
        if samples.is_empty() {
            return Self::default();
        }
        let n = samples.len();
        let mean = samples.iter().sum::<f64>() / n as f64;
        let variance = samples.iter().map(|s| (s - mean).powi(2)).sum::<f64>() / n as f64;

        let mut sorted = samples.to_vec();
        sorted.sort_by(|a, b| a.total_cmp(b));
        let rank = |p: f64| -> f64 {
            let idx = ((p * n as f64).ceil() as usize).saturating_sub(1);
            sorted[idx.min(n - 1)]
        };

        Self {
            n,
            mean_ms: mean,
            p50_ms: rank(0.50),
            p95_ms: rank(0.95),
            p99_ms: rank(0.99),
            min_ms: sorted[0],
            max_ms: sorted[n - 1],
            stddev_ms: variance.sqrt(),
        }
    }

    pub fn to_markdown_cells(&self) -> String {
        format!(
            "{} | {:.0} | {:.0} | {:.0} | {:.0} | {:.0} | {:.0}",
            self.n,
            self.mean_ms,
            self.p50_ms,
            self.p95_ms,
            self.max_ms,
            self.min_ms,
            self.stddev_ms
        )
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StageAggregator {
    samples: BTreeMap<String, Vec<f64>>,
}

impl StageAggregator {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn record(&mut self, stage: &str, ms: f64) {
        self.samples.entry(stage.to_string()).or_default().push(ms);
    }

    pub fn record_opt(&mut self, stage: &str, ms: Option<u64>) {
        if let Some(v) = ms {
            self.record(stage, v as f64);
        }
    }

    pub fn stages(&self) -> Vec<&str> {
        self.samples.keys().map(String::as_str).collect()
    }

    pub fn len_of(&self, stage: &str) -> usize {
        self.samples.get(stage).map_or(0, Vec::len)
    }

    pub fn stats(&self, stage: &str) -> Option<LatencyStats> {
        self.samples
            .get(stage)
            .map(|v| LatencyStats::from_samples(v))
    }

    pub fn all_stats(&self) -> BTreeMap<String, LatencyStats> {
        self.samples
            .iter()
            .map(|(k, v)| (k.clone(), LatencyStats::from_samples(v)))
            .collect()
    }

    pub fn total_samples(&self) -> usize {
        self.samples.values().map(Vec::len).sum()
    }
}

/// Redundancy metric. Healthy delivered evidence sits well below 0.6; the current
/// pipeline runs far above it because nothing deduplicates.
pub fn mean_pairwise_cosine(embeddings: &[Vec<f32>]) -> f64 {
    if embeddings.len() < 2 {
        return 0.0;
    }
    let (mut total, mut pairs) = (0.0_f64, 0.0_f64);
    for i in 0..embeddings.len() {
        for j in (i + 1)..embeddings.len() {
            total += cosine(&embeddings[i], &embeddings[j]) as f64;
            pairs += 1.0;
        }
    }
    if pairs == 0.0 {
        0.0
    } else {
        total / pairs
    }
}

pub fn cosine(a: &[f32], b: &[f32]) -> f32 {
    if a.is_empty() || a.len() != b.len() {
        return 0.0;
    }
    let (mut dot, mut na, mut nb) = (0.0_f32, 0.0_f32, 0.0_f32);
    for (x, y) in a.iter().zip(b.iter()) {
        dot += x * y;
        na += x * x;
        nb += y * y;
    }
    let d = na.sqrt() * nb.sqrt();
    if d <= 0.0 || !d.is_finite() {
        0.0
    } else {
        dot / d
    }
}

/// Whitespace is collapsed before matching so markdown line wrapping in delivered
/// passages does not produce false negatives.
pub fn count_matching_facts(haystack: &str, expected_facts: &[String]) -> (usize, Vec<bool>) {
    let norm = |s: &str| {
        s.split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
            .to_lowercase()
    };
    let needle = norm(haystack);
    let flags: Vec<bool> = expected_facts
        .iter()
        .map(|f| {
            let n = norm(f);
            !n.is_empty() && needle.contains(&n)
        })
        .collect();
    (flags.iter().filter(|b| **b).count(), flags)
}

/// Normalized token-level similarity in `[0, 1]`.
///
/// The testing style guide (§3) forbids asserting keyword presence as a proxy for
/// output correctness, so every quality gate in this harness scores against this
/// instead. 1.0 = identical token sequence, 0.0 = disjoint.
pub fn token_similarity(a: &[String], b: &[String]) -> f64 {
    if a.is_empty() && b.is_empty() {
        return 1.0;
    }
    if a.is_empty() || b.is_empty() {
        return 0.0;
    }
    let (long, short) = if a.len() >= b.len() { (a, b) } else { (b, a) };
    // O(n*m) rolling rows; corpora here are small (evidence blobs of a few thousand
    // tokens, facts of a few words).
    let mut prev: Vec<usize> = (0..=short.len()).collect();
    let mut cur = vec![0usize; short.len() + 1];
    for i in 1..=long.len() {
        cur[0] = i;
        for j in 1..=short.len() {
            let cost = usize::from(long[i - 1] != short[j - 1]);
            cur[j] = (prev[j] + 1).min(cur[j - 1] + 1).min(prev[j - 1] + cost);
        }
        std::mem::swap(&mut prev, &mut cur);
    }
    1.0 - (prev[short.len()] as f64 / long.len() as f64)
}

fn tokens(s: &str) -> Vec<String> {
    s.split_whitespace()
        .map(|w| {
            w.trim_matches(|c: char| !c.is_alphanumeric())
                .to_lowercase()
        })
        .filter(|w| !w.is_empty())
        .collect()
}

/// Best normalized similarity of `needle` against any window of `haystack`.
///
/// A fixed window length of `needle` plus a quarter-stride keeps recall high while
/// bounding work. Returns 0.0 for an empty needle.
pub fn best_window_similarity(haystack: &str, needle: &str) -> f64 {
    let h = tokens(haystack);
    let n = tokens(needle);
    if n.is_empty() {
        return 0.0;
    }
    if h.len() <= n.len() {
        return token_similarity(&h, &n);
    }
    let stride = (n.len() / 4).max(1);
    let mut best = 0.0_f64;
    let mut start = 0;
    while start + n.len() <= h.len() {
        best = best.max(token_similarity(&h[start..start + n.len()], &n));
        if best >= 1.0 {
            break;
        }
        start += stride;
    }
    // Also score the tail, which a stride walk can skip.
    let tail = h.len() - n.len();
    if tail > 0 {
        best = best.max(token_similarity(&h[tail..], &n));
    }
    best
}

/// Similarity threshold required to count a fact as present.
pub const FACT_MATCH_THRESHOLD: f64 = 0.90;

/// Scores each expected fact against the delivered evidence.
///
/// Returns `(mean_similarity, per_fact_scores, matched_count)`. A fact counts as
/// matched only at or above [`FACT_MATCH_THRESHOLD`], so a partial paraphrase does
/// not register as coverage.
pub fn score_facts(haystack: &str, expected_facts: &[String]) -> (f64, Vec<f64>, usize) {
    if expected_facts.is_empty() {
        return (0.0, Vec::new(), 0);
    }
    let scores: Vec<f64> = expected_facts
        .iter()
        .map(|f| best_window_similarity(haystack, f))
        .collect();
    let matched = scores
        .iter()
        .filter(|s| **s >= FACT_MATCH_THRESHOLD)
        .count();
    let mean = scores.iter().sum::<f64>() / scores.len() as f64;
    (mean, scores, matched)
}
