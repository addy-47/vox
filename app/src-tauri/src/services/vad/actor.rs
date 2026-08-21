use super::VadBackend;
use super::VadEngine as _;
use crate::core::settings::InteractionMode;
use crate::core::state::{InteractionOwner, VadCommand};
use anyhow::Result;
use serde_json::json;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use tauri::Manager;
use tokio::sync::mpsc;

#[allow(clippy::too_many_arguments)]
pub fn spawn_vad_actor<C>(
    mut vad: VadBackend,
    app: tauri::AppHandle,
    mut consumer: C,
    event_tx: mpsc::Sender<serde_json::Value>,
    stt_tx: std::sync::mpsc::Sender<crate::services::stt::SttCommand>,
    vad_rx: std::sync::mpsc::Receiver<VadCommand>,
    telemetry_tx: crossbeam_channel::Sender<crate::monitoring::aggregator::TelemetryEvent>,
    vox_event_tx: Option<std::sync::mpsc::Sender<crate::core::events::VoxEvent>>,
    is_loaded: Arc<std::sync::atomic::AtomicBool>,
) -> Result<()>
where
    C: ringbuf::traits::Consumer<Item = f32>,
{
    // ── Phase 1: Thread Prioritization (Tier 1 - Realtime) ───────────────
    use thread_priority::*;
    if let Err(e) = set_current_thread_priority(ThreadPriority::Max) {
        log::warn!(
            "[VAD] Failed to set max priority (likely non-root/cap_sys_nice): {:?}",
            e
        );
    }

    log::info!("[VAD] Starting synchronous VAD loop on dedicated thread.");

    let mut in_speech = false;
    let mut current_turn_id: u32 = 0;
    let mut utterance_buffer: Vec<f32> = Vec::new();
    let mut samples_since_partial = 0;
    let mut pre_roll_buffer = PreRollBuffer::new(8000); // 500ms pre-roll
    let mut active_frames = 0;
    let mut inactive_frames = 0;

    // Local state initialized once, updated via vad_rx to avoid hot-path locks
    let (threshold_init, noise_gate_init, mode_init, owner_init, audio_mode_init) = {
        let state: tauri::State<'_, std::sync::Arc<crate::core::state::AppState>> = app.state();
        let settings = state.settings.read().unwrap();
        let owner: InteractionOwner = state
            .owner
            .load(std::sync::atomic::Ordering::Relaxed)
            .into();
        let mode = match owner {
            InteractionOwner::Dictation => match settings.dictation.interaction_mode {
                crate::core::settings::DictationInteractionMode::Passive => {
                    InteractionMode::Passive
                }
                crate::core::settings::DictationInteractionMode::Ptt => InteractionMode::PTT,
            },
            InteractionOwner::MainWindow => settings.interaction.main_app_mode.clone(),
            InteractionOwner::Ptt => InteractionMode::PTT,
            InteractionOwner::Wizard => InteractionMode::Passive,
        };
        (
            settings.vad.threshold,
            settings.vad.ptt_noise_gate,
            mode,
            owner,
            settings.audio.output_mode.clone(),
        )
    };

    let mut threshold = threshold_init;
    let mut noise_gate = noise_gate_init;
    let mut mode = mode_init;
    let mut owner = owner_init;
    let mut audio_mode = audio_mode_init;
    let mut realtime_tx: Option<tokio::sync::mpsc::Sender<Vec<i16>>> = None;
    // Tracks whether the active realtime session is PTT-gated (true) or
    // fully passive (false). Only meaningful when realtime_tx is Some.
    let mut realtime_is_ptt: bool = false;

    let (dropped_counter, engine_shutdown) = {
        let state: tauri::State<'_, std::sync::Arc<crate::core::state::AppState>> = app.state();
        (
            state.dropped_telemetry_events.clone(),
            state.pipeline.engine_shutdown.clone(),
        )
    };

    let is_earshot = matches!(vad, VadBackend::Earshot(_));
    is_loaded.store(true, Ordering::Relaxed);
    log::info!(
        "[VAD] Entering sync loop: threshold={}, noise_gate={}, is_earshot={}, mode={:?}",
        threshold,
        noise_gate,
        is_earshot,
        mode
    );

    let mut filter_bank = crate::utils::audio_filters::FilterBank::new(16000.0);

    // 16ms chunks (256 samples at 16kHz) — matches TenVAD window_size default
    let mut chunk = vec![0.0f32; 256];

    let mut ui_emit_counter = 0;

    loop {
        // Check for global engine shutdown signal
        if engine_shutdown.load(Ordering::Relaxed) {
            log::info!("[VAD] Engine shutdown flag detected. Exiting loop.");
            is_loaded.store(false, Ordering::Relaxed);
            return Ok(());
        }

        // ── 0. Process hot-updates (Lock-Free) ───────────────────────────
        let mut should_exit = false;
        while let Ok(cmd) = vad_rx.try_recv() {
            match cmd {
                VadCommand::UpdateThreshold(v) => {
                    log::info!("[VAD] Updating threshold to {} (Hot-Reloading)...", v);
                    threshold = v;
                    if let Err(e) = vad.update_threshold(threshold) {
                        log::error!("[VAD] Failed to hot-reload detector: {}", e);
                    } else {
                        log::info!("[VAD] Detector hot-reloaded successfully.");
                    }
                }
                VadCommand::UpdateNoiseGate(v) => {
                    log::info!("[VAD] Updating noise gate to {}", v);
                    noise_gate = v;
                }
                VadCommand::UpdateMode(m) => {
                    log::info!("[VAD] Updating interaction mode to {:?}", m);
                    mode = m;
                }
                VadCommand::UpdateOwner(o) => {
                    log::info!("[VAD] Updating interaction owner to {:?}", o);
                    owner = o;

                    let state: tauri::State<'_, std::sync::Arc<crate::core::state::AppState>> =
                        app.state();
                    let settings = state.settings.read().unwrap();
                    mode = match owner {
                        InteractionOwner::Dictation => match settings.dictation.interaction_mode {
                            crate::core::settings::DictationInteractionMode::Passive => {
                                InteractionMode::Passive
                            }
                            crate::core::settings::DictationInteractionMode::Ptt => {
                                InteractionMode::PTT
                            }
                        },
                        InteractionOwner::MainWindow => settings.interaction.main_app_mode.clone(),
                        InteractionOwner::Ptt => InteractionMode::PTT,
                        InteractionOwner::Wizard => InteractionMode::Passive,
                    };
                    log::info!(
                        "[VAD] Automatically recalculated interaction mode to {:?}",
                        mode
                    );
                }
                VadCommand::UpdateAudioMode(m) => {
                    log::info!("[VAD] Updating audio output mode to {:?}", m);
                    audio_mode = m;
                }
                VadCommand::Shutdown => {
                    log::info!("[VAD] Shutdown signal received. Exiting loop.");
                    should_exit = true;
                    break;
                }
                VadCommand::StartRealtime { tx, is_ptt } => {
                    log::info!(
                        "[VAD] Starting realtime S2S audio routing (is_ptt={}).",
                        is_ptt
                    );
                    realtime_tx = Some(tx);
                    realtime_is_ptt = is_ptt;
                }
                VadCommand::StopRealtime => {
                    log::info!("[VAD] Stopping realtime S2S audio routing.");
                    realtime_tx = None;
                    realtime_is_ptt = false;
                }
            }
        }

        // Check for channel disconnection (Sender dropped in stop_engine)
        if !should_exit {
            if let Err(std::sync::mpsc::TryRecvError::Disconnected) = vad_rx.try_recv() {
                log::info!("[VAD] Command channel disconnected. Exiting loop.");
                should_exit = true;
            }
        }

        if should_exit {
            is_loaded.store(false, Ordering::Relaxed);
            return Ok(());
        }

        // Check if we have at least 16ms of audio available (256 samples at 16kHz)
        if consumer.occupied_len() >= 256 {
            consumer.pop_slice(&mut chunk);

            // ── Phase 5: High-Frequency Telemetry ────────────────────────
            // Process the chunk through our filter bank to get low, mid, and high RMS
            let (raw_low, raw_mid, raw_high) = filter_bank.process_chunk(&chunk);
            let raw_energy = calculate_rms(&chunk);

            // Gate individual bands as well, keeping mid and high relative to low/energy
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
                .try_send(crate::monitoring::aggregator::TelemetryEvent::AudioEnergy {
                    energy,
                    vad_prob: 0.0,
                    low,
                    mid,
                    high,
                })
                .is_err()
            {
                dropped_counter.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            }

            ui_emit_counter += 1;
            if ui_emit_counter >= 2 {
                use tauri::Emitter;
                let target = match owner {
                    InteractionOwner::MainWindow | InteractionOwner::Ptt => "main",
                    InteractionOwner::Dictation => "tray",
                    InteractionOwner::Wizard => "wizard",
                };
                let _ = app.emit_to(target, "audio_energy", energy);
                ui_emit_counter = 0;
            }

            // ── Highest-priority: Passive Realtime routing ────────────────────
            // In Realtime Passive mode bypass ALL local VAD/PTT logic — Gemini's
            // server-side VAD handles speech detection. Stream every audio chunk
            // directly. This MUST run before the PTT branch so that users whose
            // VAD mode is historically PTT still get audio flowing in passive
            // realtime sessions.
            if let Some(ref tx) = realtime_tx {
                if !realtime_is_ptt {
                    let i16_samples: Vec<i16> = chunk
                        .iter()
                        .map(|&x| (x.clamp(-1.0, 1.0) * 32767.0) as i16)
                        .collect();
                    if let Err(e) = tx.try_send(i16_samples) {
                        static PASSIVE_DROP: std::sync::atomic::AtomicU64 =
                            std::sync::atomic::AtomicU64::new(0);
                        let cnt = PASSIVE_DROP.fetch_add(1, Ordering::Relaxed);
                        if cnt.is_multiple_of(50) {
                            log::warn!(
                                "[VAD] [Realtime/Passive] Audio bridge backpressure — dropped {} chunks so far: {:?}",
                                cnt + 1,
                                e
                            );
                        }
                    } else {
                        static PASSIVE_ROUTE: std::sync::atomic::AtomicU64 =
                            std::sync::atomic::AtomicU64::new(0);
                        let count = PASSIVE_ROUTE.fetch_add(1, Ordering::Relaxed);
                        if count.is_multiple_of(200) {
                            log::info!(
                                "[VAD] [Realtime/Passive] Streamed audio chunk #{} to Gemini Live",
                                count + 1
                            );
                        }
                    }
                }
                // is_ptt == true: fall through to the PTT block which handles
                // gated forwarding via speech_detected.
            }

            let effective_mode = if realtime_tx.is_some() && !realtime_is_ptt {
                InteractionMode::Passive
            } else {
                mode.clone()
            };

            if effective_mode == InteractionMode::PTT {
                let state: tauri::State<'_, std::sync::Arc<crate::core::state::AppState>> =
                    app.state();
                let is_rec = state.ptt.is_recording.load(Ordering::SeqCst);

                if is_rec {
                    let mut detected = vad.predict(&chunk);

                    // Override prediction on sub-threshold noise
                    let effective_noise_gate = if is_earshot {
                        noise_gate * 1.5
                    } else {
                        noise_gate
                    };
                    if raw_energy < effective_noise_gate {
                        detected = false;
                    }

                    if detected {
                        let was_speech = state.ptt.speech_detected.swap(true, Ordering::Relaxed);
                        if !was_speech {
                            // SPEECH ONSET TRANSITION: Flush pre-roll buffer to avoid clipping the first word
                            if let Some(ref tx) = realtime_tx {
                                let pre_roll_i16: Vec<i16> = pre_roll_buffer
                                    .as_slice()
                                    .iter()
                                    .map(|&x| (x.clamp(-1.0, 1.0) * 32767.0) as i16)
                                    .collect();
                                if let Err(e) = tx.try_send(pre_roll_i16) {
                                    log::error!(
                                        "[VAD] Failed to flush PTT pre-roll audio to S2S: {:?}",
                                        e
                                    );
                                } else {
                                    log::info!("[VAD] Flushed PTT pre-roll audio on speech onset.");
                                }
                                pre_roll_buffer.clear();
                            }
                        }
                    }
                }

                // Waveform telemetry and PTT buffering
                crate::services::ptt::handle_ptt_audio_sync(&app, &chunk);

                // In Realtime PTT: Gate audio bridge forwarding
                if let Some(ref tx) = realtime_tx {
                    let state: tauri::State<'_, std::sync::Arc<crate::core::state::AppState>> =
                        app.state();
                    if state.ptt.speech_detected.load(Ordering::Relaxed) {
                        let i16_samples: Vec<i16> = chunk
                            .iter()
                            .map(|&x| (x.clamp(-1.0, 1.0) * 32767.0) as i16)
                            .collect();
                        if let Err(e) = tx.try_send(i16_samples) {
                            log::error!("[VAD] Failed to send gated PTT audio samples to realtime S2S bridge: {:?}", e);
                        } else {
                            static PTT_ROUTE_COUNT: std::sync::atomic::AtomicU64 =
                                std::sync::atomic::AtomicU64::new(0);
                            let count = PTT_ROUTE_COUNT.fetch_add(1, Ordering::Relaxed);
                            if count.is_multiple_of(200) {
                                log::debug!(
                                    "[VAD] Routing realtime PTT audio chunk (count: {})",
                                    count + 1
                                );
                            }
                        }
                    }
                }

                // In both Modular and Realtime PTT: Accumulate pre-roll when not in speech
                let state: tauri::State<'_, std::sync::Arc<crate::core::state::AppState>> =
                    app.state();
                if !state.ptt.speech_detected.load(Ordering::Relaxed) {
                    pre_roll_buffer.push(&chunk);
                }

                if in_speech {
                    in_speech = false;
                    utterance_buffer.clear();
                    samples_since_partial = 0;
                }
                continue;
            }

            // ── Phase 4: Speaker-mode mic ducking & Tray disabled gating ─────────
            {
                let state: tauri::State<'_, std::sync::Arc<crate::core::state::AppState>> =
                    app.state();

                // If VAD is owned by Dictation but dictation is disabled, bypass speech detection
                let dictation_enabled = match state.settings.read() {
                    Ok(s) => s.dictation.enabled,
                    Err(_) => true,
                };
                if owner == InteractionOwner::Dictation && !dictation_enabled {
                    continue;
                }

                let is_playing = state
                    .pipeline
                    .playback_active
                    .load(std::sync::atomic::Ordering::Relaxed);
                if realtime_tx.is_none()
                    && is_playing
                    && audio_mode == crate::core::settings::AudioOutputMode::Speaker
                {
                    // Drop this frame — do NOT advance utterance buffer or VAD state
                    continue;
                }
            }

            // Classify chunk as speech or silence
            // We must ALWAYS call vad.predict to keep its internal context windows synchronized.
            let mut detected = vad.predict(&chunk);

            // Override hallucinated speech on sub-threshold noise
            if !is_above_noise_gate(raw_energy, noise_gate, is_earshot) {
                detected = false;
            }

            if detected {
                active_frames += 1;
                inactive_frames = 0;

                if !in_speech && active_frames >= 6 {
                    in_speech = true;
                    let app_state: tauri::State<'_, std::sync::Arc<crate::core::state::AppState>> =
                        app.state();
                    current_turn_id =
                        app_state.pipeline.turn_id.fetch_add(1, Ordering::Relaxed) + 1;
                    log::info!(
                        "[VAD] >>> SPEECH START (session: {}, owner: {:?})",
                        current_turn_id,
                        owner
                    );

                    let _ = stt_tx.send(crate::services::stt::SttCommand::ResetStream);

                    if let Some(ref tx) = vox_event_tx {
                        let _ = tx.send(crate::core::events::VoxEvent::SpeechStart {
                            turn_id: current_turn_id,
                            owner,
                        });
                    }

                    let _ = event_tx.try_send(json!({
                        "type": "speech_start",
                        "session_id": current_turn_id
                    }));
                    utterance_buffer.clear();
                    utterance_buffer.extend_from_slice(pre_roll_buffer.as_slice());
                    samples_since_partial = utterance_buffer.len();
                    pre_roll_buffer.clear();
                }
            } else {
                inactive_frames += 1;
                active_frames = 0;

                if in_speech && inactive_frames >= 50 {
                    in_speech = false;
                    log::info!(
                        "[VAD] <<< SPEECH END (session: {}, owner: {:?})",
                        current_turn_id,
                        owner
                    );

                    if let Some(ref tx) = vox_event_tx {
                        let _ = tx.send(crate::core::events::VoxEvent::SpeechEnd {
                            turn_id: current_turn_id,
                            owner,
                        });
                    }

                    let _ = event_tx.try_send(json!({
                        "type": "speech_end",
                        "session_id": current_turn_id
                    }));

                    vad.flush();

                    if utterance_buffer.len() >= 4800 && realtime_tx.is_none() {
                        let _ = stt_tx.send(crate::services::stt::SttCommand::Final(
                            current_turn_id,
                            owner,
                            utterance_buffer.clone(),
                        ));
                    }

                    utterance_buffer.clear();
                    samples_since_partial = 0;
                }
            }

            // Append chunk to the appropriate buffer
            if in_speech {
                utterance_buffer.extend_from_slice(&chunk);
                samples_since_partial += chunk.len();

                if let Some(ref tx) = realtime_tx {
                    let i16_samples: Vec<i16> = chunk
                        .iter()
                        .map(|&x| {
                            let clamped = x.clamp(-1.0, 1.0);
                            (clamped * 32767.0) as i16
                        })
                        .collect();
                    let _ = tx.try_send(i16_samples);
                }

                if samples_since_partial >= 12800 {
                    if realtime_tx.is_none() {
                        let start_idx = utterance_buffer.len().saturating_sub(240000);
                        let _ = stt_tx.send(crate::services::stt::SttCommand::Partial(
                            current_turn_id,
                            owner,
                            utterance_buffer[start_idx..].to_vec(),
                        ));
                    }
                    samples_since_partial = 0;
                }
            } else {
                pre_roll_buffer.push(&chunk);
            }
        } else {
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
    }
}

/// Bounded circular-like buffer for retaining pre-roll audio before speech onset.
#[derive(Debug)]
pub(crate) struct PreRollBuffer {
    buffer: Vec<f32>,
    max_capacity: usize,
}

impl PreRollBuffer {
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

    pub fn clear(&mut self) {
        self.buffer.clear();
    }

    pub fn as_slice(&self) -> &[f32] {
        &self.buffer
    }
}

/// Calculates Root Mean Square (RMS) energy of an audio sample slice.
pub(crate) fn calculate_rms(chunk: &[f32]) -> f32 {
    if chunk.is_empty() {
        return 0.0;
    }
    (chunk.iter().map(|&x| x * x).sum::<f32>() / chunk.len() as f32).sqrt()
}

/// Evaluates if raw energy satisfies the noise gate threshold.
pub(crate) fn is_above_noise_gate(raw_energy: f32, noise_gate: f32, is_earshot: bool) -> bool {
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

        // Push 100,000 samples of silence (in chunks of 256)
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

        // Verify pre-roll capacity stays strictly bounded at 8,000 samples (500ms at 16kHz)
        assert_eq!(pre_roll.as_slice().len(), 8000);

        // Push non-silence chunk with distinct values to verify old samples are drained (FIFO)
        let marker_chunk = vec![0.99f32; 256];
        pre_roll.push(&marker_chunk);
        assert_eq!(pre_roll.as_slice().len(), 8000);

        // The last 256 samples in buffer should be marker_chunk (0.99)
        let slice = pre_roll.as_slice();
        assert_eq!(&slice[8000 - 256..], marker_chunk.as_slice());

        // Test clear
        pre_roll.clear();
        assert_eq!(pre_roll.as_slice().len(), 0);
        assert!(pre_roll.as_slice().is_empty());
    }

    #[test]
    fn test_noise_gate_rms_threshold() {
        // Test RMS energy calculation
        let silence = vec![0.0f32; 256];
        assert_eq!(calculate_rms(&silence), 0.0);

        // Constant DC signal of 0.5 -> RMS = 0.5
        let dc_signal = vec![0.5f32; 256];
        let dc_rms = calculate_rms(&dc_signal);
        assert!((dc_rms - 0.5).abs() < 1e-6);

        // Sine wave with amplitude A -> RMS = A / sqrt(2) approx 0.7071 * A
        let amplitude = 0.8f32;
        let sine_wave: Vec<f32> = (0..256)
            .map(|i| amplitude * (2.0 * std::f32::consts::PI * i as f32 / 16.0).sin())
            .collect();
        let sine_rms = calculate_rms(&sine_wave);
        let expected_rms = amplitude / 2.0f32.sqrt();
        assert!((sine_rms - expected_rms).abs() < 1e-3);

        // Test noise gate thresholding logic
        let noise_gate = 0.02f32;

        // Sub-threshold noise
        let quiet_noise = vec![0.005f32; 256];
        let quiet_rms = calculate_rms(&quiet_noise);
        assert!(quiet_rms < noise_gate);
        assert!(!is_above_noise_gate(quiet_rms, noise_gate, false));

        // Super-threshold signal
        let loud_signal = vec![0.1f32; 256];
        let loud_rms = calculate_rms(&loud_signal);
        assert!(loud_rms > noise_gate);
        assert!(is_above_noise_gate(loud_rms, noise_gate, false));

        // Test Earshot effective noise gate multiplier (1.5x multiplier)
        let _earshot_gate = noise_gate * 1.5; // 0.03
        let borderline_noise = vec![0.025f32; 256]; // RMS = 0.025
        let borderline_rms = calculate_rms(&borderline_noise);

        // Should pass standard gate (0.025 >= 0.02)
        assert!(is_above_noise_gate(borderline_rms, noise_gate, false));
        // Should fail Earshot gate (0.025 < 0.03)
        assert!(!is_above_noise_gate(borderline_rms, noise_gate, true));
    }
}
