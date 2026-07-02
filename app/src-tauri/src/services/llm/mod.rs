pub mod actor;
pub mod capability_probe;
pub mod llama_cpp;
pub mod providers;

pub use actor::{spawn_llm_worker, LlmCommand};
pub use capability_probe::CapabilityProbeEngine;
pub use llama_cpp::LlmWorker;
pub use providers::{EmbeddedProvider, LlmProvider, OpenAiCompatProvider, ProviderKind};

/// Large Language Model engine contract.
///
/// This is the lower-level interface for local GGUF inference via llama.cpp.
/// For the provider abstraction (which wraps local or remote), see `LlmProvider`.
pub trait LlmEngine {
    fn generate(
        &self,
        user_text: &str,
        system_prompt: &str,
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
