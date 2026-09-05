use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::mpsc;
use std::sync::Arc;

use anyhow::Result;
use ringbuf::traits::Consumer;
use thread_priority::{set_current_thread_priority, ThreadPriority};

use super::telemetry::process_and_emit_telemetry;
use super::utils::{f32_to_i16_pcm, PreRollBuffer};
use super::{
    VadBackend, VadEngine as _, VadOperationalMode, VAD_ACTOR_IDLE_SLEEP_MS, VAD_CHUNK_SIZE,
    VAD_MIN_UTTERANCE_SAMPLES, VAD_PARTIAL_INTERVAL_SAMPLES, VAD_PRE_ROLL_CAPACITY,
};
use crate::core::events::VoxEvent;
use crate::core::settings::{AudioOutputMode, InteractionMode};
use crate::core::state::InteractionState;
use crate::monitoring::aggregator::TelemetryEvent;
use crate::services::stt::SttCommand;
use crate::services::vad::VadCommand;
use crate::utils::audio_filters::FilterBank;

/// Result returned from windowed speech validation.
#[derive(Debug, Clone)]
pub struct VadValidationResult {
    pub is_speech_detected: bool,
    pub speech_start_sample: usize,
    pub speech_end_sample: usize,
    pub audio: Vec<f32>,
}

/// Internal mutable state maintained by the synchronous VAD actor loop.
pub struct VadActorState {
    pub threshold: f32,
    pub noise_gate: f32,
    pub speech_end_frames: usize,
    pub speech_start_frames: usize,
    pub mode: InteractionMode,
    pub operational_mode: VadOperationalMode,
    pub audio_mode: AudioOutputMode,
    pub in_speech: bool,
    pub current_turn_id: u32,
    pub active_frames: usize,
    pub inactive_frames: usize,
    pub samples_since_partial: usize,
    pub utterance_buffer: Vec<f32>,
    pub pre_roll_buffer: PreRollBuffer,
    pub realtime_tx: Option<tokio::sync::mpsc::Sender<Vec<i16>>>,
    pub pcm_scratch: Vec<i16>,
    pub partial_recycle_tx: mpsc::SyncSender<Vec<f32>>,
    pub partial_recycle_rx: mpsc::Receiver<Vec<f32>>,
    pub realtime_recycle_tx: mpsc::SyncSender<Vec<i16>>,
    pub realtime_recycle_rx: mpsc::Receiver<Vec<i16>>,

    // WindowedValidation state tracking
    pub window_active: bool,
    pub window_sample_offset: usize,
    pub window_speech_detected: bool,
    pub window_first_speech_sample: usize,
    pub window_last_speech_sample: usize,
    pub window_buffer: Vec<f32>,
}

impl VadActorState {
    /// Initializes actor state with settings snapshots and allocated buffers.
    pub fn new(
        threshold: f32,
        noise_gate: f32,
        silence_duration_ms: u32,
        speech_onset_ms: u32,
        mode: InteractionMode,
        audio_mode: AudioOutputMode,
    ) -> Self {
        let operational_mode = match mode {
            InteractionMode::Passive => VadOperationalMode::ContinuousSegmentation,
            InteractionMode::PTT => VadOperationalMode::WindowedValidation,
        };

        let speech_end_frames = (silence_duration_ms as usize / 16).max(1);
        let speech_start_frames = (speech_onset_ms as usize / 16).max(1);

        let (partial_recycle_tx, partial_recycle_rx) = mpsc::sync_channel(4);
        let (realtime_recycle_tx, realtime_recycle_rx) = mpsc::sync_channel(32);

        Self {
            threshold,
            noise_gate,
            speech_end_frames,
            speech_start_frames,
            mode,
            operational_mode,
            audio_mode,
            in_speech: false,
            current_turn_id: 0,
            active_frames: 0,
            inactive_frames: 0,
            samples_since_partial: 0,
            utterance_buffer: Vec::with_capacity(
                VAD_PRE_ROLL_CAPACITY + VAD_PARTIAL_INTERVAL_SAMPLES * 2,
            ),
            pre_roll_buffer: PreRollBuffer::new(VAD_PRE_ROLL_CAPACITY),
            realtime_tx: None,
            pcm_scratch: Vec::with_capacity(VAD_CHUNK_SIZE),
            partial_recycle_tx,
            partial_recycle_rx,
            realtime_recycle_tx,
            realtime_recycle_rx,
            window_active: false,
            window_sample_offset: 0,
            window_speech_detected: false,
            window_first_speech_sample: 0,
            window_last_speech_sample: 0,
            window_buffer: Vec::new(),
        }
    }
}

