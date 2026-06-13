use crate::core::events::VoxEvent;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::sync::mpsc;
use serde::{Deserialize, Serialize};

use crate::core::settings::RemoteModelInfo;

pub mod embedded;
pub mod openai_compat;

pub use embedded::EmbeddedProvider;
pub use openai_compat::OpenAiCompatProvider;

pub trait LlmProvider: Send + Sync {
    /// Submit a generation request; stream tokens via `tx`.
    fn generate(
        &self,
        text: &str,
        system_prompt: &str,
        turn_id: u32,
        cancel_flag: &Arc<AtomicBool>,
        tx: &mpsc::Sender<VoxEvent>,
    ) -> anyhow::Result<()>;

    /// Returns true if the provider is healthy / reachable.
    fn health_check(&self) -> bool;

    /// Returns list of model IDs the provider can serve.
    fn list_models(&self) -> anyhow::Result<Vec<RemoteModelInfo>>;

    /// Human-readable provider kind.
    fn kind(&self) -> ProviderKind;
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProviderKind {
    Embedded,
    OpenAiCompat,
}
