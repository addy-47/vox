use crate::services::harness::{ChatMessage, Role};

/// Plugin managing conversation turn history, KV-cache synchronization, and turn rollback.
#[derive(Debug, Clone, Default)]
pub struct ConversationHistoryPlugin {
    messages: Vec<ChatMessage>,
    kv_synced_index: usize,
}

impl ConversationHistoryPlugin {
    /// Creates a new empty `ConversationHistoryPlugin` instance.
    pub fn new() -> Self {
        Self {
            messages: Vec::new(),
            kv_synced_index: 0,
        }
    }

    /// Initializes a `ConversationHistoryPlugin` with an active system prompt message.
    pub fn with_system_prompt(system_prompt: String) -> Self {
        Self {
            messages: vec![ChatMessage::new(Role::System, system_prompt)],
            kv_synced_index: 0,
        }
    }

    /// Returns a slice of active conversation messages.
    pub fn messages(&self) -> &[ChatMessage] {
        &self.messages
    }

    /// Returns the count of messages currently synchronized with the model's KV-cache.
    pub fn kv_synced_index(&self) -> usize {
        self.kv_synced_index
    }

    /// Updates the index of messages synchronized with the model's KV-cache.
    pub fn set_kv_synced_index(&mut self, idx: usize) {
        self.kv_synced_index = idx.min(self.messages.len());
    }

    /// Checks if the last message in working memory is a user turn with identical text.
    pub fn is_duplicate_user_turn(&self, text: &str) -> bool {
        self.messages
            .last()
            .map(|m| m.role == Role::User && m.content.trim() == text.trim())
            .unwrap_or(false)
    }

    /// Appends a new user turn to working history, ignoring immediate duplicate turns.
    pub fn push_user_turn(&mut self, text: String) {
        if self.is_duplicate_user_turn(&text) {
            log::debug!(
                "[Harness::History] Dropping duplicate user turn: '{}'",
                text
            );
            return;
        }

        let msg = ChatMessage::new(Role::User, text);
        self.messages.push(msg);
        log::debug!(
            "[Harness::History] User turn pushed. Total messages: {}",
            self.messages.len()
        );
    }

    /// Appends a new assistant response turn and updates the KV-cache index.
    pub fn push_assistant_turn(&mut self, text: String) {
        if text.trim().is_empty() {
            return;
        }

        let msg = ChatMessage::new(Role::Assistant, text);
        self.messages.push(msg);
        self.kv_synced_index = self.messages.len();
        log::debug!(
            "[Harness::History] Assistant turn pushed. Total messages: {}. KV index: {}",
            self.messages.len(),
            self.kv_synced_index
        );
    }

    /// Rolls back the most recent assistant turn if interrupted before completion.
    pub fn rollback_last_assistant_turn(&mut self) -> Option<ChatMessage> {
        if let Some(last) = self.messages.last() {
            if last.role == Role::Assistant {
                let popped = self.messages.pop();
                self.kv_synced_index = self.messages.len();
                log::info!("[Harness::History] Interrupted assistant turn rolled back");
                return popped;
            }
        }
        None
    }

    /// Resets the conversation history with a fresh system prompt.
    pub fn reset(&mut self, system_prompt: String) {
        self.messages = vec![ChatMessage::new(Role::System, system_prompt)];
        self.kv_synced_index = 0;
        log::info!("[Harness::History] History reset with new system prompt");
    }

    /// Clears all messages and resets the KV-cache index.
    pub fn clear(&mut self) {
        self.messages.clear();
        self.kv_synced_index = 0;
    }

    /// Truncates oldest non-system dialogue turns by the given count.
    pub fn truncate_oldest_turns(&mut self, count: usize) {
        if count == 0 || self.messages.len() <= 1 {
            return;
        }

        let has_system = self
            .messages
            .first()
            .map(|m| m.role == Role::System)
            .unwrap_or(false);
        let start_idx = if has_system { 1 } else { 0 };
        let drain_count = count.min(self.messages.len().saturating_sub(start_idx));

        if drain_count > 0 {
            self.messages.drain(start_idx..start_idx + drain_count);
            self.kv_synced_index = 0;
            log::info!("[Harness::History] Truncated {} oldest turns", drain_count);
        }
    }
}
