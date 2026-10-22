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
    sink::{PlaybackReferenceTap, PlaybackStreamContext},
    AudioCapture, AudioSink, MockAudioCapture, PlaybackEngineHandles, PlaybackTelemetryHandles,
    INGESTION_BUFFER_CAPACITY_SAMPLES, INGESTION_OVERFLOW_LOG_INTERVAL, PLAYBACK_CHANNELS,
    PLAYBACK_SAMPLE_RATE, SAMPLE_RATE,
};

/// Hardware audio capture implementation wrapping a live CPAL input stream.
pub struct CpalAudioCapture {
    stream: cpal::Stream,
}

unsafe impl Send for CpalAudioCapture {}
unsafe impl Sync for CpalAudioCapture {}

impl AudioCapture for CpalAudioCapture {
    fn start(&self) -> Result<()> {
        log::info!("[Audio::Device] Starting hardware ingestion");
        self.stream
            .play()
            .map_err(|e| anyhow!("Failed to start ingestion stream: {}", e))
    }

    fn pause(&self) -> Result<()> {
        log::info!("[Audio::Device] Pausing hardware ingestion");
        self.stream
            .pause()
            .map_err(|e| anyhow!("Failed to pause ingestion stream: {}", e))
    }
}

impl Drop for CpalAudioCapture {
    fn drop(&mut self) {
        log::info!("[Audio::Device] Dropping hardware stream. Ensuring mic is released");
        if let Err(e) = self.stream.pause() {
            log::warn!("[Audio::Device] Failed to pause stream on drop: {}", e);
        }
    }
}

/// Manages the low-level audio ingestion stream for microphone capture via a pluggable AudioCapture backend.
pub struct AudioStream {
    capture: Box<dyn AudioCapture>,
}

unsafe impl Send for AudioStream {}
unsafe impl Sync for AudioStream {}

impl AudioStream {
    /// Creates and configures a new hardware audio ingestion stream using CPAL.
    pub fn new<P>(producer: P, ingestion_gate: Arc<AtomicBool>) -> Result<Self>
    where
        P: Producer<Item = f32> + Send + 'static,
    {
        let (device, config) = resolve_input_device()?;
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
            capture: Box::new(CpalAudioCapture { stream }),
        })
    }

    /// Creates an AudioStream from an arbitrary AudioCapture trait object (network, mock, etc.).
    pub fn from_capture(capture: Box<dyn AudioCapture>) -> Self {
        Self { capture }
    }

    /// Creates a mock AudioStream for integration testing without audio hardware.
    pub fn mock() -> Self {
        Self {
            capture: Box::new(MockAudioCapture),
        }
    }

    /// Starts the hardware audio stream.
    pub fn start(&self) -> Result<()> {
        self.capture.start()
    }

    /// Pauses the hardware audio stream.
    pub fn pause(&self) -> Result<()> {
        self.capture.pause()
    }
}

impl Drop for AudioStream {
    fn drop(&mut self) {
        let _ = self.capture.pause();
    }
}

/// Resolves the default audio host and queries the default input device.
fn resolve_input_device() -> Result<(cpal::Device, cpal::StreamConfig)> {
    let host = resolve_audio_host();
    log::info!("[Audio::Device] Using audio host: {:?}", host.id());

    let device = host
        .default_input_device()
        .ok_or_else(|| anyhow!("No input device found on host {:?}", host.id()))?;

    let config: cpal::StreamConfig = device.default_input_config()?.into();
    Ok((device, config))
}

/// Resolves the preferred CPAL host platform.
pub fn resolve_audio_host() -> cpal::Host {
    cpal::default_host()
}

