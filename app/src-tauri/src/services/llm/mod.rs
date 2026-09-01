pub mod actor;
pub mod catalog;
pub mod embedded;
pub mod probe;
pub mod transport;

pub use actor::{
    cool_down_llm, create_llm_provider, spawn_llm_worker, warm_up_llm, GenerationDefaults,
    GenerationPolicy, LlmCommand,
};
pub use catalog::{list_presets, lookup_preset, ProviderPresetMeta, PROVIDER_CATALOG};
pub use embedded::{EmbeddedProvider, LlmWorker, ModelFamily};
pub use probe::{CapabilityProbeEngine, ModelProbeResult};
pub use transport::{
    AuthScheme, CapabilitySource, ConnectionConfig, RemoteTransport, TokenLimitField, TransportType,
};

pub const QWEN_MODEL_DIR: &str = "llm/qwen";
pub const QWEN_MODEL_FILE: &str = "qwen-3.5-0.8b-q4_k_m.gguf";
pub const GEMMA_MODEL_DIR: &str = "llm/gemma4";
pub const GEMMA_MODEL_FILE: &str = "gemma-4-e2b-q4_k_m.gguf";

pub const DEFAULT_MAX_CONTEXT_TOKENS: usize = 2048;
pub const DEFAULT_BATCH_CHUNK_SIZE: usize = 512;
pub const DEFAULT_MAX_GENERATION_SAFETY_TOKENS: usize = 512;
pub const DEFAULT_PROBE_TIMEOUT_SECS: u64 = 12;
pub const DEFAULT_VALIDATION_TIMEOUT_SECS: u64 = 6;
pub const DEFAULT_CLIENT_CONNECT_TIMEOUT_SECS: u64 = 5;
pub const DEFAULT_CLIENT_REQUEST_TIMEOUT_SECS: u64 = 180;
pub const DEFAULT_STREAM_CHUNK_TIMEOUT_MS: u64 = 150;
pub const DEFAULT_CANCEL_POLL_INTERVAL_MS: u64 = 50;
pub const DEFAULT_PROBE_MAX_TOKENS: u32 = 40;
pub const DEFAULT_TOOL_PROBE_MAX_TOKENS: u32 = 80;
pub const DEFAULT_PROBE_TEMPERATURE: f32 = 0.1;

use crate::core::events::VoxEvent;
use crate::core::settings::LlmModelInfo;
use futures_util::future::BoxFuture;
use serde::{Deserialize, Serialize};
use std::sync::mpsc;

/// Purpose of generation, allowing default policy selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GenerationPurpose {
    Conversation,
    MemoryCompaction,
    StructuredExtraction,
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
    pub messages: Vec<crate::services::harness::ChatMessage>,
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

/// Common provider abstraction implemented by `EmbeddedProvider` and `RemoteTransport`.
pub trait LlmProvider: Send + Sync {
    /// Submits a provider-neutral generation request and streams tokens via `tx`.
    fn generate<'a>(
        &'a self,
        request: GenerationRequest,
        turn_id: u32,
        cancel: &'a tokio_util::sync::CancellationToken,
        tx: &'a mpsc::Sender<VoxEvent>,
    ) -> BoxFuture<'a, Result<(), LlmError>>;

    /// Returns static capabilities of this provider.
    fn capabilities(&self) -> &ProviderCapabilities;

    /// Returns true if the provider is healthy and reachable.
    fn health_check<'a>(&'a self) -> BoxFuture<'a, Result<(), LlmError>>;

    /// Returns list of model IDs the provider can serve.
    fn list_models<'a>(&'a self) -> BoxFuture<'a, Result<Vec<LlmModelInfo>, LlmError>>;

    /// Identifies the runtime engine type of this LLM provider.
    fn kind(&self) -> ProviderKind;

    /// Maximum supported context size in tokens.
    fn max_context_tokens(&self) -> usize {
        DEFAULT_MAX_CONTEXT_TOKENS
    }
}

/// Large Language Model engine contract for lower-level FFI.
pub trait LlmEngine {
    /// Generates completion tokens for the conversation context and dispatches them via channel.
    fn generate(
        &self,
        ctx: &crate::services::harness::ConversationContext,
        turn_id: u32,
        cancel: &tokio_util::sync::CancellationToken,
        tx: &std::sync::mpsc::Sender<crate::core::events::VoxEvent>,
    ) -> anyhow::Result<()>;
}

use llama_cpp_4::llama_backend::LlamaBackend;
use std::sync::OnceLock;

/// Returns the process-wide llama.cpp backend singleton.
pub fn global_llama_backend() -> &'static LlamaBackend {
    static BACKEND: OnceLock<LlamaBackend> = OnceLock::new();
    BACKEND.get_or_init(|| {
        let mut b = LlamaBackend::init().expect("Failed to initialise global llama.cpp backend");
        b.void_logs();
        b
    })
}
