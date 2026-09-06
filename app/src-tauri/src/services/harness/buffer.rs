use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

/// Conversational message participant roles.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    System,
    User,
    Assistant,
}

impl std::fmt::Display for Role {
    /// Formats the role enum into its canonical lowercase string identifier.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Role::System => write!(f, "system"),
            Role::User => write!(f, "user"),
            Role::Assistant => write!(f, "assistant"),
        }
    }
}

/// A structured chat message within working memory.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: Role,
    pub content: String,
    pub timestamp_ms: u64,
}

/// Context snapshot ready for LLM generation.
#[derive(Debug, Clone)]
pub struct ConversationContext {
    pub messages: Vec<ChatMessage>,
    pub token_count: usize,
    pub kv_cache_index: usize,
}

/// Computes the current Unix timestamp in milliseconds.
pub fn current_timestamp_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

/// In-memory FIFO message sequence and KV cache tracking state.
#[derive(Debug, Clone)]
pub struct MessageBuffer {
    pub messages: Vec<ChatMessage>,
    pub kv_synced_index: usize,
}

impl MessageBuffer {
    /// Initializes a new message buffer with an initial system prompt message.
    pub fn new(default_system_msg: ChatMessage) -> Self {
        Self {
            messages: vec![default_system_msg],
            kv_synced_index: 0,
        }
    }

    /// Returns a slice view of active conversation messages.
    pub fn messages(&self) -> &[ChatMessage] {
        &self.messages
    }

    /// Appends a new user turn to the conversation buffer.
    pub fn push_user_turn(&mut self, text: String) {
        let msg = ChatMessage {
            role: Role::User,
            content: text,
            timestamp_ms: current_timestamp_ms(),
        };
        self.messages.push(msg);
    }

    /// Appends a new assistant turn and updates the synchronized KV-cache index.
    pub fn push_assistant_turn(&mut self, text: String) {
        if text.trim().is_empty() {
            return;
        }
        let msg = ChatMessage {
            role: Role::Assistant,
            content: text,
            timestamp_ms: current_timestamp_ms(),
        };
        self.messages.push(msg);
        self.kv_synced_index = self.messages.len();
    }

    /// Removes the latest user turn if currently trailing the conversation buffer.
    pub fn pop_last_user_turn(&mut self) -> Option<ChatMessage> {
        if let Some(last) = self.messages.last() {
            if last.role == Role::User {
                return self.messages.pop();
            }
        }
        None
    }

    /// Checks if the last message in the buffer is a user turn with identical content.
    pub fn is_duplicate_user_turn(&self, text: &str) -> bool {
        self.messages
            .last()
            .map(|m| m.role == Role::User && m.content == text)
            .unwrap_or(false)
    }

    /// Resets the buffer with a fresh system prompt message.
    pub fn reset(&mut self, system_msg: ChatMessage) {
        self.messages = vec![system_msg];
        self.kv_synced_index = 0;
    }
}
