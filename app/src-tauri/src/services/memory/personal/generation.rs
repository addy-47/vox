use std::{
    sync::mpsc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use anyhow::{anyhow, Result};
use tokio_util::sync::CancellationToken;

use super::prompts::{
    consolidation_json_schema, CONSOLIDATION_MAX_OUTPUT_TOKENS, PERSONAL_CONSOLIDATION_TEMPERATURE,
};
use crate::services::{
    harness::{ChatMessage, Role},
    llm::{
        catalog::get_baseline_spec,
        ConversationInput, GenerationPolicy, GenerationPurpose, LlmProvider, LlmSettings,
        LlmStreamEvent, OutputConstraint, ReasoningMode,
    },
    memory::COMPACTION_SENTINEL_TURN_ID,
};

/// Dispatches an LLM generation pass and gathers streamed tokens into a single text output.
/// Selects the strict schema constraint, falling back to JSON-object when the
/// catalog baseline reports the model lacks structured-output support. Mirrors
/// `compaction_output_constraint` so both structured passes negotiate the same
/// way; the transport additionally negotiates down on a provider 400.
fn consolidation_output_constraint(model: &str) -> OutputConstraint {
    let supported = get_baseline_spec(model)
        .map(|spec| spec.supports_structured)
        .unwrap_or(true);
    if supported {
        OutputConstraint::JsonSchema {
            name: "personal_memory_consolidation".to_string(),
            schema: consolidation_json_schema(),
            strict: true,
        }
    } else {
        log::warn!(
            "[Memory::Personal] Model {model} lacks structured-output support; using JSON-object baseline."
        );
        OutputConstraint::JsonObject
    }
}

pub(super) async fn execute_personal_llm_pass(
    provider: &dyn LlmProvider,
    system_prompt: &str,
    user_content: &str,
    settings: &LlmSettings,
    structured: bool,
) -> Result<String> {
    let now_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;

    // Seeded from the same constant the explicit override below applies, so the two can never
    // disagree. A bare literal here previously shadowed the constant: editing
    // `CONSOLIDATION_MAX_OUTPUT_TOKENS` would have silently stopped affecting the policy.
    let policy = GenerationPolicy::from_settings(settings, Some(CONSOLIDATION_MAX_OUTPUT_TOKENS));
    let purpose = if structured {
        GenerationPurpose::StructuredExtraction
    } else {
        GenerationPurpose::Conversation
    };
    let mut request = policy.build_request(
        purpose,
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

    if structured {
        request.output = consolidation_output_constraint(settings.active_model());
    } else {
        request.output = OutputConstraint::Text;
    }

    // REASONING IS DISABLED — deliberately, and this reverses the setting introduced with the
    // first cut of the indexed patch engine. That cut turned reasoning ON because, under the
    // previous prose-targeting protocol, the model had to re-quote document text verbatim and
    // degenerated into whole-document echo operations without reasoning first
    // (`docs/plans/phase12/consolidation-structured--logic-plan.md` §2.2). The indexed design
    // removes that failure mode at its root: an operation carries only a small `text` field and
    // never restates the document, so there is nothing to echo and nothing to reason about.
    //
    // Measured against the server on this exact task shape (qwen3.5:9b, strict JSON schema):
    //   reasoning ON  -> `done_reason=length`, 3843 eval tokens, ~17.9k chars of thinking trace,
    //                    ZERO content tokens, at every ceiling tried (512 / 1024 / 4096).
    //                    The model reasons for the entire budget and never emits an answer, which
    //                    tripped the empty-document guard and aborted the whole cycle.
    //   reasoning OFF -> `done_reason=stop`, 55-250 eval tokens, valid minimal JSON, ~1s,
    //                    0 out-of-range indices and 0 multi-line `text` across 6 document/fact
    //                    combinations.
    // The out-of-range and multi-line indices seen with reasoning ON are a symptom of the same
    // runaway trace, not an independent engine defect.
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
        log::error!("[Memory::Personal] LLM generated empty personal memory document!");
        return Err(anyhow!("LLM generated empty personal memory document"));
    }

    Ok(cleaned.to_string())
}
