use super::accountant::TokenAccountant;
use super::buffer::{current_timestamp_ms, ChatMessage, MessageBuffer, Role};
use super::prompt_builder::{
    assemble_system_prompt, build_session_history_xml, consolidate_system_message,
};
use crate::core::constants::MemoryCollection;
use crate::services::memory::compaction::CompactionResult;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use turso::Connection;

/// Orchestrates conversation turns, dynamic FIFO sliding window, and context compaction state.
pub struct ConversationManager {
    pub(crate) buffer: MessageBuffer,
    pub(crate) accountant: TokenAccountant,
    system_prompt: ChatMessage,
    base_system_prompt: String,
    identity_facts: Vec<String>,
    dynamic_user_profile: Option<String>,

    session_compaction_contexts: Vec<String>,
    latest_compaction_facts: HashMap<String, Vec<String>>,

    opportunistic_active: bool,
    opportunistic_cancel: Arc<AtomicBool>,
}

impl ConversationManager {
    /// Creates a new ConversationManager instance with the specified context capacity.
    pub fn new(max_context_tokens: usize) -> Self {
        let base_system_prompt = crate::core::constants::SYSTEM_PROMPT_MODULAR.to_string();
        let default_sys_prompt = ChatMessage {
            role: Role::System,
            content: base_system_prompt.clone(),
            timestamp_ms: current_timestamp_ms(),
        };

        let sys_tokens = crate::services::memory::ml::estimate_tokens(&default_sys_prompt.content);
        let buffer = MessageBuffer::new(default_sys_prompt.clone());
        let accountant = TokenAccountant::new(max_context_tokens, sys_tokens);

        Self {
            buffer,
            accountant,
            system_prompt: default_sys_prompt,
            base_system_prompt,
            identity_facts: Vec::new(),
            dynamic_user_profile: None,
            session_compaction_contexts: Vec::new(),
            latest_compaction_facts: HashMap::new(),
            opportunistic_active: false,
            opportunistic_cancel: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Returns a slice view of active conversation messages.
    pub fn get_messages(&self) -> &[ChatMessage] {
        self.buffer.messages()
    }

    /// Assembles the complete system prompt from base prompt, identity facts, and dynamic profile.
    pub fn assemble_system_prompt(&self) -> String {
        assemble_system_prompt(
            &self.base_system_prompt,
            &self.identity_facts,
            self.dynamic_user_profile.as_deref(),
        )
    }

    /// Updates the dynamic user profile context retrieved from semantic search for the active turn.
    pub fn update_dynamic_user_profile(&mut self, profile: Option<String>) {
        if self.dynamic_user_profile != profile {
            self.dynamic_user_profile = profile;
            let assembled = self.assemble_system_prompt();
            let old_sys_tokens =
                crate::services::memory::ml::estimate_tokens(&self.system_prompt.content);
            let sys_tokens = crate::services::memory::ml::estimate_tokens(&assembled);
            self.system_prompt.content = assembled.clone();
            if !self.buffer.messages.is_empty() && self.buffer.messages[0].role == Role::System {
                self.buffer.messages[0].content = assembled;
            }
            self.accountant.sub_tokens(old_sys_tokens);
            self.accountant.add_tokens(sys_tokens);
            self.buffer.kv_synced_index = 0;
        }
    }

    /// Returns current total token count across active messages.
    pub fn total_token_count(&self) -> usize {
        self.accountant.total_token_count()
    }

    /// Updates the maximum allowable context token budget.
    pub fn set_max_context_tokens(&mut self, max_tokens: usize) {
        self.accountant.set_max_context_tokens(max_tokens);
    }

    /// Sets active Identity facts and reassembles the system prompt.
    pub fn set_identity_facts(&mut self, identity_facts: Vec<String>) {
        self.identity_facts = identity_facts;
        let assembled = self.assemble_system_prompt();
        let old_tokens = crate::services::memory::ml::estimate_tokens(&self.system_prompt.content);
        let new_tokens = crate::services::memory::ml::estimate_tokens(&assembled);
        self.system_prompt.content = assembled.clone();
        if !self.buffer.messages.is_empty() && self.buffer.messages[0].role == Role::System {
            self.buffer.messages[0].content = assembled;
        }
        self.accountant.sub_tokens(old_tokens);
        self.accountant.add_tokens(new_tokens);
        self.buffer.kv_synced_index = 0;
        log::info!(
            "[ConversationManager] Successfully preloaded {} Identity facts into System Prompt.",
            self.identity_facts.len()
        );
    }

    /// Preloads active Identity facts into the base system prompt block.
    pub async fn load_identity_into_system_prompt(&mut self, conn: &Connection) -> anyhow::Result<()> {
        let active_identities = crate::persistence::queries::fetch_all_active_identity(conn).await?;
        let facts = active_identities.into_iter().map(|f| f.fact).collect();
        self.set_identity_facts(facts);
        Ok(())
    }

    /// Resets conversational history and initializes a new session.
    pub fn new_session(&mut self, system_prompt: &str) {
        self.base_system_prompt = system_prompt.to_string();
        let assembled = self.assemble_system_prompt();
        let sys_msg = ChatMessage {
            role: Role::System,
            content: assembled.clone(),
            timestamp_ms: current_timestamp_ms(),
        };
        let sys_tokens = crate::services::memory::ml::estimate_tokens(&assembled);

        self.system_prompt = sys_msg.clone();
        self.buffer.reset(sys_msg);
        self.accountant.set_total_token_count(sys_tokens);
        self.session_compaction_contexts.clear();
        self.latest_compaction_facts.clear();
        self.cancel_opportunistic();
        log::info!("[ConversationManager] New session started. System prompt set.");
    }

    /// Constructs a chronological narrative context chain from session compactions up to token cap.
    pub fn build_narrative_context_chain(&self, soft_cap_tokens: usize) -> String {
        TokenAccountant::build_narrative_context_chain(
            &self.session_compaction_contexts,
            soft_cap_tokens,
        )
    }

    /// Replaces the active system prompt content and updates token calculations.
    pub fn update_system_prompt(&mut self, new_base_prompt: &str) {
        self.base_system_prompt = new_base_prompt.to_string();
        let assembled = self.assemble_system_prompt();
        if self.system_prompt.content != assembled {
            let old_sys_tokens =
                crate::services::memory::ml::estimate_tokens(&self.system_prompt.content);
            let sys_tokens = crate::services::memory::ml::estimate_tokens(&assembled);
            self.system_prompt.content = assembled.clone();
            if !self.buffer.messages.is_empty() && self.buffer.messages[0].role == Role::System {
                self.buffer.messages[0].content = assembled;
            }
            self.accountant.sub_tokens(old_sys_tokens);
            self.accountant.add_tokens(sys_tokens);
            self.buffer.kv_synced_index = 0;
        }
    }

    /// Appends a new user turn to working memory.
    pub fn push_user_turn(&mut self, text: String) {
        let tokens = crate::services::memory::ml::estimate_tokens(&text);
        self.buffer.push_user_turn(text);
        self.accountant.add_tokens(tokens);
        log::debug!(
            "[ConversationManager] User turn pushed. Total tokens: {} / {}",
            self.accountant.total_token_count(),
            self.accountant.max_context_tokens()
        );
    }

    /// Checks if the last message in working memory is a user turn with identical content.
    pub fn is_duplicate_user_turn(&self, text: &str) -> bool {
        self.buffer.is_duplicate_user_turn(text)
    }

    /// Appends a new assistant turn to working memory.
    pub fn push_assistant_turn(&mut self, text: String) {
        if text.trim().is_empty() {
            return;
        }
        let tokens = crate::services::memory::ml::estimate_tokens(&text);
        self.buffer.push_assistant_turn(text);
        self.accountant.add_tokens(tokens);
        log::debug!(
            "[ConversationManager] Assistant turn pushed. Total tokens: {} / {}. KV index: {}",
            self.accountant.total_token_count(),
            self.accountant.max_context_tokens(),
            self.buffer.kv_synced_index
        );
    }

    /// Removes the latest user turn if cancelled by barge-in.
    pub fn pop_last_user_turn(&mut self) {
        if let Some(msg) = self.buffer.pop_last_user_turn() {
            let tokens = crate::services::memory::ml::estimate_tokens(&msg.content);
            self.accountant.sub_tokens(tokens);
            log::info!("[ConversationManager] Popped pending user turn due to barge-in/cancel.");
        }
    }

    /// Computes percentage of usable context budget consumed by current conversation.
    pub fn context_utilization(&self) -> f32 {
        self.accountant.context_utilization()
    }

    /// Returns true if memory utilization has crossed the critical threshold.
    pub fn needs_threshold_maintenance(&self) -> bool {
        self.accountant.needs_threshold_maintenance()
    }

    /// Formats recent compaction narrative chain and facts into XML session history.
    pub fn build_session_history_xml(&self, soft_cap_tokens: usize) -> String {
        let narrative_chain = self.build_narrative_context_chain(soft_cap_tokens.max(50));
        build_session_history_xml(&narrative_chain, &self.latest_compaction_facts)
    }

    /// Consolidates session history XML into the root System Message.
    pub fn consolidate_system_message(&mut self, session_history: &str) {
        let mut total_tokens = self.accountant.total_token_count();
        consolidate_system_message(
            &mut self.buffer.messages,
            &self.system_prompt,
            session_history,
            &mut total_tokens,
        );
        self.accountant.set_total_token_count(total_tokens);
    }

    /// FIFO Sliding Window Shift: Drops oldest (User, Assistant) pairs until below soft threshold.
    pub fn perform_fifo_maintenance(&mut self) {
        self.accountant.perform_fifo_maintenance(&mut self.buffer);
    }

    /// Rebuilds message buffer and session compaction state from successful compaction result.
    pub fn apply_compaction_result(
        &mut self,
        result: &CompactionResult,
        last_user_turn: ChatMessage,
    ) -> HashMap<String, Vec<String>> {
        let context_summary = result.context_summary.trim().to_string();
        if !context_summary.is_empty() {
            self.session_compaction_contexts.push(context_summary);
        }

        let mut facts_9_col = result.personal_memory.clone();
        facts_9_col.remove(MemoryCollection::Narrative.as_str());
        facts_9_col.remove("Context");
        self.latest_compaction_facts = facts_9_col;

        let sys_tokens = crate::services::memory::ml::estimate_tokens(&self.system_prompt.content);
        let user_tokens = crate::services::memory::ml::estimate_tokens(&last_user_turn.content);

        self.buffer.messages = vec![self.system_prompt.clone(), last_user_turn];
        self.accountant
            .set_total_token_count(sys_tokens + user_tokens);
        self.buffer.kv_synced_index = 0;

        log::info!(
            "[ConversationManager] Compaction complete. Context rebuilt with 2 items ({} tokens, utilization {:.1}%). Total session compactions: {}.",
            self.accountant.total_token_count(),
            self.accountant.context_utilization() * 100.0,
            self.session_compaction_contexts.len()
        );

        result.diff_to_enqueue.clone()
    }

    /// Attempts to initiate an opportunistic background compaction when between soft and critical thresholds.
    pub fn try_trigger_opportunistic(
        &mut self,
    ) -> Option<(usize, Vec<ChatMessage>, Arc<AtomicBool>)> {
        if self.accountant.is_in_soft_compaction_window()
            && !self.opportunistic_active
            && self.buffer.messages.len() > 3
        {
            self.opportunistic_active = true;
            self.opportunistic_cancel = Arc::new(AtomicBool::new(false));
            log::info!(
                "[ConversationManager] Triggering Opportunistic Compaction candidate at {:.1}% utilization.",
                self.accountant.context_utilization() * 100.0
            );
            Some((
                self.buffer.messages.len(),
                self.buffer.messages.clone(),
                Arc::clone(&self.opportunistic_cancel),
            ))
        } else {
            None
        }
    }

    /// Commits opportunistic compaction results if no user turns were added during processing.
    pub fn commit_opportunistic(&mut self, snapshot_len: usize, summary_text: String) -> bool {
        if !self.opportunistic_active {
            log::info!("[ConversationManager] Commit rejected: Opportunistic compaction was inactive.");
            return false;
        }
        if self.opportunistic_cancel.load(Ordering::Relaxed) {
            self.opportunistic_active = false;
            log::info!("[ConversationManager] Commit rejected: Opportunistic compaction was cancelled.");
            return false;
        }
        if self.buffer.messages.len() != snapshot_len {
            self.opportunistic_active = false;
            log::info!(
                "[ConversationManager] Commit rejected: Race detected (expected {} items, current has {}).",
                snapshot_len,
                self.buffer.messages.len()
            );
            return false;
        }

        let last_user_turn = match self.buffer.messages.pop() {
            Some(turn) => turn,
            None => return false,
        };

        let summary_msg = ChatMessage {
            role: Role::System,
            content: format!("[Summary of prior context: {}]", summary_text),
            timestamp_ms: current_timestamp_ms(),
        };

        self.buffer.messages = vec![self.system_prompt.clone(), summary_msg, last_user_turn];

        let mut count = 0;
        for msg in &self.buffer.messages {
            count += crate::services::memory::ml::estimate_tokens(&msg.content);
        }
        self.accountant.set_total_token_count(count);
        self.buffer.kv_synced_index = 0;
        self.opportunistic_active = false;

        log::info!(
            "[ConversationManager] Opportunistic Compaction COMMITTED successfully! Utilization now {:.1}%.",
            self.accountant.context_utilization() * 100.0
        );

        true
    }

    /// Cancels active opportunistic compaction on speech detection.
    pub fn on_speech_start(&mut self) {
        self.cancel_opportunistic();
    }

    /// Aborts any running opportunistic compaction task.
    pub fn cancel_opportunistic(&mut self) {
        if self.opportunistic_active {
            self.opportunistic_cancel.store(true, Ordering::Relaxed);
            self.opportunistic_active = false;
            log::info!("[ConversationManager] Opportunistic compaction cancelled.");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Tests dynamic max_context_tokens budget mutation.
    #[test]
    fn test_set_max_context_tokens() {
        let mut cm = ConversationManager::new(2048);
        assert_eq!(cm.accountant.max_context_tokens(), 2048);
        cm.set_max_context_tokens(4096);
        assert_eq!(cm.accountant.max_context_tokens(), 4096);
        cm.set_max_context_tokens(0);
        assert_eq!(cm.accountant.max_context_tokens(), 4096);
    }

    /// Tests system prompt update while preserving identity facts structure.
    #[test]
    fn test_update_system_prompt_with_identity() {
        let mut cm = ConversationManager::new(2048);
        cm.identity_facts = vec!["User lives in Seattle.".to_string()];
        cm.update_system_prompt("You are Vox Assistant.");

        assert!(cm.system_prompt.content.starts_with("You are Vox Assistant."));
        assert!(cm.system_prompt.content.contains("<user_profile>\n[Identity]\n- User lives in Seattle.\n</user_profile>"));

        cm.new_session("You are a helpful coding assistant.");
        assert!(cm.system_prompt.content.starts_with("You are a helpful coding assistant."));
        assert!(cm.system_prompt.content.contains("<user_profile>\n[Identity]\n- User lives in Seattle.\n</user_profile>"));
        assert_eq!(cm.buffer.messages.len(), 1);
    }

    /// Tests safety of pop_last_user_turn during assistant greeting and normal user turns.
    #[test]
    fn test_pop_last_user_turn_safety() {
        let mut cm = ConversationManager::new(2048);
        cm.new_session("System prompt");
        assert_eq!(cm.buffer.messages.len(), 1);
        assert_eq!(cm.buffer.messages[0].role, Role::System);

        cm.pop_last_user_turn();
        assert_eq!(cm.buffer.messages.len(), 1);

        cm.push_assistant_turn("Hello, how can I help?".to_string());
        assert_eq!(cm.buffer.messages.len(), 2);
        assert_eq!(cm.buffer.messages[1].role, Role::Assistant);

        cm.pop_last_user_turn();
        assert_eq!(cm.buffer.messages.len(), 2);

        cm.push_user_turn("What is the weather?".to_string());
        assert_eq!(cm.buffer.messages.len(), 3);
        assert_eq!(cm.buffer.messages[2].role, Role::User);

        cm.pop_last_user_turn();
        assert_eq!(cm.buffer.messages.len(), 2);
        assert_eq!(cm.buffer.messages[1].role, Role::Assistant);
    }
}
