//! ============================================================================
//! evals/common/slices.rs — Production-Faithful Compaction Slice Planning
//! ============================================================================
//! Category     : Utility Module
//! Component    : evals harness
//! Prerequisites: see evals/README.md
//! Execution    : cargo bench --bench memory_pipeline_eval --release -- --dump-slices
//!
//! A single compaction pass over an entire case is not a production behaviour.
//! Production compacts a *slice* of turns once the tracked context crosses the
//! critical threshold, then feeds that summary back as the prior summary for the
//! next slice. Cases 05-14 are 9k-34k tokens; sent as one slice they overflow
//! `num_ctx` and Ollama silently truncates the oldest turns before the model
//! ever sees them.
//!
//! This module plans slices using the production [`ContextBudgetStage`] and the
//! production token estimator, so the boundary arithmetic cannot drift from the
//! running application. It makes no LLM calls.
//! ============================================================================

use std::path::Path;

use serde::{Deserialize, Serialize};
use vox_lib::services::{
    harness::{
        ChatMessage, ContextBudgetStage, ContextStatus, PromptTag, Role,
    },
    memory::ml::estimate_tokens,
};

use crate::common::datasets::SessionTurn;

/// Context window used by the runtime compaction pass. Mirrors
/// `LlmSettings::default().context_window`.
pub const EVAL_CONTEXT_WINDOW: u32 = 8192;

/// Generation tokens reserved by the harness budget stage. Mirrors
/// `DEFAULT_LLM_MAX_OUTPUT_TOKENS`, the value passed at `chassis.rs:68`.
pub const EVAL_RESERVED_GENERATION_TOKENS: usize = 120;

/// A planned compaction slice: the turns it covers.
///
/// Slice boundaries depend only on turn text, never on any model's output, so
/// the baseline generator and the runtime pass independently arrive at identical
/// turn ranges. Each side chains its *own* prior summary; the ranges are what
/// must match, and they are fixed before either side runs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompactionSlice {
    pub slice_index: usize,
    /// Inclusive turn numbers as they appear in the case file.
    pub from_turn: u32,
    pub to_turn: u32,
    pub turn_count: usize,
    /// Dialogue tokens in this slice, measured with the production estimator.
    pub dialogue_tokens: usize,
    /// Context utilization at compaction time, as a fraction of the usable budget.
    pub utilization_at_trigger: f32,
    pub messages: Vec<SliceMessage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SliceMessage {
    pub role: String,
    pub content: String,
}

/// Builds the production budget stage used to decide slice boundaries.
fn budget_stage() -> ContextBudgetStage {
    ContextBudgetStage::new(
        EVAL_CONTEXT_WINDOW as usize,
        EVAL_RESERVED_GENERATION_TOKENS,
    )
}

/// Renders turns into chat messages in the exact shape the compaction prompt
/// expects (`prompt.rs:109-125`).
fn turns_to_messages(turns: &[SessionTurn], from: usize, to: usize) -> Vec<ChatMessage> {
    turns[from..to]
        .iter()
        .flat_map(|t| {
            [
                ChatMessage::new(Role::User, t.user.clone()),
                ChatMessage::new(Role::Assistant, t.assistant.clone()),
            ]
        })
        .collect()
}

/// Plans compaction slices for one case using production trigger arithmetic.
///
/// Turns accumulate until the tracked context reaches `ContextStatus::Critical`
/// (85% of the usable budget), at which point the accumulated turns form a slice.
/// The tail, if it never reaches critical, forms a final slice so no turn is
/// silently dropped.
pub fn plan_slices(turns: &[SessionTurn]) -> Vec<CompactionSlice> {
    let budget = budget_stage();
    let mut slices: Vec<CompactionSlice> = Vec::new();
    let mut start = 0usize;

    while start < turns.len() {
        let mut end = start;
        let mut status = ContextStatus::Nominal;
        let mut tokens = 0usize;

        // Grow the window one turn at a time until the budget reports critical.
        while end < turns.len() {
            let candidate = turns_to_messages(turns, start, end + 1);
            let candidate_tokens = budget.calculate_tracked_tokens(&candidate);
            let (_, candidate_status) = budget.evaluate_utilization(candidate_tokens);
            tokens = candidate_tokens;
            status = candidate_status;
            end += 1;
            if candidate_status == ContextStatus::Critical {
                break;
            }
        }

        if status != ContextStatus::Critical && end >= turns.len() {
            // Final partial slice below threshold: production would not compact
            // this yet, but the eval must account for every turn.
            slices.push(build_slice(
                slices.len() + 1,
                turns,
                start,
                end,
                budget.calculate_tracked_tokens(&turns_to_messages(turns, start, end)),
                budget.usable_budget(),
            ));
            break;
        }

        slices.push(build_slice(
            slices.len() + 1,
            turns,
            start,
            end,
            tokens,
            budget.usable_budget(),
        ));
        start = end;
    }

    slices
}

