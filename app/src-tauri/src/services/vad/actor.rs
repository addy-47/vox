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

/// Evaluates voice activity on incoming frame and advances speech onset or offset counters.
#[allow(clippy::too_many_arguments)]
fn process_speech_frame(
    chunk: &[f32],
    raw_energy: f32,
    is_earshot: bool,
    vad: &mut VadBackend,
    state: &mut VadActorState,
    turn_id_atomic: &AtomicU32,
    owner: InteractionOwner,
    stt_tx: &std::sync::mpsc::Sender<SttCommand>,
    vox_event_tx: Option<&std::sync::mpsc::Sender<VoxEvent>>,
    event_tx: &mpsc::Sender<serde_json::Value>,
) {
    let is_speech =
        vad.predict(chunk) && is_above_noise_gate(raw_energy, state.noise_gate, is_earshot);

    if is_speech {
        state.active_frames += 1;
        state.inactive_frames = 0;

        if !state.in_speech && state.active_frames >= 2 {
            handle_speech_start(state, turn_id_atomic, owner, stt_tx, vox_event_tx, event_tx);
        }
    } else {
        state.inactive_frames += 1;
        state.active_frames = 0;

        if state.in_speech && state.inactive_frames >= 50 {
            handle_speech_end(vad, state, owner, stt_tx, vox_event_tx, event_tx);
        }
    }

    if state.in_speech {
        accumulate_speech_frames(chunk, state, stt_tx);
    } else {
        state.pre_roll_buffer.push(chunk);
    }
}

/// Spawns the synchronous, low-latency VAD actor thread.
#[allow(clippy::too_many_arguments)]
pub fn spawn_vad_actor<C>(
    mut vad: VadBackend,
    app: tauri::AppHandle,
    mut consumer: C,
    event_tx: mpsc::Sender<serde_json::Value>,
    stt_tx: std::sync::mpsc::Sender<SttCommand>,
    vad_rx: std::sync::mpsc::Receiver<VadCommand>,
    telemetry_tx: crossbeam_channel::Sender<crate::monitoring::aggregator::TelemetryEvent>,
    vox_event_tx: Option<std::sync::mpsc::Sender<VoxEvent>>,
    is_loaded: Arc<AtomicBool>,
    playback_active: Arc<AtomicBool>,
    turn_id_atomic: Arc<AtomicU32>,
    owner_atomic: Arc<AtomicU32>,
    is_dictation_enabled: Arc<AtomicBool>,
    engine_shutdown: Arc<AtomicBool>,
    dropped_counter: Arc<std::sync::atomic::AtomicU64>,
    initial_threshold: f32,
    initial_noise_gate: f32,
    initial_mode: InteractionMode,
    initial_audio_mode: AudioOutputMode,
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
        initial_threshold,
        initial_noise_gate,
        initial_mode,
        initial_audio_mode,
    );
    let is_earshot = matches!(vad, VadBackend::Earshot(_));
    let mut filter_bank = FilterBank::new(16000.0);
    let mut chunk = vec![0.0f32; 256];
    is_loaded.store(true, Ordering::Relaxed);

    loop {
        if engine_shutdown.load(Ordering::Relaxed) {
            log::info!("[VAD Actor] Engine shutdown detected");
            is_loaded.store(false, Ordering::Relaxed);
            return Ok(());
        }

        if process_vad_commands(&vad_rx, &mut vad, &mut state) {
            is_loaded.store(false, Ordering::Relaxed);
            return Ok(());
        }

        if consumer.occupied_len() >= 256 {
            consumer.pop_slice(&mut chunk);

            let owner: InteractionOwner = owner_atomic.load(Ordering::Relaxed).into();

            let raw_energy = emit_audio_telemetry(
                &chunk,
                &mut state,
                &app,
                owner,
                &mut filter_bank,
                &telemetry_tx,
                &dropped_counter,
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

            if should_suppress_audio(owner, &playback_active, &is_dictation_enabled, &state) {
                continue;
            }

            process_speech_frame(
                &chunk,
                raw_energy,
                is_earshot,
                &mut vad,
                &mut state,
                &turn_id_atomic,
                owner,
                &stt_tx,
                vox_event_tx.as_ref(),
                &event_tx,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pre_roll_circular_buffer_cap() {
        let mut pre_roll = PreRollBuffer::new(8000);
        assert_eq!(pre_roll.as_slice().len(), 0);
        assert!(pre_roll.as_slice().is_empty());

        let silence_chunk = vec![0.0f32; 256];
        let total_samples_pushed = 100_000;
        for _ in 0..(total_samples_pushed / 256) {
            pre_roll.push(&silence_chunk);
            assert!(
                pre_roll.as_slice().len() <= 8000,
                "Pre-roll length exceeded cap: {}",
                pre_roll.as_slice().len()
            );
        }

        assert_eq!(pre_roll.as_slice().len(), 8000);

        let marker_chunk = vec![0.99f32; 256];
        pre_roll.push(&marker_chunk);
        assert_eq!(pre_roll.as_slice().len(), 8000);

        let slice = pre_roll.as_slice();
        assert_eq!(&slice[8000 - 256..], marker_chunk.as_slice());

        pre_roll.clear();
        assert_eq!(pre_roll.as_slice().len(), 0);
        assert!(pre_roll.as_slice().is_empty());
    }

    #[test]
    fn test_noise_gate_rms_threshold() {
        let silence = vec![0.0f32; 256];
        assert_eq!(calculate_rms(&silence), 0.0);

        let dc_signal = vec![0.5f32; 256];
        let dc_rms = calculate_rms(&dc_signal);
        assert!((dc_rms - 0.5).abs() < 1e-6);

        let amplitude = 0.8f32;
        let sine_wave: Vec<f32> = (0..256)
            .map(|i| amplitude * (2.0 * std::f32::consts::PI * i as f32 / 16.0).sin())
            .collect();
        let sine_rms = calculate_rms(&sine_wave);
        let expected_rms = amplitude / 2.0f32.sqrt();
        assert!((sine_rms - expected_rms).abs() < 1e-3);

        let noise_gate = 0.02f32;

        let quiet_noise = vec![0.005f32; 256];
        let quiet_rms = calculate_rms(&quiet_noise);
        assert!(quiet_rms < noise_gate);
        assert!(!is_above_noise_gate(quiet_rms, noise_gate, false));

        let loud_signal = vec![0.1f32; 256];
        let loud_rms = calculate_rms(&loud_signal);
        assert!(loud_rms > noise_gate);
        assert!(is_above_noise_gate(loud_rms, noise_gate, false));

        assert!(is_above_noise_gate(0.025, noise_gate, false));
        assert!(!is_above_noise_gate(0.025, noise_gate, true));
    }
}
