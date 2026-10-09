use std::{
    sync::mpsc,
    time::{Duration, Instant},
};

use anyhow::{anyhow, Result};
use tokio_util::sync::CancellationToken;

use super::prompt::build_compaction_request;
use crate::{
    services::{
        harness::{ChatMessage, Role},
        llm::{GenerationRequest, LlmProvider, LlmSettings, LlmStreamEvent},
        memory::COMPACTION_SENTINEL_TURN_ID,
    },
    utils::json::parse_unified_compaction_json,
};

pub const COMPACTION_TIMEOUT_SECS: u64 = 45;
pub const MAX_COMPACTION_ATTEMPTS: usize = 1;

/// Extracted facts and complete session context resulting from unified LLM conversation compaction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompactionResult {
    pub raw_json: String,
    pub session_context: String,
    pub facts: Vec<(String, String)>,
    /// Telemetry only. `personal` facts sharing no content word with any user turn
    /// in the compacted slice. Flag-only: flagged facts are still enqueued exactly
    /// as before. Absence of overlap proves nothing by itself for inferred facts
    /// ("lives in Chicago" shares "chicago" with a weather question), so this is
    /// a floor, not a verdict — but zero overlap is always worth a look.
    pub attribution_flags: Vec<AttributionFlag>,
    /// Telemetry only. How many `personal` facts carried a `[turn N]` citation vs
    /// how many were emitted. Measures whether the citation instruction holds.
    pub personal_cited: usize,
    pub personal_total: usize,
}

/// One `personal` fact with no lexical grounding in the slice's user turns.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct AttributionFlag {
    pub fact: String,
    pub user_turns_checked: usize,
}

/// Strips a trailing `[turn N]` citation — the public alias used by the eval
/// harness so both sides judge the stored (clean) form.
pub fn strip_citation(text: &str) -> String {
    strip_turn_citation(text).0
}

/// Strips a trailing `[turn N]` citation from a personal fact, returning the
/// clean text and the cited turn number.
///
/// The citation is an eval-and-audit aid, never stored content: memory keeps the
/// fact, telemetry keeps the citation. A fact with no citation keeps its text
/// unchanged and reports `None` — flag-only, consistent with the attribution
/// check. Enforcement (refusing uncited facts) is a later decision with eval
/// evidence behind it.
fn strip_turn_citation(text: &str) -> (String, Option<u32>) {
    let trimmed = text.trim();
    // Match a trailing "[turn N]" with flexible spacing and case.
    if let Some(open) = trimmed.rfind('[') {
        if trimmed.ends_with(']') {
            let inner = trimmed[open + 1..trimmed.len() - 1].trim();
            let lower = inner.to_lowercase();
            if let Some(digits) = lower.strip_prefix("turn") {
                let digits = digits.trim();
                if !digits.is_empty() && digits.chars().all(|c| c.is_ascii_digit()) {
                    if let Ok(n) = digits.parse::<u32>() {
                        if n > 0 {
                            return (trimmed[..open].trim_end().to_string(), Some(n));
                        }
                    }
                }
            }
        }
    }
    (trimmed.to_string(), None)
}

/// Collects lowercase alphanumeric tokens of length 4+, the working definition
/// of a content word for the attribution check.
fn content_words(text: &str) -> std::collections::HashSet<String> {
    text.split_whitespace()
        .map(|w| {
            w.chars()
                .filter(|c| c.is_alphanumeric())
                .collect::<String>()
                .to_lowercase()
        })
        .filter(|w| w.len() >= 4)
        .collect()
}

/// Flags `personal` facts with no content-word overlap against the slice's user
/// turns. Deterministic and conservative: it can only under-flag, never over-flag.
fn check_personal_attribution(
    personal_facts: &[String],
    history_messages: &[ChatMessage],
) -> Vec<AttributionFlag> {
    let user_text: Vec<String> = history_messages
        .iter()
        .filter(|m| m.role == Role::User)
        .map(|m| m.content.clone())
        .collect();
    let user_words: std::collections::HashSet<String> = user_text
        .iter()
        .flat_map(|t| content_words(t))
        .collect();

    personal_facts
        .iter()
        .filter(|fact| {
            let words = content_words(fact);
            !words.is_empty() && words.is_disjoint(&user_words)
        })
        .map(|fact| AttributionFlag {
            fact: fact.clone(),
            user_turns_checked: user_text.len(),
        })
        .collect()
}

