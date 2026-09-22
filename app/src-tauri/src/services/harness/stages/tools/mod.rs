use std::sync::Arc;

use futures_util::future::BoxFuture;
use serde_json::Value;

use crate::{
    core::state::AppState,
    services::llm::{CanonicalToolDefinition, ToolFlow},
};

pub mod executor;
pub mod memory;
pub mod registry;
pub mod title;

pub use executor::{ToolExecutionOutcome, ToolExecutor};
pub use memory::MemorySearchTool;
pub use registry::{ToolFilter, ToolRegistry};
pub use title::RespondAndSetTitleTool;

/// Execution context passed to cognitive tools during turn evaluation.
pub struct ToolExecutionContext {
    pub app_state: Arc<AppState>,
    pub session_id: i64,
    pub turn_id: u32,
    pub cancel: tokio_util::sync::CancellationToken,
    pub on_sessions_changed: Option<Arc<dyn Fn() + Send + Sync>>,
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
}

/// Object-safe definition contract implemented by all cognitive tools in the Vox runtime.
pub trait ToolDefinition: Send + Sync {
    /// Canonical identifier of the tool matching the model invocation name.
    fn name(&self) -> &str;

    /// Plain-language description exposed in model system prompt / function declaration.
    fn description(&self) -> &str;

    /// JSON schema describing the required and optional arguments for this tool.
    fn parameters_schema(&self) -> Value;

    /// Behavioral flow category governing state transitions and speech delivery.
    fn flow(&self) -> ToolFlow;

    /// Executes the tool asynchronously, returning a structured observation or error.
    fn execute<'a>(
        &'a self,
        args: Value,
        ctx: &'a ToolExecutionContext,
    ) -> BoxFuture<'a, Result<ToolResult, ToolError>>;

    /// Produces a provider-neutral canonical tool definition for model declaration.
    fn to_canonical(&self) -> CanonicalToolDefinition {
        CanonicalToolDefinition {
            name: self.name().to_string(),
            description: self.description().to_string(),
            parameters: self.parameters_schema(),
            flow: self.flow(),
        }
    }
}
