use std::sync::{
    atomic::{AtomicBool, AtomicU32, Ordering},
    Arc,
};

use anyhow::{anyhow, Result};
use cpal::{
    traits::{DeviceTrait, HostTrait, StreamTrait},
    SampleFormat, StreamConfig,
};
use ringbuf::{traits::*, HeapCons};

use super::{
    sink::PlaybackStreamContext, PlaybackEngineHandles, PlaybackTelemetryHandles,
    INGESTION_BUFFER_CAPACITY_SAMPLES, INGESTION_OVERFLOW_LOG_INTERVAL, PLAYBACK_CHANNELS,
    PLAYBACK_SAMPLE_RATE,
};
use crate::core::constants::SAMPLE_RATE;

/// Manages the low-level CPAL hardware audio stream for microphone capture.
pub struct AudioStream {
    _stream: Option<cpal::Stream>,
}

unsafe impl Send for AudioStream {}
unsafe impl Sync for AudioStream {}

impl AudioStream {
    /// Creates and configures a new hardware audio ingestion stream.
    pub fn new<P>(
        producer: P,
        device_name: Option<String>,
        ingestion_gate: Arc<AtomicBool>,
    ) -> Result<Self>
    where
        P: Producer<Item = f32> + Send + 'static,
    {
        let (device, config) = resolve_input_device(device_name.as_deref())?;
        let sample_rate = config.sample_rate.0;
        let channels = config.channels as usize;

        log::info!(
            "[Audio::Device] Using input device: {}",
            device.name().unwrap_or_else(|_| "Unknown".into())
        );
        log::info!(
            "[Audio::Device] Config: {}Hz, {} channels",
            sample_rate,
            channels
        );

        let stream = build_input_stream(
            device,
            &config,
            channels,
            sample_rate,
            producer,
            ingestion_gate,
        )?;
        Ok(Self {
            _stream: Some(stream),
        })
    }

    /// Creates a mock AudioStream for integration testing without audio hardware.
    pub fn mock() -> Self {
        Self { _stream: None }
    }

    /// Starts the hardware audio stream.
    pub fn start(&self) -> Result<()> {
        if let Some(ref stream) = self._stream {
            log::info!("[Audio::Device] Starting hardware ingestion");
            stream.play()?;
        }
        Ok(())
    }
}

impl Drop for AudioStream {
    /// Ensures the hardware stream is paused cleanly when dropping.
    fn drop(&mut self) {
        log::info!("[Audio::Device] Dropping hardware stream. Ensuring mic is released");
        if let Some(ref stream) = self._stream {
            if let Err(e) = stream.pause() {
                log::warn!("[Audio::Device] Failed to pause stream on drop: {}", e);
            }
        }
    }
}

/// Resolves the default audio host and queries the requested or fallback input device.
fn resolve_input_device(device_name: Option<&str>) -> Result<(cpal::Device, cpal::StreamConfig)> {
    let host = resolve_audio_host();
    log::info!("[Audio::Device] Using audio host: {:?}", host.id());

    let device = if let Some(name) = device_name {
        log::info!(
            "[Audio::Device] Attempting to use requested device: {}",
            name
        );
        host.input_devices()?
            .find(|d| d.name().map(|n| n == name).unwrap_or(false))
            .or_else(|| {
                log::warn!(
                    "[Audio::Device] Requested device '{}' not found, falling back to default",
                    name
                );
                host.default_input_device()
            })
    } else {
        host.default_input_device()
    }
    .ok_or_else(|| anyhow!("No input device found on host {:?}", host.id()))?;

    let config: cpal::StreamConfig = device.default_input_config()?.into();
    Ok((device, config))
}

/// Resolves the preferred CPAL host platform.
pub fn resolve_audio_host() -> cpal::Host {
    cpal::default_host()
}