/// Dispatches a single compaction generation request to the provider and collects streamed tokens asynchronously.
async fn execute_compaction_attempt(
    provider: &dyn LlmProvider,
    request: &GenerationRequest,
    cancel: &CancellationToken,
) -> Result<String> {
    let (tx, rx) = mpsc::channel();
    let (async_tx, mut async_rx) = tokio::sync::mpsc::unbounded_channel();

    let pump_handle = tokio::task::spawn_blocking(move || {
        while let Ok(event) = rx.recv() {
            if async_tx.send(event).is_err() {
                break;
            }
        }
    });

    let gen_future = provider.generate(request.clone(), COMPACTION_SENTINEL_TURN_ID, cancel, &tx);

    let mut summary_content = String::new();

    let gen_started = Instant::now();
    let gen_res =
        tokio::time::timeout(Duration::from_secs(COMPACTION_TIMEOUT_SECS), gen_future).await;
    drop(tx);

    match gen_res {
        Ok(Ok(())) => {
            if let Err(e) = pump_handle.await {
                log::warn!("[MemoryCompaction] Pump task error: {:?}", e);
            }
            while let Ok(event) = async_rx.try_recv() {
                match event {
                    LlmStreamEvent::Token(token) => {
                        summary_content.push_str(&token);
                    }
                    LlmStreamEvent::ToolCall(_) => {}
                    LlmStreamEvent::Finished => {
                        log::info!(
                            "[MemoryCompaction] LlmFinished received; full summary received."
                        );
                        break;
                    }
                }
            }
            log::info!(
                "[MemoryCompaction] Generation completed in {:?}; output_chars={}\n[CompactionLLM::Output]\n{}",
                gen_started.elapsed(),
                summary_content.len(),
                summary_content.trim()
            );
        }
        Ok(Err(e)) => {
            if let Err(join_err) = pump_handle.await {
                log::warn!(
                    "[MemoryCompaction] Pump task error on failure: {:?}",
                    join_err
                );
            }
            log::warn!(
                "[MemoryCompaction] Provider generation returned error after {:?}: {}",
                gen_started.elapsed(),
                e
            );
        }
        Err(_) => {
            if let Err(join_err) = pump_handle.await {
                log::warn!(
                    "[MemoryCompaction] Pump task error on timeout: {:?}",
                    join_err
                );
            }
            log::warn!(
                "[MemoryCompaction] Compaction attempt timed out after {:?} (limit={}s)",
                gen_started.elapsed(),
                COMPACTION_TIMEOUT_SECS
            );
        }
    }

    Ok(summary_content)
}

/// Executes async LLM Context Compaction and personal fact extraction with up to 2 retry attempts.
pub async fn run_compaction(
    provider: &dyn LlmProvider,
    history_messages: &[ChatMessage],
    settings: Option<&LlmSettings>,
    cancel_token: Option<&CancellationToken>,
) -> Result<CompactionResult> {
    if history_messages.is_empty() {
        return Err(anyhow!("No history turns to compact."));
    }

    let default_cancel = CancellationToken::new();
    let effective_cancel = cancel_token.unwrap_or(&default_cancel);

    log::info!(
        "[MemoryCompaction] Running LLM Context Compaction via {:?}",
        provider.kind()
    );

    let request = build_compaction_request(history_messages, settings);
    let mut summary_content = String::new();
    let mut parsed_payload = None;
    let mut attempts = 0;

    while attempts < MAX_COMPACTION_ATTEMPTS {
        if effective_cancel.is_cancelled() {
            return Err(anyhow!("Compaction cancelled by user activity."));
        }

        attempts += 1;
        log::info!(
            "[MemoryCompaction] Compaction attempt {}/{}...",
            attempts,
            MAX_COMPACTION_ATTEMPTS
        );

        if let Ok(content) = execute_compaction_attempt(provider, &request, effective_cancel).await
        {
            summary_content = content;
            if !summary_content.trim().is_empty() {
                if let Some(resp) = parse_unified_compaction_json(&summary_content) {
                    parsed_payload = Some(resp);
                    log::info!(
                        "[MemoryCompaction] Compaction unified JSON parsed successfully on attempt {}.",
                        attempts
                    );
                    break;
                } else {
                    log::warn!(
                        "[MemoryCompaction] Compaction JSON parsing failed on attempt {}/{}.",
                        attempts,
                        MAX_COMPACTION_ATTEMPTS
                    );
                }
            }
        }
    }

    if summary_content.trim().is_empty() {
        return Err(anyhow!("Live LLM compaction produced empty summary."));
    }

    let payload = parsed_payload.unwrap_or_default();
    let formatted_context = payload.format_session_context();
    let final_context = if !formatted_context.trim().is_empty() {
        formatted_context
    } else {
        summary_content.clone()
    };

    let mut facts = Vec::new();
    let mut personal_texts = Vec::new();
    let mut personal_cited = 0usize;
    for item in &payload.personal {
        // Strip the audit citation before anything is stored: memory keeps the
        // fact, telemetry keeps the count. Uncited facts are kept (flag-only).
        let (clean, cited) = strip_turn_citation(item);
        if cited.is_some() {
            personal_cited += 1;
        }
        facts.push(("personal".to_string(), clean.clone()));
        personal_texts.push(clean);
    }
    for item in payload.objective {
        facts.push(("objective".to_string(), item));
    }
    for item in payload.workdone {
        facts.push(("workdone".to_string(), item));
    }
    for item in payload.blocker {
        facts.push(("blocker".to_string(), item));
    }
    for item in payload.next_step {
        facts.push(("next_step".to_string(), item));
    }
    for item in payload.pitfall {
        facts.push(("pitfall".to_string(), item));
    }

    Ok(CompactionResult {
        raw_json: summary_content,
        session_context: final_context,
        facts,
        attribution_flags: check_personal_attribution(&personal_texts, history_messages),
        personal_cited,
        personal_total: personal_texts.len(),
    })
}
