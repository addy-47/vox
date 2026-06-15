//! Playback Runtime — CPAL audio output with jitter buffer and 2x upsampling.
//!
//! Architecture:
//!   TtsChunk (24kHz f32) → upsample_2x() → ring buffer → CPAL callback (48kHz)
//!
//! Directive 3: upsample_2x() is a hand-rolled linear interpolator for the
//! exact 24kHz→48kHz 2× integer ratio. No FFT. No external resampling crate.
//! The CPAL callback does ZERO computation — it only drains the pre-upsampled buffer.

use anyhow::{anyhow, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{SampleFormat, StreamConfig};
use ringbuf::traits::*;
use ringbuf::{HeapCons, HeapProd};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

// ─── Resampling (Directive 3) ─────────────────────────────────────────────────

/// Upsample 24kHz mono PCM → 48kHz via linear interpolation (exact 2× ratio).
///
/// Runs at buffer-ingestion time — NEVER inside the CPAL callback.
/// Time complexity: O(n). No allocations except the output Vec.
/// Zero dependency on any external resampling library.
#[inline]
pub fn upsample_2x(input: &[f32]) -> Vec<f32> {
    if input.is_empty() {
        return Vec::new();
    }
    let len = input.len();
    let mut out = Vec::with_capacity(len * 2);
    for i in 0..len {
        let p1 = input[i];
        out.push(p1);

        // Cubic Hermite (Catmull-Rom) interpolation for the midpoint sample.
        // w = [-1/16, 9/16, 9/16, -1/16]
        let p0 = if i > 0 { input[i - 1] } else { p1 };
        let p2 = if i + 1 < len { input[i + 1] } else { p1 };
        let p3 = if i + 2 < len { input[i + 2] } else { p2 };

        let midpoint = (-p0 + 9.0 * p1 + 9.0 * p2 - p3) / 16.0;
        out.push(midpoint);
    }
    out
}

// ─── Playback Engine ──────────────────────────────────────────────────────────

pub struct PlaybackEngine {
    /// Producer half of the lock-free ring buffer (Mono 48kHz).
    producer: std::sync::Mutex<HeapProd<f32>>,
    /// `true` while CPAL is actively draining (used for mic ducking in Speaker mode).
    playback_active: Arc<AtomicBool>,
    /// Set `true` to clear the buffer and stop playback.
    cancel_flag: Arc<AtomicBool>,
    /// Set `true` to request the CPAL stream thread to discard all buffered audio.
    discard_request: Arc<AtomicBool>,
    /// The active CPAL stream — kept alive until cancelled.
    _stream: Option<cpal::Stream>,
    /// RCA Fix: Real-time safe energy telemetry (Atomic f32 via bit storage)
    _playback_energy: Arc<std::sync::atomic::AtomicU32>,
    _playback_low: Arc<std::sync::atomic::AtomicU32>,
    _playback_mid: Arc<std::sync::atomic::AtomicU32>,
    _playback_high: Arc<std::sync::atomic::AtomicU32>,
    /// Track underruns specifically when AssistantSpeaking is true.
    _playback_underruns: Arc<std::sync::atomic::AtomicU64>,
    /// Ref to the state atomic for lock-free checks.
    _is_assistant_speaking: Arc<AtomicBool>,
    /// Track current buffer level for pre-buffering logic.
    buffer_samples: Arc<std::sync::atomic::AtomicUsize>,
    /// Total samples ingested during the current turn.
    total_samples_ingested: Arc<std::sync::atomic::AtomicUsize>,
}

// Safety: cpal::Stream is not Send/Sync on some platforms (macOS), but is
// generally safe to move on Linux. We own the stream and manage its lifecycle.
unsafe impl Send for PlaybackEngine {}
unsafe impl Sync for PlaybackEngine {}

impl PlaybackEngine {
    /// Initialise CPAL output stream at 48kHz. Does not start playback yet.
    pub fn new(
        playback_active: Arc<AtomicBool>,
        cancel_flag: Arc<AtomicBool>,
        playback_energy: Arc<std::sync::atomic::AtomicU32>,
        playback_low: Arc<std::sync::atomic::AtomicU32>,
        playback_mid: Arc<std::sync::atomic::AtomicU32>,
        playback_high: Arc<std::sync::atomic::AtomicU32>,
        playback_underruns: Arc<std::sync::atomic::AtomicU64>,
        is_assistant_speaking: Arc<AtomicBool>,
    ) -> Result<Self> {
        // Create 30s buffer at 48kHz (1,440,000 samples) to prevent overflow on long TTS segments
        let rb = ringbuf::HeapRb::<f32>::new(48_000 * 30);
        let (producer, consumer) = rb.split();
        let buffer_samples = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let discard_request = Arc::new(AtomicBool::new(false));

        let stream = Self::build_cpal_stream(
            consumer,
            Arc::clone(&playback_active),
            Arc::clone(&cancel_flag),
            Arc::clone(&discard_request),
            Arc::clone(&playback_energy),
            Arc::clone(&playback_low),
            Arc::clone(&playback_mid),
            Arc::clone(&playback_high),
            Arc::clone(&playback_underruns),
            Arc::clone(&is_assistant_speaking),
            Arc::clone(&buffer_samples),
        )?;

        Ok(Self {
            producer: std::sync::Mutex::new(producer),
            playback_active,
            cancel_flag,
            discard_request,
            _stream: Some(stream),
            _playback_energy: playback_energy,
            _playback_low: playback_low,
            _playback_mid: playback_mid,
            _playback_high: playback_high,
            _playback_underruns: playback_underruns,
            _is_assistant_speaking: is_assistant_speaking,
            buffer_samples,
            total_samples_ingested: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
        })
    }

    /// Ingest a 24kHz audio chunk from TTS.
    ///
    /// 1. Upsample 2x → 48kHz (Directive 3)
    /// 2. Push to ring buffer
    /// 3. Once ≥ JITTER_PREBUFFER_SAMPLES available, mark playback_active = true
    pub fn ingest_chunk(&self, chunk_24khz: &[f32]) {
        if self.cancel_flag.load(Ordering::Relaxed) {
            return;
        }

        let upsampled = upsample_2x(chunk_24khz);
        let mut prod = self.producer.lock().unwrap();

        let pushed = prod.push_slice(&upsampled);
        if pushed < upsampled.len() {
            log::warn!(
                "[Playback] Buffer overflow — dropped {} samples",
                upsampled.len() - pushed
            );
        }

        self.buffer_samples.fetch_add(pushed, Ordering::SeqCst);
        self.total_samples_ingested.fetch_add(pushed, Ordering::SeqCst);
    }

    /// Explicitly trigger CPAL playback.
    pub fn start_playback(&self) {
        if self.cancel_flag.load(Ordering::Relaxed) {
            return;
        }
        if !self.playback_active.load(Ordering::Relaxed) {
            let current_len = self.buffer_samples.load(Ordering::SeqCst);
            if current_len > 0 {
                log::info!(
                    "[Playback] start_playback requested ({} samples buffered) — starting output",
                    current_len
                );
                self.playback_active.store(true, Ordering::Relaxed);
            }
        }
    }

    /// Cancel active playback: clear buffer and signal CPAL callback to stop.
    pub fn cancel(&self) {
        self.cancel_flag.store(true, Ordering::Relaxed);
        self.playback_active.store(false, Ordering::Relaxed);
        self.discard_request.store(true, Ordering::Relaxed);
        if let Ok(_prod) = self.producer.lock() {
            // No easy 'clear' in ringbuf v0.4 producer, but we can't block here.
            // The callback will see cancel_flag and drop its own consumer state.
        }
        self.buffer_samples.store(0, Ordering::SeqCst);
        log::info!("[Playback] Cancelled — buffer signal sent");
    }

    /// Returns `true` if the playback buffer is empty and CPAL has gone idle.
    pub fn is_idle(&self) -> bool {
        !self.playback_active.load(Ordering::Relaxed)
    }

    /// Returns the number of samples remaining in the playback buffer.
    pub fn buffer_len(&self) -> usize {
        self.buffer_samples.load(Ordering::Relaxed)
    }

    pub fn total_samples_ingested(&self) -> usize {
        self.total_samples_ingested.load(Ordering::Relaxed)
    }

    pub fn reset_samples_ingested(&self) {
        self.total_samples_ingested.store(0, Ordering::Relaxed);
    }

    // ── Private ───────────────────────────────────────────────────────────────

    fn build_cpal_stream(
        mut consumer: HeapCons<f32>,
        playback_active: Arc<AtomicBool>,
        cancel_flag: Arc<AtomicBool>,
        discard_request: Arc<AtomicBool>,
        playback_energy: Arc<std::sync::atomic::AtomicU32>,
        playback_low: Arc<std::sync::atomic::AtomicU32>,
        playback_mid: Arc<std::sync::atomic::AtomicU32>,
        playback_high: Arc<std::sync::atomic::AtomicU32>,
        playback_underruns: Arc<std::sync::atomic::AtomicU64>,
        is_assistant_speaking: Arc<AtomicBool>,
        buffer_samples: Arc<std::sync::atomic::AtomicUsize>,
    ) -> Result<cpal::Stream> {
        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .ok_or_else(|| anyhow!("[Playback] No default output device found"))?;

        log::info!("[Playback] Output device: {:?}", device.name());

        // Request 48kHz stereo output (most common default)
        let config = StreamConfig {
            channels: 2,
            sample_rate: cpal::SampleRate(48_000),
            buffer_size: cpal::BufferSize::Default,
        };

        let supported = device
            .supported_output_configs()
            .map_err(|e| anyhow!("[Playback] Failed to query output configs: {}", e))?
            .find(|c| {
                c.channels() == 2
                    && c.sample_format() == SampleFormat::F32
                    && c.min_sample_rate().0 <= 48_000
                    && c.max_sample_rate().0 >= 48_000
            });

        if supported.is_none() {
            log::warn!("[Playback] 48kHz stereo f32 not reported as supported — trying anyway");
        }

        let mut last_sample = 0.0f32;
        let mut current_volume = 1.0f32;
        let mut filter_bank = crate::utils::audio_filters::FilterBank::new(48_000.0);

        let stream = device
            .build_output_stream(
                &config,
                // ── CPAL data callback — ZERO computation (Directive 3) ──────────
                // Only drains the pre-upsampled ring buffer.
                move |output: &mut [f32], _info| {
                    if discard_request.load(Ordering::Relaxed) {
                        consumer.skip(consumer.occupied_len());
                        buffer_samples.store(0, Ordering::SeqCst);
                        discard_request.store(false, Ordering::Relaxed);
                        last_sample = 0.0;
                        current_volume = 1.0;
                        filter_bank.reset();
                    }

                    if !playback_active.load(Ordering::Relaxed)
                        || cancel_flag.load(Ordering::Relaxed)
                    {
                        // Clear the consumer if cancelled to drop old audio
                        if cancel_flag.load(Ordering::Relaxed) {
                            consumer.skip(consumer.occupied_len());
                            buffer_samples.store(0, Ordering::SeqCst);
                        }
                        last_sample = 0.0;
                        current_volume = 1.0;
                        filter_bank.reset();
                        output.fill(0.0);
                        return;
                    }

                    // Lock-free read from SPSC ring buffer
                    let frames = output.len() / 2; // stereo → mono frames
                    let mut sum_sq = 0.0;
                    let mut sum_low_sq = 0.0;
                    let mut sum_mid_sq = 0.0;
                    let mut sum_high_sq = 0.0;
                    let mut read_count = 0;

                    for frame in 0..frames {
                        let sample_opt = consumer.try_pop();
                        let (sample, target_volume) = match sample_opt {
                            Some(s) => {
                                read_count += 1;
                                last_sample = s;
                                (s, 1.0)
                            }
                            None => (last_sample, 0.0),
                        };

                        // Linear fade to avoid clicks (approx 10ms fade window at 48kHz)
                        let step = 0.002f32;
                        if current_volume < target_volume {
                            current_volume = (current_volume + step).min(target_volume);
                        } else if current_volume > target_volume {
                            current_volume = (current_volume - step).max(target_volume);
                        }

                        let played_sample = sample * current_volume;
                        sum_sq += played_sample * played_sample;

                        let (low, mid, high) = filter_bank.tick(played_sample);
                        sum_low_sq += low * low;
                        sum_mid_sq += mid * mid;
                        sum_high_sq += high * high;

                        output[frame * 2] = played_sample; // L
                        output[frame * 2 + 1] = played_sample; // R
                    }

                    // Atomic update of current buffer level for telemetry/logic
                    buffer_samples.fetch_sub(
                        read_count.min(buffer_samples.load(Ordering::Relaxed)),
                        Ordering::SeqCst,
                    );

                    let raw_energy = (sum_sq / frames as f32).sqrt();
                    let energy = (raw_energy * 15.0).clamp(0.0, 1.0);
                    playback_energy.store(energy.to_bits(), Ordering::Relaxed);

                    let raw_low = (sum_low_sq / frames as f32).sqrt();
                    let raw_mid = (sum_mid_sq / frames as f32).sqrt();
                    let raw_high = (sum_high_sq / frames as f32).sqrt();

                    let low_val = (raw_low * 15.0).clamp(0.0, 1.0).powf(0.5);
                    let mid_val = (raw_mid * 15.0).clamp(0.0, 1.0).powf(0.5);
                    let high_val = (raw_high * 15.0).clamp(0.0, 1.0).powf(0.5);

                    playback_low.store(low_val.to_bits(), Ordering::Relaxed);
                    playback_mid.store(mid_val.to_bits(), Ordering::Relaxed);
                    playback_high.store(high_val.to_bits(), Ordering::Relaxed);

                    // Buffer exhausted — signal idle
                    if consumer.is_empty() {
                        if is_assistant_speaking.load(Ordering::Relaxed) {
                            playback_underruns.fetch_add(1, Ordering::Relaxed);
                        }
                        playback_active.store(false, Ordering::Relaxed);
                        playback_energy.store(0f32.to_bits(), Ordering::Relaxed);
                        playback_low.store(0f32.to_bits(), Ordering::Relaxed);
                        playback_mid.store(0f32.to_bits(), Ordering::Relaxed);
                        playback_high.store(0f32.to_bits(), Ordering::Relaxed);
                        buffer_samples.store(0, Ordering::SeqCst);
                        last_sample = 0.0;
                        current_volume = 1.0;
                        filter_bank.reset();
                    }
                },
                move |err| {
                    log::error!("[Playback] CPAL output error: {}", err);
                },
                None,
            )
            .map_err(|e| anyhow!("[Playback] Failed to build output stream: {}", e))?;

        stream
            .play()
            .map_err(|e| anyhow!("[Playback] Failed to start stream: {}", e))?;
        log::info!("[Playback] CPAL stream started (48kHz stereo f32)");

        Ok(stream)
    }
}

impl Drop for PlaybackEngine {
    fn drop(&mut self) {
        self.cancel();
    }
}
