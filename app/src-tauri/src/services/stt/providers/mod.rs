pub mod embedded;

pub use embedded::EmbeddedSttProvider;

use serde::{Deserialize, Serialize};
use std::path::Path;

/// Provider kind identifier for speech recognition backends.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SttProviderKind {
    /// Local embedded ONNX speech recognition engine.
    Embedded,
    /// Remote cloud speech recognition service.
    Cloud,
}

/// Abstract contract for speech-to-text inference providers.
pub trait SttProvider: Send {
    /// Transcribes an incoming audio chunk in streaming mode.
    fn transcribe_chunk(&self, chunk: &[f32], is_final: bool) -> anyhow::Result<String>;

    /// Resets internal streaming buffers, transcript stitching, and model states.
    fn reset_state(&self) -> anyhow::Result<()>;

    /// Returns true if the provider backend is initialized and healthy.
    fn health_check(&self) -> bool;

    /// Returns the provider kind enum identifier.
    fn kind(&self) -> SttProviderKind;
}

use crate::core::settings::SttProviderConfig;

/// Instantiates an SttProvider instance from the specified configuration and model path.
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
