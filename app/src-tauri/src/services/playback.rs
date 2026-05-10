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
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

// ─── Resampling (Directive 3) ─────────────────────────────────────────────────

/// Upsample 24kHz mono PCM → 48kHz via linear interpolation (exact 2× ratio).
///
/// Runs at buffer-ingestion time — NEVER inside the CPAL callback.
/// Time complexity: O(n). No allocations except the output Vec.
/// Zero dependency on any external resampling library.
#[inline]
pub fn upsample_2x(input: &[f32]) -> Vec<f32> {
    let mut out = Vec::with_capacity(input.len() * 2);
    for i in 0..input.len() {
        out.push(input[i]);
        // Linear midpoint between current and next sample.
        // At the last sample, repeat to avoid out-of-bounds.
        let next = input.get(i + 1).copied().unwrap_or(input[i]);
        out.push(0.5 * (input[i] + next));
    }
    out
}

// ─── Jitter Buffer Constants ──────────────────────────────────────────────────

/// Pre-buffer 300ms of 48kHz audio before starting CPAL playback.
/// Prevents underruns on the first chunk which is often uneven in size.
const JITTER_PREBUFFER_SAMPLES: usize = 48_000 / 1000 * 300; // 14_400 samples

// ─── Playback Engine ──────────────────────────────────────────────────────────

