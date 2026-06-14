use super::VadBackend;
use crate::core::settings::InteractionMode;
use crate::core::state::{InteractionOwner, VadCommand};
use crate::services::traits::VadEngine as _;
use anyhow::Result;
use serde_json::json;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use tauri::Manager;
use tokio::sync::mpsc;

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
    let mut pre_roll_buffer: Vec<f32> = Vec::with_capacity(8000); // 500ms pre-roll
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
            InteractionOwner::Tray => settings.interaction.tray_mode.clone(),
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
                        InteractionOwner::Tray => settings.interaction.tray_mode.clone(),
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
            let raw_energy =
                (chunk.iter().map(|&x| x * x).sum::<f32>() / chunk.len() as f32).sqrt();

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
                    InteractionOwner::Tray => "tray",
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
                        if cnt % 50 == 0 {
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
                        if count % 200 == 0 {
                            log::info!(
                                "[VAD] [Realtime/Passive] Streamed audio chunk #{} to Gemini Live",
                                count + 1
                            );
                        }
                    }
                    // Keep VAD state clean while streaming to Gemini
                    if in_speech {
                        in_speech = false;
                        utterance_buffer.clear();
                        samples_since_partial = 0;
                    }
                    continue;
                }
                // is_ptt == true: fall through to the PTT block which handles
                // gated forwarding via speech_detected.
            }

            if mode == InteractionMode::PTT {
                let state: tauri::State<'_, std::sync::Arc<crate::core::state::AppState>> = app.state();
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
                                    .iter()
                                    .map(|&x| (x.clamp(-1.0, 1.0) * 32767.0) as i16)
                                    .collect();
                                if let Err(e) = tx.try_send(pre_roll_i16) {
                                    log::error!("[VAD] Failed to flush PTT pre-roll audio to S2S: {:?}", e);
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
                    let state: tauri::State<'_, std::sync::Arc<crate::core::state::AppState>> = app.state();
                    if state.ptt.speech_detected.load(Ordering::Relaxed) {
                        let i16_samples: Vec<i16> = chunk
                            .iter()
                            .map(|&x| (x.clamp(-1.0, 1.0) * 32767.0) as i16)
                            .collect();
                        if let Err(e) = tx.try_send(i16_samples) {
                            log::error!("[VAD] Failed to send gated PTT audio samples to realtime S2S bridge: {:?}", e);
                        } else {
                            static PTT_ROUTE_COUNT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
                            let count = PTT_ROUTE_COUNT.fetch_add(1, Ordering::Relaxed);
                            if count % 200 == 0 {
                                log::info!("[VAD] Routing realtime PTT audio chunk (count: {})", count + 1);
                            }
                        }
                    }
                }

                // In both Modular and Realtime PTT: Accumulate pre-roll when not in speech
                let state: tauri::State<'_, std::sync::Arc<crate::core::state::AppState>> = app.state();
                if !state.ptt.speech_detected.load(Ordering::Relaxed) {
                    pre_roll_buffer.extend_from_slice(&chunk);
                    if pre_roll_buffer.len() > 8000 {
                        let excess = pre_roll_buffer.len() - 8000;
                        pre_roll_buffer.drain(0..excess);
                    }
                }

                if in_speech {
                    in_speech = false;
                    utterance_buffer.clear();
                    samples_since_partial = 0;
                }
                continue;
            }


            // ── Phase 4: Speaker-mode mic ducking ────────────────────────────
            // Drop mic frames while playback is active in Speaker mode.
            // Prevents TTS audio from looping back through the mic and re-triggering VAD.
            // In Headset mode, mic stays live for barge-in (pipeline cancellation handles it).
            // In Realtime S2S mode, bypass entirely — cloud providers handle their own
            // echo cancellation and barge-in internally.
            {
                let state: tauri::State<'_, std::sync::Arc<crate::core::state::AppState>> =
                    app.state();
                let is_playing = state
                    .pipeline
                    .playback_active
                    .load(std::sync::atomic::Ordering::Relaxed);
                if realtime_tx.is_none() && is_playing && audio_mode == crate::core::settings::AudioOutputMode::Speaker {
                    // Drop this frame — do NOT advance utterance buffer or VAD state
                    continue;
                }
            }

            // Classify chunk as speech or silence
            // We must ALWAYS call vad.predict to keep its internal context windows synchronized.
            let mut detected = vad.predict(&chunk);

            // Override hallucinated speech on sub-threshold noise
            let effective_noise_gate = if is_earshot {
                noise_gate * 1.5
            } else {
                noise_gate
            };
            if raw_energy < effective_noise_gate {
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
                    utterance_buffer.extend_from_slice(&pre_roll_buffer);
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
                pre_roll_buffer.extend_from_slice(&chunk);
                if pre_roll_buffer.len() > 8000 {
                    let excess = pre_roll_buffer.len() - 8000;
                    pre_roll_buffer.drain(0..excess);
                }
            }
        } else {
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
    }
}
