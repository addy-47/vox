use std::{
    fmt,
    sync::{atomic::AtomicU32, mpsc, Arc},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use parking_lot::Mutex;
use tokio_util::sync::CancellationToken;

use crate::{
    core::{
        events::VoxEvent,
        state::{AppState, InteractionOwner},
    },
    persistence::VoxDb,
    pipeline::{assistant::TurnAccumulator, router::RoutingContext},
    services::{
        llm::{actor::LlmCommand, LlmProvider},
        translit::is_devanagari,
        tts::actor::TtsCommand,
    },
};

pub mod chassis;
pub mod r#loop;
pub mod stages;
pub mod steps;

pub use chassis::Harness;
pub use r#loop::execute_turn;
pub use stages::{
    budget::{ContextBudgetStage, ContextStatus},
    compaction::{CompactionParams, CompactionStage},
    history::ConversationHistoryStage,
    prompt::PromptBuilderStage,
    streaming::{ClauseChunker, StreamRoutingHandles, StreamRoutingStage},
    tools::{
        MemorySearchTool, RespondAndSetTitleTool, ToolDefinition, ToolError, ToolExecutionContext,
        ToolExecutionOutcome, ToolExecutor, ToolFilter, ToolRegistry, ToolResult,
    },
};
pub use steps::{
    enter_non_terminal_phase, step7_handle_cancelled as handle_turn_cancelled,
    CancelledTurnContext, NonTerminalContext, NonTerminalPhase, NonTerminalTrigger,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PipelineDomain {
    ModularAssistant,
    RealtimeS2S,
    Dictation,
}

/// Strongly typed terminal outcome of executing a conversational turn.
#[derive(Debug)]
pub enum TurnOutcome {
    Completed {
        turn_id: u32,
        assistant_response: String,
    },
    DuplicateIgnored {
        turn_id: u32,
    },
    Cancelled {
        turn_id: u32,
    },
    Error {
        turn_id: u32,
        message: String,
    },
}

/// Request parameters bundled for executing a conversational turn.
pub struct TurnExecutionRequest<R: tauri::Runtime> {
    pub query: String,
    pub turn_id: u32,
    pub owner: InteractionOwner,
    pub cancel: CancellationToken,
    pub routing_ctx: RoutingContext,
    pub app: tauri::AppHandle<R>,
    pub app_state: Arc<AppState>,
    pub db: Arc<VoxDb>,
    pub accumulator: Arc<Mutex<TurnAccumulator>>,
    pub tts_tx: Option<mpsc::Sender<TtsCommand>>,
    pub llm_tx: Option<mpsc::Sender<LlmCommand>>,
    pub pipeline_tx: Option<mpsc::Sender<VoxEvent>>,
    pub pending_synthesis_jobs: Arc<AtomicU32>,
    pub provider: Option<Arc<dyn LlmProvider>>,
}

pub const TOOL_EXECUTION_TIMEOUT: Duration = Duration::from_secs(10);

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

pub use crate::services::llm::{CanonicalToolCall, CanonicalToolDefinition, ToolFlow};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    System,
    User,
    Assistant,
    Tool,
}

impl fmt::Display for Role {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::System => write!(f, "system"),
            Self::User => write!(f, "user"),
            Self::Assistant => write!(f, "assistant"),
            Self::Tool => write!(f, "tool"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatMessage {
    pub role: Role,
    pub content: String,
    pub timestamp_ms: u64,
    pub tool_call_id: Option<String>,
    pub tool_calls: Option<Vec<CanonicalToolCall>>,
}

impl ChatMessage {
    pub fn new(role: Role, content: String) -> Self {
        Self {
            role,
            content,
            timestamp_ms: current_timestamp_ms(),
            tool_call_id: None,
            tool_calls: None,
        }
    }
}

impl Default for ChatMessage {
    fn default() -> Self {
        Self {
            role: Role::User,
            content: String::new(),
            timestamp_ms: 0,
            tool_call_id: None,
            tool_calls: None,
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

    pub fn extract<'a>(&self, text: &'a str) -> Option<&'a str> {
        let open = self.open_tag();
        let close = self.close_tag();
        let start = text.find(open)? + open.len();
        let end = text[start..].find(close)? + start;
        Some(&text[start..end])
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