pub struct PlaybackEngine {
    /// Shared ring buffer between ingestion thread and CPAL callback.
    buffer:         Arc<Mutex<VecDeque<f32>>>,
    /// `true` while CPAL is actively draining (used for mic ducking in Speaker mode).
    playback_active: Arc<AtomicBool>,
    /// Set `true` to clear the buffer and stop playback.
    cancel_flag:    Arc<AtomicBool>,
    /// The active CPAL stream — kept alive until cancelled.
    _stream:        Option<cpal::Stream>,
    /// RCA Fix: Real-time safe energy telemetry (Atomic f32 via bit storage)
    _playback_energy: Arc<AtomicU32>,
    /// Track underruns specifically when AssistantSpeaking is true.
    _playback_underruns: Arc<std::sync::atomic::AtomicU64>,
    /// Ref to the state atomic for lock-free checks.
    _is_assistant_speaking: Arc<AtomicBool>,
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
        playback_energy: Arc<AtomicU32>,
        playback_underruns: Arc<std::sync::atomic::AtomicU64>,
        is_assistant_speaking: Arc<AtomicBool>,
    ) -> Result<Self> {
        let buffer: Arc<Mutex<VecDeque<f32>>> =
            Arc::new(Mutex::new(VecDeque::with_capacity(JITTER_PREBUFFER_SAMPLES * 4)));

        let stream = Self::build_cpal_stream(
            Arc::clone(&buffer),
            Arc::clone(&playback_active),
            Arc::clone(&cancel_flag),
            Arc::clone(&playback_energy),
            Arc::clone(&playback_underruns),
            Arc::clone(&is_assistant_speaking),
        )?;

        Ok(Self {
            buffer,
            playback_active,
            cancel_flag,
            _stream: Some(stream),
            _playback_energy: playback_energy,
            _playback_underruns: playback_underruns,
            _is_assistant_speaking: is_assistant_speaking,
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

        // Directive 3: upsample before touching the ring buffer
        let upsampled = upsample_2x(chunk_24khz);

        let mut buf = self.buffer.lock().unwrap();
        buf.extend(upsampled.iter());

        // Start CPAL playback once we have enough pre-buffered audio
        if !self.playback_active.load(Ordering::Relaxed)
            && buf.len() >= JITTER_PREBUFFER_SAMPLES
        {
            log::info!("[Playback] Pre-buffer full ({} samples) — starting output", buf.len());
            self.playback_active.store(true, Ordering::Relaxed);
        }
    }

    /// Cancel active playback: clear buffer and signal CPAL callback to stop.
    pub fn cancel(&self) {
        self.cancel_flag.store(true, Ordering::Relaxed);
        self.playback_active.store(false, Ordering::Relaxed);
        if let Ok(mut buf) = self.buffer.lock() {
            buf.clear();
        }
        log::info!("[Playback] Cancelled — buffer cleared");
    }

    /// Returns `true` if the playback buffer is empty and CPAL has gone idle.
    pub fn is_idle(&self) -> bool {
        !self.playback_active.load(Ordering::Relaxed)
    }

    /// Returns the number of samples remaining in the playback buffer.
    pub fn buffer_len(&self) -> usize {
        if let Ok(buf) = self.buffer.lock() {
            buf.len()
        } else {
            0
        }
    }

    // ── Private ───────────────────────────────────────────────────────────────

    fn build_cpal_stream(
        buffer: Arc<Mutex<VecDeque<f32>>>,
        playback_active: Arc<AtomicBool>,
        cancel_flag: Arc<AtomicBool>,
        playback_energy: Arc<AtomicU32>,
        playback_underruns: Arc<std::sync::atomic::AtomicU64>,
        is_assistant_speaking: Arc<AtomicBool>,
    ) -> Result<cpal::Stream> {
        let host   = cpal::default_host();
        let device = host.default_output_device()
            .ok_or_else(|| anyhow!("[Playback] No default output device found"))?;

        log::info!("[Playback] Output device: {:?}", device.name());

        // Request 48kHz stereo output (most common default)
        let config = StreamConfig {
            channels:    2,
            sample_rate: cpal::SampleRate(48_000),
            buffer_size: cpal::BufferSize::Default,
        };

        let supported = device.supported_output_configs()
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

        let stream = device.build_output_stream(
            &config,
            // ── CPAL data callback — ZERO computation (Directive 3) ──────────
            // Only drains the pre-upsampled ring buffer.
            move |output: &mut [f32], _info| {
                if !playback_active.load(Ordering::Relaxed)
                    || cancel_flag.load(Ordering::Relaxed)
                {
                    // Fill silence
                    output.fill(0.0);
                    return;
                }

                let mut buf = match buffer.try_lock() {
                    Ok(b) => b,
                    Err(_) => {
                        // Lock contention — fill silence rather than block the audio interrupt
                        output.fill(0.0);
                        return;
                    }
                };

                // The ring buffer is mono 48kHz. CPAL output is stereo.
                // Write each mono sample to both L and R channels.
                let frames = output.len() / 2; // stereo → frames
                let mut sum_sq = 0.0;
                for frame in 0..frames {
                    let sample = buf.pop_front().unwrap_or(0.0);
                    sum_sq += sample * sample;
                    output[frame * 2]     = sample; // L
                    output[frame * 2 + 1] = sample; // R
                }
                
                // RCA Fix: Use AtomicU32 (f32::to_bits) instead of mpsc::send to avoid
                // memory allocation/blocking in the high-priority audio callback.
                let raw_energy = (sum_sq / frames as f32).sqrt();
                let energy = (raw_energy * 15.0).clamp(0.0, 1.0);
                playback_energy.store(energy.to_bits(), Ordering::Relaxed);

                // Buffer exhausted — signal idle
                if buf.is_empty() {
                    // If we are supposed to be in AssistantSpeaking state but buffer is empty,
                    // this is an underrun (either a stall in TTS or end of turn).
                    if is_assistant_speaking.load(Ordering::Relaxed) {
                        playback_underruns.fetch_add(1, Ordering::Relaxed);
                    }
                    playback_active.store(false, Ordering::Relaxed);
                    playback_energy.store(0f32.to_bits(), Ordering::Relaxed);
                    log::info!("[Playback] Buffer drained — playback_active = false");
                }
            },
            move |err| {
                log::error!("[Playback] CPAL output error: {}", err);
            },
            None,
        ).map_err(|e| anyhow!("[Playback] Failed to build output stream: {}", e))?;

        stream.play().map_err(|e| anyhow!("[Playback] Failed to start stream: {}", e))?;
        log::info!("[Playback] CPAL stream started (48kHz stereo f32)");

        Ok(stream)
    }
}

impl Drop for PlaybackEngine {
    fn drop(&mut self) {
        self.cancel();
    }
}
