use std::sync::Arc;

use futures_util::future::BoxFuture;
use serde_json::Value;

use crate::{
    core::{
        settings::PipelineMode,
        state::AppState,
    },
    services::llm::{CanonicalToolDefinition, ToolFlow},
};

pub mod executor;
pub mod registry;
pub mod respond_and_set_title;
pub mod search_memory;

pub use executor::{ToolExecutionOutcome, ToolExecutor};
pub use registry::{ToolFilter, ToolRegistry};
pub use respond_and_set_title::{RespondAndSetTitleTool, SetSessionTitleTool};
pub use search_memory::MemorySearchTool;

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
pub struct ToolExecutionContext {
    pub app_state: Arc<AppState>,
    pub session_id: i64,
    pub turn_id: u32,
    pub cancel: tokio_util::sync::CancellationToken,
    pub on_sessions_changed: Option<Arc<dyn Fn() + Send + Sync>>,
}

impl ToolExecutionContext {
    /// Checks if a non-placeholder title is already persisted for the active session.
    pub async fn is_title_already_set(&self) -> bool {
        if let Ok(conn) = self.app_state.db.connect() {
            if let Ok(Some(session)) = crate::persistence::sessions::fetch_session_by_id(&conn, self.session_id).await {
                if let Some(ref title) = session.title {
                    let trimmed = title.trim();
                    return !trimmed.is_empty() && trimmed != "Untitled Session" && trimmed != "New Session";
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
    pub spoken_filler: Option<String>,
    pub spoken_response: Option<String>,
}

impl ToolResult {
    /// Constructs a basic tool result with plain string observation content.
    pub fn new(content: impl Into<String>) -> Self {
        Self {
            content: content.into(),
            spoken_filler: None,
            spoken_response: None,
        }
    }

    /// Attaches a terminal spoken response to be synthesized directly for the user.
    pub fn with_spoken_response(mut self, spoken_response: impl Into<String>) -> Self {
        self.spoken_response = Some(spoken_response.into());
        self
    }

    /// Attaches an interim spoken filler to eliminate conversational dead air.
    pub fn with_spoken_filler(mut self, spoken_filler: impl Into<String>) -> Self {
        self.spoken_filler = Some(spoken_filler.into());
        self
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

    /// Behavioral flow category governing state transitions and speech delivery.
    fn flow(&self) -> ToolFlow;

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
            flow: self.flow(),
        }
    }
}