/// Builds the CPAL input ingestion stream with lock-free ringbuf forwarding.
fn build_input_stream<P>(
    device: cpal::Device,
    config: &cpal::StreamConfig,
    channels: usize,
    sample_rate: u32,
    mut producer: P,
    ingestion_gate: Arc<AtomicBool>,
) -> Result<cpal::Stream>
where
    P: Producer<Item = f32> + Send + 'static,
{
    let mut source_index: f32 = 0.0;
    let resample_ratio = sample_rate as f32 / SAMPLE_RATE as f32;
    let mut mono_buffer = Vec::with_capacity(INGESTION_BUFFER_CAPACITY_SAMPLES);
    let mut resampled_buffer = Vec::with_capacity(INGESTION_BUFFER_CAPACITY_SAMPLES);

    let stream = device.build_input_stream(
        config,
        move |data: &[f32], _: &cpal::InputCallbackInfo| {
            if !ingestion_gate.load(Ordering::Relaxed) {
                return;
            }

            mono_buffer.clear();
            resampled_buffer.clear();

            for chunk in data.chunks_exact(channels) {
                if mono_buffer.len() >= INGESTION_BUFFER_CAPACITY_SAMPLES {
                    break;
                }
                let avg: f32 = chunk.iter().sum::<f32>() / channels as f32;
                mono_buffer.push(avg);
            }

            let n_mono = mono_buffer.len();
            while (source_index as usize) < n_mono {
                if resampled_buffer.len() >= INGESTION_BUFFER_CAPACITY_SAMPLES {
                    break;
                }
                let idx = source_index as usize;
                let next_idx = (idx + 1).min(n_mono - 1);
                let frac = source_index - idx as f32;
                let sample = (1.0 - frac) * mono_buffer[idx] + frac * mono_buffer[next_idx];
                resampled_buffer.push(sample);
                source_index += resample_ratio;
            }

            source_index = (source_index - n_mono as f32).max(0.0);

            if !resampled_buffer.is_empty() {
                let pushed = producer.push_slice(&resampled_buffer);
                if pushed < resampled_buffer.len() {
                    static DROP_COUNT: AtomicU32 = AtomicU32::new(0);
                    let prev = DROP_COUNT.fetch_add(1, Ordering::Relaxed);
                    if prev.is_multiple_of(INGESTION_OVERFLOW_LOG_INTERVAL) {
                        log::warn!(
                            "[Audio::Device] Ring buffer overflow! Dropped {} chunks so far",
                            prev + 1
                        );
                    }
                }
            }
        },
        move |err| {
            log::error!("[Audio::Device] Stream error: {}", err);
        },
        None,
    )?;

    Ok(stream)
}

/// Resolves default output device and validates 48kHz stereo stream config.
pub fn resolve_output_device_and_config(host: &cpal::Host) -> Result<(cpal::Device, StreamConfig)> {
    let device = host
        .default_output_device()
        .ok_or_else(|| anyhow!("[Audio::Playback] No default output device found"))?;

    log::info!("[Audio::Playback] Output device: {:?}", device.name());

    let config = StreamConfig {
        channels: PLAYBACK_CHANNELS,
        sample_rate: cpal::SampleRate(PLAYBACK_SAMPLE_RATE),
        buffer_size: cpal::BufferSize::Default,
    };

    let supported = device
        .supported_output_configs()
        .map_err(|e| anyhow!("[Audio::Playback] Failed to query output configs: {}", e))?
        .find(|c| {
            c.channels() == PLAYBACK_CHANNELS
                && c.sample_format() == SampleFormat::F32
                && c.min_sample_rate().0 <= PLAYBACK_SAMPLE_RATE
                && c.max_sample_rate().0 >= PLAYBACK_SAMPLE_RATE
        });

    if supported.is_none() {
        log::warn!(
            "[Audio::Playback] {}Hz stereo f32 not reported as supported — trying anyway",
            PLAYBACK_SAMPLE_RATE
        );
    }

    Ok((device, config))
}

/// Builds and starts the CPAL 48kHz stereo output stream with the sacred callback sink.
pub fn build_output_stream(
    consumer: HeapCons<f32>,
    handles: PlaybackEngineHandles,
    discard_request: Arc<AtomicBool>,
    turn_armed: Arc<AtomicBool>,
    telemetry: &PlaybackTelemetryHandles,
) -> Result<cpal::Stream> {
    let host = resolve_audio_host();
    let (device, config) = resolve_output_device_and_config(&host)?;

    let mut cb_ctx =
        PlaybackStreamContext::new(consumer, handles, discard_request, turn_armed, telemetry);

    let stream = device
        .build_output_stream(
            &config,
            move |output: &mut [f32], _info| {
                cb_ctx.process_output_buffer(output);
            },
            move |err| {
                log::error!("[Audio::Playback] CPAL output error: {}", err);
            },
            None,
        )
        .map_err(|e| anyhow!("[Audio::Playback] Failed to build output stream: {}", e))?;

    stream
        .play()
        .map_err(|e| anyhow!("[Audio::Playback] Failed to start stream: {}", e))?;
    log::info!(
        "[Audio::Playback] CPAL stream started ({}Hz stereo f32)",
        PLAYBACK_SAMPLE_RATE
    );

    Ok(stream)
}
