use std::sync::{mpsc, OnceLock};

use futures_util::future::BoxFuture;
use llama_cpp_4::llama_backend::LlamaBackend;
use serde::{Deserialize, Serialize};

use crate::{
    core::settings::LlmModelInfo,
    services::harness::{ChatMessage, ConversationContext},
};

/// Purpose of generation, allowing default policy selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GenerationPurpose {
    Conversation,
    MemoryCompaction,
    StructuredExtraction,
}

/// Reasoning effort switch for generation requests. Voice-native default is off.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ReasoningMode {
    Enabled,
    #[default]
    Disabled,
}

impl ReasoningMode {
    /// Maps a boolean reasoning opt-in flag to a provider-neutral reasoning mode.
    pub fn from_enabled(enabled: bool) -> Self {
        if enabled {
            Self::Enabled
        } else {
            Self::Disabled
        }
    }
}

/// Provider-neutral generation sampling and output length options.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct GenerationOptions {
    pub temperature: Option<f32>,
    pub top_p: Option<f32>,
    pub top_k: Option<u32>,
    pub max_output_tokens: Option<u32>,
    pub stop: Vec<String>,
    pub seed: Option<u64>,
    pub reasoning: ReasoningMode,
    pub context_window: Option<u32>,
}

/// Explicit constraint on LLM output format.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
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
    pub messages: Vec<ChatMessage>,
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

/// Identifies the runtime engine type of an LLM provider.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProviderKind {
    Embedded,
    OpenAiCompat,
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

    #[error("Engine error: {0}")]
    Engine(String),

    #[error("Provider error ({status}): {message}")]
    Provider { status: u16, message: String },

    #[error("Parse error: {0}")]
    Parse(String),

    #[error("Cancelled")]
    Cancelled,

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

/// Provider streaming events emitted during generation.
#[derive(Debug, Clone)]
pub enum LlmStreamEvent {
    Token(String),
    Finished,
}

/// Common provider abstraction implemented by `EmbeddedProvider` and `RemoteTransport`.
pub trait LlmProvider: Send + Sync {
    fn generate<'a>(
        &'a self,
        request: GenerationRequest,
        turn_id: u32,
        cancel: &'a tokio_util::sync::CancellationToken,
        tx: &'a mpsc::Sender<LlmStreamEvent>,
    ) -> BoxFuture<'a, Result<(), LlmError>>;

    fn capabilities(&self) -> &ProviderCapabilities;

    fn health_check<'a>(&'a self) -> BoxFuture<'a, Result<(), LlmError>>;

    fn list_models<'a>(&'a self) -> BoxFuture<'a, Result<Vec<LlmModelInfo>, LlmError>>;

    fn kind(&self) -> ProviderKind;
}

/// Large Language Model engine contract for lower-level FFI.
pub trait LlmEngine {
    fn generate(
        &self,
        ctx: &ConversationContext,
        turn_id: u32,
        options: &GenerationOptions,
        cancel: &tokio_util::sync::CancellationToken,
        tx: &mpsc::Sender<LlmStreamEvent>,
    ) -> anyhow::Result<()>;
}

/// Returns the process-wide llama.cpp backend singleton.
pub fn global_llama_backend() -> &'static LlamaBackend {
    static BACKEND: OnceLock<LlamaBackend> = OnceLock::new();
    BACKEND.get_or_init(|| {
        let mut b = LlamaBackend::init().expect("Failed to initialise global llama.cpp backend");
        b.void_logs();
        b
    })
}
