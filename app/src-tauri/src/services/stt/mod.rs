pub mod actor;
pub mod nemotron_onnx;
pub mod providers;
pub mod qwen_onnx;
pub mod stitcher;
pub use crate::core::constants::SAMPLE_RATE;
pub use crate::core::error::SttError;
pub use actor::{spawn_stt_worker, SttActorChannels, SttActorHandles, SttCommand};
pub use stitcher::stitch_transcripts;

pub const QWEN_ASR_MODEL_DIR: &str = "stt/qwen3-asr";
pub const NEMOTRON_MODEL_DIR: &str = "stt/nemotron-3.5";
pub const MODEL_FILE_ASR_FRONTEND: &str = "conv_frontend.onnx";
pub const MODEL_FILE_ASR_ENCODER: &str = "encoder.int8.onnx";
pub const MODEL_FILE_ASR_DECODER: &str = "decoder.int8.onnx";
pub const MODEL_FILE_ASR_JOINER: &str = "joiner.int8.onnx";
pub const MODEL_FILE_ASR_TOKENS: &str = "tokens.txt";
pub const MODEL_FILE_ASR_TOKENIZER: &str = "tokenizer";

pub const NEMOTRON_NUM_THREADS: i32 = 4;
pub const QWEN_MAX_TOTAL_LEN: i32 = 2048;
pub const QWEN_MAX_NEW_TOKENS: i32 = 128;
pub const QWEN_NUM_THREADS: i32 = 4;

pub const STT_DEFAULT_INFERENCE_DURATION_MS: u64 = 300;
pub const STT_MIN_PARTIAL_THROTTLE_MS: u64 = 300;
pub const STT_PARTIAL_ERROR_PENALTY_MS: u64 = 500;
pub const STT_WORKER_RECV_TIMEOUT_MS: u64 = 150;
pub const STT_WORKER_THREAD_PRIORITY: u8 = 80;

/// Speech-to-Text inference engine contract for ONNX models.
pub trait SttEngine: Send + Sync {
    /// Transcribes a complete audio frame buffer to text.
    fn transcribe(&self, audio: &[f32]) -> anyhow::Result<String>;
}