/// Drains and applies pending hot-reload commands without acquiring locks.
fn process_vad_commands(
    vad_rx: &mpsc::Receiver<VadCommand>,
    vad: &mut VadBackend,
    state: &mut VadActorState,
) -> bool {
    while let Ok(cmd) = vad_rx.try_recv() {
        match cmd {
            VadCommand::UpdateThreshold(v) => {
                log::info!("[VAD Actor] Updating threshold to {}", v);
                state.threshold = v;
                if let Err(e) = vad.update_threshold(state.threshold) {
                    log::error!("[VAD Actor] Failed to update threshold: {}", e);
                }
            }
            VadCommand::UpdateNoiseGate(v) => {
                log::info!("[VAD Actor] Updating noise gate to {}", v);
                state.noise_gate = v;
            }
            VadCommand::UpdateSilenceDuration(ms) => {
                log::info!("[VAD Actor] Updating silence duration to {} ms", ms);
                state.speech_end_frames = (ms as usize / 16).max(1);
            }
            VadCommand::UpdateSpeechOnset(ms) => {
                log::info!("[VAD Actor] Updating speech onset to {} ms", ms);
                state.speech_start_frames = (ms as usize / 16).max(1);
            }
            VadCommand::UpdateMode(m) => {
                log::info!("[VAD Actor] Updating interaction mode to {:?}", m);
                if m == InteractionMode::PTT && state.in_speech {
                    state.in_speech = false;
                    state.utterance_buffer.clear();
                    state.samples_since_partial = 0;
                }
                state.mode = m;
            }
            VadCommand::UpdateAudioMode(m) => {
                log::info!("[VAD Actor] Updating audio output mode to {:?}", m);
                state.audio_mode = m;
            }
            VadCommand::SetOperationalMode(op) => {
                log::info!("[VAD Actor] Setting operational mode to {:?}", op);
                if op == VadOperationalMode::WindowedValidation && state.in_speech {
                    state.in_speech = false;
                    state.utterance_buffer.clear();
                    state.samples_since_partial = 0;
                }
                state.operational_mode = op;
            }
            VadCommand::StartWindowValidation => {
                log::debug!("[VAD Actor] Starting windowed speech validation");
                state.window_active = true;
                state.window_buffer.clear();
                state.pre_roll_buffer.copy_into(&mut state.window_buffer);
                state.window_sample_offset = state.window_buffer.len();
                state.pre_roll_buffer.clear();
                state.window_speech_detected = false;
                state.window_first_speech_sample = 0;
                state.window_last_speech_sample = 0;
            }
            VadCommand::StopWindowValidation { response_tx } => {
                log::debug!(
                    "[VAD Actor] Stopping windowed validation (detected={}, start={}, end={}, buffer_len={})",
                    state.window_speech_detected,
                    state.window_first_speech_sample,
                    state.window_last_speech_sample,
                    state.window_buffer.len()
                );
                state.window_active = false;
                let raw_len = state.window_buffer.len();
                let start = state.window_first_speech_sample.min(raw_len);
                let end = state.window_last_speech_sample.min(raw_len);
                let trimmed_audio =
                    if state.window_speech_detected && start < end && (end - start) >= 256 {
                        state.window_buffer[start..end].to_vec()
                    } else if state.window_speech_detected {
                        std::mem::take(&mut state.window_buffer)
                    } else {
                        Vec::new()
                    };
                state.window_buffer.clear();
                let result = VadValidationResult {
                    is_speech_detected: state.window_speech_detected,
                    speech_start_sample: state.window_first_speech_sample,
                    speech_end_sample: state.window_last_speech_sample,
                    audio: trimmed_audio,
                };
                if response_tx.send(result).is_err() {
                    log::warn!("[VAD Actor] Failed to send StopWindowValidation response");
                }
            }
            VadCommand::StartRealtime { tx, is_ptt } => {
                log::info!("[VAD Actor] Starting realtime routing (is_ptt={})", is_ptt);
                state.realtime_tx = Some(tx);
                if !is_ptt {
                    state.operational_mode = VadOperationalMode::StreamPassthrough;
                } else {
                    state.operational_mode = VadOperationalMode::WindowedValidation;
                }
            }
            VadCommand::StopRealtime => {
                log::info!("[VAD Actor] Stopping realtime routing");
                state.realtime_tx = None;
                state.operational_mode = match state.mode {
                    InteractionMode::Passive => VadOperationalMode::ContinuousSegmentation,
                    InteractionMode::PTT => VadOperationalMode::WindowedValidation,
                };
            }
            VadCommand::Shutdown => {
                log::info!("[VAD Actor] Shutdown signal received");
                return true;
            }
        }
    }
    false
}

