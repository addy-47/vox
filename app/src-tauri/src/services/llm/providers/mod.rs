use crate::core::events::VoxEvent;
use serde::{Deserialize, Serialize};
use std::sync::atomic::AtomicBool;
use std::sync::mpsc;
use std::sync::Arc;

use crate::core::settings::LlmModelInfo;

pub mod embedded;
pub mod openai_compat;

pub use embedded::EmbeddedProvider;
pub use openai_compat::OpenAiCompatProvider;

use crate::services::memory::ConversationContext;

pub trait LlmProvider: Send + Sync {
    /// Submit a generation request with conversation context; stream tokens via `tx`.
    fn generate(
        &self,
        ctx: &ConversationContext,
        turn_id: u32,
        cancel_flag: &Arc<AtomicBool>,
        tx: &mpsc::Sender<VoxEvent>,
    ) -> anyhow::Result<()>;

    /// Returns true if the provider is healthy / reachable.
    fn health_check(&self) -> bool;

    /// Returns list of model IDs the provider can serve.
    fn list_models(&self) -> anyhow::Result<Vec<LlmModelInfo>>;

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
