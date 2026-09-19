use std::{
    collections::VecDeque,
    sync::{
        atomic::{AtomicU64, Ordering},
        mpsc, Arc,
    },
};

use crossbeam_channel::Sender;

use super::{
    actor::{VadActorHandles, VadActorState},
    providers::VadBackend,
    VadEngine as _, VAD_MIN_UTTERANCE_SAMPLES, VAD_PRE_ROLL_CAPACITY,
};
use crate::{
    core::events::VoxEvent, monitoring::TelemetryEvent, services::stt::SttCommand,
    utils::audio_filters::FilterBank,
};

/// Bounded circular buffer for retaining pre-roll audio before speech onset.
#[derive(Debug)]
pub struct PreRollBuffer {
    buffer: VecDeque<f32>,
    max_capacity: usize,
}

impl PreRollBuffer {
    /// Constructs a pre-roll buffer with a fixed maximum sample capacity.
    pub fn new(max_capacity: usize) -> Self {
        Self {
            buffer: VecDeque::with_capacity(max_capacity),
            max_capacity,
        }
    }

    /// Appends new audio samples to the pre-roll buffer, popping oldest samples if capacity is exceeded.
    pub fn push(&mut self, chunk: &[f32]) {
        let chunk_len = chunk.len();
        if chunk_len >= self.max_capacity {
            self.buffer.clear();
            self.buffer
                .extend(chunk[chunk_len - self.max_capacity..].iter().copied());
            return;
        }
        let excess = (self.buffer.len() + chunk_len).saturating_sub(self.max_capacity);
        for _ in 0..excess {
            self.buffer.pop_front();
        }
        self.buffer.extend(chunk.iter().copied());
    }

    /// Clears all stored audio samples.
    pub fn clear(&mut self) {
        self.buffer.clear();
    }

    /// Returns true if the pre-roll buffer contains zero samples.
    pub fn is_empty(&self) -> bool {
        self.buffer.is_empty()
    }

    /// Copies stored samples into the target buffer in chronological order without linear shifting.
    pub fn copy_into(&self, target: &mut Vec<f32>) {
        let (front, back) = self.buffer.as_slices();
        target.extend_from_slice(front);
        target.extend_from_slice(back);
    }
}

/// Calculates Root Mean Square (RMS) energy of an audio sample slice.
pub fn calculate_rms(chunk: &[f32]) -> f32 {
    if chunk.is_empty() {
        return 0.0;
    }
    (chunk.iter().map(|&x| x * x).sum::<f32>() / chunk.len() as f32).sqrt()
}

/// Converts normalized f32 audio samples [-1.0, 1.0] to 16-bit PCM i16 samples.
pub fn f32_to_i16_pcm(chunk: &[f32], target: &mut Vec<i16>) {
    target.clear();
    target.reserve(chunk.len());
    for &sample in chunk {
        let clamped = sample.clamp(-1.0, 1.0);
        target.push((clamped * 32767.0) as i16);
    }
}

/// Calculates 3-band audio energy telemetry and dispatches to the monitoring channel.
/// Returns the raw RMS energy value for downstream threshold gating.
pub fn process_and_emit_telemetry(
    chunk: &[f32],
    filter_bank: &mut FilterBank,
    noise_gate: f32,
    telemetry_tx: &Sender<TelemetryEvent>,
    dropped_counter: &Arc<AtomicU64>,
) -> f32 {
    let (raw_low, raw_mid, raw_high) = filter_bank.process_chunk(chunk);
    let raw_energy = calculate_rms(chunk);

    let gated_raw = if raw_energy > noise_gate {
        raw_energy
    } else {
        0.0
    };
    let energy = (gated_raw * 12.0).clamp(0.0, 1.0).powf(0.5);

    let gated_low = if raw_low > noise_gate { raw_low } else { 0.0 };
    let gated_mid = if raw_mid > noise_gate { raw_mid } else { 0.0 };
    let gated_high = if raw_high > noise_gate { raw_high } else { 0.0 };

    let low = (gated_low * 12.0).clamp(0.0, 1.0).powf(0.5);
    let mid = (gated_mid * 12.0).clamp(0.0, 1.0).powf(0.5);
    let high = (gated_high * 12.0).clamp(0.0, 1.0).powf(0.5);

    if telemetry_tx
        .try_send(TelemetryEvent::AudioEnergy {
            energy,
            vad_prob: 0.0,
            low,
            mid,
            high,
        })
        .is_err()
    {
        dropped_counter.fetch_add(1, Ordering::Relaxed);
    }

    raw_energy
}

