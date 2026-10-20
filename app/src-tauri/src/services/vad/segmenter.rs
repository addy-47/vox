use std::{
    collections::VecDeque,
    sync::{
        atomic::{AtomicU64, Ordering},
        mpsc, Arc,
    },
};

use crossbeam_channel::Sender;

use super::{
    actor::{VadActorChannels, VadActorHandles, VadActorState},
    providers::VadBackend,
    VadEngine as _, VAD_INPUT_SAMPLE_RATE, VAD_MIN_UTTERANCE_SAMPLES, VAD_PRE_ROLL_CAPACITY,
    VAD_PRE_ROLL_FLUSH_SAMPLES,
};
use crate::{
    core::{
        events::{InteractionOwner, VoxEvent},
        state::InteractionState,
    },
    monitoring::TelemetryEvent,
    services::stt::SttCommand,
    utils::audio_filters::FilterBank,
};

/// Fast downward adaptation rate for ambient noise floor when room gets quieter.
const NOISE_FLOOR_ALPHA_DOWN: f32 = 0.05;
/// Slow upward adaptation rate for ambient noise floor during steady non-speech background noise.
const NOISE_FLOOR_ALPHA_UP: f32 = 0.002;
/// Minimum clamped ambient noise floor RMS.
const MIN_NOISE_FLOOR_RMS: f32 = 0.0005;
/// Maximum clamped ambient noise floor RMS so loud environments never deafen onset detection.
const MAX_NOISE_FLOOR_RMS: f32 = 0.025;
/// Minimum dynamic onset energy gate (RMS) preserving sensitivity in extremely quiet rooms.
const MIN_ONSET_GATE_RMS: f32 = 0.0012;

