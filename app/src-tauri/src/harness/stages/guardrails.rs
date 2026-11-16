use std::collections::{BTreeMap, BTreeSet};
use std::hash::{Hash, Hasher};

use serde_json::Value;

use crate::harness::tools::ToolExecutionOutcome;

const MAX_SLIDING_WINDOW: usize = 10;
const IDENTICAL_CALL_THRESHOLD: usize = 3;
const PING_PONG_THRESHOLD: usize = 4;
const ERROR_CASCADE_THRESHOLD: usize = 3;

/// Compact representation of a completed tool call for pattern and loop detection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolCallFingerprint {
    pub step_index: u32,
    pub tool_name: String,
    pub args_hash: u64,
    pub result_hash: u64,
    pub is_error: bool,
}

/// Turn-scoped guardrail state maintaining call fingerprints and a monotonic strike counter.
#[derive(Debug, Clone, Default)]
pub struct TurnGuardrailsState {
    pub fingerprints: Vec<ToolCallFingerprint>,
    pub strike_count: u8,
}

/// Action directive determined by guardrail evaluation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GuardrailAction {
    Proceed,
    Strike1Warning { warning_msg: String },
    Strike2CircuitBreak,
}

/// Guardrail engine for detecting non-productive tool loops and ping-pong patterns.
#[derive(Debug, Clone, Default)]
pub struct GuardrailsStage;

impl GuardrailsStage {
    /// Evaluates a completed batch of tool outcomes against the turn's sliding window.
    pub fn evaluate_batch(
        &self,
        turn_id: u32,
        step_index: u32,
        state: &mut TurnGuardrailsState,
        outcomes: &[ToolExecutionOutcome],
    ) -> GuardrailAction {
        for outcome in outcomes {
            let fp = ToolCallFingerprint {
                step_index,
                tool_name: outcome.tool_name.clone(),
                args_hash: compute_canonical_json_hash(&outcome.arguments),
                result_hash: compute_str_hash(&outcome.result.content),
                is_error: outcome.is_error,
            };
            state.fingerprints.push(fp);
        }

        if state.fingerprints.len() > MAX_SLIDING_WINDOW {
            let overflow = state.fingerprints.len() - MAX_SLIDING_WINDOW;
            state.fingerprints.drain(0..overflow);
        }

        let detected = check_identical(&state.fingerprints)
            .map(|t| ("identical", t))
            .or_else(|| check_ping_pong(&state.fingerprints).map(|t| ("ping_pong", t)))
            .or_else(|| check_error_cascade(&state.fingerprints).map(|t| ("error_cascade", t)));

        match detected {
            Some((pattern, tool_name)) => {
                if state.strike_count == 0 {
                    state.strike_count = 1;
                    log::warn!(
                        "[GuardrailAudit] turn_id={} step_index={} pattern={} strike=1 tool={}",
                        turn_id,
                        step_index,
                        pattern,
                        tool_name
                    );
                    let warning_msg = format!(
                        "\n\n[SYSTEM WARNING: Repetitive tool call pattern detected ({}). Do not repeat this exact tool call. Pivot to a different tool, use different parameters, or explain the roadblock to the user.]",
                        pattern
                    );
                    GuardrailAction::Strike1Warning { warning_msg }
                } else {
                    state.strike_count = 2;
                    log::warn!(
                        "[GuardrailAudit] turn_id={} step_index={} pattern={} strike=2 tool={}",
                        turn_id,
                        step_index,
                        pattern,
                        tool_name
                    );
                    GuardrailAction::Strike2CircuitBreak
                }
            }
            None => {
                if state.strike_count >= 2 {
                    GuardrailAction::Strike2CircuitBreak
                } else {
                    GuardrailAction::Proceed
                }
            }
        }
    }
}

/// Computes a deterministic 64-bit hash of a JSON value using canonical key ordering.
pub fn compute_canonical_json_hash(val: &Value) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    hash_canonical_json(val, &mut hasher);
    hasher.finish()
}

fn hash_canonical_json<H: std::hash::Hasher>(val: &Value, hasher: &mut H) {
    match val {
        Value::Null => 0u8.hash(hasher),
        Value::Bool(b) => {
            1u8.hash(hasher);
            b.hash(hasher);
        }
        Value::Number(n) => {
            2u8.hash(hasher);
            n.to_string().hash(hasher);
        }
        Value::String(s) => {
            3u8.hash(hasher);
            s.hash(hasher);
        }
        Value::Array(arr) => {
            4u8.hash(hasher);
            arr.len().hash(hasher);
            for item in arr {
                hash_canonical_json(item, hasher);
            }
        }
        Value::Object(map) => {
            5u8.hash(hasher);
            map.len().hash(hasher);
            let mut entries: Vec<(&String, &Value)> = map.iter().collect();
            entries.sort_by(|(k1, _), (k2, _)| k1.cmp(k2));
            for (k, v) in entries {
                k.hash(hasher);
                hash_canonical_json(v, hasher);
            }
        }
    }
}

/// Computes a deterministic 64-bit hash of a string slice.
pub fn compute_str_hash(data: &str) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    data.hash(&mut hasher);
    hasher.finish()
}

fn check_identical(fingerprints: &[ToolCallFingerprint]) -> Option<String> {
    if fingerprints.len() < IDENTICAL_CALL_THRESHOLD {
        return None;
    }
    let last = fingerprints.last()?;
    let mut matching_steps = BTreeSet::new();
    for fp in fingerprints.iter().rev() {
        if fp.tool_name == last.tool_name
            && fp.args_hash == last.args_hash
            && fp.result_hash == last.result_hash
        {
            matching_steps.insert(fp.step_index);
        }
    }
    if matching_steps.len() >= IDENTICAL_CALL_THRESHOLD {
        Some(last.tool_name.clone())
    } else {
        None
    }
}

fn check_ping_pong(fingerprints: &[ToolCallFingerprint]) -> Option<String> {
    if fingerprints.len() < PING_PONG_THRESHOLD {
        return None;
    }
    let n = fingerprints.len();
    let f0 = &fingerprints[n - 4];
    let f1 = &fingerprints[n - 3];
    let f2 = &fingerprints[n - 2];
    let f3 = &fingerprints[n - 1];

    if f0.step_index < f1.step_index
        && f1.step_index < f2.step_index
        && f2.step_index < f3.step_index
        && f0.tool_name == f2.tool_name
        && f1.tool_name == f3.tool_name
        && f0.tool_name != f1.tool_name
        && f0.args_hash == f2.args_hash
        && f1.args_hash == f3.args_hash
    {
        Some(f3.tool_name.clone())
    } else {
        None
    }
}

fn check_error_cascade(fingerprints: &[ToolCallFingerprint]) -> Option<String> {
    if fingerprints.is_empty() {
        return None;
    }
    let mut step_results: BTreeMap<u32, bool> = BTreeMap::new();
    for fp in fingerprints {
        let entry = step_results.entry(fp.step_index).or_insert(true);
        if !fp.is_error {
            *entry = false;
        }
    }
    let steps: Vec<(u32, bool)> = step_results.into_iter().collect();
    if steps.len() < ERROR_CASCADE_THRESHOLD {
        return None;
    }
    let m = steps.len();
    let s0 = steps[m - 3];
    let s1 = steps[m - 2];
    let s2 = steps[m - 1];

    if s1.0 == s0.0 + 1 && s2.0 == s1.0 + 1 && s0.1 && s1.1 && s2.1 {
        let last_tool = fingerprints.last().map(|f| f.tool_name.clone()).unwrap_or_default();
        Some(last_tool)
    } else {
        None
    }
}
