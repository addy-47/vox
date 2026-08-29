use super::{
    PLAYBACK_BUFFER_SAMPLES, PLAYBACK_CHANNELS, PLAYBACK_DEFAULT_VOLUME, PLAYBACK_ENERGY_EXPONENT,
    PLAYBACK_ENERGY_MULTIPLIER, PLAYBACK_SAMPLE_RATE, PLAYBACK_VOLUME_RAMP_STEP,
};
use anyhow::{anyhow, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{SampleFormat, StreamConfig};
use parking_lot::Mutex;
use ringbuf::traits::*;
use ringbuf::{HeapCons, HeapProd};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// Upsample 24kHz mono PCM to 48kHz via cubic Hermite interpolation into a reusable buffer.
#[inline]
pub fn upsample_2x_into(input: &[f32], out: &mut Vec<f32>) {
    out.clear();
    if input.is_empty() {
        return;
    }
    let len = input.len();
    out.reserve(len * 2);
    for i in 0..len {
        let p1 = input[i];
        out.push(p1);

        let p0 = if i > 0 { input[i - 1] } else { p1 };
        let p2 = if i + 1 < len { input[i + 1] } else { p1 };
        let p3 = if i + 2 < len { input[i + 2] } else { p2 };

        let midpoint = (-p0 + 9.0 * p1 + 9.0 * p2 - p3) / 16.0;
        out.push(midpoint);
    }
}

/// Upsample 24kHz mono PCM to 48kHz via cubic Hermite interpolation.
#[inline]
pub fn upsample_2x(input: &[f32]) -> Vec<f32> {
    let mut out = Vec::with_capacity(input.len() * 2);
    upsample_2x_into(input, &mut out);
    out
}

/// Core audio output playback engine managing CPAL stream draining and telemetry.
pub struct PlaybackEngine {
    producer: Mutex<(HeapProd<f32>, Vec<f32>)>,
    playback_active: Arc<AtomicBool>,
    cancel_flag: Arc<AtomicBool>,
    discard_request: Arc<AtomicBool>,
    _stream: Option<cpal::Stream>,
}

/// Telemetry and visualization atomics passed to the playback engine.
#[derive(Clone)]
pub struct PlaybackTelemetryHandles {
    pub energy: Arc<std::sync::atomic::AtomicU32>,
    pub low: Arc<std::sync::atomic::AtomicU32>,
    pub mid: Arc<std::sync::atomic::AtomicU32>,
    pub high: Arc<std::sync::atomic::AtomicU32>,
    pub underruns: Arc<std::sync::atomic::AtomicU64>,
}

unsafe impl Send for PlaybackEngine {}
unsafe impl Sync for PlaybackEngine {}

impl PlaybackEngine {
    /// Initialise CPAL output stream at 48kHz without starting playback immediately.
    pub fn new(
        playback_active: Arc<AtomicBool>,
        cancel_flag: Arc<AtomicBool>,
        is_assistant_speaking: Arc<AtomicBool>,
        telemetry: PlaybackTelemetryHandles,
    ) -> Result<Self> {
        let rb = ringbuf::HeapRb::<f32>::new(PLAYBACK_BUFFER_SAMPLES);
        let (producer, consumer) = rb.split();
        let discard_request = Arc::new(AtomicBool::new(false));

        let stream = Self::build_cpal_stream(
            consumer,
            Arc::clone(&playback_active),
            Arc::clone(&cancel_flag),
            Arc::clone(&discard_request),
            Arc::clone(&is_assistant_speaking),
            &telemetry,
        )?;

        Ok(Self {
            producer: Mutex::new((producer, Vec::with_capacity(4096))),
            playback_active,
            cancel_flag,
            discard_request,
            _stream: Some(stream),
        })
    }

    /// Ingest a 24kHz audio chunk from TTS, upsample 2x to 48kHz, and push to ring buffer.
    pub fn ingest_chunk(&self, chunk_24khz: &[f32]) {
        if self.cancel_flag.load(Ordering::Relaxed) {
            return;
        }

        let mut guard = self.producer.lock();
        let (ref mut prod, ref mut scratch) = *guard;
        upsample_2x_into(chunk_24khz, scratch);

        let pushed = prod.push_slice(scratch);
        if pushed < scratch.len() {
            log::warn!(
                "[Audio::Playback] Buffer overflow — dropped {} samples",
                scratch.len() - pushed
            );
        }
    }

    /// Explicitly triggers CPAL playback if samples are available in the buffer.
    pub fn start_playback(&self) {
        if self.cancel_flag.load(Ordering::Relaxed) {
            return;
        }
        if !self.playback_active.load(Ordering::Relaxed) {
            let current_len = self.producer.lock().0.occupied_len();
            if current_len > 0 {
                log::info!(
                    "[Audio::Playback] start_playback requested ({} samples buffered) — starting output",
                    current_len
                );
                self.playback_active.store(true, Ordering::Relaxed);
            }
        }
    }

    /// Cancels active playback, signals consumer discard, and resets buffer count.
    pub fn cancel(&self) {
        self.cancel_flag.store(true, Ordering::Relaxed);
        self.playback_active.store(false, Ordering::Relaxed);
        self.discard_request.store(true, Ordering::Relaxed);
        log::info!("[Audio::Playback] Cancelled — buffer signal sent");
    }

    /// Returns the number of unplayed audio samples remaining in the buffer.
    pub fn buffer_len(&self) -> usize {
        self.producer.lock().0.occupied_len()
    }

    /// Builds and starts the CPAL 48kHz stereo output stream.
    fn build_cpal_stream(
        consumer: HeapCons<f32>,
        playback_active: Arc<AtomicBool>,
        cancel_flag: Arc<AtomicBool>,
        discard_request: Arc<AtomicBool>,
        is_assistant_speaking: Arc<AtomicBool>,
        telemetry: &PlaybackTelemetryHandles,
    ) -> Result<cpal::Stream> {
        let host = cpal::default_host();
        let (device, config) = resolve_output_device_and_config(&host)?;

        let mut cb_ctx = PlaybackStreamContext {
            consumer,
            playback_active,
            cancel_flag,
            discard_request,
            playback_energy: Arc::clone(&telemetry.energy),
            playback_low: Arc::clone(&telemetry.low),
            playback_mid: Arc::clone(&telemetry.mid),
            playback_high: Arc::clone(&telemetry.high),
            playback_underruns: Arc::clone(&telemetry.underruns),
            is_assistant_speaking,
            last_sample: 0.0,
            current_volume: PLAYBACK_DEFAULT_VOLUME,
            filter_bank: crate::utils::audio_filters::FilterBank::new(PLAYBACK_SAMPLE_RATE as f32),
        };

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
}

/// Resolves default output device and validates 48kHz stereo stream config.
fn resolve_output_device_and_config(host: &cpal::Host) -> Result<(cpal::Device, StreamConfig)> {
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

/// Context state held by the real-time CPAL output stream callback.
struct PlaybackStreamContext {
    consumer: HeapCons<f32>,
    playback_active: Arc<AtomicBool>,
    cancel_flag: Arc<AtomicBool>,
    discard_request: Arc<AtomicBool>,
    playback_energy: Arc<std::sync::atomic::AtomicU32>,
    playback_low: Arc<std::sync::atomic::AtomicU32>,
    playback_mid: Arc<std::sync::atomic::AtomicU32>,
    playback_high: Arc<std::sync::atomic::AtomicU32>,
    playback_underruns: Arc<std::sync::atomic::AtomicU64>,
    is_assistant_speaking: Arc<AtomicBool>,
    last_sample: f32,
    current_volume: f32,
    filter_bank: crate::utils::audio_filters::FilterBank,
}

impl PlaybackStreamContext {
    /// Handles buffer drain, cancellation, discard requests, and audio output generation.
    fn process_output_buffer(&mut self, output: &mut [f32]) {
        if self.discard_request.load(Ordering::Relaxed) {
            self.consumer.skip(self.consumer.occupied_len());
            self.discard_request.store(false, Ordering::Relaxed);
            self.reset_telemetry_state();
        }

        if !self.playback_active.load(Ordering::Relaxed) || self.cancel_flag.load(Ordering::Relaxed)
        {
            if self.cancel_flag.load(Ordering::Relaxed) {
                self.consumer.skip(self.consumer.occupied_len());
            }
            self.reset_telemetry_state();
            output.fill(0.0);
            return;
        }

        self.drain_and_telemetry(output);
    }

    /// Resets filter bank and smoothing state when playback stops or is discarded.
    fn reset_telemetry_state(&mut self) {
        self.last_sample = 0.0;
        self.current_volume = PLAYBACK_DEFAULT_VOLUME;
        self.filter_bank.reset();
    }

    /// Drains ringbuf audio frames, applies click-free volume ramp, and calculates telemetry.
    fn drain_and_telemetry(&mut self, output: &mut [f32]) {
        let frames = output.len() / PLAYBACK_CHANNELS as usize;
        let mut sum_sq = 0.0;
        let mut sum_low_sq = 0.0;
        let mut sum_mid_sq = 0.0;
        let mut sum_high_sq = 0.0;

        for frame in 0..frames {
            let sample_opt = self.consumer.try_pop();
            let (sample, target_volume) = match sample_opt {
                Some(s) => {
                    self.last_sample = s;
                    (s, PLAYBACK_DEFAULT_VOLUME)
                }
                None => (self.last_sample, 0.0),
            };

            if self.current_volume < target_volume {
                self.current_volume =
                    (self.current_volume + PLAYBACK_VOLUME_RAMP_STEP).min(target_volume);
            } else if self.current_volume > target_volume {
                self.current_volume =
                    (self.current_volume - PLAYBACK_VOLUME_RAMP_STEP).max(target_volume);
            }

            let played_sample = sample * self.current_volume;
            sum_sq += played_sample * played_sample;

            let (low, mid, high) = self.filter_bank.tick(played_sample);
            sum_low_sq += low * low;
            sum_mid_sq += mid * mid;
            sum_high_sq += high * high;

            output[frame * 2] = played_sample;
            output[frame * 2 + 1] = played_sample;
        }

        self.update_energy_metrics(frames, sum_sq, sum_low_sq, sum_mid_sq, sum_high_sq);

        if self.consumer.is_empty() {
            if self.is_assistant_speaking.load(Ordering::Relaxed) {
                self.playback_underruns.fetch_add(1, Ordering::Relaxed);
            }
            self.playback_active.store(false, Ordering::Relaxed);
            self.playback_energy
                .store(0f32.to_bits(), Ordering::Relaxed);
            self.playback_low.store(0f32.to_bits(), Ordering::Relaxed);
            self.playback_mid.store(0f32.to_bits(), Ordering::Relaxed);
            self.playback_high.store(0f32.to_bits(), Ordering::Relaxed);
            self.reset_telemetry_state();
        }
    }

    /// Updates atomic playback energy, low, mid, and high band metrics.
    fn update_energy_metrics(
        &self,
        frames: usize,
        sum_sq: f32,
        sum_low_sq: f32,
        sum_mid_sq: f32,
        sum_high_sq: f32,
    ) {
        let raw_energy = (sum_sq / frames as f32).sqrt();
        let energy = (raw_energy * PLAYBACK_ENERGY_MULTIPLIER).clamp(0.0, 1.0);
        self.playback_energy
            .store(energy.to_bits(), Ordering::Relaxed);

        let raw_low = (sum_low_sq / frames as f32).sqrt();
        let raw_mid = (sum_mid_sq / frames as f32).sqrt();
        let raw_high = (sum_high_sq / frames as f32).sqrt();

        let low_val = (raw_low * PLAYBACK_ENERGY_MULTIPLIER)
            .clamp(0.0, 1.0)
            .powf(PLAYBACK_ENERGY_EXPONENT);
        let mid_val = (raw_mid * PLAYBACK_ENERGY_MULTIPLIER)
            .clamp(0.0, 1.0)
            .powf(PLAYBACK_ENERGY_EXPONENT);
        let high_val = (raw_high * PLAYBACK_ENERGY_MULTIPLIER)
            .clamp(0.0, 1.0)
            .powf(PLAYBACK_ENERGY_EXPONENT);

        self.playback_low
            .store(low_val.to_bits(), Ordering::Relaxed);
        self.playback_mid
            .store(mid_val.to_bits(), Ordering::Relaxed);
        self.playback_high
            .store(high_val.to_bits(), Ordering::Relaxed);
    }
}

impl Drop for PlaybackEngine {
    /// Cleans up and halts output on engine drop.
    fn drop(&mut self) {
        self.cancel();
    }
}
