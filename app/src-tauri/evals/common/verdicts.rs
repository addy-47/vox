//! ============================================================================
//! evals/common/verdicts.rs — Typed Judge Contracts & JSON Extraction
//! ============================================================================
//! Category     : Utility Module
//! Component    : evals harness
//! Prerequisites: see evals/README.md
//! Metrics      : every score in summary.md originates here or from DB state
//!
//! Every judge in this harness returns strict JSON validated into one of the
//! structs below. A judge that cannot be parsed yields `JudgeStatus::Invalid`,
//! never a pass. No metric in this harness is ever read out of prose.
//! ============================================================================

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};

/// The six compaction buckets, in the canonical order used by the wire schema.
pub const COMPACTION_CATEGORIES: [&str; 6] = [
    "personal",
    "objective",
    "workdone",
    "blocker",
    "next_step",
    "pitfall",
];

/// One fact as it appears in a compaction JSON document, carrying its flattened
/// index so the judge can address it unambiguously.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlatFact {
    /// 1-based position across the whole document, buckets concatenated in
    /// [`COMPACTION_CATEGORIES`] order.
    pub index: usize,
    pub category: String,
    pub text: String,
}

/// Flattens a compaction document into an addressable fact list.
///
/// Missing or non-array buckets are treated as empty rather than an error: the
/// production schema marks all six required, but a lenient parse fallback is a
/// real runtime outcome and must remain inspectable.
pub fn flatten_compaction(doc: &serde_json::Value) -> Vec<FlatFact> {
    let mut out = Vec::new();
    for category in COMPACTION_CATEGORIES {
        let bucket = doc.get(category).and_then(|v| v.as_array());
        if let Some(items) = bucket {
            for item in items {
                if let Some(text) = item.as_str() {
                    out.push(FlatFact {
                        index: out.len() + 1,
                        category: category.to_string(),
                        text: text.to_string(),
                    });
                }
            }
        }
    }
    out
}

