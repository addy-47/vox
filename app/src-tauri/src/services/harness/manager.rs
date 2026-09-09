use super::{
    buffer::{current_timestamp_ms, ChatMessage, MessageBuffer, Role},
    prompt_builder::assemble_system_prompt,
};
use crate::{
    core::constants::SYSTEM_PROMPT_MODULAR, persistence::sessions::TurnRow,
    services::memory::ml::estimate_tokens,
};

/// Orchestrates pure conversational turns, personal memory document injection, and system prompt assembly.
pub struct ConversationManager {
    pub(crate) buffer: MessageBuffer,
    system_prompt: ChatMessage,
    base_system_prompt: String,
    personal_memory: Option<String>,
    dynamic_user_profile: Option<String>,
}

impl ConversationManager {
    /// Creates a new ConversationManager instance.
    pub fn new() -> Self {
        let base_system_prompt = SYSTEM_PROMPT_MODULAR.to_string();
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
            personal_memory: None,
            dynamic_user_profile: None,
        }
    }

    /// Returns a slice view of active conversation messages.
    pub fn get_messages(&self) -> &[ChatMessage] {
        self.buffer.messages()
    }

    /// Assembles the complete system prompt from base prompt, personal memory markdown, and dynamic profile.
    pub fn assemble_system_prompt(&self) -> String {
        assemble_system_prompt(
            &self.base_system_prompt,
            self.personal_memory.as_deref(),
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

    /// Sets the personal memory document, truncating to budget if it exceeds `context_window * max_context_share`.
    pub fn set_personal_memory(
        &mut self,
        content: Option<String>,
        context_window: usize,
        max_context_share: f32,
    ) {
        let budget = ((context_window as f32) * max_context_share) as usize;
        if let Some(text) = content {
            let tokens = estimate_tokens(&text);
            if tokens > budget {
                log::warn!(
                    "[ConversationManager] Personal memory exceeds budget ({} / {} tokens). Truncating.",
                    tokens,
                    budget
                );
                let max_chars = budget.saturating_mul(4);
                let truncated = if text.len() > max_chars {
                    text.chars().take(max_chars).collect::<String>()
                } else {
                    text
                };
                self.personal_memory = Some(truncated);
            } else {
                self.personal_memory = Some(text);
            }
        } else {
            self.personal_memory = None;
        }

        let assembled = self.assemble_system_prompt();
        self.system_prompt.content = assembled.clone();
        if !self.buffer.messages.is_empty() && self.buffer.messages[0].role == Role::System {
            self.buffer.messages[0].content = assembled;
        }
        self.buffer.kv_synced_index = 0;
    }

    /// Sets identity facts by formatting as markdown bullets into personal memory.
    pub fn set_identity_facts(
        &mut self,
        identity_facts: Vec<String>,
        context_window: usize,
        max_context_share: f32,
    ) {
        if identity_facts.is_empty() {
            self.set_personal_memory(None, context_window, max_context_share);
        } else {
            let formatted = identity_facts
                .into_iter()
                .map(|f| format!("- {}", f))
                .collect::<Vec<_>>()
                .join("\n");
            self.set_personal_memory(Some(formatted), context_window, max_context_share);
        }
    }

    /// Synchronously restores session continuation context into working memory without async I/O.
    pub fn restore_session_continuation(
        &mut self,
        base_prompt: &str,
        personal_memory: Option<String>,
        latest_summary: Option<String>,
        turns: Vec<TurnRow>,
        context_window: usize,
        max_context_share: f32,
    ) {
        self.base_system_prompt = base_prompt.to_string();
        self.set_personal_memory(personal_memory, context_window, max_context_share);

        let assembled = self.assemble_system_prompt();
        let sys_msg = ChatMessage {
            role: Role::System,
            content: assembled.clone(),
            timestamp_ms: current_timestamp_ms(),
        };

        self.system_prompt = sys_msg.clone();
        self.buffer.reset(sys_msg);

        if let Some(summary) = latest_summary {
            if !summary.trim().is_empty() {
                self.buffer.push_assistant_turn(format!(
                    "<context_summary>\n{}\n</context_summary>",
                    summary.trim()
                ));
            }
        }

        for turn in turns {
            if !turn.user_text.trim().is_empty() {
                self.buffer.push_user_turn(turn.user_text);
            }
            if !turn.assistant_text.trim().is_empty() {
                self.buffer.push_assistant_turn(turn.assistant_text);
            }
        }
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

    /// Tests system prompt update while preserving personal memory structure.
    #[test]
    fn test_update_system_prompt_with_personal_memory() {
        let mut cm = ConversationManager::new();
        cm.personal_memory = Some("User lives in Seattle.".to_string());
        cm.update_system_prompt("You are Vox Assistant.");

        assert!(cm
            .system_prompt
            .content
            .starts_with("You are Vox Assistant."));
        assert!(cm
            .system_prompt
            .content
            .contains("<user_profile>\nUser lives in Seattle.\n</user_profile>"));

        cm.new_session("You are a helpful coding assistant.");
        assert!(cm
            .system_prompt
            .content
            .starts_with("You are a helpful coding assistant."));
        assert!(cm
            .system_prompt
            .content
            .contains("<user_profile>\nUser lives in Seattle.\n</user_profile>"));
        assert_eq!(cm.buffer.messages.len(), 1);
    }
}
