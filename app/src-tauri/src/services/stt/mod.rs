pub mod actor;
pub mod embedded;
pub mod nemotron;
pub mod qwen;
pub mod stitcher;

pub use crate::core::constants::SAMPLE_RATE;
pub use crate::core::error::SttError;
pub use actor::{spawn_stt_worker, SttActorChannels, SttActorHandles, SttCommand};
pub use embedded::EmbeddedSttProvider;
pub use stitcher::stitch_transcripts;

use crate::core::settings::SttProviderConfig;
use serde::{Deserialize, Serialize};
use std::path::Path;

pub const QWEN_ASR_MODEL_DIR: &str = "stt/qwen3-asr";
pub const NEMOTRON_MODEL_DIR: &str = "stt/nemotron-3.5";
pub const MODEL_FILE_ASR_FRONTEND: &str = "conv_frontend.onnx";
pub const MODEL_FILE_ASR_ENCODER: &str = "encoder.int8.onnx";
pub const MODEL_FILE_ASR_DECODER: &str = "decoder.int8.onnx";
pub const MODEL_FILE_ASR_JOINER: &str = "joiner.int8.onnx";
pub const MODEL_FILE_ASR_TOKENS: &str = "tokens.txt";
pub const MODEL_FILE_ASR_TOKENIZER: &str = "tokenizer";

pub const QWEN_MAX_TOTAL_LEN: i32 = 2048;
pub const QWEN_MAX_NEW_TOKENS: i32 = 128;

pub const STT_DEFAULT_INFERENCE_DURATION_MS: u64 = 300;
pub const STT_MIN_PARTIAL_THROTTLE_MS: u64 = 300;
pub const STT_PARTIAL_ERROR_PENALTY_MS: u64 = 500;
pub const STT_WORKER_RECV_TIMEOUT_MS: u64 = 150;
pub const STT_WORKER_THREAD_PRIORITY: u8 = 80;

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

/// Instantiates an SttProvider instance from the specified configuration and model path.
pub fn create_stt_provider(
    provider_config: &SttProviderConfig,
    model_path: &Path,
    num_threads: u32,
) -> anyhow::Result<Box<dyn SttProvider>> {
    match provider_config {
        SttProviderConfig::Embedded { model_type } => {
            Ok(Box::new(EmbeddedSttProvider::new(model_path, model_type, num_threads)?))
        }
        SttProviderConfig::Cloud { provider, .. } => {
            anyhow::bail!("Unknown cloud STT provider: \"{}\"", provider)
        }
    }
}
