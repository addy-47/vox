use anyhow::Result;
use tauri::Manager;
use tokio::sync::mpsc;
use serde_json::json;
use crate::core::state::{VadCommand, InteractionOwner};
use crate::core::settings::InteractionMode;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use crate::services::traits::VadEngine as _;
use super::VadBackend;

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
    C: ringbuf::traits::Consumer<Item = f32> 
{
    // ── Phase 1: Thread Prioritization (Tier 1 - Realtime) ───────────────
    use thread_priority::*;
    if let Err(e) = set_current_thread_priority(ThreadPriority::Max) {
        log::warn!("[VAD] Failed to set max priority (likely non-root/cap_sys_nice): {:?}", e);
    }

    log::info!("[VAD] Starting synchronous VAD loop on dedicated thread.");
    
    let mut in_speech = false;
    let mut current_turn_id: u32 = 0;
    let mut utterance_buffer: Vec<f32> = Vec::new();
    let mut samples_since_partial = 0;
    let mut pre_roll_buffer: Vec<f32> = Vec::with_capacity(8000); // 500ms pre-roll

    // Local state initialized once, updated via vad_rx to avoid hot-path locks
        let (threshold_init, noise_gate_init, mode_init, owner_init, audio_mode_init) = {
            let state: tauri::State<'_, std::sync::Arc<crate::core::state::AppState>> = app.state();
            let settings = state.settings.read().unwrap();
            let owner: InteractionOwner = state.owner.load(std::sync::atomic::Ordering::Relaxed).into();
            let mode = match owner {
                InteractionOwner::Tray => settings.interaction.tray_mode.clone(),
                InteractionOwner::MainWindow => settings.interaction.main_app_mode.clone(),
                InteractionOwner::Ptt => InteractionMode::PTT,
                InteractionOwner::Wizard => InteractionMode::Passive,
            };
            (settings.vad.threshold, settings.vad.ptt_noise_gate, mode, owner, settings.audio.output_mode.clone())
        };
        
        let mut threshold = threshold_init;
        let mut noise_gate = noise_gate_init;
        let mut mode = mode_init;
        let mut owner = owner_init;
        let mut audio_mode = audio_mode_init;
    
    let (dropped_counter, engine_shutdown) = {
        let state: tauri::State<'_, std::sync::Arc<crate::core::state::AppState>> = app.state();
        (state.dropped_telemetry_events.clone(), state.pipeline.engine_shutdown.clone())
    };

    is_loaded.store(true, Ordering::Relaxed);
    log::info!("[VAD] Entering sync loop: threshold={}, noise_gate={}, mode={:?}", threshold, noise_gate, mode);

    
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
            // Calculate RMS energy for the 16ms chunk
            let raw_energy = (chunk.iter().map(|&x| x * x).sum::<f32>() / chunk.len() as f32).sqrt();
            
            let gated_raw = if raw_energy > noise_gate { raw_energy } else { 0.0 };
            let energy = (gated_raw * 8.0).clamp(0.0, 1.0);
            
            if telemetry_tx.try_send(crate::monitoring::aggregator::TelemetryEvent::AudioEnergy {
                energy,
                vad_prob: 0.0,
            }).is_err() {
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

            if mode == InteractionMode::PTT {
                crate::services::ptt::handle_ptt_audio_sync(&app, &chunk);
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
            {
                let state: tauri::State<'_, std::sync::Arc<crate::core::state::AppState>> = app.state();
                let is_playing = state.pipeline.playback_active.load(std::sync::atomic::Ordering::Relaxed);
                if is_playing && audio_mode == crate::core::settings::AudioOutputMode::Speaker {
                    // Drop this frame — do NOT advance utterance buffer or VAD state
                    continue;
                }
            }

            // Classify chunk as speech or silence
            let detected = vad.predict(&chunk);
            
            if detected {
                if !in_speech {
                    in_speech = true;
                    current_turn_id += 1;
                    log::info!("[VAD] >>> SPEECH START (session: {}, owner: {:?})", current_turn_id, owner);
                    
                    let _ = stt_tx.send(crate::services::stt::SttCommand::ResetStream);

                    if let Some(ref tx) = vox_event_tx {
                        let _ = tx.send(crate::core::events::VoxEvent::SpeechStart { turn_id: current_turn_id, owner });
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

                utterance_buffer.extend_from_slice(&chunk);
                samples_since_partial += chunk.len();

                if samples_since_partial >= 12800 {
                    let start_idx = utterance_buffer.len().saturating_sub(240000);
                    let _ = stt_tx.send(crate::services::stt::SttCommand::Partial(
                        current_turn_id, 
                        owner,
                        utterance_buffer[start_idx..].to_vec()
                    ));
                    samples_since_partial = 0;
                }
            } else {
                if in_speech {
                    in_speech = false;
                    log::info!("[VAD] <<< SPEECH END (session: {}, owner: {:?})", current_turn_id, owner);
                    
                    if let Some(ref tx) = vox_event_tx {
                        let _ = tx.send(crate::core::events::VoxEvent::SpeechEnd { turn_id: current_turn_id, owner });
                    }
                    
                    let _ = event_tx.try_send(json!({ 
                        "type": "speech_end",
                        "session_id": current_turn_id 
                    }));

                    vad.flush();
                    
                    if utterance_buffer.len() >= 3200 { 
                        let _ = stt_tx.send(crate::services::stt::SttCommand::Final(
                            current_turn_id, 
                            owner,
                            utterance_buffer.clone()
                        ));
                    }
                    
                    utterance_buffer.clear();
                    samples_since_partial = 0;
                }
                
                if !in_speech {
                    pre_roll_buffer.extend_from_slice(&chunk);
                    if pre_roll_buffer.len() > 8000 {
                        let excess = pre_roll_buffer.len() - 8000;
                        pre_roll_buffer.drain(0..excess);
                    }
                }
            }
        } else {
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
    }
}