/// Checks whether audio processing should be ducked due to speaker output or explicit suppression.
fn should_suppress_audio(
    audio_suppressed: &AtomicBool,
    state_atomic: &AtomicU32,
    state: &VadActorState,
) -> bool {
    if audio_suppressed.load(Ordering::Relaxed) {
        return true;
    }

    state.realtime_tx.is_none()
        && InteractionState::from(state_atomic.load(Ordering::Relaxed))
            == InteractionState::Speaking
        && state.audio_mode == AudioOutputMode::Speaker
}

/// Configuration settings for the VAD actor.
#[derive(Debug, Clone)]
pub struct VadActorConfig {
    pub initial_threshold: f32,
    pub initial_noise_gate: f32,
    pub initial_silence_duration_ms: u32,
    pub initial_speech_onset_ms: u32,
    pub initial_mode: InteractionMode,
    pub initial_audio_mode: AudioOutputMode,
}

/// Shared atomic handles and flags passed to the VAD actor.
#[derive(Clone)]
pub struct VadActorHandles {
    pub state_atomic: Arc<AtomicU32>,
    pub turn_id_atomic: Arc<AtomicU32>,
    pub audio_suppressed: Arc<AtomicBool>,
    pub engine_shutdown: Arc<AtomicBool>,
    pub dropped_counter: Arc<AtomicU64>,
    pub ingestion_gate: Arc<AtomicBool>,
}

/// Communication channels utilized by the VAD actor.
pub struct VadActorChannels {
    pub stt_tx: mpsc::Sender<SttCommand>,
    pub vad_rx: mpsc::Receiver<VadCommand>,
    pub telemetry_tx: crossbeam_channel::Sender<TelemetryEvent>,
    pub vox_event_tx: Option<mpsc::Sender<VoxEvent>>,
}