fn build_slice(
    index: usize,
    turns: &[SessionTurn],
    from: usize,
    to: usize,
    tracked_tokens: usize,
    usable_budget: usize,
) -> CompactionSlice {
    let messages = turns_to_messages(turns, from, to)
        .into_iter()
        .map(|m| SliceMessage {
            role: m.role.to_string(),
            content: m.content,
        })
        .collect();

    CompactionSlice {
        slice_index: index,
        from_turn: turns[from].turn,
        to_turn: turns[to - 1].turn,
        turn_count: to - from,
        dialogue_tokens: tracked_tokens,
        utilization_at_trigger: if usable_budget == 0 {
            0.0
        } else {
            tracked_tokens as f32 / usable_budget as f32
        },
        messages,
    }
}

/// Serializes planned slices to `<case_dir>/slices/slice_NN.json`.
///
/// One file per slice so the baseline generator can stream them independently and
/// so a reviewer can read one slice without opening a multi-megabyte document.
pub fn dump_slices(slices: &[CompactionSlice], case_dir: &Path) -> std::io::Result<Vec<String>> {
    let dir = case_dir.join("slices");
    std::fs::create_dir_all(&dir)?;
    let mut written = Vec::new();
    for slice in slices {
        let path = dir.join(format!("slice_{:02}.json", slice.slice_index));
        let body = serde_json::to_string_pretty(slice)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        std::fs::write(&path, body)?;
        written.push(path.to_string_lossy().to_string());
    }
    Ok(written)
}

/// Reads slices back from disk, ordered by slice index.
pub fn load_slices(case_dir: &Path) -> std::io::Result<Vec<CompactionSlice>> {
    let dir = case_dir.join("slices");
    let mut paths: Vec<_> = std::fs::read_dir(&dir)?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "json"))
        .collect();
    paths.sort();

    let mut out = Vec::new();
    for path in paths {
        let body = std::fs::read_to_string(&path)?;
        out.push(serde_json::from_str(&body).map_err(|e| {
            std::io::Error::new(std::io::ErrorKind::InvalidData, e)
        })?);
    }
    Ok(out)
}

/// Converts a planned slice back into chat messages for the production
/// compaction call.
///
/// `prior_summary` is the session context produced by the previous slice on the
/// *same* side of the comparison. It is injected as a `<session_context>`
/// system message, which is where `build_compaction_request` looks for it
/// (`harness/mod.rs:198`, `prompt.rs:100`).
pub fn slice_to_chat_messages(slice: &CompactionSlice, prior_summary: Option<&str>) -> Vec<ChatMessage> {
    let mut out = Vec::new();
    if let Some(summary) = prior_summary {
        if !summary.trim().is_empty() {
            out.push(ChatMessage::new(
                Role::System,
                PromptTag::SessionContext.wrap(summary),
            ));
        }
    }
    for m in &slice.messages {
        let role = match m.role.as_str() {
            "user" => Role::User,
            "assistant" => Role::Assistant,
            _ => Role::System,
        };
        out.push(ChatMessage::new(role, m.content.clone()));
    }
    out
}

/// Total token count of the whole case, for reporting.
pub fn case_total_tokens(turns: &[SessionTurn]) -> usize {
    turns_to_messages(turns, 0, turns.len())
        .iter()
        .map(|m| estimate_tokens(&m.content))
        .sum()
}
