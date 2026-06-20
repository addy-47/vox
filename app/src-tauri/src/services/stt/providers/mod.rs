//! STT Provider trait — abstraction for speech-to-text engines.
//!
//! Mirrors the `LlmProvider` / `TtsProvider` pattern in `services/llm/providers/`
//! and `services/tts/providers/`.
//!
//! Each provider type implements this trait, and the STT worker dispatches
//! to the active provider via `Box<dyn SttProvider>`.

pub mod embedded;

pub use embedded::EmbeddedSttProvider;

use serde::{Deserialize, Serialize};
use std::path::Path;

/// Provider kind identifier — used for serialization and frontend display.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SttProviderKind {
    /// Local embedded ASR engine (Qwen3-ASR or Nemotron-3.5).
    Embedded,
    /// Cloud STT provider (Google, Deepgram, Whisperflow, etc.).
    /// Individual provider is a configuration detail within the Cloud variant.
    Cloud,
}

/// Speech-to-Text provider contract.
///
/// # Thread Safety
/// - `&self` methods: providers use interior mutability (`Mutex`) when needed.
/// - `Send` but not `Sync`: the STT worker owns the provider exclusively on its thread.
pub trait SttProvider: Send {
    /// One-shot full transcription (no state maintained).
    fn transcribe(&self, audio: &[f32]) -> anyhow::Result<String>;

    /// Streaming/partial transcription.
    ///
    /// `chunk` is the full accumulated utterance for the current turn.
    /// The provider maintains its own internal buffer/state for incremental processing.
    /// On `is_final = true`, the provider flushes any remaining internal state.
    fn transcribe_chunk(&self, chunk: &[f32], is_final: bool) -> anyhow::Result<String>;

    /// Reset all internal streaming state (buffer, stitching, engine state).
    fn reset_state(&self) -> anyhow::Result<()>;

    /// Returns `true` if the provider is healthy / ready.
    fn health_check(&self) -> bool;

    /// Returns the provider kind for identification.
    fn kind(&self) -> SttProviderKind;
}

use crate::core::settings::SttProviderConfig;

/// Factory: create an `SttProvider` from a configuration and model path.
pub fn create_stt_provider(
    provider_config: &SttProviderConfig,
    model_path: &Path,
) -> anyhow::Result<Box<dyn SttProvider>> {
    match provider_config {
        SttProviderConfig::Embedded { model_type } => {
            Ok(Box::new(EmbeddedSttProvider::new(model_path, model_type)?))
        }
        SttProviderConfig::Cloud { provider, .. } => {
            anyhow::bail!("Unknown cloud STT provider: \"{}\"", provider)
        }
    }
}
