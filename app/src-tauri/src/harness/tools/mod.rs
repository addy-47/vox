use std::sync::Arc;

use futures_util::future::BoxFuture;
use serde_json::Value;

use crate::{
    core::{events::PipelineMode, state::AppState},
    services::llm::CanonicalToolDefinition,
};

pub mod bash;
pub mod executor;
pub mod manage_task;
pub mod registry;
pub mod search_memory;
pub mod set_title;
pub mod web_search;

pub use bash::BashTool;
pub use executor::{ToolExecutionOutcome, ToolExecutor};
pub use manage_task::ManageTaskTool;
pub use registry::{ToolFilter, ToolRegistry};
pub use search_memory::MemorySearchTool;
pub use set_title::{SetSessionTitleTool, SetTitleTool};
pub use web_search::WebSearchTool;

/// Concurrency mode declaring whether a tool can execute in parallel or requires an exclusive barrier.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolExecutionMode {
    /// Read-only or idempotent operations safe to run concurrently with other Parallel calls.
    Parallel,
    /// Mutating, stateful, or control-flow operations that form strict sequential barriers.
    Exclusive,
}

/// Truncation policy declaring how oversized observations should be sliced and spilled.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TruncationPolicy {
    /// Never truncate (used by metadata tools like set_title).
    None,
    /// Preserves head and tail slices based on head_ratio (e.g. 0.8 for search_memory and web_search).
    HeadTail { head_ratio: f32 },
    /// Preserves tail-heavy slice (e.g. 0.8 for terminal commands and test logs).
    TailHeavy { tail_ratio: f32 },
}

/// Control flow directive returned by cognitive tools to alter ReAct loop iteration.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum ToolControlAction {
    /// Normal continuation of ReAct loop.
    #[default]
    Continue,
    /// Suspends ReAct cognitive loop to await user clarification or input.
    SuspendForUser {
        question: String,
        options: Option<Vec<String>>,
    },
}

/// Operational domain classification defining where a tool can be utilized.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum ToolDomain {
    Modular,
    Realtime,
    #[default]
    All,
}

impl ToolDomain {
    /// Evaluates whether this tool domain is eligible for the active pipeline mode.
    pub fn matches(&self, mode: PipelineMode) -> bool {
        match self {
            ToolDomain::All => true,
            ToolDomain::Modular => mode == PipelineMode::Modular,
            ToolDomain::Realtime => mode == PipelineMode::Realtime,
        }
    }
}

/// Execution context passed to cognitive tools during turn evaluation.
#[derive(Clone)]
pub struct ToolExecutionContext {
    pub app_state: Arc<AppState>,
    pub session_id: i64,
    pub turn_id: u32,
    pub step_id: String,
    pub step_index: u32,
    pub cancel: tokio_util::sync::CancellationToken,
    pub on_sessions_changed: Option<Arc<dyn Fn() + Send + Sync>>,
    pub max_observation_tokens: Option<usize>,
    pub auto_promote_timeout: std::time::Duration,
    pub foreground_hard_timeout: std::time::Duration,
}

impl ToolExecutionContext {
    /// Constructs a standard execution context with no observation token budget constraint.
    pub fn new(
        app_state: Arc<AppState>,
        session_id: i64,
        turn_id: u32,
        cancel: tokio_util::sync::CancellationToken,
        on_sessions_changed: Option<Arc<dyn Fn() + Send + Sync>>,
    ) -> Self {
        Self {
            app_state,
            session_id,
            turn_id,
            step_id: String::new(),
            step_index: 0,
            cancel,
            on_sessions_changed,
            max_observation_tokens: None,
            auto_promote_timeout: crate::harness::chassis::HARNESS_AUTO_PROMOTE_TIMEOUT,
            foreground_hard_timeout: crate::harness::chassis::HARNESS_FOREGROUND_HARD_TIMEOUT,
        }
    }

    /// Attaches promotion timeouts to the execution context.
    pub fn with_promotion_timeouts(
        mut self,
        auto_promote: std::time::Duration,
        hard_ceiling: std::time::Duration,
    ) -> Self {
        self.auto_promote_timeout = auto_promote;
        self.foreground_hard_timeout = hard_ceiling;
        self
    }

