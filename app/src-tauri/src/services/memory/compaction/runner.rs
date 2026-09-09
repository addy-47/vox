use std::{sync::mpsc, time::Duration};

use anyhow::{anyhow, Result};
use tokio_util::sync::CancellationToken;

use super::prompt::build_compaction_request;
use crate::{
    core::settings::LlmSettings,
    services::{
        harness::buffer::ChatMessage,
        llm::{GenerationRequest, LlmProvider, LlmStreamEvent},
        memory::COMPACTION_SENTINEL_TURN_ID,
    },
    utils::json::parse_unified_compaction_json,
};

/// Extracted facts and summary resulting from unified LLM conversation compaction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompactionResult {
    pub raw_json: String,
    pub context_summary: String,
    pub facts: Vec<(String, String)>,
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

    let gen_res = tokio::time::timeout(Duration::from_secs(45), gen_future).await;
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
                    LlmStreamEvent::Finished => {
                        log::info!(
                            "[MemoryCompaction] LlmFinished received; full summary received."
                        );
                        break;
                    }
                }
            }
        }
        Ok(Err(e)) => {
            if let Err(join_err) = pump_handle.await {
                log::warn!(
                    "[MemoryCompaction] Pump task error on failure: {:?}",
                    join_err
                );
            }
            log::warn!(
                "[MemoryCompaction] Provider generation returned error: {}",
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
            log::warn!("[MemoryCompaction] Compaction attempt timed out after 45s");
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
    let max_attempts = 2;

    while attempts < max_attempts {
        if effective_cancel.is_cancelled() {
            return Err(anyhow!("Compaction cancelled by user activity."));
        }

        attempts += 1;
        log::info!(
            "[MemoryCompaction] Compaction attempt {}/{}...",
            attempts,
            max_attempts
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
                        max_attempts
                    );
                }
            }
        }
    }

    if summary_content.trim().is_empty() {
        return Err(anyhow!("Live LLM compaction produced empty summary."));
    }

    let payload = parsed_payload.unwrap_or_default();
    let final_summary = if !payload.context_summary.trim().is_empty() {
        payload.context_summary
    } else {
        summary_content.clone()
    };

    let mut facts = Vec::new();
    for item in payload.personal {
        facts.push(("personal".to_string(), item));
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
        context_summary: final_summary,
        facts,
    })
}
