pub mod actor;
pub mod capabilities;
pub mod capability_probe;
pub mod llama_cpp;
pub mod policy;
pub mod probe;
pub mod providers;
pub mod types;

pub use actor::{spawn_llm_worker, LlmCommand};
pub use capabilities::{
    CapabilityObservation, CapabilityRegistry, CapabilitySource, ModelCapabilities,
};
pub use capability_probe::CapabilityProbeEngine;
pub use llama_cpp::LlmWorker;
pub use policy::GenerationPolicy;
pub use probe::ActiveProbeEngine;
pub use providers::{EmbeddedProvider, LlmProvider, OpenAiCompatProvider, ProviderKind};
pub use types::*;

// ─── LLM Model Constants ───────────────────────────────────────────────────
pub const MODEL_DIR_LLM: &str = "llm/llama";
pub const MODEL_FILE_LLM_GGUF: &str = "llama-3.2-1b-q4_k_m.gguf";
pub const MODEL_DIR_LLM_GEMMA: &str = "llm/gemma4";
pub const MODEL_FILE_LLM_GEMMA_GGUF: &str = "gemma-4-e2b-q4_k_m.gguf";

/// Large Language Model engine contract.
///
/// This is the lower-level interface for local GGUF inference via llama.cpp.
/// For the provider abstraction (which wraps local or remote), see `LlmProvider`.
use crate::services::memory::ConversationContext;

pub trait LlmEngine {
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
