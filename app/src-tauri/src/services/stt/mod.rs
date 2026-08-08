pub mod actor;
pub mod nemotron_onnx;
pub mod providers;
pub mod qwen_onnx;
pub use crate::core::error::SttError;
pub use actor::{spawn_stt_worker, SttCommand};
pub use qwen_onnx::SAMPLE_RATE;

// ─── STT Model Constants ───────────────────────────────────────────────────
pub const MODEL_DIR_STT_QWEN: &str = "stt/qwen3-asr";
pub const MODEL_DIR_STT_NEMOTRON: &str = "stt/nemotron-3.5";
pub const MODEL_FILE_ASR_FRONTEND: &str = "conv_frontend.onnx";
pub const MODEL_FILE_ASR_ENCODER: &str = "encoder.int8.onnx";
pub const MODEL_FILE_ASR_DECODER: &str = "decoder.int8.onnx";
pub const MODEL_FILE_ASR_TOKENIZER: &str = "tokenizer";

/// Speech-to-Text engine contract (internal — not public).
///
/// This trait is implemented by the individual engine types
/// (`nemotron_onnx::SttEngine`, `qwen_onnx::SttEngine`) and wrapped
/// by the `EmbeddedSttProvider` in the `providers` module.
/// External consumers should use the `SttProvider` trait instead.
pub(crate) trait SttEngine: Send + Sync {
    fn transcribe(&self, audio: &[f32]) -> anyhow::Result<String>;
    #[allow(dead_code)]
    fn transcribe_chunk(&self, chunk: &[f32], is_final: bool) -> anyhow::Result<String>;
    #[allow(dead_code)]
    fn reset_state(&self) -> anyhow::Result<()>;
}
