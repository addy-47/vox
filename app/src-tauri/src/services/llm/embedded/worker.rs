use super::family::ModelFamily;
use anyhow::{anyhow, Result};
use llama_cpp_4::{
    context::params::LlamaContextParams, context::LlamaContext, llama_backend::LlamaBackend,
    model::params::LlamaModelParams, model::LlamaModel, token::LlamaToken,
};
use parking_lot::Mutex;
use std::num::NonZeroU32;
use std::path::Path;

unsafe impl Send for LlmWorker {}
unsafe impl Sync for LlmWorker {}

/// In-memory KV cache tracking state for prompt prefix reuse.
pub struct CacheState {
    pub system_prompt: String,
    pub system_tokens_len: usize,
    pub current_seq_tokens_len: usize,
}

/// In-process llama.cpp model worker executing token generation.
pub struct LlmWorker {
    pub(crate) model: LlamaModel,
    pub(crate) backend: &'static LlamaBackend,
    pub(crate) ctx_size: u32,
    pub(crate) n_threads: u32,
    pub(crate) family: ModelFamily,
    pub(crate) ctx: Mutex<Option<LlamaContext<'static>>>,
    pub(crate) cache_state: Mutex<Option<CacheState>>,
}

impl LlmWorker {
    /// Loads a local GGUF model and constructs an in-process llama.cpp worker.
    pub fn new(model_path: &Path, ctx_size: u32, n_threads: u32) -> Result<Self> {
        log::info!("[LLM] >>> Initializing llama.cpp backend...");

        let resolved = model_path
            .canonicalize()
            .unwrap_or_else(|_| model_path.to_path_buf());

        if !resolved.exists() {
            return Err(anyhow!("[LLM] GGUF not found: {:?}", resolved));
        }

        let family = ModelFamily::detect(&resolved);
        log::info!("[LLM] Detected model family: {:?}", family);

        let backend = crate::services::llm::global_llama_backend();

        let model_params = LlamaModelParams::default()
            .with_n_gpu_layers(0)
            .with_use_mlock(true);

        log::info!("[LLM] Loading GGUF: {:?}", resolved);
        let model = match LlamaModel::load_from_file(backend, &resolved, &model_params) {
            Ok(m) => m,
            Err(e) => {
                log::warn!(
                    "[LLM] Failed to load model with mlock: {}. Retrying without mlock...",
                    e
                );
                let fallback_params = LlamaModelParams::default()
                    .with_n_gpu_layers(0)
                    .with_use_mlock(false);
                LlamaModel::load_from_file(backend, &resolved, &fallback_params)
                    .map_err(|e2| anyhow!("[LLM] Failed to load model without mlock: {}", e2))?
            }
        };

        log::info!(
            "[LLM] Model loaded. family={:?} ctx_size={} n_threads={}",
            family,
            ctx_size,
            n_threads
        );

        Ok(Self {
            model,
            backend,
            ctx_size,
            n_threads,
            family,
            ctx: Mutex::new(None),
            cache_state: Mutex::new(None),
        })
    }

    /// Converts a llama token ID into its raw UTF-8 byte representation.
    pub(crate) fn token_to_bytes(&self, token: LlamaToken) -> Vec<u8> {
        self.model
            .token_to_bytes(token, llama_cpp_4::model::Special::Plaintext)
            .unwrap_or_default()
    }

    /// Returns the configured context size in tokens.
    pub fn ctx_size(&self) -> u32 {
        self.ctx_size
    }

    /// Ensures the LlamaContext is lazily initialized on demand.
    pub(crate) fn init_context(&self) -> Result<()> {
        let mut ctx_lock = self.ctx.lock();
        if ctx_lock.is_none() {
            log::info!("[LLM] Lazy initializing LlamaContext on stable execution address...");
            let effective_ctx = self.ctx_size.max(512);
            let ctx_params = LlamaContextParams::default()
                .with_n_ctx(NonZeroU32::new(effective_ctx))
                .with_n_threads(self.n_threads as i32)
                .with_n_threads_batch(self.n_threads as i32)
                .with_n_batch(crate::services::llm::DEFAULT_BATCH_CHUNK_SIZE as u32)
                .with_n_ubatch(crate::services::llm::DEFAULT_BATCH_CHUNK_SIZE as u32);

            let ctx = self
                .model
                .new_context(self.backend, ctx_params)
                .map_err(|e| anyhow!("[LLM] Lazy context creation failed: {}", e))?;

            let static_ctx: LlamaContext<'static> = unsafe { std::mem::transmute(ctx) };
            *ctx_lock = Some(static_ctx);
        }
        Ok(())
    }
}
