use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::Arc;

use anyhow::Result;
use ringbuf::traits::Consumer;
use thread_priority::{set_current_thread_priority, ThreadPriority};

use super::telemetry::process_and_emit_telemetry;
use super::utils::{f32_to_i16_pcm, PreRollBuffer};
use super::{
    VadBackend, VadEngine as _, VadOperationalMode, VAD_ACTOR_IDLE_SLEEP_MS, VAD_CHUNK_SIZE,
    VAD_MAX_PARTIAL_WINDOW_SAMPLES, VAD_MIN_UTTERANCE_SAMPLES, VAD_PARTIAL_INTERVAL_SAMPLES,
    VAD_PRE_ROLL_CAPACITY, VAD_SPEECH_END_FRAMES, VAD_SPEECH_START_FRAMES,
};
use crate::core::events::VoxEvent;
use crate::core::settings::{AudioOutputMode, InteractionMode};
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
        mode: InteractionMode,
        audio_mode: AudioOutputMode,
    ) -> Self {
        let operational_mode = match mode {
            InteractionMode::Passive => VadOperationalMode::ContinuousSegmentation,
            InteractionMode::PTT => VadOperationalMode::WindowedValidation,
        };

        Self {
            threshold,
            noise_gate,
            mode,
            operational_mode,
            audio_mode,
            in_speech: false,
            current_turn_id: 0,
            active_frames: 0,
            inactive_frames: 0,
            samples_since_partial: 0,
            utterance_buffer: Vec::new(),
            pre_roll_buffer: PreRollBuffer::new(VAD_PRE_ROLL_CAPACITY),
            realtime_tx: None,
            pcm_scratch: Vec::with_capacity(VAD_CHUNK_SIZE),
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
    vad_rx: &std::sync::mpsc::Receiver<VadCommand>,
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
        && crate::core::state::InteractionState::from(state_atomic.load(Ordering::Relaxed))
            == crate::core::state::InteractionState::Speaking
        && state.audio_mode == AudioOutputMode::Speaker
}

/// Configuration settings for the VAD actor.
#[derive(Debug, Clone)]
pub struct VadActorConfig {
    pub initial_threshold: f32,
    pub initial_noise_gate: f32,
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
    pub turn_token: Arc<parking_lot::Mutex<tokio_util::sync::CancellationToken>>,
    pub turn_epoch: Arc<std::sync::atomic::AtomicU64>,
}

/// Communication channels utilized by the VAD actor.
pub struct VadActorChannels {
    pub stt_tx: std::sync::mpsc::Sender<SttCommand>,
    pub vad_rx: std::sync::mpsc::Receiver<VadCommand>,
    pub telemetry_tx: crossbeam_channel::Sender<TelemetryEvent>,
    pub vox_event_tx: Option<std::sync::mpsc::Sender<VoxEvent>>,
}

/// Handles speech start event transition, stream resets, and pre-roll transfer.
fn handle_speech_start(
    state: &mut VadActorState,
    handles: &VadActorHandles,
    stt_tx: &std::sync::mpsc::Sender<SttCommand>,
    vox_event_tx: Option<&std::sync::mpsc::Sender<VoxEvent>>,
) {
    state.in_speech = true;
    handles.turn_epoch.fetch_add(1, Ordering::Relaxed);
    {
        let mut guard = handles.turn_token.lock();
        guard.cancel();
        *guard = tokio_util::sync::CancellationToken::new();
    }
    state.current_turn_id = handles.turn_id_atomic.fetch_add(1, Ordering::Relaxed) + 1;

    log::info!("[VAD Actor] Speech Start (turn: {})", state.current_turn_id);

    if let Err(e) = stt_tx.send(SttCommand::ResetStream) {
        log::warn!("[VAD Actor] Failed to send ResetStream to STT: {}", e);
    }

    if let Some(tx) = vox_event_tx {
        if let Err(e) = tx.send(VoxEvent::SpeechStart {
            turn_id: state.current_turn_id,
        }) {
            log::warn!("[VAD Actor] Failed to send SpeechStart event: {}", e);
        }
    }

    state.utterance_buffer.clear();
    state.pre_roll_buffer.copy_into(&mut state.utterance_buffer);
    state.samples_since_partial = state.utterance_buffer.len();
    state.pre_roll_buffer.clear();
}

/// Handles speech end event transition, flushing VAD detector and dispatching final STT audio.
fn handle_speech_end(
    vad: &mut VadBackend,
    state: &mut VadActorState,
    stt_tx: &std::sync::mpsc::Sender<SttCommand>,
    vox_event_tx: Option<&std::sync::mpsc::Sender<VoxEvent>>,
) {
    state.in_speech = false;
    log::info!("[VAD Actor] Speech End (turn: {})", state.current_turn_id);

    if let Some(tx) = vox_event_tx {
        if let Err(e) = tx.send(VoxEvent::SpeechEnd {
            turn_id: state.current_turn_id,
            audio_buffer: state.utterance_buffer.clone(),
        }) {
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

/// Accumulates streaming audio frames during active speech and triggers periodic partial STT transcriptions.
fn accumulate_speech_frames(
    chunk: &[f32],
    state: &mut VadActorState,
    stt_tx: &std::sync::mpsc::Sender<SttCommand>,
) {
    state.utterance_buffer.extend_from_slice(chunk);
    state.samples_since_partial += chunk.len();

    if state.samples_since_partial >= VAD_PARTIAL_INTERVAL_SAMPLES {
        if state.realtime_tx.is_none() {
            let start_idx = state
                .utterance_buffer
                .len()
                .saturating_sub(VAD_MAX_PARTIAL_WINDOW_SAMPLES);
            let slice: Arc<[f32]> = state.utterance_buffer[start_idx..].into();
            if let Err(e) = stt_tx.send(SttCommand::Partial(state.current_turn_id, slice)) {
                log::warn!("[VAD Actor] Failed to send Partial audio to STT: {}", e);
            }
        }
        state.samples_since_partial = 0;
    }
}

/// Executes ContinuousSegmentation mode for autonomous speech bounding.
fn process_continuous_segmentation(
    chunk: &[f32],
    raw_energy: f32,
    vad: &mut VadBackend,
    state: &mut VadActorState,
    handles: &VadActorHandles,
    stt_tx: &std::sync::mpsc::Sender<SttCommand>,
    vox_event_tx: Option<&std::sync::mpsc::Sender<VoxEvent>>,
) {
    let is_speech = vad.predict(chunk) && vad.is_above_noise_gate(raw_energy, state.noise_gate);

    if is_speech {
        state.active_frames += 1;
        state.inactive_frames = 0;

        if !state.in_speech && state.active_frames >= VAD_SPEECH_START_FRAMES {
            handle_speech_start(state, handles, stt_tx, vox_event_tx);
        }
    } else {
        state.inactive_frames += 1;
        state.active_frames = 0;

        if state.in_speech && state.inactive_frames >= VAD_SPEECH_END_FRAMES {
            handle_speech_end(vad, state, stt_tx, vox_event_tx);
        }
    }

    if state.in_speech {
        accumulate_speech_frames(chunk, state, stt_tx);
    } else {
        state.pre_roll_buffer.push(chunk);
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
        f32_to_i16_pcm(chunk, &mut state.pcm_scratch);
        if let Err(e) = tx.try_send(state.pcm_scratch.clone()) {
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
