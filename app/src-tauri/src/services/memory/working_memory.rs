use crate::core::constants::{TRANSITION_MESSAGES_EN, TRANSITION_MESSAGES_HI};
use crate::services::llm::{LlmProvider, ProviderKind};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    System,
    User,
    Assistant,
}

// Deleted ProfileUpdate struct

impl std::fmt::Display for Role {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Role::System => write!(f, "system"),
            Role::User => write!(f, "user"),
            Role::Assistant => write!(f, "assistant"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: Role,
    pub content: String,
    pub timestamp_ms: u64,
}

pub struct ConversationContext {
    pub messages: Vec<ChatMessage>,
    pub token_count: usize,
    pub kv_cache_index: usize,
}

use super::estimate_tokens;

fn current_timestamp_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

pub struct ConversationManager {
    messages: Vec<ChatMessage>,
    system_prompt: ChatMessage,

    total_token_count: usize,
    max_context_tokens: usize,
    reserved_generation_tokens: usize,
    critical_threshold: f32, // Default: 0.85
    soft_threshold: f32,     // Default: 0.65

    kv_synced_index: usize,

    opportunistic_active: bool,
    opportunistic_cancel: Arc<AtomicBool>,
}

impl ConversationManager {
    pub fn new(max_context_tokens: usize) -> Self {
        let default_sys_prompt = ChatMessage {
            role: Role::System,
            content: crate::core::constants::SYSTEM_PROMPT_MODULAR.to_string(),
            timestamp_ms: current_timestamp_ms(),
        };

        let sys_tokens = estimate_tokens(&default_sys_prompt.content);

        Self {
            messages: vec![default_sys_prompt.clone()],
            system_prompt: default_sys_prompt,
            total_token_count: sys_tokens,
            max_context_tokens,
            reserved_generation_tokens: 512,
            critical_threshold: 0.85,
            soft_threshold: 0.65,
            kv_synced_index: 0,
            opportunistic_active: false,
            opportunistic_cancel: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn set_max_context_tokens(&mut self, max_tokens: usize) {
        if max_tokens > 0 {
            self.max_context_tokens = max_tokens;
            log::info!(
                "[WorkingMemory] Updated max_context_tokens to {}",
                max_tokens
            );
        }
    }

    pub fn new_session(&mut self, system_prompt: &str) {
        let sys_msg = ChatMessage {
            role: Role::System,
            content: system_prompt.to_string(),
            timestamp_ms: current_timestamp_ms(),
        };
        let sys_tokens = estimate_tokens(system_prompt);

        self.system_prompt = sys_msg.clone();
        self.messages = vec![sys_msg];
        self.total_token_count = sys_tokens;
        self.kv_synced_index = 0;
        self.cancel_opportunistic();
        log::info!("[WorkingMemory] New session started. System prompt set.");
    }

    pub fn update_system_prompt(&mut self, new_system_prompt: &str) {
        if self.system_prompt.content != new_system_prompt {
            let sys_tokens = estimate_tokens(new_system_prompt);
            let old_sys_tokens = estimate_tokens(&self.system_prompt.content);
            self.system_prompt.content = new_system_prompt.to_string();
            if !self.messages.is_empty() && self.messages[0].role == Role::System {
                self.messages[0].content = new_system_prompt.to_string();
                self.total_token_count = self.total_token_count.saturating_sub(old_sys_tokens) + sys_tokens;
            }
            self.kv_synced_index = 0;
        }
    }

    pub fn push_user_turn(&mut self, text: String) {
        let tokens = estimate_tokens(&text);
        let msg = ChatMessage {
            role: Role::User,
            content: text,
            timestamp_ms: current_timestamp_ms(),
        };
        self.messages.push(msg);
        self.total_token_count += tokens;
        log::debug!(
            "[WorkingMemory] User turn pushed. Total tokens: {} / {}",
            self.total_token_count,
            self.max_context_tokens
        );
    }

    pub fn push_assistant_turn(&mut self, text: String) {
        if text.trim().is_empty() {
            return;
        }
        let tokens = estimate_tokens(&text);
        let msg = ChatMessage {
            role: Role::Assistant,
            content: text,
            timestamp_ms: current_timestamp_ms(),
        };
        self.messages.push(msg);
        self.total_token_count += tokens;
        self.kv_synced_index = self.messages.len();
        log::debug!(
            "[WorkingMemory] Assistant turn pushed. Total tokens: {} / {}. KV index: {}",
            self.total_token_count,
            self.max_context_tokens,
            self.kv_synced_index
        );
    }

    pub fn pop_last_user_turn(&mut self) {
        if let Some(last) = self.messages.last() {
            if last.role == Role::User {
                let popped = self.messages.pop();
                if let Some(msg) = popped {
                    let tokens = estimate_tokens(&msg.content);
                    self.total_token_count = self.total_token_count.saturating_sub(tokens);
                    log::info!("[WorkingMemory] Popped pending user turn due to barge-in/cancel.");
                }
            }
        }
    }

    pub fn context_utilization(&self) -> f32 {
        let usable_budget = self
            .max_context_tokens
            .saturating_sub(self.reserved_generation_tokens)
            .max(1);
        self.total_token_count as f32 / usable_budget as f32
    }

    pub fn get_messages(&self) -> &[ChatMessage] {
        &self.messages
    }

    pub fn needs_threshold_maintenance(&self) -> bool {
        self.context_utilization() >= self.critical_threshold
    }

    /// Prepares context for LLM generation.
    /// If critical threshold is reached, executes Threshold Maintenance.
    /// Returns (ConversationContext, Option<TransitionSpeechText>, HashMap<String, Vec<String>>).
    pub fn build_context(
        &mut self,
        provider_kind: ProviderKind,
        is_devanagari: bool,
        llm_provider: Option<&dyn LlmProvider>,
    ) -> (ConversationContext, Option<String>, std::collections::HashMap<String, Vec<String>>) {
        self.cancel_opportunistic();

        let mut transition_speech = None;
        let mut personal_memory = std::collections::HashMap::new();

        if self.needs_threshold_maintenance() {
            log::warn!(
                "[WorkingMemory] Critical threshold reached ({:.1}% utilization). Performing Maintenance...",
                self.context_utilization() * 100.0
            );

            // Select transition message
            let msg_set = if is_devanagari {
                TRANSITION_MESSAGES_HI
            } else {
                TRANSITION_MESSAGES_EN
            };
            let random_idx = (current_timestamp_ms() as usize) % msg_set.len();
            transition_speech = Some(msg_set[random_idx].to_string());

            // Strategy selection
            let use_fifo = match provider_kind {
                ProviderKind::Embedded => self.max_context_tokens <= 4096,
                ProviderKind::OpenAiCompat => false,
            };

            if use_fifo || llm_provider.is_none() {
                self.perform_fifo_maintenance();
            } else if let Some(provider) = llm_provider {
                match self.perform_compaction_maintenance(provider) {
                    Ok(updates) => {
                        personal_memory = updates;
                    }
                    Err(e) => {
                        log::error!(
                            "[WorkingMemory] LLM compaction failed: {}. Falling back to FIFO.",
                            e
                        );
                        self.perform_fifo_maintenance();
                    }
                }
            }
        }

        let kv_idx = if provider_kind == ProviderKind::Embedded {
            self.kv_synced_index
        } else {
            0
        };

        let ctx = ConversationContext {
            messages: self.messages.clone(),
            token_count: self.total_token_count,
            kv_cache_index: kv_idx,
        };

        (ctx, transition_speech, personal_memory)
    }

    /// FIFO Sliding Window Shift: Drops oldest (User, Assistant) pairs until below soft threshold.
    fn perform_fifo_maintenance(&mut self) {
        log::info!("[WorkingMemory] Executing FIFO Sliding Window shift...");

        while self.messages.len() > 3 && self.context_utilization() > self.soft_threshold {
            // Keep index 0 (system prompt). Look for oldest User/Assistant pair at index 1 and 2
            let mut removed_tokens = 0;
            if self.messages.len() >= 3
                && self.messages[1].role == Role::User
                && self.messages[2].role == Role::Assistant
            {
                removed_tokens += estimate_tokens(&self.messages[1].content);
                removed_tokens += estimate_tokens(&self.messages[2].content);
                self.messages.remove(1);
                self.messages.remove(1);
            } else if self.messages.len() >= 2 {
                removed_tokens += estimate_tokens(&self.messages[1].content);
                self.messages.remove(1);
            } else {
                break;
            }
            self.total_token_count = self.total_token_count.saturating_sub(removed_tokens);
        }

        // Force KV cache re-sync
        self.kv_synced_index = 0;
        log::info!(
            "[WorkingMemory] FIFO shift complete. Retained {} messages ({} tokens, utilization {:.1}%).",
            self.messages.len(),
            self.total_token_count,
            self.context_utilization() * 100.0
        );
    }

    /// LLM-driven Context Compaction: Summarizes history messages[1..N-1] into a single high-density summary block and extracts user profile updates.
    fn perform_compaction_maintenance(
        &mut self,
        provider: &dyn LlmProvider,
    ) -> anyhow::Result<std::collections::HashMap<String, Vec<String>>> {
        if self.messages.len() <= 3 {
            self.perform_fifo_maintenance();
            return Ok(std::collections::HashMap::new());
        }

        log::info!("[WorkingMemory] Executing LLM Context Compaction via {:?}", provider.kind());

        let last_user_turn = self.messages.pop().ok_or_else(|| anyhow::anyhow!("No user turn"))?;
        let mut history_text = String::new();
        for msg in &self.messages[1..] {
            history_text.push_str(&format!("{}: {}\n\n", msg.role, msg.content));
        }

        let temp_ctx = ConversationContext {
            messages: vec![
                ChatMessage {
                    role: Role::System,
                    content: crate::core::constants::COMPACTION_SYSTEM_PROMPT.to_string(),
                    timestamp_ms: current_timestamp_ms(),
                },
                ChatMessage {
                    role: Role::User,
                    content: format!(
                        "<conversation_history>\n{}\n</conversation_history>\n\n\
                         <task>\n\
                         Extract facts from the <conversation_history> above into the 10 collections from <schema>, following every rule in <rules>.\n\
                         Return ONLY the JSON object, starting with {{ and ending with }}.\n\
                         </task>",
                        history_text
                    ),
                    timestamp_ms: current_timestamp_ms(),
                },
            ],
            token_count: estimate_tokens(&history_text)
                + estimate_tokens(crate::core::constants::COMPACTION_SYSTEM_PROMPT)
                + 150,
            kv_cache_index: 0,
        };

        let mut summary_content = String::new();
        let mut personal_memory = std::collections::HashMap::new();
        let mut attempts = 0;
        let max_attempts = 2;

        while attempts < max_attempts {
            attempts += 1;
            summary_content.clear();
            let cancel_flag = Arc::new(AtomicBool::new(false));
            let (tx, rx) = std::sync::mpsc::channel();

            log::info!("[WorkingMemory] Compaction attempt {}/{}...", attempts, max_attempts);
            if provider.generate(&temp_ctx, 999_999, &cancel_flag, &tx).is_ok() {
                while let Ok(event) = rx.recv_timeout(std::time::Duration::from_secs(45)) {
                    match event {
                        crate::core::events::VoxEvent::LlmToken { token, .. } => {
                            summary_content.push_str(&token);
                        }
                        crate::core::events::VoxEvent::LlmFinished { .. } => break,
                        _ => {}
                    }
                }
            }

            if !summary_content.trim().is_empty() {
                if let Some(resp) = crate::utils::json::parse_compaction_json(&summary_content) {
                    personal_memory = resp;
                    log::info!("[WorkingMemory] Compaction JSON parsed successfully on attempt {}.", attempts);
                    break;
                } else {
                    log::warn!("[WorkingMemory] Compaction JSON parsing failed on attempt {}/{}.", attempts, max_attempts);
                }
            }
        }

        if summary_content.trim().is_empty() {
            log::warn!("[WorkingMemory] Live LLM compaction produced empty summary. Falling back to FIFO shift.");
            self.messages.push(last_user_turn);
            self.perform_fifo_maintenance();
            return Ok(std::collections::HashMap::new());
        }

        if personal_memory.is_empty() {
            log::warn!("[WorkingMemory] LLM compaction returned non-JSON/malformed content after retries. Treating as raw summary fallback.");
        }

        let final_summary = personal_memory
            .get("Context")
            .and_then(|v| v.first())
            .cloned()
            .unwrap_or_else(|| {
                summary_content.clone()
            });

        if final_summary.trim().is_empty() {
            log::warn!("[WorkingMemory] Live LLM compaction produced empty summary. Falling back to FIFO shift.");
            self.messages.push(last_user_turn);
            self.perform_fifo_maintenance();
            return Ok(std::collections::HashMap::new());
        }

        let summary_msg = ChatMessage {
            role: Role::System,
            content: format!("[Compacted History Summary: {}]", final_summary.trim()),
            timestamp_ms: current_timestamp_ms(),
        };

        let summary_tokens = estimate_tokens(&summary_msg.content);
        let sys_tokens = estimate_tokens(&self.system_prompt.content);
        let user_tokens = estimate_tokens(&last_user_turn.content);

        self.messages = vec![
            self.system_prompt.clone(),
            summary_msg,
            last_user_turn,
        ];

        self.total_token_count = sys_tokens + summary_tokens + user_tokens;
        self.kv_synced_index = 0; // Reset KV cache index for re-encode

        log::info!(
            "[WorkingMemory] Compaction complete. Rebuilt context with 3 items ({} tokens, utilization {:.1}%). Extracted {} personal facts.",
            self.total_token_count,
            self.context_utilization() * 100.0,
            personal_memory.values().map(|v| v.len()).sum::<usize>()
        );

        Ok(personal_memory)
    }

    pub fn try_trigger_opportunistic(&mut self) -> Option<(usize, Vec<ChatMessage>, Arc<AtomicBool>)> {
        if self.context_utilization() > self.soft_threshold
            && self.context_utilization() < self.critical_threshold
            && !self.opportunistic_active
            && self.messages.len() > 3
        {
            self.opportunistic_active = true;
            self.opportunistic_cancel = Arc::new(AtomicBool::new(false));
            log::info!(
                "[WorkingMemory] Triggering Opportunistic Compaction candidate at {:.1}% utilization.",
                self.context_utilization() * 100.0
            );
            Some((
                self.messages.len(),
                self.messages.clone(),
                Arc::clone(&self.opportunistic_cancel),
            ))
        } else {
            None
        }
    }

    pub fn commit_opportunistic(&mut self, snapshot_len: usize, summary_text: String) -> bool {
        if !self.opportunistic_active {
            log::info!("[WorkingMemory] Commit rejected: Opportunistic compaction was inactive.");
            return false;
        }
        if self.opportunistic_cancel.load(Ordering::Relaxed) {
            self.opportunistic_active = false;
            log::info!("[WorkingMemory] Commit rejected: Opportunistic compaction was cancelled.");
            return false;
        }
        if self.messages.len() != snapshot_len {
            self.opportunistic_active = false;
            log::info!(
                "[WorkingMemory] Commit rejected: Race detected (expected {} items, current has {}).",
                snapshot_len,
                self.messages.len()
            );
            return false;
        }

        let last_user_turn = self.messages.pop().unwrap();
        let summary_msg = ChatMessage {
            role: Role::System,
            content: format!("[Summary of prior context: {}]", summary_text),
            timestamp_ms: current_timestamp_ms(),
        };

        self.messages = vec![
            self.system_prompt.clone(),
            summary_msg,
            last_user_turn,
        ];

        let mut count = 0;
        for msg in &self.messages {
            count += estimate_tokens(&msg.content);
        }
        self.total_token_count = count;
        self.kv_synced_index = 0;
        self.opportunistic_active = false;

        log::info!(
            "[WorkingMemory] Opportunistic Compaction COMMITTED successfully! Utilization now {:.1}%.",
            self.context_utilization() * 100.0
        );

        true
    }

    pub fn on_pipeline_idle(&mut self) {
        // Handled via try_trigger_opportunistic
    }

    pub fn on_speech_start(&mut self) {
        self.cancel_opportunistic();
    }

    fn cancel_opportunistic(&mut self) {
        if self.opportunistic_active {
            self.opportunistic_cancel.store(true, Ordering::Relaxed);
            self.opportunistic_active = false;
            log::info!("[WorkingMemory] Opportunistic compaction cancelled.");
        }
    }

    pub fn latest_summary(&self) -> String {
        for msg in self.messages.iter().rev() {
            if msg.role == Role::System {
                if let Some(s) = msg.content.strip_prefix("[Compacted History Summary: ") {
                    return s.strip_suffix("]").unwrap_or(s).to_string();
                }
                if let Some(s) = msg.content.strip_prefix("[Summary of prior context: ") {
                    return s.strip_suffix("]").unwrap_or(s).to_string();
                }
            }
        }
        String::new()
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_conversation_manager_fifo() {
        let mut mgr = ConversationManager::new(1024);
        mgr.new_session("System prompt");

        // Push enough long turns to cross 85% threshold
        for i in 0..15 {
            mgr.push_user_turn(format!("User question turn {} with significant padding text to build token count...", i));
            mgr.push_assistant_turn(format!("Assistant detailed answer turn {} with extra explanation words to fill context budget...", i));
        }

        assert!(mgr.needs_threshold_maintenance());

        let (ctx, speech, _) = mgr.build_context(ProviderKind::Embedded, false, None);

        assert!(speech.is_some());
        assert!(!mgr.needs_threshold_maintenance());
        assert!(ctx.messages.len() < 31);
    }

    struct MockProvider {
        response_text: String,
    }

    use crate::services::llm::providers::LlmProvider;
    use crate::core::events::VoxEvent;
    use crate::services::llm::ProviderKind;
    use crate::core::settings::LlmModelInfo;
    use std::sync::mpsc;
    use std::sync::atomic::AtomicBool;

    impl LlmProvider for MockProvider {
        fn generate(
            &self,
            _ctx: &ConversationContext,
            turn_id: u32,
            _cancel_flag: &Arc<AtomicBool>,
            tx: &mpsc::Sender<VoxEvent>,
        ) -> anyhow::Result<()> {
            let _ = tx.send(VoxEvent::LlmToken {
                turn_id,
                token: self.response_text.clone(),
            });
            let _ = tx.send(VoxEvent::LlmFinished {
                turn_id,
            });
            Ok(())
        }

        fn health_check(&self) -> bool { true }
        fn list_models(&self) -> anyhow::Result<Vec<LlmModelInfo>> { Ok(Vec::new()) }
        fn kind(&self) -> ProviderKind { ProviderKind::Embedded }
    }

    #[test]
    fn test_perform_compaction_maintenance_json() {
        let mut mgr = ConversationManager::new(1024);
        mgr.new_session("System prompt");
        mgr.push_user_turn("Hi, my name is Alex and I love coding in Rust.".to_string());
        mgr.push_assistant_turn("Hello Alex! Rust is a great language.".to_string());
        mgr.push_user_turn("Yes, it is.".to_string());

        let mock_response = r#"{
            "Identity": ["Works as a software engineer.", "User's name is Alex."],
            "Preferences": ["User loves coding in Rust."],
            "Context": ["The user introduced himself as Alex and expressed his love for Rust."]
        }"#.to_string();

        let provider = MockProvider { response_text: mock_response };
        let updates = mgr.perform_compaction_maintenance(&provider).unwrap();

        assert_eq!(updates.len(), 3);
        assert_eq!(updates.get("Identity").unwrap().len(), 2);
        assert_eq!(updates.get("Preferences").unwrap().len(), 1);
        assert_eq!(updates.get("Context").unwrap().len(), 1);
    }

    #[test]
    fn test_perform_compaction_maintenance_markdown_fences() {
        let mut mgr = ConversationManager::new(1024);
        mgr.new_session("System prompt");
        mgr.push_user_turn("Hi, my name is Alex.".to_string());
        mgr.push_assistant_turn("Hello!".to_string());
        mgr.push_user_turn("Yes.".to_string());

        let mock_response = r#"```json
        {
            "Identity": ["User's name is Alex."],
            "Context": ["Alex introduced himself."]
        }
        ```"#.to_string();

        let provider = MockProvider { response_text: mock_response };
        let updates = mgr.perform_compaction_maintenance(&provider).unwrap();

        assert_eq!(updates.len(), 2);
        assert_eq!(updates.get("Identity").unwrap()[0], "User's name is Alex.");
    }

    #[test]
    fn test_perform_compaction_maintenance_fallback_prose() {
        let mut mgr = ConversationManager::new(1024);
        mgr.new_session("System prompt");
        mgr.push_user_turn("Hi, my name is Alex.".to_string());
        mgr.push_assistant_turn("Hello!".to_string());
        mgr.push_user_turn("Yes.".to_string());

        // Plain prose instead of JSON
        let mock_response = "The user is Alex. He loves system programming.".to_string();

        let provider = MockProvider { response_text: mock_response };
        let updates = mgr.perform_compaction_maintenance(&provider).unwrap();

        // Should fall back to prose, return no profile updates, but successfully compact the conversation
        assert_eq!(updates.len(), 0);
        // The message queue should have the summary message
        assert_eq!(mgr.messages[1].role, Role::System);
        assert!(mgr.messages[1].content.contains("The user is Alex. He loves system programming."));
    }

    #[test]
    fn test_fix_missing_commas() {
        let bad_json = r#"{
            "summary": "This is a summary"
            "profile_updates": [
                {
                    "category": "Identity"
                    "key": "name"
                    "value": "Alex"
                }
            ]
        }"#;
        let fixed = crate::utils::json::fix_missing_commas_in_json(bad_json);
        let parsed: serde_json::Value = serde_json::from_str(&fixed).expect("Failed to parse fixed JSON");
        assert_eq!(parsed["summary"], "This is a summary");
    }

    #[test]
    fn test_resilient_deserialization_of_compaction_response() {
        let json_data = r#"{
            "summary": "User codes in Rust.",
            "personal_memory": {
                "Identity": ["Alex"],
                "Preferences": ["Rust"]
            }
        }"#;
        #[derive(Debug, Deserialize)]
        struct UnifiedCompactionPayload {
            summary: String,
            personal_memory: std::collections::HashMap<String, Vec<String>>,
        }
        let resp: UnifiedCompactionPayload = serde_json::from_str(json_data).expect("Failed to deserialize compaction response");
        assert_eq!(resp.summary, "User codes in Rust.");
        assert_eq!(resp.personal_memory.get("Identity").unwrap()[0], "Alex");
        assert_eq!(resp.personal_memory.get("Preferences").unwrap()[0], "Rust");
    }
}
