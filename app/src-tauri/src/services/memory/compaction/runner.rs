use super::prompt::build_compaction_request;
use crate::core::events::VoxEvent;
use crate::services::llm::LlmProvider;
use crate::services::harness::buffer::ChatMessage;
use anyhow::{anyhow, Result};
use std::collections::HashMap;
use tokio_util::sync::CancellationToken;

/// Extracted facts and summary resulting from LLM conversation compaction.
#[derive(Debug, Clone)]
pub struct CompactionResult {
    pub context_summary: String,
    pub personal_memory: HashMap<String, Vec<String>>,
    pub diff_to_enqueue: HashMap<String, Vec<String>>,
}

/// Dispatches a single compaction generation request to the provider and collects streamed tokens asynchronously.
async fn execute_compaction_attempt(
    provider: &dyn LlmProvider,
    request: &crate::services::llm::GenerationRequest,
    cancel: &CancellationToken,
) -> Result<String> {
    let (tx, rx) = std::sync::mpsc::channel();
    let (async_tx, mut async_rx) = tokio::sync::mpsc::unbounded_channel();

    let pump_handle = tokio::task::spawn_blocking(move || {
        while let Ok(event) = rx.recv() {
            if async_tx.send(event).is_err() {
                break;
            }
        }
    });

    let gen_future = provider.generate(
        request.clone(),
        crate::core::constants::COMPACTION_SENTINEL_TURN_ID,
        cancel,
        &tx,
    );

    let mut summary_content = String::new();

    let gen_res = tokio::time::timeout(std::time::Duration::from_secs(45), gen_future).await;
    drop(tx);

    match gen_res {
        Ok(Ok(())) => {
            let _ = pump_handle.await;
            while let Ok(event) = async_rx.try_recv() {
                match event {
                    VoxEvent::LlmToken { token, .. } => {
                        summary_content.push_str(&token);
                    }
                    VoxEvent::LlmFinished { .. } => break,
                    _ => {}
                }
            }
        }
        Ok(Err(e)) => {
            let _ = pump_handle.await;
            log::warn!("[MemoryCompaction] Provider generation returned error: {}", e);
        }
        Err(_) => {
            let _ = pump_handle.await;
            log::warn!("[MemoryCompaction] Compaction attempt timed out after 45s");
        }
    }

    Ok(summary_content)
}

/// Executes async LLM Context Compaction and personal fact extraction with up to 2 retry attempts.
pub async fn run_compaction(
    provider: &dyn LlmProvider,
    history_messages: &[ChatMessage],
    settings: Option<&crate::core::settings::LlmSettings>,
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
    let mut personal_memory = HashMap::new();
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

        if let Ok(content) = execute_compaction_attempt(provider, &request, effective_cancel).await {
            summary_content = content;
            if !summary_content.trim().is_empty() {
                if let Some(resp) = crate::utils::json::parse_compaction_json(&summary_content) {
                    personal_memory = resp;
                    log::info!(
                        "[MemoryCompaction] Compaction JSON parsed successfully on attempt {}.",
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

    let final_summary = personal_memory
        .get("Narrative")
        .or_else(|| personal_memory.get("Context"))
        .and_then(|v| v.first())
        .cloned()
        .unwrap_or_else(|| summary_content.clone());

    if final_summary.trim().is_empty() {
        return Err(anyhow!("Live LLM compaction produced empty summary."));
    }

    Ok(CompactionResult {
        context_summary: final_summary,
        personal_memory: personal_memory.clone(),
        diff_to_enqueue: personal_memory,
    })
}