// ----------------------------------------------------------------------------
// Compaction judge: runtime output measured against the Gemma baseline ceiling
// ----------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeFactClass {
    /// Present in the baseline document.
    MatchedBaseline,
    /// Absent from the baseline but supported by the dialogue.
    NovelButValid,
    /// Absent from the baseline and not supported. Counts as a hallucination.
    Ungrounded,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeFactVerdict {
    /// 1-based index into the flattened runtime document.
    pub index: usize,
    pub verdict: RuntimeFactClass,
    /// 1-based index into the flattened baseline document, when matched.
    #[serde(default)]
    pub baseline_index: Option<usize>,
    /// Why, quoting a turn number for novel/ungrounded verdicts. Empty when the
    /// judge omits it; the classification carries the verdict, not the prose.
    #[serde(default, deserialize_with = "null_to_default")]
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BaselineMiss {
    /// 1-based index into the flattened baseline document.
    pub index: usize,
    #[serde(default, deserialize_with = "null_to_default")]
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CompactionVerdict {
    /// One entry per runtime fact, in flattened order.
    pub runtime_facts: Vec<RuntimeFactVerdict>,
    /// Baseline facts the runtime did not produce.
    #[serde(default)]
    pub baseline_missed: Vec<BaselineMiss>,
    #[serde(default, deserialize_with = "null_to_default")]
    pub summary: String,
}

// ----------------------------------------------------------------------------
// Ingestion judge: were the deduplication decisions correct, both directions?
// ----------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DecisionVerdict {
    /// The merge was right: these two facts should be one.
    Correct,
    /// The merge was wrong: two distinct facts were collapsed.
    Incorrect,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NearMissVerdict {
    /// Below threshold, but the judge believes these are the same fact.
    ShouldHaveMerged,
    /// Below threshold, and correctly left separate.
    Distinct,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DecisionVerdictEntry {
    pub queue_id: i64,
    pub verdict: DecisionVerdict,
    #[serde(default, deserialize_with = "null_to_default")]
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NearMissVerdictEntry {
    pub queue_id: i64,
    pub verdict: NearMissVerdict,
    #[serde(default, deserialize_with = "null_to_default")]
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IngestionVerdict {
    /// One entry per deduplication that actually happened.
    pub decisions: Vec<DecisionVerdictEntry>,
    /// One entry per near-miss pair that was left unmerged.
    #[serde(default)]
    pub near_misses: Vec<NearMissVerdictEntry>,
    #[serde(default, deserialize_with = "null_to_default")]
    pub summary: String,
}

// ----------------------------------------------------------------------------
// Consolidation judge: coverage, justified deletions, grounded prose
// ----------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObservationCoverage {
    /// The observation id from the database (e.g. `fact_1734_...`).
    #[serde(default, deserialize_with = "null_to_default")]
    pub obs_id: String,
    pub represented: bool,
    /// The resulting block carrying this observation, when represented.
    #[serde(default)]
    pub block_id: Option<String>,
    #[serde(default, deserialize_with = "null_to_default")]
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeletionAudit {
    #[serde(default, deserialize_with = "null_to_default")]
    pub block_id: String,
    /// True only when an observation directly invalidated the deleted block.
    pub justified: bool,
    /// The observation that invalidated it, when one exists.
    #[serde(default)]
    pub invalidating_obs: Option<String>,
    #[serde(default, deserialize_with = "null_to_default")]
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UpdateAudit {
    #[serde(default, deserialize_with = "null_to_default")]
    pub block_id: String,
    /// False when the replacement text drifted onto a different subject.
    pub kept_subject: bool,
    #[serde(default, deserialize_with = "null_to_default")]
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Hallucination {
    #[serde(default, deserialize_with = "null_to_default")]
    pub block_id: String,
    #[serde(default, deserialize_with = "null_to_default")]
    pub phrase: String,
    pub ungrounded: bool,
    #[serde(default, deserialize_with = "null_to_default")]
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InheritedConcern {
    #[serde(default, deserialize_with = "null_to_default")]
    pub block_id: String,
    #[serde(default, deserialize_with = "null_to_default")]
    pub phrase: String,
    #[serde(default, deserialize_with = "null_to_default")]
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConsolidationVerdict {
    /// One entry per candidate observation presented to the judge.
    pub observations: Vec<ObservationCoverage>,
    /// One entry per `delete` operation the model proposed.
    #[serde(default)]
    pub deletes: Vec<DeletionAudit>,
    #[serde(default)]
    pub updates: Vec<UpdateAudit>,
    #[serde(default)]
    pub hallucinations: Vec<Hallucination>,
    /// Pre-existing blocks no supplied observation supports. Reported, never
    /// counted as this pass's hallucinations.
    #[serde(default)]
    pub inherited_concerns: Vec<InheritedConcern>,
    #[serde(default, deserialize_with = "null_to_default")]
    pub summary: String,
}

// ----------------------------------------------------------------------------
// Extraction + validation
// ----------------------------------------------------------------------------

/// Outcome of one judge call. `Invalid` is a first-class result: a judge whose
/// output cannot be parsed contributes nothing to any metric and is reported
/// separately so it can never be mistaken for a pass.
#[derive(Debug, Clone)]
pub enum JudgeStatus<T> {
    Parsed(T),
    Invalid { reason: String },
}

impl<T> JudgeStatus<T> {
    pub fn is_invalid(&self) -> bool {
        matches!(self, Self::Invalid { .. })
    }

    pub fn invalid_reason(&self) -> Option<&str> {
        match self {
            Self::Invalid { reason } => Some(reason),
            Self::Parsed(_) => None,
        }
    }
}

/// Deserializes a string field that may be absent, null, or a non-string scalar.
/// The judge occasionally emits `"reason": null` or omits optional text; either
/// must degrade to an empty string rather than invalidating the whole verdict.
/// The classifications carry the verdict, never the prose around them.
fn null_to_default<'de, D>(d: D) -> Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let v = serde_json::Value::deserialize(d)?;
    match v {
        serde_json::Value::String(s) => Ok(s),
        serde_json::Value::Null => Ok(String::new()),
        serde_json::Value::Number(n) => Ok(n.to_string()),
        serde_json::Value::Bool(b) => Ok(b.to_string()),
        other => Ok(other.to_string()),
    }
}

/// Recovers a JSON object from a model response.
///
/// Handles the four failure modes observed in practice: a fenced ```json block,
/// leading or trailing prose around the object, trailing commas, and raw control
/// characters inside strings. Strict schema decoding still happens in serde;
/// this only removes the wrapper.
pub fn extract_json_object(raw: &str) -> Result<serde_json::Value> {
    let mut candidate = raw.trim();

    // Strip a fenced block if present, keeping the fenced body.
    if let Some(start) = candidate.find("```") {
        let after = &candidate[start + 3..];
        // Drop an optional language tag on the fence line.
        let body_start = after.find('\n').map(|i| i + 1).unwrap_or(0);
        let body = &after[body_start..];
        if let Some(end) = body.find("```") {
            candidate = body[..end].trim();
        }
    }

    // Take the outermost balanced object.
    let open = candidate
        .find('{')
        .ok_or_else(|| anyhow!("Judge response contains no JSON object: {}", truncate(raw)))?;
    let close = candidate
        .rfind('}')
        .ok_or_else(|| anyhow!("Judge response has an unterminated JSON object: {}", truncate(raw)))?;
    if close < open {
        return Err(anyhow!(
            "Judge response JSON braces are inverted: {}",
            truncate(raw)
        ));
    }
    let mut body = candidate[open..=close].to_string();

    // Strip raw control characters that models emit inside strings.
    body = body
        .chars()
        .filter(|c| !c.is_control() || *c == '\n' || *c == '\t')
        .collect();

    // Remove trailing commas before a closing brace or bracket, allowing any
    // whitespace (including newlines) between the comma and the bracket. The
    // judge pretty-prints, so a trailing comma is almost never adjacent.
    body = strip_trailing_commas(&body);

    serde_json::from_str(&body)
        .map_err(|e| anyhow!("Judge response is not valid JSON ({}): {}", e, truncate(raw)))
}

/// Removes `,` characters that sit immediately before a closing `}` or `]`,
/// ignoring any whitespace between the comma and the bracket.
///
/// Operates on raw characters, so a comma inside a string literal that happens
/// to precede a bracket would also be stripped. Judge verdicts never contain
/// that pattern in field values, and a lost comma there is preferable to a lost
/// verdict.
fn strip_trailing_commas(body: &str) -> String {
    let chars: Vec<char> = body.chars().collect();
    let mut out = String::with_capacity(body.len());
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == ',' {
            let mut j = i + 1;
            while j < chars.len() && chars[j].is_whitespace() {
                j += 1;
            }
            if j < chars.len() && (chars[j] == '}' || chars[j] == ']') {
                // Trailing comma: skip it, keep the whitespace and bracket.
                i += 1;
                continue;
            }
        }
        out.push(chars[i]);
        i += 1;
    }
    out
}

fn truncate(s: &str) -> String {    let t = s.trim();
    if t.chars().count() <= 400 {
        t.to_string()
    } else {
        format!("{}…", t.chars().take(400).collect::<String>())
    }
}

/// Parses and validates a judge response into a typed verdict.
pub fn parse_verdict<T: serde::de::DeserializeOwned>(
    raw: &str,
) -> JudgeStatus<T> {
    let value = match extract_json_object(raw) {
        Ok(v) => v,
        Err(e) => {
            return JudgeStatus::Invalid {
                reason: e.to_string(),
            }
        }
    };
    match serde_json::from_value::<T>(value) {
        Ok(parsed) => JudgeStatus::Parsed(parsed),
        Err(e) => JudgeStatus::Invalid {
            reason: format!("Judge JSON did not match the expected schema: {}", e),
        },
    }
}

// ----------------------------------------------------------------------------
// Deterministic aggregation
// ----------------------------------------------------------------------------

/// Validates a parsed verdict against the documents it was asked about.
///
/// A judge that emits an index with no corresponding fact, skips an index, or
/// addresses a baseline fact that does not exist is not reasoning about the
/// inventory it was given. Callers use the counts to exclude phantoms from
/// every metric.
#[derive(Debug, Clone, Default)]
pub struct VerdictValidation {
    pub phantom_runtime: usize,
    pub missing_runtime: usize,
    pub phantom_baseline: usize,
}

pub fn validate_compaction_verdict(
    v: &CompactionVerdict,
    runtime_facts: usize,
    baseline_facts: usize,
) -> VerdictValidation {
    let mut out = VerdictValidation::default();
    let mut seen = std::collections::HashSet::new();
    for f in &v.runtime_facts {
        if f.index == 0 || f.index > runtime_facts || !seen.insert(f.index) {
            out.phantom_runtime += 1;
        }
        if let Some(b) = f.baseline_index {
            if b == 0 || b > baseline_facts {
                out.phantom_baseline += 1;
            }
        }
    }
    out.missing_runtime = runtime_facts.saturating_sub(seen.len());
    for m in &v.baseline_missed {
        if m.index == 0 || m.index > baseline_facts {
            out.phantom_baseline += 1;
        }
    }
    out
}

/// Counts produced from a validated verdict. These are the only numbers that
/// reach `summary.md`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CompactionCounts {
    pub runtime_facts: usize,
    pub baseline_facts: usize,
    pub matched_baseline: usize,
    pub novel_but_valid: usize,
    pub ungrounded: usize,
    pub baseline_missed: usize,
    /// Matched plus novel: runtime facts the judge accepts as supported.
    pub accepted_runtime: usize,
}

pub fn count_compaction(v: &CompactionVerdict, baseline_facts: usize) -> CompactionCounts {
    let mut c = CompactionCounts {
        baseline_facts,
        ..Default::default()
    };
    for f in &v.runtime_facts {
        c.runtime_facts += 1;
        match f.verdict {
            RuntimeFactClass::MatchedBaseline => c.matched_baseline += 1,
            RuntimeFactClass::NovelButValid => c.novel_but_valid += 1,
            RuntimeFactClass::Ungrounded => c.ungrounded += 1,
        }
    }
    c.baseline_missed = v.baseline_missed.len();
    c.accepted_runtime = c.matched_baseline + c.novel_but_valid;
    c
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct IngestionCounts {
    pub merges_total: usize,
    pub merges_correct: usize,
    pub merges_incorrect: usize,
    pub near_misses_total: usize,
    pub near_misses_should_have_merged: usize,
    pub near_misses_distinct: usize,
}

pub fn count_ingestion(v: &IngestionVerdict) -> IngestionCounts {
    let mut c = IngestionCounts::default();
    for d in &v.decisions {
        c.merges_total += 1;
        match d.verdict {
            DecisionVerdict::Correct => c.merges_correct += 1,
            DecisionVerdict::Incorrect => c.merges_incorrect += 1,
        }
    }
    for n in &v.near_misses {
        c.near_misses_total += 1;
        match n.verdict {
            NearMissVerdict::ShouldHaveMerged => c.near_misses_should_have_merged += 1,
            NearMissVerdict::Distinct => c.near_misses_distinct += 1,
        }
    }
    c
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ConsolidationCounts {
    pub observations_total: usize,
    pub observations_represented: usize,
    pub observations_dropped: usize,
    pub deletes_total: usize,
    pub deletes_unjustified: usize,
    pub updates_total: usize,
    pub updates_drifted: usize,
    pub ungrounded_spans: usize,
    pub inherited_concerns: usize,
}

pub fn count_consolidation(v: &ConsolidationVerdict) -> ConsolidationCounts {
    let mut c = ConsolidationCounts::default();
    for o in &v.observations {
        c.observations_total += 1;
        if o.represented {
            c.observations_represented += 1;
        } else {
            c.observations_dropped += 1;
        }
    }
    for d in &v.deletes {
        c.deletes_total += 1;
        if !d.justified {
            c.deletes_unjustified += 1;
        }
    }
    for u in &v.updates {
        c.updates_total += 1;
        if !u.kept_subject {
            c.updates_drifted += 1;
        }
    }
    c.ungrounded_spans = v.hallucinations.iter().filter(|h| h.ungrounded).count();
    c.inherited_concerns = v.inherited_concerns.len();
    c
}

/// Renders a ratio as `numerator/denominator` or `N/A` when the denominator is
/// zero. A zero denominator is never reported as 0% or 100%.
pub fn ratio_str(numerator: usize, denominator: usize) -> String {
    if denominator == 0 {
        "N/A (0 denominator)".to_string()
    } else {
        format!(
            "{}/{} = {:.1}%",
            numerator,
            denominator,
            100.0 * numerator as f64 / denominator as f64
        )
    }
}
