// ─── Audio Subsystem Constants ───────────────────────────────────────────────
pub const CLONE_SAMPLE_RATE: u32 = 24_000;
pub const PLAYBACK_SAMPLE_RATE: u32 = 48_000;
pub const PLAYBACK_CHANNELS: u16 = 2;
pub const PLAYBACK_BUFFER_CAPACITY_SECS: usize = 30;
pub const PLAYBACK_BUFFER_SAMPLES: usize = PLAYBACK_SAMPLE_RATE as usize * PLAYBACK_BUFFER_CAPACITY_SECS;
pub const PLAYBACK_DEFAULT_VOLUME: f32 = 1.0;
pub const PLAYBACK_VOLUME_RAMP_STEP: f32 = 0.002;
pub const PLAYBACK_ENERGY_MULTIPLIER: f32 = 15.0;
pub const PLAYBACK_ENERGY_EXPONENT: f32 = 0.5;

pub const INGESTION_BUFFER_CAPACITY_SAMPLES: usize = 8192;
pub const INGESTION_OVERFLOW_LOG_INTERVAL: u32 = 100;

pub const PCM_I16_SCALE: f32 = 32767.0;
pub const PCM_U8_SCALE: f32 = 128.0;
pub const PCM_U16_SCALE: f32 = 32768.0;
pub const PCM_S16_SCALE: f32 = 32768.0;
pub const PCM_S32_SCALE: f32 = 2147483648.0;

pub mod decode;
pub mod device;
pub mod engine;
pub mod playback;

pub use decode::{
    decode_bytes_to_24khz_mono, decode_to_24khz_mono, truncate_to, write_wav_f32, write_wav_f32_raw,
    DecodedAudio,
};
pub use device::AudioStream;
pub use engine::{start_audio_engine, stop_audio_engine};
pub use playback::PlaybackEngine;