/// Minimum speech samples (~1.25s @ 16kHz) before early-utterance pause protection relaxes.
const EARLY_UTTERANCE_PROTECTION_SAMPLES: usize = 20_000;
/// Minimum silence hangover frames (50 frames = 800ms) during the first 1.25s of an utterance.
const EARLY_UTTERANCE_MIN_END_FRAMES: usize = 50;
/// Extra hangover frames (22 frames = 352ms) added when STT partial indicates an incomplete clause.
const INCOMPLETE_CLAUSE_EXTRA_FRAMES: usize = 22;
/// Maximum adaptive hangover frames (68 frames = 1088ms) to keep turn-taking responsive.
const MAX_ADAPTIVE_END_FRAMES: usize = 68;

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

    /// Returns the number of samples currently held in the pre-roll buffer.
    pub fn len(&self) -> usize {
        self.buffer.len()
    }

    /// Copies all stored samples into the target buffer in chronological order without linear shifting.
    pub fn copy_into(&self, target: &mut Vec<f32>) {
        let (front, back) = self.buffer.as_slices();
        target.extend_from_slice(front);
        target.extend_from_slice(back);
    }

    /// Copies up to the most recent `max_samples` into `target` in chronological order.
    pub fn copy_tail_into(&self, target: &mut Vec<f32>, max_samples: usize) {
        let total = self.buffer.len();
        if total <= max_samples {
            self.copy_into(target);
            return;
        }
        let skip = total - max_samples;
        let (front, back) = self.buffer.as_slices();
        if skip < front.len() {
            target.extend_from_slice(&front[skip..]);
            target.extend_from_slice(back);
        } else {
            let back_skip = skip - front.len();
            target.extend_from_slice(&back[back_skip..]);
        }
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

/// Updates the asymmetric continuous ambient noise floor tracker during non-speech intervals.
#[inline]
fn update_noise_floor(state: &mut VadActorState, raw_energy: f32) {
    if raw_energy < state.noise_floor_ema {
        state.noise_floor_ema = (1.0 - NOISE_FLOOR_ALPHA_DOWN) * state.noise_floor_ema
            + NOISE_FLOOR_ALPHA_DOWN * raw_energy;
    } else {
        state.noise_floor_ema = (1.0 - NOISE_FLOOR_ALPHA_UP) * state.noise_floor_ema
            + NOISE_FLOOR_ALPHA_UP * raw_energy;
    }
    state.noise_floor_ema = state
        .noise_floor_ema
        .clamp(MIN_NOISE_FLOOR_RMS, MAX_NOISE_FLOOR_RMS);
}

/// Computes the dynamic SNR-aware energy gate for speech onset.
#[inline]
fn compute_dynamic_onset_gate(
    state: &VadActorState,
    noise_gate_multiplier: f32,
    interaction_state: InteractionState,
) -> f32 {
    let base_gate = state.noise_gate * noise_gate_multiplier;
    // Require higher SNR margin during Speaking playback to reject residual acoustic echo
    let snr_multiplier = if interaction_state == InteractionState::Speaking {
        3.2
    } else {
        2.0
    };
    // The floor is already clamped to [MIN_NOISE_FLOOR_RMS, MAX_NOISE_FLOOR_RMS], so the
    // product stays bounded. An upper clamp here would saturate below `floor * multiplier`
    // in loud ambient conditions (steady fan/hum), collapsing the SNR margin to ~0 dB and
    // letting noise bursts pass the onset gate — only the absolute lower bound is clamped.
    let adaptive_gate = (state.noise_floor_ema * snr_multiplier).max(MIN_ONSET_GATE_RMS);
    // Blend user-configured floor with live room SNR floor so quiet rooms remain sensitive
    adaptive_gate.max(base_gate * 0.5)
}

/// Computes the dynamic silence hangover frame count before SpeechEnd is emitted.
#[inline]
fn compute_dynamic_end_frames(state: &VadActorState, handles: &VadActorHandles) -> usize {
    let base_frames = state.speech_end_frames.max(25);

    // 1. Cadence-derived hangover: adapt to the user's observed intra-sentence pause rhythm (+220ms margin)
    let cadence_ms = state.smoothed_pause_duration_ms + 220.0;
    let cadence_frames = (cadence_ms as usize / 16).max(base_frames);

    let mut target_frames = base_frames.max(cadence_frames);

    // 2. Early-utterance protection: speakers frequently pause after the first 2-3 words ("What can you...")
    if state.speech_sample_counter < EARLY_UTTERANCE_PROTECTION_SAMPLES {
        target_frames = target_frames.max(EARLY_UTTERANCE_MIN_END_FRAMES);
    }

    // 3. Semantic / syntax continuation hint from live STT partials (conjunctions, prepositions, <=3 words)
    if handles.incomplete_clause_hint.load(Ordering::Relaxed) {
        target_frames += INCOMPLETE_CLAUSE_EXTRA_FRAMES;
    }

    target_frames.clamp(base_frames, MAX_ADAPTIVE_END_FRAMES)
}

/// Handles speech start event transition, stream resets, and pre-roll transfer.
pub fn handle_speech_start(
    state: &mut VadActorState,
    handles: &VadActorHandles,
    stt_tx: &mpsc::Sender<SttCommand>,
    vox_event_tx: Option<&mpsc::Sender<VoxEvent>>,
) {
    state.in_speech = true;
    state.speech_sample_counter = 0;
    handles
        .incomplete_clause_hint
        .store(false, Ordering::Relaxed);

    let prev_turn_id = handles.turn_id_atomic.load(Ordering::Relaxed);
    let interaction_state = InteractionState::from(handles.state_atomic.load(Ordering::Relaxed));
    let owner: InteractionOwner = handles.owner_atomic.load(Ordering::Relaxed).into();

    // Predict the monotonic turn ID that `on_speech_start` will allocate on the Router thread
    // so the pre-roll StreamChunk and subsequent chunks share the exact same turn_id.
    let expected_turn_id = if matches!(
        owner,
        InteractionOwner::Assistant | InteractionOwner::Dictation
    ) && matches!(
        interaction_state,
        InteractionState::Ready
            | InteractionState::Thinking
            | InteractionState::Speaking
            | InteractionState::Working
    ) {
        prev_turn_id.wrapping_add(1)
    } else {
        prev_turn_id
    };
    state.current_turn_id = expected_turn_id;

    log::info!(
        "[VAD Actor] Speech Start (turn: {}, prev_atomic: {}, noise_floor_rms: {:.5})",
        state.current_turn_id,
        prev_turn_id,
        state.noise_floor_ema
    );

    if let Some(tx) = vox_event_tx {
        if let Err(e) = tx.send(VoxEvent::SpeechStart { owner }) {
            log::warn!("[VAD Actor] Failed to send SpeechStart event: {}", e);
        }
    }

    state.utterance_buffer.clear();
    let pr_total = state.pre_roll_buffer.len();
    state
        .pre_roll_buffer
        .copy_tail_into(&mut state.utterance_buffer, VAD_PRE_ROLL_FLUSH_SAMPLES);
    state.samples_since_partial = state.utterance_buffer.len();
    log::info!(
        "[VAD Actor] Speech Start (turn: {}) pre_roll_total={} flushed_pre_roll={} utterance_samples={}",
        state.current_turn_id,
        pr_total,
        state.utterance_buffer.len(),
        state.utterance_buffer.len()
    );

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
    state.onset_score = 0.0;
    state.active_frames = 0;
    state.inactive_frames = 0;
    state.speech_sample_counter = 0;
    handles
        .incomplete_clause_hint
        .store(false, Ordering::Relaxed);

    let atomic_tid = handles.turn_id_atomic.load(Ordering::Relaxed);
    if atomic_tid > 0 {
        state.current_turn_id = atomic_tid;
    }
    log::info!("[VAD Actor] Speech End (turn: {})", state.current_turn_id);

    let owner: InteractionOwner = handles.owner_atomic.load(Ordering::Relaxed).into();

    if let Some(tx) = vox_event_tx {
        if let Err(e) = tx.send(VoxEvent::SpeechEnd { owner }) {
            log::warn!("[VAD Actor] Failed to send SpeechEnd event: {}", e);
        }
    }

    vad.flush();

    if state.utterance_buffer.len() >= VAD_MIN_UTTERANCE_SAMPLES && state.realtime_tx.is_none() {
        if let Err(e) = stt_tx.send(SttCommand::Final {
            turn_id: state.current_turn_id,
            audio: state.utterance_buffer.clone(),
            owner,
        }) {
            log::warn!("[VAD Actor] Failed to send Final audio to STT: {}", e);
        }
    }

    log::info!(
        "[VAD Actor] Speech End (turn: {}) utterance_samples={}",
        state.current_turn_id,
        state.utterance_buffer.len()
    );
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
    state.speech_sample_counter += chunk.len();

    if state.realtime_tx.is_none() {
        if let Err(e) = stt_tx.send(SttCommand::StreamChunk {
            turn_id: state.current_turn_id,
            audio: chunk.to_vec(),
        }) {
            log::warn!("[VAD Actor] Failed to send streaming chunk to STT: {}", e);
        }
    }
}

/// Executes ContinuousSegmentation mode with SNR-adaptive leaky onset and cadence-aware hangover.
pub fn process_continuous_segmentation(
    chunk: &[f32],
    raw_energy: f32,
    vad: &mut VadBackend,
    state: &mut VadActorState,
    handles: &VadActorHandles,
    stt_tx: &mpsc::Sender<SttCommand>,
    vox_event_tx: Option<&mpsc::Sender<VoxEvent>>,
) {
    let vad_detected = vad.predict(chunk);
    let interaction_state = InteractionState::from(handles.state_atomic.load(Ordering::Relaxed));

    if !state.in_speech {
        let onset_gate =
            compute_dynamic_onset_gate(state, vad.noise_gate_multiplier(), interaction_state);
        let is_onset_speech = vad_detected && raw_energy >= onset_gate;

        // Require slightly firmer confirmation when barging in over active playback
        let base_start_frames = state.speech_start_frames.max(2) as f32;
        let required_onset_score = if interaction_state == InteractionState::Speaking {
            (base_start_frames * 1.5).max(5.0)
        } else {
            base_start_frames
        };

        if is_onset_speech {
            // Boost onset score for high-SNR speech bursts so clear syllables trigger faster
            let snr_boost = if raw_energy >= onset_gate * 2.5 {
                1.35
            } else {
                1.0
            };
            state.onset_score += snr_boost;
            state.active_frames += 1;
            state.inactive_frames = 0;

            if state.onset_score >= required_onset_score {
                handle_speech_start(state, handles, stt_tx, vox_event_tx);
                accumulate_speech_frames(chunk, state, stt_tx);
            } else {
                // Retain sub-threshold onset frames in pre-roll so the entire rising edge is preserved
                state.pre_roll_buffer.push(chunk);
            }
        } else {
            // Leaky decay instead of instant zero reset so unvoiced plosives (/p/, /t/, /k/) don't wipe progress
            state.onset_score = (state.onset_score - 0.5).max(0.0);
            if state.onset_score == 0.0 {
                state.active_frames = 0;
            }
            state.inactive_frames += 1;
            // Track the ambient floor on every frame that did not qualify as onset speech.
            // Restricting updates to `!vad_detected` frames biased the floor downward: the
            // loudest noise bursts (which the neural detector labels "speech") never reached
            // the tracker, so the gate stayed below the true ambient level precisely when a
            // steady noise source (fan/hum) produced transient peaks above it.
            update_noise_floor(state, raw_energy);
            state.pre_roll_buffer.push(chunk);
        }
    } else {
        // Sync current_turn_id once Router has committed the new turn_id
        let atomic_tid = handles.turn_id_atomic.load(Ordering::Relaxed);
        if atomic_tid > state.current_turn_id {
            state.current_turn_id = atomic_tid;
        }

        if vad_detected {
            // If recovering from a mid-sentence micro-pause (>= 4 frames = 64ms), adapt cadence EMA
            if state.inactive_frames >= 4 {
                let pause_ms = (state.inactive_frames * 16) as f32;
                state.smoothed_pause_duration_ms =
                    0.75 * state.smoothed_pause_duration_ms + 0.25 * pause_ms;
            }
            state.active_frames += 1;
            state.inactive_frames = 0;
            accumulate_speech_frames(chunk, state, stt_tx);
        } else {
            state.inactive_frames += 1;
            state.active_frames = 0;

            let dynamic_end_threshold = compute_dynamic_end_frames(state, handles);
            if state.inactive_frames >= dynamic_end_threshold {
                handle_speech_end(vad, state, handles, stt_tx, vox_event_tx);
                state.pre_roll_buffer.push(chunk);
            } else {
                accumulate_speech_frames(chunk, state, stt_tx);
                // Also keep feeding recent trailing silence into pre_roll_buffer so if SpeechEnd
                // fires and the user resumes speaking immediately (< 200ms later), pre_roll_buffer
                // already holds the boundary context!
                if state.inactive_frames > dynamic_end_threshold.saturating_sub(20) {
                    state.pre_roll_buffer.push(chunk);
                }
            }
        }
    }
}

/// Executes WindowedValidation mode to evaluate speech presence and sample boundaries in caller-owned windows.
pub fn process_windowed_validation(
    chunk: &[f32],
    raw_energy: f32,
    vad: &mut VadBackend,
    state: &mut VadActorState,
    channels: &VadActorChannels,
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
                .saturating_sub(VAD_PRE_ROLL_FLUSH_SAMPLES.min(VAD_PRE_ROLL_CAPACITY));
        }
        state.window_last_speech_sample = state.window_sample_offset + chunk.len();
    }

    state.window_sample_offset += chunk.len();
    state.pre_roll_buffer.push(chunk);
    if state.window_stream_partials {
        if let Err(e) = channels.stt_tx.send(SttCommand::StreamChunk {
            turn_id: state.current_turn_id,
            audio: chunk.to_vec(),
        }) {
            log::warn!("[VAD Actor] Failed to send windowed chunk to STT: {}", e);
        }
    }
    check_window_autostop(state, channels.vox_event_tx.as_ref());
}

