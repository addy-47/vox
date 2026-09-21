pub mod actor;
pub mod catalog;
pub mod embedded;
pub mod factory;
pub mod provider;
pub mod transport;

pub use actor::{
    cool_down_llm, spawn_llm_worker, warm_up_llm, GenerationDefaults, GenerationPolicy, LlmCommand,
    LlmWarmUpHandles,
};
pub use catalog::{
    list_models, list_presets, lookup_preset, probe_capabilities, provider_catalog,
    CapabilityProbeEngine, CapabilityProvenance, ModelProbeResult, ModelSpec, ProviderPresetMeta,
};
pub use embedded::{EmbeddedProvider, LlmWorker, ModelFamily};
pub use factory::{create_llm_provider, create_llm_provider_from_llm_settings};
pub use provider::{
    global_llama_backend, CanonicalToolCall, CanonicalToolDefinition, ConversationInput,
    GenerationOptions, GenerationPurpose, GenerationRequest, LlmEngine, LlmError, LlmProvider,
    LlmStreamEvent, OutputConstraint, ProviderCapabilities, ProviderKind, ReasoningMode, Support,
    ToolFlow,
};
pub use transport::{
    AuthScheme, CapabilitySource, ConnectionConfig, RemoteTransport, TokenLimitField, TransportType,
};

pub const QWEN_MODEL_DIR: &str = "llm/qwen";
pub const QWEN_MODEL_FILE: &str = "qwen-3.5-0.8b-q4_k_m.gguf";
pub const GEMMA_MODEL_DIR: &str = "llm/gemma4";
pub const GEMMA_MODEL_FILE: &str = "gemma-4-e2b-q4_k_m.gguf";

pub const DEFAULT_BATCH_CHUNK_SIZE: usize = 512;
pub const DEFAULT_MAX_GENERATION_SAFETY_TOKENS: usize = 512;
pub const DEFAULT_CLIENT_CONNECT_TIMEOUT_SECS: u64 = 5;
pub const DEFAULT_CLIENT_REQUEST_TIMEOUT_SECS: u64 = 180;
