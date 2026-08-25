use crate::core::constants::COMPACTION_SYSTEM_PROMPT;
use crate::core::events::VoxEvent;
use crate::services::llm::LlmProvider;
use crate::services::memory::working_memory::{ChatMessage, Role};
use anyhow::{anyhow, Result};
use std::collections::HashMap;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

/// Extracted facts and summary resulting from LLM conversation compaction.
#[derive(Debug, Clone)]
pub struct CompactionResult {
    pub context_summary: String,
    pub personal_memory: HashMap<String, Vec<String>>,
    pub diff_to_enqueue: HashMap<String, Vec<String>>,
}

/// Builds the provider-neutral GenerationRequest for compaction.
fn build_compaction_request(
    history_messages: &[ChatMessage],
    settings: Option<&crate::core::settings::LlmSettings>,
) -> crate::services::llm::GenerationRequest {
    let mut history_text = String::new();
    for msg in history_messages {
        history_text.push_str(&format!("{}: {}\n\n", msg.role, msg.content));
    }

    let user_content = format!(
        "<conversation_history>\n{}\n</conversation_history>\n\n\
         <task>\n\
         Analyze the <conversation_history> above and extract all stated facts into the 6 collections from the <output_schema>.\n\
         Output ONLY the JSON object starting with {{ and ending with }}.\n\
         </task>",
        history_text
    );

    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;

    let default_settings = crate::core::settings::LlmSettings::default();
    let effective_settings = settings.unwrap_or(&default_settings);
    let policy = crate::services::llm::GenerationPolicy::from_settings(effective_settings);

    policy.build_request(
        crate::services::llm::GenerationPurpose::MemoryCompaction,
        crate::services::llm::ConversationInput {
            messages: vec![
                ChatMessage {
                    role: Role::System,
                    content: COMPACTION_SYSTEM_PROMPT.to_string(),
                    timestamp_ms: now_ms,
                },
                ChatMessage {
                    role: Role::User,
                    content: user_content,
                    timestamp_ms: now_ms,
                },
            ],
        },
    )
}

/// Dispatches a single compaction generation request to the provider and collects tokens.
fn execute_compaction_attempt(
    provider: &dyn LlmProvider,
    request: &crate::services::llm::GenerationRequest,
) -> Result<String> {
    let cancel_flag = Arc::new(AtomicBool::new(false));
    let (tx, rx) = std::sync::mpsc::channel();
    let fut = provider.generate(request.clone(), 999_999, &cancel_flag, &tx);

    let res = match tokio::runtime::Handle::try_current() {
        Ok(h) => {
            if h.runtime_flavor() == tokio::runtime::RuntimeFlavor::MultiThread {
                tokio::task::block_in_place(|| h.block_on(fut))
            } else {
                std::thread::scope(|s| {
                    s.spawn(|| {
                        tokio::runtime::Builder::new_current_thread()
                            .enable_all()
                            .build()
                            .expect("Failed to build worker runtime")
                            .block_on(fut)
                    })
                    .join()
                    .unwrap()
                })
            }
        }
        Err(_) => tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("Failed to build temporary tokio runtime")
            .block_on(fut),
    };

    let mut summary_content = String::new();
    if res.is_ok() {
        while let Ok(event) = rx.recv_timeout(std::time::Duration::from_secs(45)) {
            match event {
                VoxEvent::LlmToken { token, .. } => {
                    summary_content.push_str(&token);
                }
                VoxEvent::LlmFinished { .. } => break,
                _ => {}
            }
        }
    }

    Ok(summary_content)
}

/// Executes LLM Context Compaction and personal fact extraction.
pub fn run_compaction(
    provider: &dyn LlmProvider,
    history_messages: &[ChatMessage],
    _last_user_turn: &ChatMessage,
    _last_context_summary: Option<&str>,
    settings: Option<&crate::core::settings::LlmSettings>,
) -> Result<CompactionResult> {
    if history_messages.is_empty() {
        return Err(anyhow!("No history turns to compact."));
    }

    log::info!(
        "[MemoryIngestion] Running LLM Context Compaction via {:?}",
        provider.kind()
    );

    let request = build_compaction_request(history_messages, settings);
    let mut summary_content = String::new();
    let mut personal_memory = HashMap::new();
    let mut attempts = 0;
    let max_attempts = 2;

    while attempts < max_attempts {
        attempts += 1;
        log::info!(
            "[MemoryIngestion] Compaction attempt {}/{}...",
            attempts,
            max_attempts
        );

        if let Ok(content) = execute_compaction_attempt(provider, &request) {
            summary_content = content;
            if !summary_content.trim().is_empty() {
                if let Some(resp) = crate::utils::json::parse_compaction_json(&summary_content) {
                    personal_memory = resp;
                    log::info!(
                        "[MemoryIngestion] Compaction JSON parsed successfully on attempt {}.",
                        attempts
                    );
                    break;
                } else {
                    log::warn!(
                        "[MemoryIngestion] Compaction JSON parsing failed on attempt {}/{}.",
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::llm::ProviderKind;

    #[test]
    fn test_compaction_empty_history() {
        struct MockProvider;
        impl LlmProvider for MockProvider {
            fn kind(&self) -> ProviderKind {
                ProviderKind::Embedded
            }
            fn health_check(&self) -> bool {
                true
            }
            fn capabilities(&self) -> &crate::services::llm::types::ProviderCapabilities {
                static CAPS: std::sync::OnceLock<
                    crate::services::llm::types::ProviderCapabilities,
                > = std::sync::OnceLock::new();
                CAPS.get_or_init(crate::services::llm::types::ProviderCapabilities::default)
            }
            fn list_models(
                &self,
            ) -> Result<
                Vec<crate::core::settings::LlmModelInfo>,
                crate::services::llm::types::LlmError,
            > {
                Ok(vec![])
            }
            fn generate<'a>(
                &'a self,
                _request: crate::services::llm::types::GenerationRequest,
                _turn_id: u32,
                _cancel_flag: &'a Arc<AtomicBool>,
                _event_tx: &'a std::sync::mpsc::Sender<VoxEvent>,
            ) -> futures_util::future::BoxFuture<
                'a,
                Result<(), crate::services::llm::types::LlmError>,
            > {
                Box::pin(async { Ok(()) })
            }
        }

        let provider = MockProvider;
        let history: Vec<ChatMessage> = vec![];
        let last_user_turn = ChatMessage {
            role: Role::User,
            content: "Hello".to_string(),
            timestamp_ms: 0,
        };
        let res = run_compaction(&provider, &history, &last_user_turn, None, None);
        assert!(
            res.is_err(),
            "run_compaction should return Err when history is empty"
        );
    }
}
