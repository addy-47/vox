use std::{
    fmt,
    time::{SystemTime, UNIX_EPOCH},
};

use crate::services::translit::is_devanagari;

pub mod plugins;
pub mod session;
pub mod watcher;

pub use plugins::{
    budget::{ContextBudgetPlugin, ContextStatus},
    compaction::{CompactionParams, CompactionPlugin},
    history::ConversationHistoryPlugin,
    prompt::PromptBuilderPlugin,
    stream::{StreamRoutingHandles, StreamRoutingPlugin},
};
pub use session::{HarnessSession, PipelineDomain, TurnPreparation};
pub use watcher::QuietCompactionWatcher;

pub const TRANSITION_MESSAGES_EN: &[&str] = &[
    "Give me a moment to gather my thoughts.",
    "Let me think about that for a second.",
    "One moment while I process that.",
    "Thinking through this, just a second.",
    "Just a second, reviewing what we have.",
    "Let me check on that for you.",
    "Organizing my notes, one moment.",
    "Working on that right now.",
    "Let me pull together the details.",
    "Give me just a second here.",
];

pub const TRANSITION_MESSAGES_HI: &[&str] = &[
    "एक पल रुकिए, मैं सोच रहा हूँ।",
    "ज़रा सा समय दीजिए, मैं समझ रहा हूँ।",
    "एक क्षण रुकिए, मैं इसकी समीक्षा कर रहा हूँ।",
    "बस एक सेकंड, मैं देखता हूँ।",
    "मैं इस पर विचार कर रहा हूँ, एक पल।",
    "रुकिए, मैं इसकी जानकारी एकत्र कर रहा हूँ।",
    "एक पल दीजिए, मैं सब व्यवस्थित कर रहा हूँ।",
    "मैं अभी इस पर काम कर रहा हूँ।",
    "ज़रा रुकिए, मैं विवरण निकालता हूँ।",
    "बस एक सेकंड रुकिए।",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    System,
    User,
    Assistant,
}

impl fmt::Display for Role {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::System => write!(f, "system"),
            Self::User => write!(f, "user"),
            Self::Assistant => write!(f, "assistant"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatMessage {
    pub role: Role,
    pub content: String,
    pub timestamp_ms: u64,
}

impl ChatMessage {
    pub fn new(role: Role, content: String) -> Self {
        Self {
            role,
            content,
            timestamp_ms: current_timestamp_ms(),
        }
    }

    pub fn with_timestamp(role: Role, content: String, timestamp_ms: u64) -> Self {
        Self {
            role,
            content,
            timestamp_ms,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ConversationContext {
    pub messages: Vec<ChatMessage>,
    pub token_count: usize,
    pub kv_cache_index: usize,
}

pub fn current_timestamp_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PromptTag {
    UserIdentity,
    SessionContext,
    PastTurns,
}

impl PromptTag {
    pub fn open_tag(&self) -> &'static str {
        match self {
            Self::UserIdentity => "<user_identity>",
            Self::SessionContext => "<session_context>",
            Self::PastTurns => "<past_turns>",
        }
    }

    pub fn close_tag(&self) -> &'static str {
        match self {
            Self::UserIdentity => "</user_identity>",
            Self::SessionContext => "</session_context>",
            Self::PastTurns => "</past_turns>",
        }
    }

    pub fn wrap(&self, content: &str) -> String {
        format!("{}{}{}", self.open_tag(), content, self.close_tag())
    }
}

pub fn select_filler_phrase(query: &str, turn_id: u32) -> &'static str {
    let has_devanagari = is_devanagari(query);
    let catalog = if has_devanagari {
        TRANSITION_MESSAGES_HI
    } else {
        TRANSITION_MESSAGES_EN
    };
    let index = (turn_id as usize) % catalog.len();
    catalog[index]
}
