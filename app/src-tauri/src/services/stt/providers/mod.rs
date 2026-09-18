pub mod embedded;
pub mod nemotron;
pub mod qwen;

pub use embedded::EmbeddedSttProvider;
pub use nemotron::SttEngine as NemotronEngine;
pub use qwen::SttEngine as QwenEngine;
use serde::{Deserialize, Serialize};

/// Provider kind identifier for speech recognition backends.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SttProviderKind {
    Embedded,
    Cloud,
}

/// Abstract contract for speech-to-text inference providers.
pub trait SttProvider: Send {
    fn transcribe_chunk(&self, chunk: &[f32], is_final: bool) -> anyhow::Result<String>;
    fn reset_state(&self) -> anyhow::Result<()>;
    fn health_check(&self) -> bool;
    fn kind(&self) -> SttProviderKind;
}

/// Speech-to-Text inference engine contract for ONNX models.
pub trait SttEngine: Send + Sync {
    fn transcribe(&self, audio: &[f32]) -> anyhow::Result<String>;
    fn accept_audio_chunk(&self, _audio: &[f32]) -> anyhow::Result<()> {
        Ok(())
    }
    fn get_partial_result(&self) -> anyhow::Result<String> {
        Ok(String::new())
    }
    fn finalize_stream(&self) -> anyhow::Result<String> {
        Ok(String::new())
    }
    fn reset_stream(&self) -> anyhow::Result<()> {
        Ok(())
    }
}