/// Handles speech start event transition, stream resets, and pre-roll transfer.
pub fn handle_speech_start(
    state: &mut VadActorState,
    handles: &VadActorHandles,
    stt_tx: &mpsc::Sender<SttCommand>,
    vox_event_tx: Option<&mpsc::Sender<VoxEvent>>,
) {
    state.in_speech = true;
    state.current_turn_id = handles.turn_id_atomic.load(Ordering::Relaxed);

    log::info!("[VAD Actor] Speech Start (turn: {})", state.current_turn_id);

    if let Some(tx) = vox_event_tx {
        if let Err(e) = tx.send(VoxEvent::SpeechStart) {
            log::warn!("[VAD Actor] Failed to send SpeechStart event: {}", e);
        }
    }

    state.utterance_buffer.clear();
    state.pre_roll_buffer.copy_into(&mut state.utterance_buffer);
    state.samples_since_partial = state.utterance_buffer.len();

    if !state.utterance_buffer.is_empty() && state.realtime_tx.is_none() {
        if let Err(e) = stt_tx.send(SttCommand::StreamChunk {
            turn_id: state.current_turn_id,
            audio: state.utterance_buffer.clone(),
        }) {
            log::warn!("[VAD Actor] Failed to send pre-roll chunk to STT: {}", e);
        }
    }

    state.pre_roll_buffer.clear();
}

/// Handles speech end event transition, flushing VAD detector and dispatching final STT audio.
pub fn handle_speech_end(
    vad: &mut VadBackend,
    state: &mut VadActorState,
    handles: &VadActorHandles,
    stt_tx: &mpsc::Sender<SttCommand>,
    vox_event_tx: Option<&mpsc::Sender<VoxEvent>>,
) {
    state.in_speech = false;
    state.current_turn_id = handles.turn_id_atomic.load(Ordering::Relaxed);
    log::info!("[VAD Actor] Speech End (turn: {})", state.current_turn_id);

    if let Some(tx) = vox_event_tx {
        if let Err(e) = tx.send(VoxEvent::SpeechEnd) {
            log::warn!("[VAD Actor] Failed to send SpeechEnd event: {}", e);
        }
    }

    vad.flush();

    if state.utterance_buffer.len() >= VAD_MIN_UTTERANCE_SAMPLES && state.realtime_tx.is_none() {
        if let Err(e) = stt_tx.send(SttCommand::Final(
            state.current_turn_id,
            state.utterance_buffer.clone(),
        )) {
            log::warn!("[VAD Actor] Failed to send Final audio to STT: {}", e);
        }
    }

    state.utterance_buffer.clear();
    state.samples_since_partial = 0;
}

/// Accumulates streaming audio frames during active speech and forwards chunks to STT worker.
pub fn accumulate_speech_frames(
    chunk: &[f32],
    state: &mut VadActorState,
    stt_tx: &mpsc::Sender<SttCommand>,
) {
    state.utterance_buffer.extend_from_slice(chunk);
    state.samples_since_partial += chunk.len();

    if state.realtime_tx.is_none() {
        if let Err(e) = stt_tx.send(SttCommand::StreamChunk {
            turn_id: state.current_turn_id,
            audio: chunk.to_vec(),
        }) {
            log::warn!("[VAD Actor] Failed to send streaming chunk to STT: {}", e);
        }
    }
}

/// Executes ContinuousSegmentation mode for autonomous speech bounding.
pub fn process_continuous_segmentation(
    chunk: &[f32],
    raw_energy: f32,
    vad: &mut VadBackend,
    state: &mut VadActorState,
    handles: &VadActorHandles,
    stt_tx: &mpsc::Sender<SttCommand>,
    vox_event_tx: Option<&mpsc::Sender<VoxEvent>>,
) {
    let is_speech = vad.predict(chunk) && vad.is_above_noise_gate(raw_energy, state.noise_gate);
    let (speech_start_threshold, speech_end_threshold) = if vad.is_onnx() {
        (1, 1)
    } else {
        (state.speech_start_frames, state.speech_end_frames)
    };

    if is_speech {
        state.active_frames += 1;
        state.inactive_frames = 0;

        if !state.in_speech && state.active_frames >= speech_start_threshold {
            handle_speech_start(state, handles, stt_tx, vox_event_tx);
        }

        if state.in_speech {
            state.current_turn_id = handles.turn_id_atomic.load(Ordering::Relaxed);
            accumulate_speech_frames(chunk, state, stt_tx);
        }
    } else {
        state.inactive_frames += 1;
        state.active_frames = 0;

        if state.in_speech {
            if state.inactive_frames >= speech_end_threshold {
                handle_speech_end(vad, state, handles, stt_tx, vox_event_tx);
                state.pre_roll_buffer.push(chunk);
            } else {
                state.current_turn_id = handles.turn_id_atomic.load(Ordering::Relaxed);
                accumulate_speech_frames(chunk, state, stt_tx);
            }
        } else {
            state.pre_roll_buffer.push(chunk);
        }
    }
}

