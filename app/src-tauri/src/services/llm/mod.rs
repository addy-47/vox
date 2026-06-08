pub mod actor;
pub mod llama_cpp;
pub use actor::{spawn_llm_worker, LlmCommand};
pub use llama_cpp::LlmWorker;

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
