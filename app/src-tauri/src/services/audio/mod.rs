use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64};
use std::sync::mpsc::Sender;
use std::sync::Arc;

use crate::core::events::VoxEvent;

pub const CLONE_SAMPLE_RATE: u32 = 24_000;
pub const PLAYBACK_SAMPLE_RATE: u32 = 48_000;
pub const PLAYBACK_CHANNELS: u16 = 2;
pub const PLAYBACK_BUFFER_CAPACITY_SECS: usize = 30;
pub const PLAYBACK_BUFFER_SAMPLES: usize =
    PLAYBACK_SAMPLE_RATE as usize * PLAYBACK_BUFFER_CAPACITY_SECS;
pub const PLAYBACK_DEFAULT_VOLUME: f32 = 1.0;
pub const PLAYBACK_VOLUME_RAMP_STEP: f32 = 0.002;
pub const PLAYBACK_ENERGY_MULTIPLIER: f32 = 15.0;
pub const PLAYBACK_ENERGY_EXPONENT: f32 = 0.5;
pub const MODULAR_PREROLL_THRESHOLD_SAMPLES: usize = 12_000;
pub const REALTIME_PREROLL_THRESHOLD_SAMPLES: usize = 3_840;
pub const PLAYBACK_PRODUCER_SCRATCH_CAPACITY: usize = 4096;

pub const INGESTION_BUFFER_CAPACITY_SAMPLES: usize = 16_384;
pub const INGESTION_OVERFLOW_LOG_INTERVAL: u32 = 100;

pub const PCM_I16_SCALE: f32 = 32767.0;
pub const PCM_U8_SCALE: f32 = 128.0;
pub const PCM_U16_SCALE: f32 = 32768.0;
pub const PCM_S16_SCALE: f32 = 32768.0;
pub const PCM_S32_SCALE: f32 = 2147483648.0;

pub const SINC_CHUNK_SIZE_INPUT: usize = 320;
pub const SINC_CHUNK_SIZE_OUTPUT: usize = 512;
pub const SINC_WINDOW_LEN: usize = 256;
pub const SINC_OVERSAMPLING_FACTOR: usize = 128;
pub const SINC_CUTOFF_FREQUENCY: f32 = 0.95;

pub mod decode;
pub mod device;
pub mod playback;
pub mod resampler;
pub(crate) mod sink;

pub use decode::{
    decode_bytes_to_24khz_mono, decode_to_24khz_mono, truncate_to, write_wav_f32,
    write_wav_f32_raw, DecodedAudio,
};
pub use device::{build_output_stream, resolve_output_device_and_config, AudioStream};
pub use playback::PlaybackEngine;
pub use resampler::{upsample_2x, upsample_2x_into, AudioResampler};

/// Telemetry and visualization atomics passed to the playback engine.
#[derive(Clone)]
pub struct PlaybackTelemetryHandles {
    pub energy: Arc<AtomicU32>,
    pub low: Arc<AtomicU32>,
    pub mid: Arc<AtomicU32>,
    pub high: Arc<AtomicU32>,
    pub underruns: Arc<AtomicU64>,
}

/// Handles and state atomics for initializing or wrapping a playback engine.
#[derive(Clone)]
pub struct PlaybackEngineHandles {
    pub cancel_flag: Arc<AtomicBool>,
    pub state_atomic: Arc<AtomicU32>,
    pub current_turn_id: Arc<AtomicU32>,
    pub pending_synthesis_jobs: Arc<AtomicU32>,
    pub event_tx: Sender<VoxEvent>,
}
