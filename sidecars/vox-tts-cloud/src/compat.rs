use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, AtomicU32},
        mpsc::Sender,
        Arc,
    },
};

pub mod core {
    pub mod error {
        #[derive(Debug, thiserror::Error)]
        pub enum TtsError {
            #[error("Auth failed: {message}")]
            Auth { message: String },
            #[error("Rate limited: {message}")]
            RateLimited { message: String },
            #[error("Voice not found: {voice}")]
            VoiceNotFound { voice: String },
            #[error("Cancelled")]
            Cancelled,
            #[error("Synthesis failed: {message}")]
            SynthesisFailed { message: String },
        }
    }

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
        use std::sync::Arc;
        use crate::core::events::AudioIntent;

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
        pub mod auth {
            use std::time::{Duration, Instant};
            use crate::core::error::TtsError;

            const TOKEN_REFRESH_MARGIN_SECS: u64 = 60;

            pub struct CachedToken {
                value: String,
                expires_at: Instant,
            }

            impl CachedToken {
                pub fn new(value: String, ttl: Duration) -> Self {
                    Self {
                        value,
                        expires_at: Instant::now() + ttl,
                    }
                }

                pub fn get(&self) -> Option<&str> {
                    if Instant::now() + Duration::from_secs(TOKEN_REFRESH_MARGIN_SECS) < self.expires_at {
                        Some(&self.value)
                    } else {
                        None
                    }
                }
            }

            pub fn normalize_azure_endpoint(region_or_endpoint: &str) -> String {
                let trimmed = region_or_endpoint.trim().trim_end_matches('/');
                if trimmed.contains("://") {
                    trimmed.to_string()
                } else {
                    format!("https://{trimmed}.tts.speech.microsoft.com")
                }
            }

            pub fn classify_http_status(status: u16) -> TtsError {
                match status {
                    401 | 403 => TtsError::Auth {
                        message: format!("Cloud TTS rejected credentials (HTTP {status})"),
                    },
                    402 | 429 => TtsError::RateLimited {
                        message: format!("Cloud TTS quota or rate limit hit (HTTP {status})"),
                    },
                    _ => TtsError::SynthesisFailed {
                        message: format!("Cloud TTS HTTP {status}"),
                    },
                }
            }
        }

        pub mod voice {
            use std::path::Path;
            use anyhow::Result;

            pub fn write_f32_wav(path: &Path, samples: &[f32], sample_rate: u32) -> Result<()> {
                let spec = hound::WavSpec {
                    channels: 1,
                    sample_rate,
                    bits_per_sample: 16,
                    sample_format: hound::SampleFormat::Int,
                };
                let mut writer = hound::WavWriter::create(path, spec)?;
                for &s in samples {
                    let clamped = s.clamp(-1.0, 1.0);
                    let s16 = (clamped * 32767.0) as i16;
                    writer.write_sample(s16)?;
                }
                writer.finalize()?;
                Ok(())
            }
        }

        #[derive(Debug, Clone, Copy, PartialEq)]
        pub struct ParamRange {
            pub min: f32,
            pub max: f32,
            pub step: f32,
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub enum TtsVoiceSource {
            Cloud,
            Custom,
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub enum TtsExecutionKind {
            Cloud,
            Embedded,
        }

        #[derive(Debug, Clone, PartialEq)]
        pub struct ProviderCaps {
            pub voices: TtsVoiceSource,
            pub clone: bool,
            pub speed_range: ParamRange,
            pub execution: TtsExecutionKind,
        }

        pub const GOOGLE_TTS_SAMPLE_RATE: u32 = 24000;
        pub const GOOGLE_TTS_MAX_TEXT_BYTES: usize = 5000;
        pub const GOOGLE_TTS_STREAMING_VOICE_INFIX: &str = "Chirp3-HD";

        pub const AZURE_TTS_API_PATH: &str = "/cognitiveservices/v1";
        pub const AZURE_TTS_VOICES_PATH: &str = "/cognitiveservices/voices/list";
        pub const AZURE_TTS_OUTPUT_FORMAT: &str = "raw-24khz-16bit-mono-pcm";
        pub const AZURE_TTS_SAMPLE_RATE: u32 = 24000;
        pub const AZURE_TTS_TIMEOUT_SECS: u64 = 15;

        pub const TTS_CHUNK_SIZE: usize = 2048;
        pub const MIN_SPEED: f32 = 0.5;
        pub const MAX_SPEED: f32 = 2.0;
    }
}

pub mod utils {
    use std::path::PathBuf;

    pub struct Paths {
        pub data: PathBuf,
    }

    pub mod paths {
        use super::Paths;
        use std::sync::OnceLock;

        pub fn get() -> &'static Paths {
            static PATHS: OnceLock<Paths> = OnceLock::new();
            PATHS.get_or_init(|| {
                let root = dirs::home_dir().unwrap_or_else(|| PathBuf::from(".")).join(".vox");
                Paths {
                    data: root.join("data"),
                }
            })
        }
    }
}

pub const fn speed_range(min: f32, max: f32) -> services::tts::ParamRange {
    services::tts::ParamRange { min, max, step: 0.05 }
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
    fn caps() -> services::tts::ProviderCaps where Self: Sized;
    fn synthesize_chunk(&self, text: &str, ctx: &SynthesisContext<'_>) -> anyhow::Result<()>;
    fn set_speed(&self, speed: f32);
    fn health_check(&self) -> bool;
}
