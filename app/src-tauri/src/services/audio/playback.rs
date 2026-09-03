use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::mpsc::Sender;
use std::sync::Arc;

use anyhow::Result;
use parking_lot::Mutex;
use ringbuf::traits::*;
use ringbuf::HeapProd;

use super::resampler::upsample_2x_into;
use super::{
    build_output_stream, AudioResampler, MODULAR_PREROLL_THRESHOLD_SAMPLES,
    PLAYBACK_BUFFER_SAMPLES, PLAYBACK_PRODUCER_SCRATCH_CAPACITY,
    REALTIME_PREROLL_THRESHOLD_SAMPLES, SINC_CHUNK_SIZE_OUTPUT,
};
pub use super::{PlaybackEngineHandles, PlaybackTelemetryHandles};
use crate::core::events::VoxEvent;
use crate::services::realtime::{RealtimeAudioConfig, DEFAULT_OUTPUT_SAMPLE_RATE};

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

        let stream = build_output_stream(
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

    /// Ingest a 24kHz audio chunk from TTS, upsample 2x to 48kHz, and push to ring buffer using default pre-roll.
    pub fn ingest_chunk(&self, chunk_24khz: &[f32]) {
        self.ingest_chunk_with_threshold(chunk_24khz, MODULAR_PREROLL_THRESHOLD_SAMPLES);
    }

    /// Ingest a 24kHz audio chunk, upsample 2x to 48kHz, push to ring buffer, and check against custom pre-roll threshold.
    pub fn ingest_chunk_with_threshold(&self, chunk_24khz: &[f32], preroll_threshold: usize) {
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
            if occupied >= preroll_threshold {
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

    /// Ingest a raw PCM i16 chunk with arbitrary sample rate, normalize, resample to 24kHz if needed, and push with realtime pre-roll cushion.
    pub fn ingest_chunk_i16(&self, chunk_i16: &[i16], resampler: &mut Option<AudioResampler>) {
        if self.cancel_flag.load(Ordering::Relaxed) || chunk_i16.is_empty() {
            return;
        }

        let pcm_resampled: Option<Vec<i16>> = if let Some(ref mut r) = resampler {
            match r.process_i16(chunk_i16) {
                Ok(out) => Some(out),
                Err(e) => {
                    log::error!("[Audio::Playback] Resampling error: {:?}", e);
                    return;
                }
            }
        } else {
            None
        };

        let slice = match pcm_resampled {
            Some(ref v) => v.as_slice(),
            None => chunk_i16,
        };

        let mut f32_chunk = Vec::with_capacity(slice.len());
        for &s in slice {
            f32_chunk.push(s as f32 / super::PCM_S16_SCALE);
        }

        self.ingest_chunk_with_threshold(&f32_chunk, REALTIME_PREROLL_THRESHOLD_SAMPLES);
    }

    /// Flushes pre-roll cushion on generation completion, immediately arming playback if unplayed samples exist.
    pub fn flush_pre_roll(&self) {
        if self.cancel_flag.load(Ordering::Relaxed) {
            return;
        }

        if !self.turn_armed.load(Ordering::Relaxed) {
            let occupied = self.producer.lock().0.occupied_len();
            if occupied > 0 {
                self.turn_armed.store(true, Ordering::Relaxed);
                let tid = self.current_turn_id.load(Ordering::Relaxed);
                if let Err(e) = self
                    .event_tx
                    .send(VoxEvent::PlaybackStarted { turn_id: tid })
                {
                    log::warn!(
                        "[Audio::Playback] Failed to emit PlaybackStarted on flush_pre_roll: {}",
                        e
                    );
                } else {
                    log::info!(
                        "[Audio::Playback] Pre-roll flushed ({} samples) — PlaybackStarted emitted (turn {})",
                        occupied,
                        tid
                    );
                }
            }
        }
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
}
