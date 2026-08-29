pub mod actor;
pub mod catalog;
pub mod config;
pub mod embedded;
pub mod llama_cpp;
pub mod policy;
pub mod probe;
pub mod transport;
pub mod types;

pub use actor::{cool_down_llm, create_llm_provider, spawn_llm_worker, warm_up_llm, LlmCommand};
pub use catalog::{list_presets, lookup_preset, ProviderPresetMeta, PROVIDER_CATALOG};
pub use config::{AuthScheme, CapabilitySource, ConnectionConfig, TokenLimitField, TransportType};
pub use embedded::EmbeddedProvider;
pub use llama_cpp::LlmWorker;
pub use policy::GenerationPolicy;
pub use probe::CapabilityProbeEngine;
pub use transport::RemoteTransport;
pub use types::*;

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
use std::sync::atomic::AtomicBool;
use std::sync::mpsc;
use std::sync::Arc;

/// Common provider abstraction implemented by `EmbeddedProvider` and `RemoteTransport`.
pub trait LlmProvider: Send + Sync {
    /// Submits a provider-neutral generation request and streams tokens via `tx`.
    fn generate<'a>(
        &'a self,
        request: GenerationRequest,
        turn_id: u32,
        cancel_flag: &'a Arc<AtomicBool>,
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
        ctx: &crate::services::memory::ConversationContext,
        turn_id: u32,
        cancel_flag: &std::sync::Arc<std::sync::atomic::AtomicBool>,
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
