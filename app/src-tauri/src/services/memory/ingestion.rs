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

/// Executes LLM Context Compaction & Personal Fact Extraction (v5 §5.1 / §5.3 Ingestion).
/// Summarizes context history and extracts structured profile facts.
pub fn run_compaction(
    provider: &dyn LlmProvider,
    history_messages: &[ChatMessage],
    _last_user_turn: &ChatMessage,
    current_personal_memory: &HashMap<String, Vec<String>>,
) -> Result<CompactionResult> {
    if history_messages.is_empty() {
        return Err(anyhow!("No history turns to compact."));
    }

    log::info!("[MemoryIngestion] Running LLM Context Compaction via {:?}", provider.kind());

    let mut history_text = String::new();
    for msg in history_messages {
        history_text.push_str(&format!("{}: {}\n\n", msg.role, msg.content));
    }

    let is_first = current_personal_memory.is_empty()
        || current_personal_memory.values().all(|v| v.is_empty());

    let user_content = if is_first {
        format!(
            "<conversation_history>\n{}\n</conversation_history>\n\n\
             <task>\n\
             Analyze the <conversation_history> above and extract all stated user facts into the 10 flat collections from the <schema>.\n\
             Follow every rule in <rules> and <boundary_disambiguation>. Output ONLY the JSON object starting with {{ and ending with }}.\n\
             </task>",
            history_text
        )
    } else {
        let serialized_memory = serde_json::to_string_pretty(current_personal_memory).unwrap_or_default();
        format!(
            "<known_facts>\n{}\n</known_facts>\n\n\
             <conversation_history>\n{}\n</conversation_history>\n\n\
             <task>\n\
             Analyze the new <conversation_history> turns against <known_facts>.\n\
             Extract ONLY BRAND-NEW facts or explicit updates introduced in <conversation_history>.\n\
             CRITICAL: NEVER re-extract, re-word, or output facts already present in <known_facts>.\n\
             For collections with no new facts, return an empty array [].\n\
             Follow every rule in <rules> and <boundary_disambiguation>. Output ONLY the JSON object starting with {{ and ending with }}.\n\
             </task>",
            serialized_memory,
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
        .get("Context")
        .and_then(|v| v.first())
        .cloned()
        .unwrap_or_else(|| summary_content.clone());

    if final_summary.trim().is_empty() {
        return Err(anyhow!("Live LLM compaction produced empty summary."));
    }

    // Merge new extracted facts into current_personal_memory state
    let mut updated_personal_memory = current_personal_memory.clone();
    let mut diff_to_enqueue = HashMap::new();

    for (col, new_facts) in &personal_memory {
        if new_facts.is_empty() {
            continue;
        }
        let entry = updated_personal_memory.entry(col.clone()).or_default();
        let mut unique_additions = Vec::new();
        for fact in new_facts {
            if !entry.contains(fact) {
                entry.push(fact.clone());
                unique_additions.push(fact.clone());
            }
        }
        if !unique_additions.is_empty() {
            diff_to_enqueue.insert(col.clone(), unique_additions);
        }
    }

    Ok(CompactionResult {
        context_summary: final_summary,
        personal_memory: updated_personal_memory,
        diff_to_enqueue,
    })
}
