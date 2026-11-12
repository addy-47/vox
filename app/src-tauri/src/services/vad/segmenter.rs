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

/// Minimum silence frames (~304ms @ 16ms/frame) before querying SmartTurn EOU acoustic prosody.
const SMART_TURN_SILENCE_FRAMES: usize = 19;
/// Hard ceiling on mid-thought silence grace period (75 frames = 1200ms) to ensure turn always closes.
const MAX_SILENCE_CEILING_FRAMES: usize = 75;

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

/// Handles speech start event transition, stream resets, and pre-roll transfer.
pub fn handle_speech_start(
    state: &mut VadActorState,
    handles: &VadActorHandles,
    stt_tx: &mpsc::Sender<SttCommand>,
    vox_event_tx: Option<&mpsc::Sender<VoxEvent>>,
) {
    state.in_speech = true;
    state.voiced_samples = 0;

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
        "[VAD Actor] Speech Start (turn: {}, prev_atomic: {})",
        state.current_turn_id,
        prev_turn_id
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
    let voiced_samples = state.voiced_samples;
    state.in_speech = false;
    state.onset_score = 0.0;
    state.active_frames = 0;
    state.inactive_frames = 0;
    state.voiced_samples = 0;

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

    if voiced_samples >= VAD_MIN_UTTERANCE_SAMPLES && state.realtime_tx.is_none() {
        if let Err(e) = stt_tx.send(SttCommand::Final {
            turn_id: state.current_turn_id,
            audio: state.utterance_buffer.clone(),
            owner,
        }) {
            log::warn!("[VAD Actor] Failed to send Final audio to STT: {}", e);
        }
    } else if state.realtime_tx.is_none() {
        // Critical fix: prevent pipeline deadlock in Thinking when voiced_samples < VAD_MIN_UTTERANCE_SAMPLES.
        // If voiced audio falls below the minimum utterance threshold, STT is skipped, so we must emit an
        // empty TranscriptFinal directly to the Router to cleanly transition Thinking back to Ready.
        if let Some(tx) = vox_event_tx {
            if let Err(e) = tx.send(VoxEvent::TranscriptFinal {
                turn_id: state.current_turn_id,
                text: String::new(),
                owner,
            }) {
                log::warn!(
                    "[VAD Actor] Failed to send empty TranscriptFinal fallback: {}",
                    e
                );
            }
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

    if state.realtime_tx.is_none() {
        if let Err(e) = stt_tx.send(SttCommand::StreamChunk {
            turn_id: state.current_turn_id,
            audio: chunk.to_vec(),
        }) {
            log::warn!("[VAD Actor] Failed to send streaming chunk to STT: {}", e);
        }
    }
}

/// Executes ContinuousSegmentation mode with acoustic VAD and SmartTurn prosodic turn-taking.
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
        // Continuous adaptive noise floor tracking on non-speech frames
        // During Speaking, require higher SNR margin and minimum gate floor (0.038 RMS)
        // to cleanly reject residual acoustic loudspeaker echo before AEC fully converges.
        let base_gate = state.noise_gate * vad.noise_gate_multiplier();
        let onset_gate = if interaction_state == InteractionState::Speaking {
            (state.noise_floor_ema * 3.5).max(base_gate * 2.0).max(0.038)
        } else {
            (state.noise_floor_ema * 2.0).max(base_gate * 0.5)
        };
        let is_speech_chunk = vad_detected && raw_energy >= onset_gate;

        // Require firmer confirmation when barging in over active playback to prevent echo bleed
        let base_start_frames = state.speech_start_frames.max(2) as f32;
        let required_onset_score = if interaction_state == InteractionState::Speaking {
            (base_start_frames * 2.0).max(4.0)
        } else {
            base_start_frames
        };

        if is_speech_chunk {
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
                state.voiced_samples += chunk.len();
                accumulate_speech_frames(chunk, state, stt_tx);
            } else {
                state.pre_roll_buffer.push(chunk);
            }
        } else {
            // Leaky decay so brief unvoiced plosives (/p/, /t/, /k/) don't immediately wipe onset progress
            state.onset_score = (state.onset_score - 0.5).max(0.0);
            if state.onset_score == 0.0 {
                state.active_frames = 0;
            }
            state.inactive_frames += 1;
            // Adapt ambient noise floor EMA during non-speech frames
            if raw_energy < state.noise_floor_ema {
                state.noise_floor_ema = 0.95 * state.noise_floor_ema + 0.05 * raw_energy;
            } else {
                state.noise_floor_ema = 0.998 * state.noise_floor_ema + 0.002 * raw_energy;
            }
            state.noise_floor_ema = state.noise_floor_ema.clamp(0.0005, 0.035);
            state.pre_roll_buffer.push(chunk);
        }
    } else {
        // Sync current_turn_id once Router has committed the new turn_id
        let atomic_tid = handles.turn_id_atomic.load(Ordering::Relaxed);
        if atomic_tid > state.current_turn_id {
            state.current_turn_id = atomic_tid;
        }

        if vad_detected {
            state.active_frames += 1;
            state.inactive_frames = 0;
            state.voiced_samples += chunk.len();
            accumulate_speech_frames(chunk, state, stt_tx);
        } else {
            state.inactive_frames += 1;
            state.active_frames = 0;

            let base_end_frames = state.speech_end_frames.max(1);
            let is_turn_complete = if let Some(ref detector) = state.turn_detector {
                if state.inactive_frames >= SMART_TURN_SILENCE_FRAMES {
                    match detector.predict_eou(&state.utterance_buffer) {
                        Ok(Some(prob)) => {
                            if prob >= 0.6 {
                                true
                            } else {
                                // Mid-thought pause: hold turn open up to 1200ms ceiling (75 frames)
                                state.inactive_frames >= MAX_SILENCE_CEILING_FRAMES
                            }
                        }
                        _ => state.inactive_frames >= base_end_frames,
                    }
                } else {
                    false
                }
            } else {
                state.inactive_frames >= base_end_frames
            };

            if is_turn_complete {
                handle_speech_end(vad, state, handles, stt_tx, vox_event_tx);
                state.pre_roll_buffer.push(chunk);
            } else {
                accumulate_speech_frames(chunk, state, stt_tx);
                if state.inactive_frames > base_end_frames.saturating_sub(20) {
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
    use std::sync::atomic::AtomicBool;

    use super::*;
    use crate::{
        config::AudioOutputMode,
        core::events::{InteractionMode, InteractionOwner},
        services::vad::providers::EarshotVadEngine,
    };

    /// Deterministic driver for `process_continuous_segmentation`.
    ///
    /// The real Earshot backend is used, but its threshold is flipped between frames
    /// (-1.0 => predict true, 1.0 => predict false) so the detector output is scripted.
    /// No model weights and no threads, so the state machine is fully reproducible.
    struct Rig {
        state: VadActorState,
        handles: VadActorHandles,
        vad: VadBackend,
        stt_tx: mpsc::Sender<SttCommand>,
        stt_rx: mpsc::Receiver<SttCommand>,
        event_tx: mpsc::Sender<VoxEvent>,
        event_rx: mpsc::Receiver<VoxEvent>,
    }

    impl Rig {
        fn new(onset_ms: u32, silence_ms: u32, noise_gate: f32) -> Self {
            let (stt_tx, stt_rx) = mpsc::channel();
            let (event_tx, event_rx) = mpsc::channel();
            let handles = VadActorHandles {
                state_atomic: Arc::new(std::sync::atomic::AtomicU32::new(
                    InteractionState::Ready as u32,
                )),
                turn_id_atomic: Arc::new(std::sync::atomic::AtomicU32::new(0)),
                audio_suppressed: Arc::new(AtomicBool::new(false)),
                engine_shutdown: Arc::new(AtomicBool::new(false)),
                dropped_counter: Arc::new(AtomicU64::new(0)),
                purged_frames_counter: Arc::new(AtomicU64::new(0)),
                ingestion_gate: Arc::new(AtomicBool::new(true)),
                owner_atomic: Arc::new(std::sync::atomic::AtomicU32::new(
                    InteractionOwner::Assistant as u32,
                )),
            };
            Self {
                state: VadActorState::new(
                    0.5,
                    noise_gate,
                    silence_ms,
                    onset_ms,
                    InteractionMode::Passive,
                    AudioOutputMode::Speaker,
                ),
                handles,
                vad: VadBackend::Earshot(
                    EarshotVadEngine::new(1.0).expect("earshot detector must initialise"),
                ),
                stt_tx,
                stt_rx,
                event_tx,
                event_rx,
            }
        }

        fn step(&mut self, speech: bool) {
            let amplitude = if speech { 0.05 } else { 0.0 };
            self.step_amp(amplitude, speech);
        }

        fn step_amp(&mut self, amplitude: f32, speech: bool) {
            self.vad
                .update_threshold(if speech { -1.0 } else { 1.0 })
                .expect("threshold update must succeed");
            let frame = vec![amplitude; 256];
            let energy = calculate_rms(&frame);
            process_continuous_segmentation(
                &frame,
                energy,
                &mut self.vad,
                &mut self.state,
                &self.handles,
                &self.stt_tx,
                Some(&self.event_tx),
            );
        }

        fn events(&mut self) -> Vec<VoxEvent> {
            self.event_rx.try_iter().collect()
        }

        fn commands(&mut self) -> Vec<SttCommand> {
            self.stt_rx.try_iter().collect()
        }
    }

    fn stream_and_final(commands: Vec<SttCommand>) -> (Vec<f32>, Option<Vec<f32>>) {
        let mut streamed = Vec::new();
        let mut final_audio = None;
        for cmd in commands {
            match cmd {
                SttCommand::StreamChunk { audio, .. } => streamed.extend(audio),
                SttCommand::Final { audio, .. } => final_audio = Some(audio),
                _ => {}
            }
        }
        (streamed, final_audio)
    }

    #[test]
    fn onset_requires_configured_confirmation_frames() {
        let mut rig = Rig::new(32, 400, 0.01);
        rig.step(true);
        assert!(
            rig.events().is_empty(),
            "a single frame must not open a turn"
        );
        rig.step(true);
        let events = rig.events();
        assert!(
            matches!(events.as_slice(), [VoxEvent::SpeechStart { .. }]),
            "exactly one SpeechStart expected, got {:?}",
            events
        );
    }

    #[test]
    fn unvoiced_dropout_does_not_reset_onset_progress() {
        let mut rig = Rig::new(32, 400, 0.01);
        rig.step(true);
        rig.step(false);
        assert!(
            rig.events().is_empty(),
            "the dropout frame must not open a turn by itself"
        );
        rig.step(true);
        assert_eq!(
            rig.events().len(),
            1,
            "the third frame must open the turn; a reset would require a fourth"
        );
    }

    #[test]
    fn pre_roll_tail_is_forwarded_as_the_first_stream_chunk() {
        let mut rig = Rig::new(32, 400, 0.01);
        rig.step_amp(0.05, true);
        rig.step_amp(0.06, true);

        let (streamed, _) = stream_and_final(rig.commands());
        let chunks: Vec<&[f32]> = streamed.chunks_exact(256).collect();
        assert_eq!(chunks.len(), 2, "one pre-roll chunk and one onset chunk");
        assert!(
            chunks[0].iter().all(|&s| (s - 0.05).abs() < 1e-6),
            "first chunk must be the pre-roll frame"
        );
        assert!(
            chunks[1].iter().all(|&s| (s - 0.06).abs() < 1e-6),
            "second chunk must be the onset frame"
        );
    }

    #[test]
    fn streamed_audio_matches_final_audio_without_overlap() {
        let mut rig = Rig::new(32, 100, 0.01);
        for _ in 0..25 {
            rig.step(true); // >= 4800 voiced samples to qualify as an utterance
        }
        for _ in 0..50 {
            rig.step(false); // early-utterance hangover; 50th frame ends the turn
        }

        let (streamed, final_audio) = stream_and_final(rig.commands());
        let final_audio = final_audio.expect("a qualifying utterance must emit Final");
        assert!(
            !streamed.is_empty(),
            "the utterance must have been streamed"
        );
        assert_eq!(
            streamed.len(),
            final_audio.len(),
            "streamed length must equal final length"
        );
        assert_eq!(
            streamed, final_audio,
            "final audio must be exactly the concatenation of streamed chunks"
        );
    }

    #[test]
    fn one_utterance_uses_one_turn_id_across_pre_roll_stream_and_final() {
        let mut rig = Rig::new(32, 100, 0.01);
        rig.handles.turn_id_atomic.store(7, Ordering::Relaxed);
        rig.step(true);
        rig.step(true); // opens the turn and predicts turn 8
        rig.handles.turn_id_atomic.store(8, Ordering::Relaxed); // router commits
        for _ in 0..23 {
            rig.step(true); // >= 4800 voiced samples to emit Final
        }
        for _ in 0..50 {
            rig.step(false);
        }

        let ids: Vec<u32> = rig
            .commands()
            .into_iter()
            .filter_map(|cmd| match cmd {
                SttCommand::StreamChunk { turn_id, .. } => Some(turn_id),
                SttCommand::Final { turn_id, .. } => Some(turn_id),
                _ => None,
            })
            .collect();
        assert!(
            ids.len() >= 3,
            "expected pre-roll, body and final, got {:?}",
            ids
        );
        assert!(
            ids.iter().all(|&id| id == 8),
            "every command of one utterance must share turn 8, got {:?}",
            ids
        );
    }

    #[test]
    fn barge_in_over_playback_requires_firmer_onset() {
        let mut rig = Rig::new(32, 400, 0.01);
        rig.handles
            .state_atomic
            .store(InteractionState::Speaking as u32, Ordering::Relaxed);
        rig.step(true);
        rig.step(true);
        rig.step(true);
        assert!(
            rig.events().is_empty(),
            "three frames must not barge in over playback"
        );
        rig.step(true);
        assert_eq!(rig.events().len(), 1, "the fourth frame must barge in");
    }


    #[test]
    fn silence_emits_no_events_and_no_stt_commands() {
        let mut rig = Rig::new(32, 400, 0.01);
        for _ in 0..100 {
            rig.step(false);
        }
        assert!(rig.events().is_empty(), "silence must not emit VAD events");
        assert!(
            rig.commands().is_empty(),
            "silence must not reach the STT worker"
        );
        assert!(!rig.state.in_speech, "silence must not open a turn");
    }

    #[test]
    fn onset_progress_decays_to_zero_after_sustained_silence() {
        let mut rig = Rig::new(32, 400, 0.01);
        rig.step(true);
        for _ in 0..10 {
            rig.step(false);
        }
        assert_eq!(rig.state.active_frames, 0);
        assert_eq!(rig.state.onset_score, 0.0);
        assert!(rig.events().is_empty());
    }

    #[test]
    fn speech_end_clears_utterance_and_repopulates_pre_roll() {
        let mut rig = Rig::new(32, 100, 0.01);
        rig.step(true);
        rig.step(true);
        for _ in 0..50 {
            rig.step(false);
        }
        assert!(!rig.state.in_speech);
        assert!(
            rig.state.utterance_buffer.is_empty(),
            "utterance buffer must be cleared after SpeechEnd"
        );
        assert!(
            !rig.state.pre_roll_buffer.is_empty(),
            "pre-roll must hold the boundary frame for an immediate resume"
        );
    }

    #[test]
    fn utterance_below_minimum_emits_no_final() {
        let mut rig = Rig::new(32, 100, 0.01);
        rig.step(true);
        rig.step(true);
        for _ in 0..60 {
            rig.step(false);
        }
        let (_, final_audio) = stream_and_final(rig.commands());
        assert!(
            final_audio.is_none(),
            "an utterance below the minimum must not emit Final"
        );
    }

    #[test]
    fn preroll_ring_buffer_drops_oldest_samples_at_capacity() {
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
    fn preroll_copy_tail_returns_most_recent_samples() {
        let mut buf = PreRollBuffer::new(10);
        buf.push(&[1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0]);
        let mut tail = Vec::new();
        buf.copy_tail_into(&mut tail, 4);
        assert_eq!(tail, vec![5.0, 6.0, 7.0, 8.0]);
    }

    #[test]
    fn preroll_oversized_chunk_keeps_tail() {
        let mut buf = PreRollBuffer::new(5);
        buf.push(&[1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0]);
        let mut out = Vec::new();
        buf.copy_into(&mut out);
        assert_eq!(out, vec![4.0, 5.0, 6.0, 7.0, 8.0]);
    }

    #[test]
    fn preroll_clear_leaves_empty_buffer() {
        let mut buf = PreRollBuffer::new(4);
        buf.push(&[1.0, 2.0]);
        buf.clear();
        let mut out = Vec::new();
        buf.copy_into(&mut out);
        assert!(out.is_empty());
    }

    #[test]
    fn rms_handles_empty_zero_and_full_scale() {
        assert_eq!(calculate_rms(&[]), 0.0);
        assert_eq!(calculate_rms(&[0.0, 0.0, 0.0]), 0.0);
        let rms = calculate_rms(&[1.0, 1.0, 1.0]);
        assert!((rms - 1.0).abs() < 1e-5);
    }

    #[test]
    fn pcm_conversion_clamps_out_of_range_samples() {
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
    fn window_autostop_fires_once_silence_exceeds_threshold() {
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
    fn window_autostop_ignores_silence_before_first_speech() {
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
    fn window_autostop_does_nothing_without_config() {
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
    fn tier1_fallback_uses_configured_silence_duration() {
        // silence_ms = 160 => 10 frames of 16ms
        let mut rig = Rig::new(32, 160, 0.01);
        rig.step(true);
        rig.step(true); // turn opens
        assert!(rig.state.in_speech);

        for _ in 0..9 {
            rig.step(false);
        }
        assert!(
            rig.state.in_speech,
            "turn must stay open before silence duration is reached"
        );

        rig.step(false); // 10th frame reaches configured silence duration
        assert!(
            !rig.state.in_speech,
            "turn must close upon reaching configured silence duration"
        );
        let events = rig.events();
        assert!(
            events.iter().any(|e| matches!(e, VoxEvent::SpeechEnd { .. })),
            "SpeechEnd must be emitted at silence threshold"
        );
    }
}
