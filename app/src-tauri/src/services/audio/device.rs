use anyhow::Result;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use ringbuf::traits::*;

/// Manages the low-level CPAL hardware audio stream.
pub struct AudioStream {
    _stream: cpal::Stream,
}

unsafe impl Send for AudioStream {}
unsafe impl Sync for AudioStream {}

impl AudioStream {
    /// Creates and configures a new hardware audio ingestion stream.
    pub fn new<P>(producer: P, device_name: Option<String>) -> Result<Self>
    where
        P: Producer<Item = f32> + Send + 'static,
    {
        let (device, config) = resolve_input_device(device_name.as_deref())?;
        let sample_rate = config.sample_rate.0;
        let channels = config.channels as usize;

        log::info!(
            "[AUDIO] Using input device: {}",
            device.name().unwrap_or_else(|_| "Unknown".into())
        );
        log::info!("[AUDIO] Config: {}Hz, {} channels", sample_rate, channels);

        let stream = build_input_stream(device, &config, channels, sample_rate, producer)?;
        Ok(Self { _stream: stream })
    }

    /// Starts the hardware audio stream.
    pub fn start(&self) -> Result<()> {
        log::info!("[AUDIO] Starting hardware ingestion...");
        self._stream.play()?;
        Ok(())
    }
}

/// Resolves the default audio host and queries the requested or fallback input device.
fn resolve_input_device(device_name: Option<&str>) -> Result<(cpal::Device, cpal::StreamConfig)> {
    let host = resolve_audio_host();
    log::info!("[AUDIO] Using audio host: {:?}", host.id());

    let device = if let Some(name) = device_name {
        log::info!("[AUDIO] Attempting to use requested device: {}", name);
        host.input_devices()?
            .find(|d| d.name().map(|n| n == name).unwrap_or(false))
            .or_else(|| {
                log::warn!(
                    "[AUDIO] Requested device '{}' not found, falling back to default",
                    name
                );
                host.default_input_device()
            })
    } else {
        host.default_input_device()
    }
    .ok_or_else(|| anyhow::anyhow!("No input device found on host {:?}", host.id()))?;

    let config: cpal::StreamConfig = device.default_input_config()?.into();
    Ok((device, config))
}

/// Resolves the preferred CPAL host platform.
fn resolve_audio_host() -> cpal::Host {
    cpal::default_host()
}

/// Builds the CPAL input ingestion stream with lock-free ringbuf forwarding.
fn build_input_stream<P>(
    device: cpal::Device,
    config: &cpal::StreamConfig,
    channels: usize,
    sample_rate: u32,
    mut producer: P,
) -> Result<cpal::Stream>
where
    P: Producer<Item = f32> + Send + 'static,
{
    let mut source_index: f32 = 0.0;
    let resample_ratio = sample_rate as f32 / 16000.0;
    let mut mono_buffer = Vec::with_capacity(8192);
    let mut resampled_buffer = Vec::with_capacity(8192);

    let stream = device.build_input_stream(
        config,
        move |data: &[f32], _: &cpal::InputCallbackInfo| {
            mono_buffer.clear();
            resampled_buffer.clear();

            for chunk in data.chunks_exact(channels) {
                let avg: f32 = chunk.iter().sum::<f32>() / channels as f32;
                mono_buffer.push(avg);
            }

            let n_mono = mono_buffer.len();
            while (source_index as usize) < n_mono {
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
                    static DROP_COUNT: std::sync::atomic::AtomicU32 =
                        std::sync::atomic::AtomicU32::new(0);
                    let prev = DROP_COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    if prev.is_multiple_of(100) {
                        log::warn!(
                            "[AUDIO] Ring buffer overflow! Dropped {} chunks so far.",
                            prev + 1
                        );
                    }
                }
            }
        },
        move |err| {
            log::error!("[AUDIO] Stream error: {}", err);
        },
        None,
    )?;

    Ok(stream)
}

impl Drop for AudioStream {
    /// Ensures the hardware stream is paused cleanly when dropping.
    fn drop(&mut self) {
        log::info!("[AUDIO] Dropping hardware stream. Ensuring mic is released.");
        if let Err(e) = self._stream.pause() {
            log::warn!("[AUDIO] Failed to pause stream on drop: {}", e);
        }
    }
}