/// Fires a single auto-stop event once post-speech silence exceeds the caller-configured threshold.
fn check_window_autostop(state: &mut VadActorState, vox_event_tx: Option<&mpsc::Sender<VoxEvent>>) {
    if !state.window_active || !state.window_speech_detected || state.window_autostop_fired {
        return;
    }
    let Some(autostop_ms) = state.window_autostop_silence_ms else {
        return;
    };
    let threshold_samples = autostop_ms as usize * VAD_INPUT_SAMPLE_RATE as usize / 1000;
    let silence_samples = state
        .window_sample_offset
        .saturating_sub(state.window_last_speech_sample);
    if silence_samples < threshold_samples {
        return;
    }
    state.window_autostop_fired = true;
    log::info!(
        "[VAD Actor] Windowed silence auto-stop (silence {:.2}s)",
        silence_samples as f32 / VAD_INPUT_SAMPLE_RATE as f32
    );
    if let Some(tx) = vox_event_tx {
        if let Err(e) = tx.send(VoxEvent::PttStop {
            owner: state.window_owner,
        }) {
            log::warn!("[VAD Actor] Failed to send auto-stop PttStop event: {}", e);
        }
    }
}

/// Executes StreamPassthrough mode for direct low-latency routing to realtime cloud sinks.
pub fn process_stream_passthrough(chunk: &[f32], state: &mut VadActorState) {
    if let Some(ref tx) = state.realtime_tx {
        let mut pcm = Vec::with_capacity(chunk.len());
        f32_to_i16_pcm(chunk, &mut pcm);
        if let Err(e) = tx.try_send(pcm) {
            log::trace!("[VAD Actor] Passthrough queue full or disconnected: {}", e);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{config::AudioOutputMode, core::events::InteractionMode};

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
    fn test_preroll_copy_tail_into() {
        let mut buf = PreRollBuffer::new(10);
        buf.push(&[1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0]);
        let mut tail = Vec::new();
        buf.copy_tail_into(&mut tail, 4);
        assert_eq!(tail, vec![5.0, 6.0, 7.0, 8.0]);
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

    #[test]
    fn test_window_autostop_fires_after_threshold() {
        let mut state = VadActorState::new(
            0.5,
            0.01,
            400,
            32,
            InteractionMode::PTT,
            AudioOutputMode::Speaker,
        );
        state.window_active = true;
        state.window_speech_detected = true;
        state.window_sample_offset = 50_000;
        state.window_last_speech_sample = 20_000;
        state.window_autostop_silence_ms = Some(1200);

        let (tx, rx) = mpsc::channel();
        check_window_autostop(&mut state, Some(&tx));

        assert!(state.window_autostop_fired);
        assert!(matches!(rx.try_recv().ok(), Some(VoxEvent::PttStop { .. })));
    }

    #[test]
    fn test_window_autostop_ignores_prespeech_silence() {
        let mut state = VadActorState::new(
            0.5,
            0.01,
            400,
            32,
            InteractionMode::PTT,
            AudioOutputMode::Speaker,
        );
        state.window_active = true;
        state.window_speech_detected = false;
        state.window_sample_offset = 100_000;
        state.window_last_speech_sample = 0;
        state.window_autostop_silence_ms = Some(1200);

        let (tx, rx) = mpsc::channel();
        check_window_autostop(&mut state, Some(&tx));

        assert!(!state.window_autostop_fired);
        assert!(rx.try_recv().is_err());
    }

    #[test]
    fn test_window_autostop_disabled_without_config() {
        let mut state = VadActorState::new(
            0.5,
            0.01,
            400,
            32,
            InteractionMode::PTT,
            AudioOutputMode::Speaker,
        );
        state.window_active = true;
        state.window_speech_detected = true;
        state.window_sample_offset = 100_000;
        state.window_last_speech_sample = 0;
        state.window_autostop_silence_ms = None;

        let (tx, rx) = mpsc::channel();
        check_window_autostop(&mut state, Some(&tx));

        assert!(!state.window_autostop_fired);
        assert!(rx.try_recv().is_err());
    }

    #[test]
    fn test_onset_gate_preserves_snr_margin_at_high_floor() {
        let mut state = VadActorState::new(
            0.5,
            0.003,
            400,
            64,
            InteractionMode::Passive,
            AudioOutputMode::Speaker,
        );
        state.noise_floor_ema = 0.0227;

        let gate = compute_dynamic_onset_gate(&state, 1.0, InteractionState::Ready);
        assert!(
            (gate - 0.0454).abs() < 1e-4,
            "2x SNR margin must survive at loud ambient floors (got {})",
            gate
        );

        let echo_gate = compute_dynamic_onset_gate(&state, 1.0, InteractionState::Speaking);
        assert!(
            (echo_gate - 0.0726).abs() < 1e-4,
            "3.2x echo gate must survive at loud ambient floors (got {})",
            echo_gate
        );
    }

    #[test]
    fn test_onset_gate_clamps_to_minimum_in_quiet_rooms() {
        let mut state = VadActorState::new(
            0.5,
            0.003,
            400,
            64,
            InteractionMode::Passive,
            AudioOutputMode::Speaker,
        );
        state.noise_floor_ema = MIN_NOISE_FLOOR_RMS;

        let gate = compute_dynamic_onset_gate(&state, 1.0, InteractionState::Ready);
        assert!(
            gate >= MIN_ONSET_GATE_RMS,
            "gate must never fall below the absolute minimum (got {})",
            gate
        );
    }

    #[test]
    fn test_update_noise_floor_asymmetric_convergence_and_clamps() {
        let mut state = VadActorState::new(
            0.5,
            0.003,
            400,
            64,
            InteractionMode::Passive,
            AudioOutputMode::Speaker,
        );
        state.noise_floor_ema = 0.01;

        for _ in 0..200 {
            update_noise_floor(&mut state, 0.001);
        }
        assert!(
            state.noise_floor_ema < 0.002,
            "fast downward adaptation expected (got {})",
            state.noise_floor_ema
        );

        for _ in 0..200 {
            update_noise_floor(&mut state, MIN_NOISE_FLOOR_RMS);
        }
        assert!(
            (state.noise_floor_ema - MIN_NOISE_FLOOR_RMS).abs() < 1e-6,
            "floor must clamp to MIN_NOISE_FLOOR_RMS"
        );

        for _ in 0..50_000 {
            update_noise_floor(&mut state, 1.0);
        }
        assert!(
            (state.noise_floor_ema - MAX_NOISE_FLOOR_RMS).abs() < 1e-6,
            "floor must clamp to MAX_NOISE_FLOOR_RMS"
        );
    }
}