    /// Attaches step tracking metadata to the execution context.
    pub fn with_step(mut self, step_id: impl Into<String>, step_index: u32) -> Self {
        self.step_id = step_id.into();
        self.step_index = step_index;
        self
    }

    /// Attaches an observation token budget ceiling (used by Modular retrieval tools).
    pub fn with_observation_budget(mut self, max_tokens: usize) -> Self {
        self.max_observation_tokens = Some(max_tokens);
        self
    }
    /// Checks if a non-placeholder title is already persisted for the active session.
    pub async fn is_title_already_set(&self) -> bool {
        if let Ok(conn) = self.app_state.db.connect() {
            if let Ok(Some(session)) =
                crate::persistence::sessions::fetch_session_by_id(&conn, self.session_id).await
            {
                if let Some(ref title) = session.title {
                    let trimmed = title.trim();
                    return !trimmed.is_empty()
                        && trimmed != "Untitled Session"
                        && trimmed != "New Session";
                }
            }
        }
        false
    }
}

/// Normalized result of an executed cognitive tool.
#[derive(Debug, Clone)]
pub struct ToolResult {
    pub content: String,
    pub control: ToolControlAction,
    pub truncated: bool,
    pub spillover_path: Option<String>,
}

impl ToolResult {
    /// Constructs a basic tool result with plain string observation content.
    pub fn new(content: impl Into<String>) -> Self {
        Self {
            content: content.into(),
            control: ToolControlAction::Continue,
            truncated: false,
            spillover_path: None,
        }
    }

}

/// Normalized errors encountered during tool execution.
#[derive(Debug, thiserror::Error)]
pub enum ToolError {
    #[error("Tool execution failed: {0}")]
    ExecutionFailed(String),

    #[error("Invalid tool arguments: {0}")]
    InvalidArguments(String),

    #[error("Tool execution timed out")]
    Timeout,

    #[error("Tool execution cancelled")]
    Cancelled,

    #[error("Tool '{0}' does not support pipeline mode {1:?}")]
    UnsupportedDomain(String, PipelineMode),
}

/// Object-safe definition contract implemented by all cognitive tools in the Vox runtime.
pub trait ToolDefinition: Send + Sync {
    /// Canonical identifier of the tool matching the model invocation name.
    fn name(&self) -> &str;

    /// Declares the operational domain affinity for this tool. Defaults to Universal (`ToolDomain::All`).
    fn domain(&self) -> ToolDomain {
        ToolDomain::All
    }

    /// Plain-language description exposed in model system prompt / function declaration.
    fn description(&self, mode: PipelineMode) -> &str;

    /// JSON schema describing the required and optional arguments for this tool in the active mode.
    fn parameters_schema(&self, mode: PipelineMode) -> Value;

    /// Concurrency mode declaring whether this tool can run in parallel waves or forms an exclusive barrier.
    fn execution_mode(&self, _args: &Value) -> ToolExecutionMode {
        ToolExecutionMode::Parallel
    }

    /// Whether this tool manages its own internal timeout promotion (e.g. bash auto-backgrounding).
    fn handles_own_timeout(&self) -> bool {
        false
    }

    /// Whether this tool supports background promotion if execution takes time.
    fn supports_background_promotion(&self) -> bool {
        false
    }

    /// Declares the observation truncation policy for this tool. Defaults to HeadTail (0.8).
    fn truncation_policy(&self) -> TruncationPolicy {
        TruncationPolicy::HeadTail { head_ratio: 0.8 }
    }

    /// Executes the tool asynchronously, returning a structured observation or error.
    fn execute<'a>(
        &'a self,
        mode: PipelineMode,
        args: Value,
        ctx: &'a ToolExecutionContext,
    ) -> BoxFuture<'a, Result<ToolResult, ToolError>>;

    /// Produces a provider-neutral canonical tool definition for model declaration in the active mode.
    fn to_canonical(&self, mode: PipelineMode) -> CanonicalToolDefinition {
        CanonicalToolDefinition {
            name: self.name().to_string(),
            description: self.description(mode).to_string(),
            parameters: self.parameters_schema(mode),
        }
    }
}
