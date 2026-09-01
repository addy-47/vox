use super::resampler::upsample_2x_into;
use super::{
    AudioResampler, PLAYBACK_BUFFER_SAMPLES, PLAYBACK_CHANNELS, PLAYBACK_DEFAULT_VOLUME,
    PLAYBACK_ENERGY_EXPONENT, PLAYBACK_ENERGY_MULTIPLIER, PLAYBACK_PRODUCER_SCRATCH_CAPACITY,
    PLAYBACK_SAMPLE_RATE, PLAYBACK_VOLUME_RAMP_STEP, PREROLL_THRESHOLD_SAMPLES,
    SINC_CHUNK_SIZE_OUTPUT,
};
use crate::core::events::VoxEvent;
use crate::core::state::InteractionState;
use crate::services::realtime::{RealtimeAudioConfig, DEFAULT_OUTPUT_SAMPLE_RATE};
use crate::utils::audio_filters::FilterBank;
use anyhow::{anyhow, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{SampleFormat, StreamConfig};
use parking_lot::Mutex;
use ringbuf::traits::*;
use ringbuf::{HeapCons, HeapProd};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::mpsc::Sender;
use std::sync::Arc;

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

/// Core audio playback engine managing a CPAL stream, SPSC lock-free ring buffer, and volume ramps.
pub struct PlaybackEngine {
    producer: Mutex<(HeapProd<f32>, Vec<f32>)>,
    cancel_flag: Arc<AtomicBool>,
    discard_request: Arc<AtomicBool>,
    event_tx: Sender<VoxEvent>,
    current_turn_id: Arc<AtomicU32>,
    turn_armed: Arc<AtomicBool>,
    pending_synthesis_jobs: Arc<AtomicU32>,
    stream: Option<cpal::Stream>,
}

/// Context state held by the real-time CPAL output stream callback.
struct PlaybackStreamContext {
    consumer: HeapCons<f32>,
    handles: PlaybackEngineHandles,
    discard_request: Arc<AtomicBool>,
    turn_armed: Arc<AtomicBool>,
    playback_energy: Arc<AtomicU32>,
    playback_low: Arc<AtomicU32>,
    playback_mid: Arc<AtomicU32>,
    playback_high: Arc<AtomicU32>,
    playback_underruns: Arc<AtomicU64>,
    last_sample: f32,
    current_volume: f32,
    filter_bank: FilterBank,
}

unsafe impl Send for PlaybackEngine {}
unsafe impl Sync for PlaybackEngine {}

impl Drop for PlaybackEngine {
    /// Cleans up and halts output on engine drop.
    fn drop(&mut self) {
        self.cancel();
    }
}

impl PlaybackEngine {
    /// Initialise CPAL output stream at 48kHz without starting playback immediately.
    pub fn new(
        handles: PlaybackEngineHandles,
        telemetry: PlaybackTelemetryHandles,
    ) -> Result<Self> {
        let rb = ringbuf::HeapRb::<f32>::new(PLAYBACK_BUFFER_SAMPLES);
        let (producer, consumer) = rb.split();
        let discard_request = Arc::new(AtomicBool::new(false));
        let turn_armed = Arc::new(AtomicBool::new(false));

        let stream = Self::build_cpal_stream(
            consumer,
            handles.clone(),
            Arc::clone(&discard_request),
            Arc::clone(&turn_armed),
            &telemetry,
        )?;

        Ok(Self {
            producer: Mutex::new((
                producer,
                Vec::with_capacity(PLAYBACK_PRODUCER_SCRATCH_CAPACITY),
            )),
            cancel_flag: handles.cancel_flag,
            discard_request,
            event_tx: handles.event_tx,
            current_turn_id: handles.current_turn_id,
            turn_armed,
            pending_synthesis_jobs: handles.pending_synthesis_jobs,
            stream: Some(stream),
        })
    }

    /// Creates a PlaybackEngine from constituent components without building a hardware stream.
    pub fn from_parts(
        producer: HeapProd<f32>,
        handles: PlaybackEngineHandles,
        discard_request: Arc<AtomicBool>,
        turn_armed: Arc<AtomicBool>,
        stream: Option<cpal::Stream>,
    ) -> Self {
        Self {
            producer: Mutex::new((
                producer,
                Vec::with_capacity(PLAYBACK_PRODUCER_SCRATCH_CAPACITY),
            )),
            cancel_flag: handles.cancel_flag,
            discard_request,
            event_tx: handles.event_tx,
            current_turn_id: handles.current_turn_id,
            turn_armed,
            pending_synthesis_jobs: handles.pending_synthesis_jobs,
            stream,
        }
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

        // Gate 1 (Start): Preroll cushion threshold check before dispatching PlaybackStarted
        if !self.turn_armed.load(Ordering::Relaxed) {
            let occupied = prod.occupied_len();
            if occupied >= PREROLL_THRESHOLD_SAMPLES {
                self.turn_armed.store(true, Ordering::Relaxed);
                let tid = self.current_turn_id.load(Ordering::Relaxed);
                if let Err(e) = self
                    .event_tx
                    .send(VoxEvent::PlaybackStarted { turn_id: tid })
                {
                    log::warn!("[Audio::Playback] Failed to emit PlaybackStarted: {}", e);
                } else {
                    log::info!(
                        "[Audio::Playback] Preroll cushion satisfied ({} samples) — PlaybackStarted emitted (turn {})",
                        occupied,
                        tid
                    );
                }
            }
        }
    }

    /// Ingest a raw PCM i16 chunk with arbitrary sample rate, normalize, resample to 24kHz if needed, and push.
    pub fn ingest_chunk_i16(&self, chunk_i16: &[i16], resampler: &mut Option<AudioResampler>) {
        if self.cancel_flag.load(Ordering::Relaxed) || chunk_i16.is_empty() {
            return;
        }

        let pcm_24k = if let Some(ref mut r) = resampler {
            match r.process_i16(chunk_i16) {
                Ok(out) => out,
                Err(e) => {
                    log::error!("[Audio::Playback] Resampling error: {:?}", e);
                    return;
                }
            }
        } else {
            chunk_i16.to_vec()
        };

        let mut f32_chunk = Vec::with_capacity(pcm_24k.len());
        for &s in &pcm_24k {
            f32_chunk.push(s as f32 / super::PCM_S16_SCALE);
        }

        self.ingest_chunk(&f32_chunk);
    }

    /// Spawns an async receiver worker reading PCM i16 from a Tokio channel and streaming into PlaybackEngine.
    pub fn spawn_pcm_stream_worker(
        self: &Arc<Self>,
        mut rx: tokio::sync::mpsc::Receiver<Vec<i16>>,
        config: RealtimeAudioConfig,
        handle: &tokio::runtime::Handle,
    ) {
        let engine = Arc::clone(self);
        handle.spawn(async move {
            let mut resampler = if config.requires_output_resampling
                || config.output_sample_rate != DEFAULT_OUTPUT_SAMPLE_RATE
            {
                match AudioResampler::new(
                    config.output_sample_rate,
                    DEFAULT_OUTPUT_SAMPLE_RATE,
                    SINC_CHUNK_SIZE_OUTPUT,
                ) {
                    Ok(r) => Some(r),
                    Err(e) => {
                        log::error!(
                            "[Audio::Playback] Failed to create output resampler: {:?}",
                            e
                        );
                        None
                    }
                }
            } else {
                None
            };

            while let Some(pcm) = rx.recv().await {
                engine.ingest_chunk_i16(&pcm, &mut resampler);
            }
        });
    }

    /// Returns true if an active CPAL audio hardware output stream is bound.
    pub fn has_active_stream(&self) -> bool {
        self.stream.is_some()
    }

    /// Returns a clone of the pending synthesis jobs atomic counter.
    pub fn pending_synthesis_jobs(&self) -> Arc<AtomicU32> {
        Arc::clone(&self.pending_synthesis_jobs)
    }

    /// Cancels active playback, signals consumer discard, and resets buffer count.
    pub fn cancel(&self) {
        self.cancel_flag.store(true, Ordering::Relaxed);
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
        handles: PlaybackEngineHandles,
        discard_request: Arc<AtomicBool>,
        turn_armed: Arc<AtomicBool>,
        telemetry: &PlaybackTelemetryHandles,
    ) -> Result<cpal::Stream> {
        let host = cpal::default_host();
        let (device, config) = resolve_output_device_and_config(&host)?;

        let mut cb_ctx = PlaybackStreamContext {
            consumer,
            handles,
            discard_request,
            turn_armed,
            playback_energy: Arc::clone(&telemetry.energy),
            playback_low: Arc::clone(&telemetry.low),
            playback_mid: Arc::clone(&telemetry.mid),
            playback_high: Arc::clone(&telemetry.high),
            playback_underruns: Arc::clone(&telemetry.underruns),
            last_sample: 0.0,
            current_volume: PLAYBACK_DEFAULT_VOLUME,
            filter_bank: FilterBank::new(PLAYBACK_SAMPLE_RATE as f32),
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

impl PlaybackStreamContext {
    /// Handles buffer drain, cancellation, discard requests, and audio output generation.
    fn process_output_buffer(&mut self, output: &mut [f32]) {
        if self.discard_request.load(Ordering::Relaxed) {
            self.consumer.skip(self.consumer.occupied_len());
            self.discard_request.store(false, Ordering::Relaxed);
            self.turn_armed.store(false, Ordering::Relaxed);
            self.reset_telemetry_state();
        }

        let is_speaking = InteractionState::from(self.handles.state_atomic.load(Ordering::Relaxed))
            == InteractionState::Speaking;

        // Drain allowed if state is Speaking OR if turn is armed (pre-roll threshold met)
        let playback_active = is_speaking || self.turn_armed.load(Ordering::Relaxed);

        if !playback_active || self.handles.cancel_flag.load(Ordering::Relaxed) {
            if self.handles.cancel_flag.load(Ordering::Relaxed) {
                self.consumer.skip(self.consumer.occupied_len());
                self.turn_armed.store(false, Ordering::Relaxed);
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
            let pending_jobs = self.handles.pending_synthesis_jobs.load(Ordering::Relaxed);
            let armed = self.turn_armed.load(Ordering::Relaxed);

            if pending_jobs > 0 {
                // Mid-turn gap: next clause is in-flight synthesizing
                self.playback_underruns.fetch_add(1, Ordering::Relaxed);
            } else if armed {
                // Gate 2 (End): Genuinely done with all clauses in turn
                self.turn_armed.store(false, Ordering::Relaxed);
                let tid = self.handles.current_turn_id.load(Ordering::Relaxed);
                if let Err(e) = self
                    .handles
                    .event_tx
                    .send(VoxEvent::PlaybackFinished { turn_id: tid })
                {
                    log::warn!("[Audio::Playback] Failed to emit PlaybackFinished: {}", e);
                } else {
                    log::info!(
                        "[Audio::Playback] Playback completed — PlaybackFinished emitted (turn {})",
                        tid
                    );
                }
            }

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
