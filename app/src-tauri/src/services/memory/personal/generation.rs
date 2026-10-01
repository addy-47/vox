use std::{
    sync::mpsc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use anyhow::{anyhow, Result};
use tokio_util::sync::CancellationToken;

use super::prompts::{CONSOLIDATION_MAX_OUTPUT_TOKENS, PERSONAL_CONSOLIDATION_TEMPERATURE};
use crate::services::{
    harness::{ChatMessage, Role},
    llm::{
        catalog::get_baseline_spec, ConversationInput, GenerationPolicy, GenerationPurpose,
        LlmProvider, LlmSettings, LlmStreamEvent, OutputConstraint, ReasoningMode,
    },
    memory::COMPACTION_SENTINEL_TURN_ID,
};

/// Dispatches an LLM generation pass and gathers streamed tokens into a single text output.
fn consolidation_output_constraint(model: &str, schema: serde_json::Value) -> OutputConstraint {
    let supported = get_baseline_spec(model)
        .map(|spec| spec.supports_structured)
        .unwrap_or(true);
    if supported {
        OutputConstraint::JsonSchema {
            name: "personal_memory_consolidation".to_string(),
            schema,
            strict: true,
        }
    } else {
        log::warn!(
            "[Memory::Personal] Model {model} lacks structured-output support; using JSON-object baseline."
        );
        OutputConstraint::JsonObject
    }
}

/// Runs one structured consolidation LLM pass and collects the streamed tokens into a single payload.
pub(super) async fn execute_personal_llm_pass(
    provider: &dyn LlmProvider,
    system_prompt: &str,
    user_content: &str,
    settings: &LlmSettings,
    schema: serde_json::Value,
) -> Result<String> {
    let now_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;

    let policy = GenerationPolicy::from_settings(settings, Some(CONSOLIDATION_MAX_OUTPUT_TOKENS));
    let mut request = policy.build_request(
        GenerationPurpose::StructuredExtraction,
        ConversationInput {
            messages: vec![
                ChatMessage {
                    role: Role::System,
                    content: system_prompt.to_string(),
                    timestamp_ms: now_ms,
                    tool_call_id: None,
                    tool_calls: None,
                },
                ChatMessage {
                    role: Role::User,
                    content: user_content.to_string(),
                    timestamp_ms: now_ms,
                    tool_call_id: None,
                    tool_calls: None,
                },
            ],
        },
    );

    request.output = consolidation_output_constraint(settings.active_model(), schema);
    request.options.reasoning = ReasoningMode::Disabled;
    request.options.max_output_tokens = Some(CONSOLIDATION_MAX_OUTPUT_TOKENS);
    request.options.temperature = Some(PERSONAL_CONSOLIDATION_TEMPERATURE);
    request.options.context_window = Some(settings.context_window);

    log::info!(
        "[Memory::Personal::Request] model={} purpose={:?} output={:?} temperature={:?} max_output_tokens={:?} context_window={:?} reasoning={:?} top_p={:?} top_k={:?} seed={:?} stop_count={} tools_present={} messages={} system_chars={} user_chars={}",
        settings.active_model(),
        request.purpose,
        request.output,
        request.options.temperature,
        request.options.max_output_tokens,
        request.options.context_window,
        request.options.reasoning,
        request.options.top_p,
        request.options.top_k,
        request.options.seed,
        request.options.stop.len(),
        request.tools.is_some(),
        request.input.messages.len(),
        request.input.messages.first().map_or(0, |message| message.content.len()),
        request.input.messages.get(1).map_or(0, |message| message.content.len()),
    );
    let gen_start = std::time::Instant::now();

    let cancel = CancellationToken::new();
    let (tx, rx) = mpsc::channel();
    let (async_tx, mut async_rx) = tokio::sync::mpsc::unbounded_channel();

    let pump_handle = tokio::task::spawn_blocking(move || {
        while let Ok(event) = rx.recv() {
            if async_tx.send(event).is_err() {
                break;
            }
        }
    });

    let gen_future = provider.generate(request, COMPACTION_SENTINEL_TURN_ID, &cancel, &tx);
    let gen_res = tokio::time::timeout(Duration::from_secs(45), gen_future).await;
    drop(tx);

    let mut output = String::new();
    match gen_res {
        Ok(Ok(())) => {
            if let Err(e) = pump_handle.await {
                log::warn!("[PersonalMemory] Token pump task join error: {}", e);
            }
            while let Ok(event) = async_rx.try_recv() {
                if let LlmStreamEvent::Token(token) = event {
                    output.push_str(&token);
                }
            }
            log::info!(
                "[Memory::Personal] LLM pass completed in {:.2?} (received {} chars)",
                gen_start.elapsed(),
                output.len()
            );
            log::debug!("[Memory::Personal::RawResponse]\n{}", output);
        }
        Ok(Err(e)) => {
            if let Err(join_err) = pump_handle.await {
                log::warn!("[PersonalMemory] Token pump task join error: {}", join_err);
            }
            log::error!(
                "[Memory::Personal] LLM generation error after {:.2?}: {}",
                gen_start.elapsed(),
                e
            );
            return Err(anyhow!("LLM generation error: {}", e));
        }
        Err(_) => {
            if let Err(join_err) = pump_handle.await {
                log::warn!("[PersonalMemory] Token pump task join error: {}", join_err);
            }
            log::error!(
                "[Memory::Personal] LLM consolidation timed out after 45s (elapsed: {:.2?})",
                gen_start.elapsed()
            );
            return Err(anyhow!(
                "LLM personal memory consolidation timed out after 45s"
            ));
        }
    }

    let cleaned = output.trim();
    if cleaned.is_empty() {
        log::error!("[Memory::Personal] LLM generated an empty consolidation payload!");
        return Err(anyhow!("LLM generated an empty consolidation payload"));
    }

    Ok(cleaned.to_string())
}
