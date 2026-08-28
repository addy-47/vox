pub mod actor;
pub mod capability_probe;
pub mod llama_cpp;
pub mod policy;
pub mod providers;
pub mod types;

pub use actor::{cool_down_llm, create_llm_provider, spawn_llm_worker, warm_up_llm, LlmCommand};
pub use capability_probe::CapabilityProbeEngine;
pub use llama_cpp::LlmWorker;
pub use policy::GenerationPolicy;
pub use providers::{EmbeddedProvider, LlmProvider, OpenAiCompatProvider, ProviderKind};
pub use types::*;

pub const CTX_FLOOR_NON_EMBEDDED: u32 = 8_192;
pub const DEFAULT_CLOUD_MODEL_CTX: u32 = 1_000_000;
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

/// Large Language Model engine contract.
///
/// This is the lower-level interface for local GGUF inference via llama.cpp.
/// For the provider abstraction (which wraps local or remote), see `LlmProvider`.
use crate::services::memory::ConversationContext;

pub trait LlmEngine {
    /// Generates completion tokens for the conversation context and dispatches them via channel.
    fn generate(
        &self,
        ctx: &ConversationContext,
        turn_id: u32,
        cancel_flag: &std::sync::Arc<std::sync::atomic::AtomicBool>,
        tx: &std::sync::mpsc::Sender<crate::core::events::VoxEvent>,
    ) -> anyhow::Result<()>;
}

use llama_cpp_4::llama_backend::LlamaBackend;
use std::sync::OnceLock;

/// Returns the process-wide llama.cpp backend singleton.
///
/// Both [`LlmWorker`] and the NeuTTS TTS engine share this reference so
/// that `LlamaBackend::init()` is called exactly once per process.
pub fn global_llama_backend() -> &'static LlamaBackend {
    static BACKEND: OnceLock<LlamaBackend> = OnceLock::new();
    BACKEND.get_or_init(|| {
        let mut b = LlamaBackend::init().expect("Failed to initialise global llama.cpp backend");
        b.void_logs();
        b
    })
}