/// Handles speech start event transition, stream resets, and pre-roll transfer.
fn handle_speech_start(
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
fn handle_speech_end(
    vad: &mut VadBackend,
    state: &mut VadActorState,
    stt_tx: &mpsc::Sender<SttCommand>,
    vox_event_tx: Option<&mpsc::Sender<VoxEvent>>,
) {
    state.in_speech = false;
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
fn accumulate_speech_frames(
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
fn process_continuous_segmentation(
    chunk: &[f32],
    raw_energy: f32,
    vad: &mut VadBackend,
    state: &mut VadActorState,
    handles: &VadActorHandles,
    stt_tx: &mpsc::Sender<SttCommand>,
    vox_event_tx: Option<&mpsc::Sender<VoxEvent>>,
) {
    let is_speech = vad.predict(chunk) && vad.is_above_noise_gate(raw_energy, state.noise_gate);

    if is_speech {
        state.active_frames += 1;
        state.inactive_frames = 0;

        if !state.in_speech && state.active_frames >= state.speech_start_frames {
            handle_speech_start(state, handles, stt_tx, vox_event_tx);
        }

        if state.in_speech {
            accumulate_speech_frames(chunk, state, stt_tx);
        }
    } else {
        state.inactive_frames += 1;
        state.active_frames = 0;

        if state.in_speech {
            if state.inactive_frames >= state.speech_end_frames {
                handle_speech_end(vad, state, stt_tx, vox_event_tx);
                state.pre_roll_buffer.push(chunk);
            } else {
                accumulate_speech_frames(chunk, state, stt_tx);
            }
        } else {
            state.pre_roll_buffer.push(chunk);
        }
    }
}

/// Executes WindowedValidation mode to evaluate speech presence and sample boundaries in caller-owned windows.
fn process_windowed_validation(
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
fn process_stream_passthrough(chunk: &[f32], state: &mut VadActorState) {
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

/// Spawns the synchronous, low-latency VAD actor thread.
pub fn spawn_vad_actor<C>(
    mut vad: VadBackend,
    mut consumer: C,
    channels: VadActorChannels,
    handles: VadActorHandles,
    config: VadActorConfig,
) -> Result<()>
where
    C: Consumer<Item = f32>,
{
    if let Err(e) = set_current_thread_priority(ThreadPriority::Max) {
        log::warn!("[VAD Actor] Thread priority elevation failed: {:?}", e);
    }

    log::info!("[VAD Actor] Starting synchronous VAD loop on dedicated thread");

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let mut state = VadActorState::new(
            config.initial_threshold,
            config.initial_noise_gate,
            config.initial_silence_duration_ms,
            config.initial_speech_onset_ms,
            config.initial_mode,
            config.initial_audio_mode,
        );
        let mut filter_bank = FilterBank::new(16000.0);
        let mut chunk = vec![0.0f32; VAD_CHUNK_SIZE];

        loop {
            if handles.engine_shutdown.load(Ordering::Relaxed) {
                log::info!("[VAD Actor] Engine shutdown detected");
                return Ok(());
            }

            if process_vad_commands(&channels.vad_rx, &mut vad, &mut state) {
                return Ok(());
            }

            if !handles.ingestion_gate.load(Ordering::Relaxed) {
                if !state.pre_roll_buffer.is_empty()
                    || !state.utterance_buffer.is_empty()
                    || !state.window_buffer.is_empty()
                    || state.in_speech
                    || state.window_active
                {
                    state.pre_roll_buffer.clear();
                    state.utterance_buffer.clear();
                    state.window_buffer.clear();
                    state.in_speech = false;
                    state.window_active = false;
                    log::debug!(
                        "[VAD Actor] Ingestion gate closed — purged in-flight audio buffers"
                    );
                }
                if consumer.occupied_len() >= VAD_CHUNK_SIZE {
                    consumer.pop_slice(&mut chunk);
                }
                std::thread::sleep(std::time::Duration::from_millis(VAD_ACTOR_IDLE_SLEEP_MS));
                continue;
            }

            if consumer.occupied_len() >= VAD_CHUNK_SIZE {
                consumer.pop_slice(&mut chunk);

                let raw_energy = process_and_emit_telemetry(
                    &chunk,
                    &mut filter_bank,
                    state.noise_gate,
                    &channels.telemetry_tx,
                    &handles.dropped_counter,
                );

                if should_suppress_audio(&handles.audio_suppressed, &handles.state_atomic, &state) {
                    continue;
                }

                match state.operational_mode {
                    VadOperationalMode::StreamPassthrough => {
                        process_stream_passthrough(&chunk, &mut state);
                    }
                    VadOperationalMode::ContinuousSegmentation => {
                        process_continuous_segmentation(
                            &chunk,
                            raw_energy,
                            &mut vad,
                            &mut state,
                            &handles,
                            &channels.stt_tx,
                            channels.vox_event_tx.as_ref(),
                        );
                    }
                    VadOperationalMode::WindowedValidation => {
                        process_windowed_validation(&chunk, raw_energy, &mut vad, &mut state);
                    }
                }
            } else {
                std::thread::sleep(std::time::Duration::from_millis(VAD_ACTOR_IDLE_SLEEP_MS));
            }
        }
    }));

    match result {
        Ok(res) => res,
        Err(err) => {
            log::error!("[VAD Actor] Panic in VAD loop: {:?}", err);
            Err(anyhow::anyhow!("VAD actor thread panicked"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::settings::AudioOutputMode;
    use crate::core::state::InteractionState;
    use std::sync::atomic::{AtomicBool, AtomicU32};

    fn make_state(
        audio_mode: AudioOutputMode,
        state_val: u32,
        realtime_tx: Option<tokio::sync::mpsc::Sender<Vec<i16>>>,
    ) -> (VadActorState, Arc<AtomicU32>, Arc<AtomicBool>) {
        let mut s = VadActorState::new(0.5, 0.001, 800, 32, InteractionMode::Passive, audio_mode);
        s.realtime_tx = realtime_tx;
        let state_atomic = Arc::new(AtomicU32::new(state_val));
        let suppressed = Arc::new(AtomicBool::new(false));
        (s, state_atomic, suppressed)
    }

    /// Tests should_suppress_audio returns false when state is not Speaking.
    #[test]
    fn test_suppression_requires_speaking_state() {
        let (state, atomic, suppressed) = make_state(
            AudioOutputMode::Speaker,
            InteractionState::Ready as u32,
            None,
        );
        assert!(!should_suppress_audio(&suppressed, &atomic, &state));
        let (state2, atomic2, suppressed2) = make_state(
            AudioOutputMode::Speaker,
            InteractionState::Listening as u32,
            None,
        );
        assert!(!should_suppress_audio(&suppressed2, &atomic2, &state2));
    }

    /// Tests suppression active only for Speaker + Speaking + no realtime_tx.
    #[test]
    fn test_suppression_speaker_speaking_no_realtime() {
        let (state, atomic, suppressed) = make_state(
            AudioOutputMode::Speaker,
            InteractionState::Speaking as u32,
            None,
        );
        assert!(should_suppress_audio(&suppressed, &atomic, &state));
    }

    /// Tests Headset never suppresses even while Speaking.
    #[test]
    fn test_headset_never_suppresses() {
        let (state, atomic, suppressed) = make_state(
            AudioOutputMode::Headset,
            InteractionState::Speaking as u32,
            None,
        );
        assert!(!should_suppress_audio(&suppressed, &atomic, &state));
    }

    /// Tests realtime_tx Some bypasses suppression (passthrough mode).
    #[test]
    fn test_realtime_bypasses_suppression() {
        let (tx, _rx) = tokio::sync::mpsc::channel(1);
        let (state, atomic, suppressed) = make_state(
            AudioOutputMode::Speaker,
            InteractionState::Speaking as u32,
            Some(tx),
        );
        assert!(!should_suppress_audio(&suppressed, &atomic, &state));
    }

    /// Tests explicit audio_suppressed flag forces suppression regardless of state/mode.
    #[test]
    fn test_audio_suppressed_flag_forces_suppression() {
        let (state, atomic, suppressed) = make_state(
            AudioOutputMode::Headset,
            InteractionState::Ready as u32,
            None,
        );
        suppressed.store(true, Ordering::Relaxed);
        assert!(should_suppress_audio(&suppressed, &atomic, &state));
        let (state2, atomic2, suppressed2) = make_state(
            AudioOutputMode::Speaker,
            InteractionState::Ready as u32,
            None,
        );
        suppressed2.store(true, Ordering::Relaxed);
        assert!(should_suppress_audio(&suppressed2, &atomic2, &state2));
    }

    /// Tests VadValidationResult trimming: speech window within buffer bounds.
    #[test]
    fn test_window_validation_trimming_logic() {
        let mut s = VadActorState::new(
            0.5,
            0.001,
            800,
            32,
            InteractionMode::PTT,
            AudioOutputMode::Speaker,
        );
        s.window_active = true;
        s.window_buffer = vec![0.0; 1000];
        s.window_speech_detected = true;
        s.window_first_speech_sample = 100;
        s.window_last_speech_sample = 900;
        let raw_len = s.window_buffer.len();
        let start = s.window_first_speech_sample.min(raw_len);
        let end = s.window_last_speech_sample.min(raw_len);
        assert_eq!(start, 100);
        assert_eq!(end, 900);
        assert!(start < end && (end - start) >= 256);
    }
}
