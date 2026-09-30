pub mod actor;
pub mod config;
pub mod factory;
pub mod providers;
pub mod voice;
pub use actor::{cool_down_tts, spawn_tts_worker, warm_up_tts, TtsCommand};
pub use config::{
    ParamRange, ProviderCaps, TtsActiveProvider, TtsChatterboxConfig, TtsChatterboxRemoteConfig,
    TtsEdgeConfig, TtsKokoroConfig, TtsProviderConfig, TtsSettings, TtsSupertonicConfig,
    TtsVoiceSource, TtsZipvoiceConfig,
};
pub use factory::{caps_for_id, create_tts_provider, resolve_reference_audio};
pub use providers::{
    chatterbox::ChatterboxEngine,
    chatterbox_remote::ChatterboxRemoteProvider,
    edge_tts::{
        EdgeTtsProvider, EDGE_TTS_DEFAULT_VOICE, EDGE_TTS_HINDI_VOICE, EDGE_TTS_USER_AGENT,
        EDGE_TTS_VOICES_URL_BASE,
    },
    kokoro::KokoroEngine,
    supertonic::TtsEngine,
    zipvoice::{resolve_zipvoice_reference, ZipvoiceEngine, ZipvoiceReference, ZipvoiceVoiceEntry},
    TtsProvider,
};
pub use voice::VoiceProfile;

pub use crate::core::error::TtsError;

pub const TTS_SAMPLE_RATE: u32 = 24000;
pub const SUPER_SAMPLE_RATE: u32 = 44100;
pub const TTS_CHUNK_SIZE: usize = 2048;

pub const MIN_SPEED: f32 = 0.7;
pub const MAX_SPEED: f32 = 2.0;
pub const MIN_SPEED_EDGE: f32 = 0.5;
pub const MAX_SPEED_EDGE: f32 = 2.0;
pub const DEFAULT_SPEED: f32 = 1.0;

pub const SUPERTONIC_MODEL_DIR: &str = "tts/supertonic-3";
pub const KOKORO_MODEL_DIR: &str = "tts/kokoro";
pub const CHATTERBOX_MODEL_DIR: &str = "tts/chatterbox";
pub const MODEL_FILE_TTS_CHATTERBOX_T3: &str = "t3-q4_0.gguf";
pub const MODEL_FILE_TTS_CHATTERBOX_S3GEN: &str = "s3gen-f16.gguf";
pub const ZIPVOICE_MODEL_DIR: &str = "tts/zipvoice";
