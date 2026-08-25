use super::VadBackend;
use super::VadEngine as _;
use crate::core::events::VoxEvent;
use crate::core::settings::{AudioOutputMode, InteractionMode};
use crate::core::state::{InteractionOwner, VadCommand};
use crate::services::stt::SttCommand;
use crate::utils::audio_filters::FilterBank;
use anyhow::Result;
use serde_json::json;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;
use tauri::{AppHandle, Emitter};
use tokio::sync::mpsc;

/// Internal mutable state maintained by the synchronous VAD actor loop.
pub struct VadActorState {
    pub threshold: f32,
    pub noise_gate: f32,
    pub mode: InteractionMode,
    pub audio_mode: AudioOutputMode,
    pub in_speech: bool,
    pub current_turn_id: u32,
    pub active_frames: usize,
    pub inactive_frames: usize,
    pub samples_since_partial: usize,
    pub utterance_buffer: Vec<f32>,
    pub pre_roll_buffer: PreRollBuffer,
    pub realtime_tx: Option<tokio::sync::mpsc::Sender<Vec<i16>>>,
    pub realtime_is_ptt: bool,
    pub ui_emit_counter: usize,
}

impl VadActorState {
    /// Initializes actor state with settings snapshots and allocated buffers.
    pub fn new(
        threshold: f32,
        noise_gate: f32,
        mode: InteractionMode,
        audio_mode: AudioOutputMode,
    ) -> Self {
        Self {
            threshold,
            noise_gate,
            mode,
            audio_mode,
            in_speech: false,
            current_turn_id: 0,
            active_frames: 0,
            inactive_frames: 0,
            samples_since_partial: 0,
            utterance_buffer: Vec::new(),
            pre_roll_buffer: PreRollBuffer::new(8000),
            realtime_tx: None,
            realtime_is_ptt: false,
            ui_emit_counter: 0,
        }
    }
}

/// Drains and applies pending hot-reload commands without acquiring locks.
fn process_vad_commands(
    vad_rx: &std::sync::mpsc::Receiver<VadCommand>,
    vad: &mut VadBackend,
    state: &mut VadActorState,
) -> bool {
    let mut should_exit = false;
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
                state.mode = m;
            }
            VadCommand::UpdateAudioMode(m) => {
                log::info!("[VAD Actor] Updating audio output mode to {:?}", m);
                state.audio_mode = m;
            }
            VadCommand::StartRealtime { tx, is_ptt } => {
                log::info!("[VAD Actor] Starting realtime routing (is_ptt={})", is_ptt);
                state.realtime_tx = Some(tx);
                state.realtime_is_ptt = is_ptt;
            }
            VadCommand::StopRealtime => {
                log::info!("[VAD Actor] Stopping realtime routing");
                state.realtime_tx = None;
                state.realtime_is_ptt = false;
            }
            VadCommand::Shutdown => {
                log::info!("[VAD Actor] Shutdown signal received");
                should_exit = true;
                break;
            }
        }
    }

    if !should_exit {
        if let Err(std::sync::mpsc::TryRecvError::Disconnected) = vad_rx.try_recv() {
            log::info!("[VAD Actor] Command channel disconnected. Exiting loop.");
            should_exit = true;
        }
    }

    should_exit
}

