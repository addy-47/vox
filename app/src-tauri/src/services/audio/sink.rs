use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::Arc;

use ringbuf::traits::*;
use ringbuf::HeapCons;

use super::{
    PlaybackEngineHandles, PlaybackTelemetryHandles, PLAYBACK_CHANNELS, PLAYBACK_DEFAULT_VOLUME,
    PLAYBACK_ENERGY_EXPONENT, PLAYBACK_ENERGY_MULTIPLIER, PLAYBACK_SAMPLE_RATE,
    PLAYBACK_VOLUME_RAMP_STEP,
};
use crate::core::events::VoxEvent;
use crate::core::state::InteractionState;
use crate::utils::audio_filters::FilterBank;

/// Context state held exclusively by the real-time CPAL output stream callback.
pub(crate) struct PlaybackStreamContext {
    pub(crate) consumer: HeapCons<f32>,
    pub(crate) handles: PlaybackEngineHandles,
    pub(crate) discard_request: Arc<AtomicBool>,
    pub(crate) turn_armed: Arc<AtomicBool>,
    pub(crate) playback_energy: Arc<AtomicU32>,
    pub(crate) playback_low: Arc<AtomicU32>,
    pub(crate) playback_mid: Arc<AtomicU32>,
    pub(crate) playback_high: Arc<AtomicU32>,
    pub(crate) playback_underruns: Arc<AtomicU64>,
    pub(crate) last_sample: f32,
    pub(crate) current_volume: f32,
    pub(crate) filter_bank: FilterBank,
}

impl PlaybackStreamContext {
    /// Creates a new output stream callback context.
    pub(crate) fn new(
        consumer: HeapCons<f32>,
        handles: PlaybackEngineHandles,
        discard_request: Arc<AtomicBool>,
        turn_armed: Arc<AtomicBool>,
        telemetry: &PlaybackTelemetryHandles,
    ) -> Self {
        Self {
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
        }
    }

    /// Handles buffer drain, cancellation, discard requests, and audio output generation.
    pub(crate) fn process_output_buffer(&mut self, output: &mut [f32]) {
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
