use crate::core::events::VoxEvent;
use crate::core::settings::LlmModelInfo;
use futures_util::future::BoxFuture;
use serde::{Deserialize, Serialize};
use std::sync::atomic::AtomicBool;
use std::sync::mpsc;
use std::sync::Arc;

pub mod embedded;
pub mod lm_studio;
pub mod ollama;
pub mod openai;
pub mod openai_compat;

pub use embedded::EmbeddedProvider;
pub use lm_studio::LmStudioAdapter;
pub use ollama::OllamaAdapter;
pub use openai::{ChatCompletionsAdapter, ResponsesAdapter};
pub use openai_compat::OpenAiCompatProvider;

use crate::services::llm::types::{GenerationRequest, LlmError, ProviderCapabilities};

pub trait LlmProvider: Send + Sync {
    /// Submit a provider-neutral generation request; stream tokens via `tx`.
    fn generate<'a>(
        &'a self,
        request: GenerationRequest,
        turn_id: u32,
        cancel_flag: &'a Arc<AtomicBool>,
        tx: &'a mpsc::Sender<VoxEvent>,
    ) -> BoxFuture<'a, Result<(), LlmError>>;

    /// Returns static or dynamic capabilities of this provider.
    fn capabilities(&self) -> &ProviderCapabilities;

    /// Returns true if the provider is healthy / reachable.
    fn health_check(&self) -> bool;

    /// Returns list of model IDs the provider can serve.
    fn list_models(&self) -> Result<Vec<LlmModelInfo>, LlmError>;

    /// Human-readable provider kind.
    fn kind(&self) -> ProviderKind;

    /// Maximum supported context size in tokens.
    fn max_context_tokens(&self) -> usize {
        2048
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProviderKind {
    Embedded,
    OpenAiCompat,
}
