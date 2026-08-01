use anyhow::{anyhow, Result};
use std::collections::HashMap;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use crate::services::llm::LlmProvider;
use crate::services::memory::working_memory::{ChatMessage, ConversationContext, Role};
use crate::services::memory::estimate_tokens;
use crate::core::constants::COMPACTION_SYSTEM_PROMPT;
use crate::core::events::VoxEvent;

#[derive(Debug, Clone)]
pub struct CompactionResult {
    pub context_summary: String,
    pub personal_memory: HashMap<String, Vec<String>>,
    pub diff_to_enqueue: HashMap<String, Vec<String>>,
}

/// Executes LLM Context Compaction & Personal Fact Extraction.
/// Summarizes context history and extracts structured profile facts.
pub fn run_compaction(
    provider: &dyn LlmProvider,
    history_messages: &[ChatMessage],
    _last_user_turn: &ChatMessage,
    last_context_summary: Option<&str>,
) -> Result<CompactionResult> {
    if history_messages.is_empty() {
        return Err(anyhow!("No history turns to compact."));
    }

    log::info!("[MemoryIngestion] Running LLM Context Compaction via {:?}", provider.kind());

    let mut history_text = String::new();
    for msg in history_messages {
        history_text.push_str(&format!("{}: {}\n\n", msg.role, msg.content));
    }

    let user_content = if let Some(prev_ctx) = last_context_summary.filter(|s| !s.trim().is_empty()) {
        format!(
            "<previous_context>\n{}\n</previous_context>\n\n\
             <conversation_history>\n{}\n</conversation_history>\n\n\
             <task>\n\
             Analyze the <conversation_history> above and extract all stated facts into the 6 collections from the <schema>.\n\
             Use <previous_context> to maintain a cumulative, updated summary in the Narrative collection.\n\
             Follow every rule in <rules> and <boundary_disambiguation>. Output ONLY the JSON object starting with {{ and ending with }}.\n\
             </task>",
            prev_ctx.trim(),
            history_text
        )
    } else {
        format!(
            "<conversation_history>\n{}\n</conversation_history>\n\n\
             <task>\n\
             Analyze the <conversation_history> above and extract all stated facts into the 6 collections from the <schema>.\n\
             Follow every rule in <rules> and <boundary_disambiguation>. Output ONLY the JSON object starting with {{ and ending with }}.\n\
             </task>",
            history_text
        )
    };

    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;

    let temp_ctx = ConversationContext {
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
        token_count: estimate_tokens(&history_text)
            + estimate_tokens(COMPACTION_SYSTEM_PROMPT)
            + 150,
        kv_cache_index: 0,
    };

    let mut summary_content = String::new();
    let mut personal_memory = HashMap::new();
    let mut attempts = 0;
    let max_attempts = 2;

    while attempts < max_attempts {
        attempts += 1;
        summary_content.clear();
        let cancel_flag = Arc::new(AtomicBool::new(false));
        let (tx, rx) = std::sync::mpsc::channel();

        log::info!("[MemoryIngestion] Compaction attempt {}/{}...", attempts, max_attempts);
        if provider.generate(&temp_ctx, 999_999, &cancel_flag, &tx).is_ok() {
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

        if !summary_content.trim().is_empty() {
            if let Some(resp) = crate::utils::json::parse_compaction_json(&summary_content) {
                personal_memory = resp;
                log::info!("[MemoryIngestion] Compaction JSON parsed successfully on attempt {}.", attempts);
                break;
            } else {
                log::warn!("[MemoryIngestion] Compaction JSON parsing failed on attempt {}/{}.", attempts, max_attempts);
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
            fn list_models(&self) -> Result<Vec<crate::core::settings::LlmModelInfo>> {
                Ok(vec![])
            }
            fn generate(
                &self,
                _context: &ConversationContext,
                _max_tokens: u32,
                _cancel_flag: &Arc<AtomicBool>,
                _event_tx: &std::sync::mpsc::Sender<VoxEvent>,
            ) -> Result<()> {
                Ok(())
            }
        }

        let provider = MockProvider;
        let history: Vec<ChatMessage> = vec![];
        let last_user_turn = ChatMessage {
            role: Role::User,
            content: "Hello".to_string(),
            timestamp_ms: 0,
        };
        let res = run_compaction(&provider, &history, &last_user_turn, None);
        assert!(res.is_err(), "run_compaction should return Err when history is empty");
    }
}
