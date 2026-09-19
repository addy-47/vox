pub mod actor;
pub mod factory;
pub mod providers;
pub mod stitcher;

pub use actor::{spawn_stt_worker, SttActorChannels, SttActorHandles, SttCommand};
pub use factory::{create_stt_instance_from_settings, create_stt_provider};
pub use providers::{
    EmbeddedSttProvider, NemotronEngine, QwenEngine, SttEngine, SttProvider, SttProviderKind,
};
pub use stitcher::stitch_transcripts;

pub use crate::{core::error::SttError, services::audio::SAMPLE_RATE};

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