/// Executes WindowedValidation mode to evaluate speech presence and sample boundaries in caller-owned windows.
pub fn process_windowed_validation(
    chunk: &[f32],
    raw_energy: f32,
    vad: &mut VadBackend,
    state: &mut VadActorState,
) {
    if !state.window_active {
        state.pre_roll_buffer.push(chunk);
        return;
    }

    state.window_buffer.extend_from_slice(chunk);
    let is_speech = vad.predict(chunk) && vad.is_above_noise_gate(raw_energy, state.noise_gate);

    if is_speech {
        if !state.window_speech_detected {
            state.window_speech_detected = true;
            state.window_first_speech_sample = state
                .window_sample_offset
                .saturating_sub(VAD_PRE_ROLL_CAPACITY);
        }
        state.window_last_speech_sample = state.window_sample_offset + chunk.len();
    }

    state.window_sample_offset += chunk.len();
    state.pre_roll_buffer.push(chunk);
}

/// Executes StreamPassthrough mode for direct low-latency routing to realtime cloud sinks.
pub fn process_stream_passthrough(chunk: &[f32], state: &mut VadActorState) {
    if let Some(ref tx) = state.realtime_tx {
        let mut pcm = state
            .realtime_recycle_rx
            .try_recv()
            .unwrap_or_else(|_| Vec::with_capacity(chunk.len()));
        f32_to_i16_pcm(chunk, &mut pcm);
        if let Err(e) = tx.try_send(pcm) {
            log::trace!("[VAD Actor] Passthrough queue full or disconnected: {}", e);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_preroll_push_and_capacity() {
        let mut buf = PreRollBuffer::new(10);
        buf.push(&[1.0, 2.0, 3.0]);
        let mut out = Vec::new();
        buf.copy_into(&mut out);
        assert_eq!(out, vec![1.0, 2.0, 3.0]);

        buf.push(&[4.0, 5.0, 6.0, 7.0, 8.0]);
        buf.push(&[9.0, 10.0, 11.0]);
        let mut out2 = Vec::new();
        buf.copy_into(&mut out2);
        assert_eq!(out2.len(), 10);
        assert_eq!(out2[0], 2.0);
        assert_eq!(out2[9], 11.0);
    }

    #[test]
    fn test_preroll_large_chunk_truncates() {
        let mut buf = PreRollBuffer::new(5);
        buf.push(&[1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0]);
        let mut out = Vec::new();
        buf.copy_into(&mut out);
        assert_eq!(out, vec![4.0, 5.0, 6.0, 7.0, 8.0]);
    }

    #[test]
    fn test_preroll_clear_and_empty_copy() {
        let mut buf = PreRollBuffer::new(4);
        buf.push(&[1.0, 2.0]);
        buf.clear();
        let mut out = Vec::new();
        buf.copy_into(&mut out);
        assert!(out.is_empty());
    }

    #[test]
    fn test_calculate_rms_boundaries() {
        assert_eq!(calculate_rms(&[]), 0.0);
        assert_eq!(calculate_rms(&[0.0, 0.0, 0.0]), 0.0);
        let rms = calculate_rms(&[1.0, 1.0, 1.0]);
        assert!((rms - 1.0).abs() < 1e-5);
    }

    #[test]
    fn test_f32_to_i16_pcm_clamping() {
        let mut out = Vec::new();
        f32_to_i16_pcm(&[1.0, -1.0, 0.0, 2.0, -2.0, 0.5], &mut out);
        assert_eq!(out[0], 32767);
        assert_eq!(out[1], -32767);
        assert_eq!(out[2], 0);
        assert_eq!(out[3], 32767);
        assert_eq!(out[4], -32767);
        assert_eq!(out[5], (0.5 * 32767.0) as i16);
    }
}