/// Calculates and emits audio energy telemetry to the monitoring pipeline and UI.
fn emit_audio_telemetry(
    chunk: &[f32],
    state: &mut VadActorState,
    app: &AppHandle,
    owner: InteractionOwner,
    filter_bank: &mut FilterBank,
    telemetry_tx: &crossbeam_channel::Sender<crate::monitoring::aggregator::TelemetryEvent>,
    dropped_counter: &Arc<std::sync::atomic::AtomicU64>,
) -> f32 {
    let (raw_low, raw_mid, raw_high) = filter_bank.process_chunk(chunk);
    let raw_energy = calculate_rms(chunk);

    let gated_raw = if raw_energy > state.noise_gate {
        raw_energy
    } else {
        0.0
    };
    let energy = (gated_raw * 12.0).clamp(0.0, 1.0).powf(0.5);

    let gated_low = if raw_low > state.noise_gate {
        raw_low
    } else {
        0.0
    };
    let gated_mid = if raw_mid > state.noise_gate {
        raw_mid
    } else {
        0.0
    };
    let gated_high = if raw_high > state.noise_gate {
        raw_high
    } else {
        0.0
    };

    let low = (gated_low * 12.0).clamp(0.0, 1.0).powf(0.5);
    let mid = (gated_mid * 12.0).clamp(0.0, 1.0).powf(0.5);
    let high = (gated_high * 12.0).clamp(0.0, 1.0).powf(0.5);

    if telemetry_tx
        .try_send(crate::monitoring::aggregator::TelemetryEvent::AudioEnergy {
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

    state.ui_emit_counter += 1;
    if state.ui_emit_counter >= 2 {
        let target = match owner {
            InteractionOwner::Assistant => "main",
            InteractionOwner::Dictation => "tray",
        };
        let _ = app.emit_to(target, "audio_energy", energy);
        state.ui_emit_counter = 0;
    }

    raw_energy
}

/// Streams raw audio chunks directly to an active realtime server session when running in passive mode.
fn stream_passive_realtime(chunk: &[f32], state: &VadActorState) {
    if let Some(ref tx) = state.realtime_tx {
        if !state.realtime_is_ptt {
            let i16_samples: Vec<i16> = chunk
                .iter()
                .map(|&x| (x.clamp(-1.0, 1.0) * 32767.0) as i16)
                .collect();
            let _ = tx.try_send(i16_samples);
        }
    }
}

/// Checks whether audio processing should be ducked due to speaker output or disabled dictation.
fn should_suppress_audio(
    owner: InteractionOwner,
    playback_active: &AtomicBool,
    dictation_enabled: &AtomicBool,
    state: &VadActorState,
) -> bool {
    if owner == InteractionOwner::Dictation && !dictation_enabled.load(Ordering::Relaxed) {
        return true;
    }

    let is_playing = playback_active.load(Ordering::Relaxed);
    state.realtime_tx.is_none() && is_playing && state.audio_mode == AudioOutputMode::Speaker
}

/// Handles speech start event transition, stream resets, and pre-roll transfer.
fn handle_speech_start(
    state: &mut VadActorState,
    turn_id_atomic: &AtomicU32,
    owner: InteractionOwner,
    stt_tx: &std::sync::mpsc::Sender<SttCommand>,
    vox_event_tx: Option<&std::sync::mpsc::Sender<VoxEvent>>,
    event_tx: &mpsc::Sender<serde_json::Value>,
) {
    state.in_speech = true;
    state.current_turn_id = turn_id_atomic.fetch_add(1, Ordering::Relaxed) + 1;

    log::info!(
        "[VAD Actor] Speech Start (turn: {}, owner: {:?})",
        state.current_turn_id,
        owner
    );

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

    let _ = event_tx.try_send(json!({
        "type": "speech_start",
        "session_id": state.current_turn_id
    }));

    state.utterance_buffer.clear();
    state
        .utterance_buffer
        .extend_from_slice(state.pre_roll_buffer.as_slice());
    state.samples_since_partial = state.utterance_buffer.len();
    state.pre_roll_buffer.clear();
}

/// Handles speech end event transition, flushing VAD detector and dispatching final STT audio.
fn handle_speech_end(
    vad: &mut VadBackend,
    state: &mut VadActorState,
    owner: InteractionOwner,
    stt_tx: &std::sync::mpsc::Sender<SttCommand>,
    vox_event_tx: Option<&std::sync::mpsc::Sender<VoxEvent>>,
    event_tx: &mpsc::Sender<serde_json::Value>,
) {
    state.in_speech = false;
    log::info!(
        "[VAD Actor] Speech End (turn: {}, owner: {:?})",
        state.current_turn_id,
        owner
    );

    if let Some(tx) = vox_event_tx {
        if let Err(e) = tx.send(VoxEvent::SpeechEnd {
            turn_id: state.current_turn_id,
            audio_buffer: state.utterance_buffer.clone(),
        }) {
            log::warn!("[VAD Actor] Failed to send SpeechEnd event: {}", e);
        }
    }

    let _ = event_tx.try_send(json!({
        "type": "speech_end",
        "session_id": state.current_turn_id
    }));

    vad.flush();

    if state.utterance_buffer.len() >= 4800 && state.realtime_tx.is_none() {
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

    if let Some(ref tx) = state.realtime_tx {
        let i16_samples: Vec<i16> = chunk
            .iter()
            .map(|&x| (x.clamp(-1.0, 1.0) * 32767.0) as i16)
            .collect();
        let _ = tx.try_send(i16_samples);
    }

    if state.samples_since_partial >= 12800 {
        if state.realtime_tx.is_none() {
            let start_idx = state.utterance_buffer.len().saturating_sub(240000);
            if let Err(e) = stt_tx.send(SttCommand::Partial(
                state.current_turn_id,
                state.utterance_buffer[start_idx..].to_vec(),
            )) {
                log::warn!("[VAD Actor] Failed to send Partial audio to STT: {}", e);
            }
        }
        state.samples_since_partial = 0;
    }
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
    pub is_loaded: Arc<AtomicBool>,
    pub playback_active: Arc<AtomicBool>,
    pub turn_id_atomic: Arc<AtomicU32>,
    pub owner_atomic: Arc<AtomicU32>,
    pub is_dictation_enabled: Arc<AtomicBool>,
    pub engine_shutdown: Arc<AtomicBool>,
    pub dropped_counter: Arc<std::sync::atomic::AtomicU64>,
}

/// Communication channels utilized by the VAD actor.
pub struct VadActorChannels {
    pub event_tx: mpsc::Sender<serde_json::Value>,
    pub stt_tx: std::sync::mpsc::Sender<SttCommand>,
    pub vad_rx: std::sync::mpsc::Receiver<VadCommand>,
    pub telemetry_tx: crossbeam_channel::Sender<crate::monitoring::aggregator::TelemetryEvent>,
    pub vox_event_tx: Option<std::sync::mpsc::Sender<VoxEvent>>,
}

struct VadFrameContext<'a> {
    is_earshot: bool,
    turn_id_atomic: &'a AtomicU32,
    owner: InteractionOwner,
    stt_tx: &'a std::sync::mpsc::Sender<SttCommand>,
    vox_event_tx: Option<&'a std::sync::mpsc::Sender<VoxEvent>>,
    event_tx: &'a mpsc::Sender<serde_json::Value>,
}

/// Evaluates voice activity on incoming frame and advances speech onset or offset counters.
fn process_speech_frame(
    chunk: &[f32],
    raw_energy: f32,
    vad: &mut VadBackend,
    state: &mut VadActorState,
    ctx: &VadFrameContext<'_>,
) {
    let is_speech =
        vad.predict(chunk) && is_above_noise_gate(raw_energy, state.noise_gate, ctx.is_earshot);

    if is_speech {
        state.active_frames += 1;
        state.inactive_frames = 0;

        if !state.in_speech && state.active_frames >= 2 {
            handle_speech_start(
                state,
                ctx.turn_id_atomic,
                ctx.owner,
                ctx.stt_tx,
                ctx.vox_event_tx,
                ctx.event_tx,
            );
        }
    } else {
        state.inactive_frames += 1;
        state.active_frames = 0;

        if state.in_speech && state.inactive_frames >= 50 {
            handle_speech_end(
                vad,
                state,
                ctx.owner,
                ctx.stt_tx,
                ctx.vox_event_tx,
                ctx.event_tx,
            );
        }
    }

    if state.in_speech {
        accumulate_speech_frames(chunk, state, ctx.stt_tx);
    } else {
        state.pre_roll_buffer.push(chunk);
    }
}

/// Spawns the synchronous, low-latency VAD actor thread.
pub fn spawn_vad_actor<C>(
    mut vad: VadBackend,
    app: tauri::AppHandle,
    mut consumer: C,
    channels: VadActorChannels,
    handles: VadActorHandles,
    config: VadActorConfig,
) -> Result<()>
where
    C: ringbuf::traits::Consumer<Item = f32>,
{
    use thread_priority::*;
    if let Err(e) = set_current_thread_priority(ThreadPriority::Max) {
        log::warn!("[VAD Actor] Thread priority elevation failed: {:?}", e);
    }

    log::info!("[VAD Actor] Starting synchronous VAD loop on dedicated thread");

    let mut state = VadActorState::new(
        config.initial_threshold,
        config.initial_noise_gate,
        config.initial_mode,
        config.initial_audio_mode,
    );
    let is_earshot = matches!(vad, VadBackend::Earshot(_));
    let mut filter_bank = FilterBank::new(16000.0);
    let mut chunk = vec![0.0f32; 256];
    handles.is_loaded.store(true, Ordering::Relaxed);

    loop {
        if handles.engine_shutdown.load(Ordering::Relaxed) {
            log::info!("[VAD Actor] Engine shutdown detected");
            handles.is_loaded.store(false, Ordering::Relaxed);
            return Ok(());
        }

        if process_vad_commands(&channels.vad_rx, &mut vad, &mut state) {
            handles.is_loaded.store(false, Ordering::Relaxed);
            return Ok(());
        }

        if consumer.occupied_len() >= 256 {
            consumer.pop_slice(&mut chunk);

            let owner: InteractionOwner = handles.owner_atomic.load(Ordering::Relaxed).into();

            let raw_energy = emit_audio_telemetry(
                &chunk,
                &mut state,
                &app,
                owner,
                &mut filter_bank,
                &channels.telemetry_tx,
                &handles.dropped_counter,
            );

            stream_passive_realtime(&chunk, &state);

            let effective_mode = if state.realtime_tx.is_some() && !state.realtime_is_ptt {
                InteractionMode::Passive
            } else {
                state.mode.clone()
            };

            if effective_mode == InteractionMode::PTT {
                state.pre_roll_buffer.push(&chunk);
                if state.in_speech {
                    state.in_speech = false;
                    state.utterance_buffer.clear();
                    state.samples_since_partial = 0;
                }
                continue;
            }

            if should_suppress_audio(owner, &handles.playback_active, &handles.is_dictation_enabled, &state) {
                continue;
            }

            let frame_ctx = VadFrameContext {
                is_earshot,
                turn_id_atomic: &handles.turn_id_atomic,
                owner,
                stt_tx: &channels.stt_tx,
                vox_event_tx: channels.vox_event_tx.as_ref(),
                event_tx: &channels.event_tx,
            };

            process_speech_frame(
                &chunk,
                raw_energy,
                &mut vad,
                &mut state,
                &frame_ctx,
            );
        } else {
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
    }
}

/// Bounded circular-like buffer for retaining pre-roll audio before speech onset.
#[derive(Debug)]
pub struct PreRollBuffer {
    buffer: Vec<f32>,
    max_capacity: usize,
}

impl PreRollBuffer {
    /// Constructs a pre-roll buffer with a fixed maximum sample capacity.
    pub fn new(max_capacity: usize) -> Self {
        Self {
            buffer: Vec::with_capacity(max_capacity),
            max_capacity,
        }
    }

    /// Appends new audio samples to the pre-roll buffer, draining oldest samples if capacity is exceeded.
    pub fn push(&mut self, chunk: &[f32]) {
        self.buffer.extend_from_slice(chunk);
        if self.buffer.len() > self.max_capacity {
            let excess = self.buffer.len() - self.max_capacity;
            self.buffer.drain(0..excess);
        }
    }

    /// Clears all stored audio samples.
    pub fn clear(&mut self) {
        self.buffer.clear();
    }

    /// Returns a slice view of the current pre-roll audio buffer.
    pub fn as_slice(&self) -> &[f32] {
        &self.buffer
    }
}

/// Calculates Root Mean Square (RMS) energy of an audio sample slice.
pub fn calculate_rms(chunk: &[f32]) -> f32 {
    if chunk.is_empty() {
        return 0.0;
    }
    (chunk.iter().map(|&x| x * x).sum::<f32>() / chunk.len() as f32).sqrt()
}

/// Evaluates if raw energy satisfies the noise gate threshold.
pub fn is_above_noise_gate(raw_energy: f32, noise_gate: f32, is_earshot: bool) -> bool {
    let effective_noise_gate = if is_earshot {
        noise_gate * 1.5
    } else {
        noise_gate
    };
    raw_energy >= effective_noise_gate
}