/// Builds the CPAL input ingestion stream with lock-free ringbuf forwarding and anti-aliasing downsampling.
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
    let mut filtered_buffer = Vec::with_capacity(INGESTION_BUFFER_CAPACITY_SAMPLES);
    let mut resampled_buffer = Vec::with_capacity(INGESTION_BUFFER_CAPACITY_SAMPLES);

    let stream = device.build_input_stream(
        config,
        move |data: &[f32], _: &cpal::InputCallbackInfo| {
            if !ingestion_gate.load(Ordering::Relaxed) {
                return;
            }

            mono_buffer.clear();
            filtered_buffer.clear();
            resampled_buffer.clear();

            for chunk in data.chunks_exact(channels) {
                if mono_buffer.len() >= INGESTION_BUFFER_CAPACITY_SAMPLES {
                    break;
                }
                let avg: f32 = chunk.iter().sum::<f32>() / channels as f32;
                mono_buffer.push(avg);
            }

            let n_mono = mono_buffer.len();
            if n_mono == 0 {
                return;
            }

            // Anti-aliasing low-pass filter (3-tap moving average) prior to decimation
            // to prevent high-frequency noise (8kHz–22kHz) from aliasing into the speech band.
            filtered_buffer.resize(n_mono, 0.0);
            if n_mono >= 3 {
                filtered_buffer[0] = mono_buffer[0];
                for i in 1..n_mono - 1 {
                    filtered_buffer[i] = 0.25 * mono_buffer[i - 1]
                        + 0.5 * mono_buffer[i]
                        + 0.25 * mono_buffer[i + 1];
                }
                filtered_buffer[n_mono - 1] = mono_buffer[n_mono - 1];
            } else {
                filtered_buffer.copy_from_slice(&mono_buffer);
            }

            while (source_index as usize) < n_mono {
                if resampled_buffer.len() >= INGESTION_BUFFER_CAPACITY_SAMPLES {
                    break;
                }
                let idx = source_index as usize;
                let next_idx = (idx + 1).min(n_mono - 1);
                let frac = source_index - idx as f32;
                let sample = (1.0 - frac) * filtered_buffer[idx] + frac * filtered_buffer[next_idx];
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

/// Concrete hardware audio playback sink wrapping a live CPAL stream.
pub struct CpalAudioSink {
    _stream: cpal::Stream,
}

unsafe impl Send for CpalAudioSink {}
unsafe impl Sync for CpalAudioSink {}

impl AudioSink for CpalAudioSink {}

/// Builds and starts the CPAL 48kHz stereo output stream with the sacred callback sink.
pub fn build_output_stream(
    consumer: HeapCons<f32>,
    handles: PlaybackEngineHandles,
    discard_request: Arc<AtomicBool>,
    turn_armed: Arc<AtomicBool>,
    telemetry: &PlaybackTelemetryHandles,
    reference_tap: Option<PlaybackReferenceTap>,
) -> Result<Box<dyn AudioSink>> {
    let host = resolve_audio_host();
    let (device, config) = resolve_output_device_and_config(&host)?;

    let mut cb_ctx = PlaybackStreamContext::new(
        consumer,
        handles,
        discard_request,
        turn_armed,
        telemetry,
        reference_tap,
    );

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

    Ok(Box::new(CpalAudioSink { _stream: stream }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::audio::{MockAudioCapture, MockAudioSink};

    /// PURPOSE:  The mock capture must support the full capture lifecycle, or headless runs cannot exercise the pipeline.
    /// SCENARIO: A mock AudioStream is started and paused.
    /// ASSERTS:  Start and pause both succeed.
    /// CLASS:    happy-path
    /// REQUIRES: none
    /// MUTANT:   Make AudioStream::mock() fail on start.
    #[test]
    fn audio_stream_mock_starts_and_pauses() {
        let stream = AudioStream::mock();
        assert!(stream.start().is_ok());
        assert!(stream.pause().is_ok());
    }

    /// PURPOSE:  A custom capture must be injectable, or the capture path cannot be tested headlessly.
    /// SCENARIO: A stream is built from a MockAudioCapture and then started and paused.
    /// ASSERTS:  Start and pause both succeed.
    /// CLASS:    happy-path
    /// REQUIRES: none
    /// MUTANT:   Ignore the capture passed to AudioStream::from_capture.
    #[test]
    fn audio_stream_accepts_an_injected_capture() {
        let capture = Box::new(MockAudioCapture);
        let stream = AudioStream::from_capture(capture);
        assert!(stream.start().is_ok());
        assert!(stream.pause().is_ok());
    }

    /// PURPOSE:  A mock sink must be constructible, or playback tests cannot run without a device.
    /// SCENARIO: A MockAudioSink is boxed as a trait object and dropped.
    /// ASSERTS:  The sink constructs and drops without panicking.
    /// CLASS:    happy-path
    /// REQUIRES: none
    /// MUTANT:   Make the Drop impl of MockAudioSink panic.
    #[test]
    fn audio_sink_mock_can_be_constructed_and_dropped() {
        let sink: Box<dyn AudioSink> = Box::new(MockAudioSink);
        drop(sink);
    }
}
