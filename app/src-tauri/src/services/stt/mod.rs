pub mod actor;
pub mod nemotron_onnx;
pub mod providers;
pub mod qwen_onnx;
pub use actor::{spawn_stt_worker, SttCommand};
pub use qwen_onnx::SAMPLE_RATE;

/// Speech-to-Text engine contract (internal — not public).
///
/// This trait is implemented by the individual engine types
/// (`nemotron_onnx::SttEngine`, `qwen_onnx::SttEngine`) and wrapped
/// by the `EmbeddedSttProvider` in the `providers` module.
/// External consumers should use the `SttProvider` trait instead.
pub(crate) trait SttEngine: Send + Sync {
    fn transcribe(&self, audio: &[f32]) -> anyhow::Result<String>;
    fn transcribe_chunk(&self, chunk: &[f32], is_final: bool) -> anyhow::Result<String>;
    fn reset_state(&self) -> anyhow::Result<()>;
}
