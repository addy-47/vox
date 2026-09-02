pub mod family;
pub mod generate;
pub mod worker;

pub use family::ModelFamily;
pub use worker::LlmWorker;

use crate::core::events::VoxEvent;
use crate::core::settings::LlmModelInfo;
use crate::services::harness::ConversationContext;
use crate::services::llm::{
    GenerationRequest, LlmEngine, LlmError, ProviderCapabilities, ProviderKind, Support,
};
use futures_util::future::BoxFuture;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::sync::Arc;

/// In-process embedded LLM provider backed by `llama.cpp`.
pub struct EmbeddedProvider {
    model_path: PathBuf,
    ctx_size: u32,
    engine: Arc<LlmWorker>,
    capabilities: ProviderCapabilities,
}

impl EmbeddedProvider {
    /// Loads a local GGUF model into memory and instantiates the embedded provider.
    pub fn new(model_path: &Path, ctx_size: u32, n_threads: u32) -> Result<Self, LlmError> {
        let engine = LlmWorker::new(model_path, ctx_size, n_threads)
            .map_err(|e| LlmError::Engine(e.to_string()))?;

        let capabilities = ProviderCapabilities {
            temperature: Support::Supported,
            top_p: Support::Supported,
            top_k: Support::Supported,
            max_output_tokens: Support::Supported,
            json_object: Support::Supported,
            json_schema: Support::Unsupported,
            streaming: Support::Supported,
            seed: Support::Supported,
        };

        Ok(Self {
            model_path: model_path.to_path_buf(),
            ctx_size,
            engine: Arc::new(engine),
            capabilities,
        })
    }

    /// Lists all `.gguf` model files located within the given directory.
    pub fn list_models_in_dir(dir: &Path) -> Result<Vec<LlmModelInfo>, LlmError> {
        let mut models = Vec::new();
        if dir.exists() {
            if let Ok(entries) = std::fs::read_dir(dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_file() && path.extension().and_then(|e| e.to_str()) == Some("gguf") {
                        if let Some(filename) = path.file_name().and_then(|s| s.to_str()) {
                            let metadata = entry.metadata().ok();
                            let size_bytes = metadata.map(|m| m.len());
                            let clean_name = filename
                                .strip_suffix(".gguf")
                                .unwrap_or(filename)
                                .replace(['_', '-'], " ");

                            models.push(LlmModelInfo {
                                id: filename.to_string(),
                                name: clean_name,
                                size_bytes,
                                quantization: None,
                                family: None,
                                provider_kind: "embedded".to_string(),
                                capabilities: None,
                            });
                        }
                    }
                }
            }
        }
        Ok(models)
    }
}

impl crate::services::llm::LlmProvider for EmbeddedProvider {
    fn generate<'a>(
        &'a self,
        request: GenerationRequest,
        turn_id: u32,
        cancel: &'a tokio_util::sync::CancellationToken,
        tx: &'a mpsc::Sender<VoxEvent>,
    ) -> BoxFuture<'a, Result<(), LlmError>> {
        Box::pin(async move {
            let conv_ctx = ConversationContext {
                messages: request.input.messages,
                token_count: 0,
                kv_cache_index: 0,
            };

            self.engine
                .generate(&conv_ctx, turn_id, cancel, tx)
                .map_err(|e| LlmError::Engine(e.to_string()))
        })
    }

    fn capabilities(&self) -> &ProviderCapabilities {
        &self.capabilities
    }

    fn health_check<'a>(&'a self) -> BoxFuture<'a, Result<(), LlmError>> {
        Box::pin(async move {
            if self.model_path.exists() {
                Ok(())
            } else {
                Err(LlmError::Engine(format!(
                    "Embedded model path not found: {:?}",
                    self.model_path
                )))
            }
        })
    }

    fn list_models<'a>(&'a self) -> BoxFuture<'a, Result<Vec<LlmModelInfo>, LlmError>> {
        Box::pin(async move {
            let dir = self.model_path.parent().unwrap_or_else(|| Path::new("."));
            Self::list_models_in_dir(dir)
        })
    }

    fn kind(&self) -> ProviderKind {
        ProviderKind::Embedded
    }

    fn max_context_tokens(&self) -> usize {
        self.ctx_size as usize
    }
}
