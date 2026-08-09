use serde::{Deserialize, Serialize};

/// Purpose of generation, allowing default policy selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GenerationPurpose {
    Conversation,
    MemoryCompaction,
    StructuredExtraction,
}

/// Provider-neutral generation sampling and output length options.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[derive(Default)]
pub struct GenerationOptions {
    pub temperature: Option<f32>,
    pub top_p: Option<f32>,
    pub top_k: Option<u32>,
    pub max_output_tokens: Option<u32>,
    pub stop: Vec<String>,
    pub seed: Option<u64>,
}


/// Explicit constraint on LLM output format.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[derive(Default)]
pub enum OutputConstraint {
    #[default]
    Text,
    JsonObject,
    JsonSchema {
        name: String,
        schema: serde_json::Value,
        strict: bool,
    },
}


/// Neutral container for input messages.
#[derive(Debug, Clone)]
pub struct ConversationInput {
    pub messages: Vec<crate::services::memory::ChatMessage>,
}

/// Provider-neutral generation request payload.
#[derive(Debug, Clone)]
pub struct GenerationRequest {
    pub input: ConversationInput,
    pub options: GenerationOptions,
    pub output: OutputConstraint,
    pub purpose: GenerationPurpose,
}

/// Feature support classification for capabilities.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Support {
    Supported,
    Unsupported,
    Unknown,
}

/// Capability matrix for an LLM provider/backend.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderCapabilities {
    pub temperature: Support,
    pub top_p: Support,
    pub top_k: Support,
    pub max_output_tokens: Support,
    pub json_object: Support,
    pub json_schema: Support,
    pub streaming: Support,
    pub seed: Support,
}

impl Default for ProviderCapabilities {
    fn default() -> Self {
        Self {
            temperature: Support::Supported,
            top_p: Support::Supported,
            top_k: Support::Unknown,
            max_output_tokens: Support::Supported,
            json_object: Support::Supported,
            json_schema: Support::Supported,
            streaming: Support::Supported,
            seed: Support::Unknown,
        }
    }
}

/// Structured, normalized errors produced by LLM providers.
#[derive(Debug, thiserror::Error)]
pub enum LlmError {
    #[error("Authentication failed")]
    Authentication,

    #[error("Model not found")]
    ModelNotFound,

    #[error("Invalid request: {0}")]
    InvalidRequest(String),

    #[error("Unsupported parameter: {parameter}")]
    UnsupportedParameter { parameter: String },

    #[error("Context limit exceeded")]
    ContextLimitExceeded,

    #[error("Rate limited")]
    RateLimited,

    #[error("Timeout")]
    Timeout,

    #[error("Transport error: {0}")]
    Transport(String),

    #[error("Provider error ({status}): {message}")]
    Provider { status: u16, message: String },

    #[error("Parse error: {0}")]
    Parse(String),

    #[error("Cancelled")]
    Cancelled,

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}
