pub mod actor;
pub mod nemotron_onnx;
pub mod providers;
pub mod qwen_onnx;
pub use crate::core::error::SttError;
pub use actor::{spawn_stt_worker, SttCommand};
pub use qwen_onnx::SAMPLE_RATE;

pub const MODEL_DIR_STT_QWEN: &str = "stt/qwen3-asr";
pub const MODEL_DIR_STT_NEMOTRON: &str = "stt/nemotron-3.5";
pub const MODEL_FILE_ASR_FRONTEND: &str = "conv_frontend.onnx";
pub const MODEL_FILE_ASR_ENCODER: &str = "encoder.int8.onnx";
pub const MODEL_FILE_ASR_DECODER: &str = "decoder.int8.onnx";
pub const MODEL_FILE_ASR_TOKENIZER: &str = "tokenizer";

/// Internal Speech-to-Text inference engine contract for ONNX models.
pub(crate) trait SttEngine: Send + Sync {
    /// Transcribes a complete audio frame buffer to text.
    fn transcribe(&self, audio: &[f32]) -> anyhow::Result<String>;
    /// Transcribes an audio chunk in streaming mode.
    #[allow(dead_code)]
    fn transcribe_chunk(&self, chunk: &[f32], is_final: bool) -> anyhow::Result<String>;
    /// Resets internal streaming states and recurrent caches.
    #[allow(dead_code)]
    fn reset_state(&self) -> anyhow::Result<()>;
}
