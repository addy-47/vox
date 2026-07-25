pub mod actor;
pub mod providers;
pub use actor::{spawn_tts_worker, TtsCommand};
pub use crate::core::error::TtsError;
pub use providers::chatterbox::ChatterboxEngine;
pub use providers::chatterbox_remote::ChatterboxRemoteProvider;
pub use providers::supertonic::TtsEngine;
pub use providers::{TtsProvider, TtsProviderKind};

// ─── TTS Model Constants ───────────────────────────────────────────────────
pub const MODEL_DIR_TTS_SUPER: &str = "tts/supertonic-3";
pub const MODEL_FILE_TTS_SUPER_TEXT_ENCODER: &str = "text_encoder.int8.onnx";
pub const MODEL_FILE_TTS_SUPER_DURATION_PREDICTOR: &str = "duration_predictor.int8.onnx";
pub const MODEL_FILE_TTS_SUPER_VECTOR_ESTIMATOR: &str = "vector_estimator.int8.onnx";
pub const MODEL_FILE_TTS_SUPER_VOCODER: &str = "vocoder.int8.onnx";
pub const MODEL_FILE_TTS_SUPER_CONFIG: &str = "tts.json";
pub const MODEL_FILE_TTS_SUPER_INDEXER: &str = "unicode_indexer.bin";
pub const MODEL_FILE_TTS_SUPER_VOICE: &str = "voice.bin";

pub const MODEL_DIR_TTS_CHATTERBOX: &str = "tts/chatterbox";
pub const MODEL_FILE_TTS_CHATTERBOX_T3: &str = "t3-q4_0.gguf";
pub const MODEL_FILE_TTS_CHATTERBOX_S3GEN: &str = "s3gen-f16.gguf";
