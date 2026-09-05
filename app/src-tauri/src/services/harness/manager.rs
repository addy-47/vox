use super::buffer::{current_timestamp_ms, ChatMessage, MessageBuffer, Role};
use super::prompt_builder::assemble_system_prompt;
use turso::Connection;

/// Orchestrates pure conversational turns and system prompt assembly.
pub struct ConversationManager {
    pub(crate) buffer: MessageBuffer,
    system_prompt: ChatMessage,
    base_system_prompt: String,
    identity_facts: Vec<String>,
    dynamic_user_profile: Option<String>,
}

impl ConversationManager {
    /// Creates a new ConversationManager instance.
    pub fn new() -> Self {
        let base_system_prompt = crate::core::constants::SYSTEM_PROMPT_MODULAR.to_string();
        let default_sys_prompt = ChatMessage {
            role: Role::System,
            content: base_system_prompt.clone(),
            timestamp_ms: current_timestamp_ms(),
        };

        let buffer = MessageBuffer::new(default_sys_prompt.clone());

        Self {
            buffer,
            system_prompt: default_sys_prompt,
            base_system_prompt,
            identity_facts: Vec::new(),
            dynamic_user_profile: None,
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
            self.system_prompt.content = assembled.clone();
            if !self.buffer.messages.is_empty() && self.buffer.messages[0].role == Role::System {
                self.buffer.messages[0].content = assembled;
            }
            self.buffer.kv_synced_index = 0;
        }
    }

    /// Sets active Identity facts and reassembles the system prompt, enforcing max personal context share.
    pub fn set_identity_facts(
        &mut self,
        identity_facts: Vec<String>,
        context_window: usize,
        max_context_share: f32,
    ) {
        let budget = ((context_window as f32) * max_context_share) as usize;
        let mut bounded_facts = Vec::new();
        let mut total_tokens = 0;

        // Bounded newest-first (facts are typically ordered chronological, reverse iterate to preserve freshest)
        for fact in identity_facts.into_iter().rev() {
            let tokens = crate::services::memory::ml::estimate_tokens(&fact);
            if total_tokens + tokens > budget && !bounded_facts.is_empty() {
                log::warn!(
                    "[ConversationManager] Identity facts reached budget cap ({} / {} tokens). Older facts truncated.",
                    total_tokens,
                    budget
                );
                break;
            }
            total_tokens += tokens;
            bounded_facts.push(fact);
        }
        bounded_facts.reverse();

        self.identity_facts = bounded_facts;
        let assembled = self.assemble_system_prompt();
        self.system_prompt.content = assembled.clone();
        if !self.buffer.messages.is_empty() && self.buffer.messages[0].role == Role::System {
            self.buffer.messages[0].content = assembled;
        }
        self.buffer.kv_synced_index = 0;
        log::info!(
            "[ConversationManager] Successfully preloaded {} Identity facts into System Prompt ({} tokens, budget {}).",
            self.identity_facts.len(),
            total_tokens,
            budget
        );
    }

    /// Preloads active Identity facts into the base system prompt block with token budgeting.
    pub async fn load_identity_into_system_prompt(
        &mut self,
        conn: &Connection,
        context_window: usize,
        max_context_share: f32,
    ) -> anyhow::Result<()> {
        let active_identities =
            crate::persistence::queries::fetch_all_active_identity(conn).await?;
        let facts = active_identities.into_iter().map(|f| f.fact).collect();
        self.set_identity_facts(facts, context_window, max_context_share);
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

        self.system_prompt = sys_msg.clone();
        self.buffer.reset(sys_msg);
        log::info!("[ConversationManager] New session started. System prompt set.");
    }

    /// Replaces the active system prompt content.
    pub fn update_system_prompt(&mut self, new_base_prompt: &str) {
        self.base_system_prompt = new_base_prompt.to_string();
        let assembled = self.assemble_system_prompt();
        if self.system_prompt.content != assembled {
            self.system_prompt.content = assembled.clone();
            if !self.buffer.messages.is_empty() && self.buffer.messages[0].role == Role::System {
                self.buffer.messages[0].content = assembled;
            }
            self.buffer.kv_synced_index = 0;
        }
    }

    /// Appends a new user turn to working memory.
    pub fn push_user_turn(&mut self, text: String) {
        self.buffer.push_user_turn(text);
        log::debug!(
            "[ConversationManager] User turn pushed. Total messages: {}",
            self.buffer.messages.len()
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
        self.buffer.push_assistant_turn(text);
        log::debug!(
            "[ConversationManager] Assistant turn pushed. Total messages: {}. KV index: {}",
            self.buffer.messages.len(),
            self.buffer.kv_synced_index
        );
    }

    /// Returns a reference to the active system prompt chat message.
    pub fn system_prompt(&self) -> &ChatMessage {
        &self.system_prompt
    }
}

impl Default for ConversationManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Tests system prompt update while preserving identity facts structure.
    #[test]
    fn test_update_system_prompt_with_identity() {
        let mut cm = ConversationManager::new();
        cm.identity_facts = vec!["User lives in Seattle.".to_string()];
        cm.update_system_prompt("You are Vox Assistant.");

        assert!(cm
            .system_prompt
            .content
            .starts_with("You are Vox Assistant."));
        assert!(cm
            .system_prompt
            .content
            .contains("<user_profile>\n[Identity]\n- User lives in Seattle.\n</user_profile>"));

        cm.new_session("You are a helpful coding assistant.");
        assert!(cm
            .system_prompt
            .content
            .starts_with("You are a helpful coding assistant."));
        assert!(cm
            .system_prompt
            .content
            .contains("<user_profile>\n[Identity]\n- User lives in Seattle.\n</user_profile>"));
        assert_eq!(cm.buffer.messages.len(), 1);
    }
}
