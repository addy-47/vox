pub mod actor;
pub mod providers;
pub mod voice;
pub use crate::core::error::TtsError;
pub use actor::{
    cool_down_tts, create_tts_provider, resolve_reference_audio, spawn_tts_worker, warm_up_tts,
    TtsClauseChunker, TtsCommand,
};
pub use providers::chatterbox::ChatterboxEngine;
pub use providers::chatterbox_remote::ChatterboxRemoteProvider;
pub use providers::edge_tts::EdgeTtsProvider;
pub use providers::supertonic::TtsEngine;
pub use providers::{TtsProvider, TtsProviderKind};

// ─── TTS Audio & Synthesis Constants ─────────────────────────────────────────
pub const TTS_SAMPLE_RATE: u32 = 24000;
pub const SUPER_SAMPLE_RATE: u32 = 44100;
pub const TTS_CHUNK_SIZE: usize = 2048;

// ─── Quality & Speed Constraints ─────────────────────────────────────────────
pub const MIN_QUALITY_STEPS: u32 = 2;
pub const MAX_QUALITY_STEPS_CHATTERBOX: u32 = 10;
pub const MAX_QUALITY_STEPS_SUPERTONIC: u32 = 12;
pub const MIN_SPEED: f32 = 0.7;
pub const MAX_SPEED: f32 = 2.0;
pub const MIN_SPEED_EDGE: f32 = 0.5;
pub const MAX_SPEED_EDGE: f32 = 2.0;
pub const DEFAULT_SPEED: f32 = 1.0;

// ─── Voice Cloning & Pre-Baking Constants ────────────────────────────────────
pub const MIN_VOICE_CLONE_DURATION_SECS: f32 = 1.0;
pub const TARGET_VOICE_SAMPLE_DURATION_SECS: f32 = 30.0;

// ─── Network & Remote Endpoints ──────────────────────────────────────────────
pub const EDGE_TTS_HOST: &str = "speech.platform.bing.com";
pub const EDGE_TTS_PORT: u16 = 443;
pub const EDGE_TTS_ORIGIN: &str = "chrome-extension://jdiccldimpdaibmpdkjnbmckianbfold";
pub const EDGE_TTS_USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/143.0.0.0 Safari/537.36 Edg/143.0.0.0";
pub const EDGE_TTS_SEC_MS_GEC_VERSION: &str = "1-143.0.3650.75";
pub const EDGE_TTS_WIN_EPOCH: u64 = 11_644_473_600;
pub const EDGE_TTS_DEFAULT_VOICE: &str = "en-US-AriaNeural";
pub const EDGE_TTS_VOICES_URL_BASE: &str = "https://speech.platform.bing.com/consumer/speech/synthesize/readaloud/voices/list?trustedclienttoken=";
pub const EDGE_TTS_WS_URL_BASE: &str = "wss://speech.platform.bing.com/consumer/speech/synthesize/readaloud/edge/v1";

// ─── TTS Model Constants ─────────────────────────────────────────────────────
pub const MODEL_DIR_TTS: &str = "tts";
pub const SUPERTONIC_MODEL_DIR: &str = "tts/supertonic-3";
pub const MODEL_FILE_TTS_SUPER_TEXT_ENCODER: &str = "text_encoder.int8.onnx";
pub const MODEL_FILE_TTS_SUPER_DURATION_PREDICTOR: &str = "duration_predictor.int8.onnx";
pub const MODEL_FILE_TTS_SUPER_VECTOR_ESTIMATOR: &str = "vector_estimator.int8.onnx";
pub const MODEL_FILE_TTS_SUPER_VOCODER: &str = "vocoder.int8.onnx";
pub const MODEL_FILE_TTS_SUPER_CONFIG: &str = "tts.json";
pub const MODEL_FILE_TTS_SUPER_INDEXER: &str = "unicode_indexer.bin";
pub const MODEL_FILE_TTS_SUPER_VOICE: &str = "voice.bin";

pub const CHATTERBOX_MODEL_DIR: &str = "tts/chatterbox";
pub const MODEL_DIRNAME_CHATTERBOX: &str = "chatterbox";
pub const MODEL_FILE_TTS_CHATTERBOX_T3: &str = "t3-q4_0.gguf";
pub const MODEL_FILE_TTS_CHATTERBOX_S3GEN: &str = "s3gen-f16.gguf";

