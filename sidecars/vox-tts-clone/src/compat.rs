use std::sync::{
    atomic::{AtomicBool, AtomicU32},
    mpsc::Sender,
    Arc,
};

pub mod core {
    pub mod events {
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub enum AudioIntent {
            Speech,
            System,
            Feedback,
        }

        pub enum VoxEvent {
            AudioChunk(Vec<f32>),
        }
    }
}

pub mod services {
    pub mod audio {
        use crate::core::events::AudioIntent;
        use std::sync::Arc;

        pub struct PlaybackEngine {
            tx: tokio::sync::mpsc::Sender<Vec<f32>>,
        }

        impl PlaybackEngine {
            pub fn new(tx: tokio::sync::mpsc::Sender<Vec<f32>>) -> Arc<Self> {
                Arc::new(Self { tx })
            }

            pub fn ingest_chunk_with_intent(&self, chunk: &[f32], _intent: AudioIntent) {
                let _ = self.tx.blocking_send(chunk.to_vec());
            }
        }
    }

    pub mod tts {
        #[derive(Debug, Clone, Copy, PartialEq)]
        pub struct ParamRange {
            pub min: f32,
            pub max: f32,
            pub step: f32,
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub enum TtsVoiceSource {
            Custom,
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub enum TtsExecutionKind {
            Embedded,
        }

        #[derive(Debug, Clone, PartialEq)]
        pub struct ProviderCaps {
            pub voices: TtsVoiceSource,
            pub clone: bool,
            pub speed_range: ParamRange,
            pub execution: TtsExecutionKind,
        }

        pub const MODEL_FILE_TTS_CHATTERBOX_T3: &str = "t3.gguf";
        pub const MODEL_FILE_TTS_CHATTERBOX_S3GEN: &str = "s3gen.gguf";
        pub const TTS_SAMPLE_RATE: u32 = 24000;
        pub const TTS_CHUNK_SIZE: usize = 2048;
        pub const MIN_SPEED: f32 = 0.5;
        pub const MAX_SPEED: f32 = 2.0;
    }
}

pub const fn speed_range(min: f32, max: f32) -> services::tts::ParamRange {
    services::tts::ParamRange {
        min,
        max,
        step: 0.05,
    }
}

pub struct SynthesisContext<'a> {
    pub turn_id: u32,
    pub intent: core::events::AudioIntent,
    pub cancel: Arc<AtomicBool>,
    pub playback: &'a Arc<services::audio::PlaybackEngine>,
    pub event_tx: Sender<core::events::VoxEvent>,
    pub telemetry_rtf: Option<&'a Arc<AtomicU32>>,
}

pub trait TtsProvider: Send + Sync {
    fn caps() -> services::tts::ProviderCaps
    where
        Self: Sized;
    fn synthesize_chunk(&self, text: &str, ctx: &SynthesisContext<'_>) -> anyhow::Result<()>;
    fn set_speed(&self, speed: f32);
    fn health_check(&self) -> bool;
}
