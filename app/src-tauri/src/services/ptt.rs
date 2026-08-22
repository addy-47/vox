use crate::core::events::VoxEvent;
use crate::core::state::AppState;
use crate::services::stt::SttCommand;
use serde_json::json;
use std::sync::atomic::Ordering;
use tauri::{AppHandle, Emitter, Manager, State};

// ─── Commands ────────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn ptt_start(
    app: AppHandle,
    _owner: Option<crate::core::state::InteractionOwner>,
) -> Result<(), String> {
    let state: State<'_, std::sync::Arc<AppState>> = app.state();

    // 1. Force the active owner to Ptt during recording
    let actual_owner = crate::core::state::InteractionOwner::Ptt;
    state.owner.store(actual_owner as u32, Ordering::Relaxed);
    if let Some(engine) = state.engine.lock().await.as_ref() {
        let _ = engine
            .vad_tx
            .send(crate::core::state::VadCommand::UpdateOwner(actual_owner));
    }

    // 2. Enforce mode guard: reject PTT if the target is in Passive mode
    let interaction_mode = {
        let settings = state.settings.read().unwrap();
        match actual_owner {
            crate::core::state::InteractionOwner::Dictation => {
                match settings.dictation.interaction_mode {
                    crate::core::settings::DictationInteractionMode::Passive => {
                        crate::core::settings::InteractionMode::Passive
                    }
                    crate::core::settings::DictationInteractionMode::Ptt => {
                        crate::core::settings::InteractionMode::PTT
                    }
                }
            }
            crate::core::state::InteractionOwner::MainWindow
            | crate::core::state::InteractionOwner::Ptt => {
                settings.interaction.mode.clone()
            }
            crate::core::state::InteractionOwner::Wizard => {
                crate::core::settings::InteractionMode::Passive
            }
        }
    };
    if interaction_mode != crate::core::settings::InteractionMode::PTT {
        log::warn!(
            "[PTT] Cannot start PTT recording in {:?} mode",
            interaction_mode
        );
        return Err("Cannot start PTT in Passive mode".to_string());
    }

    if state
        .ptt
        .is_recording
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        log::warn!("[PTT] ptt_start called while already recording. Ignoring.");
        return Ok(());
    }

    let is_realtime = {
        let settings = state.settings.read().unwrap();
        settings.interaction.pipeline_mode == crate::core::settings::PipelineMode::Realtime
    };

    let turn = {
        // Sync PTT turn with global pipeline turn by pre-incrementing it.
        // This ensures the PTT turn uses a unique, active turn ID that passes
        // the pipeline's double-final guard.
        let new_turn = state.pipeline.turn_id.fetch_add(1, Ordering::Relaxed) + 1;
        state.ptt.turn_id.store(new_turn, Ordering::Relaxed);

        reset_ptt_state_inner(&state.ptt);
        new_turn
    };

    let owner = actual_owner;

    if is_realtime {
        let rt_guard = state.realtime_engine.lock().await;
        if let Some(ref rt_engine) = *rt_guard {
            rt_engine
                .activity_start()
                .map_err(|e| format!("Failed to signal activity_start: {}", e))?;
        }
    } else {
        // Phase 5: Notify pipeline to cancel any ongoing playback (barge-in)
        if let Some(engine) = state.engine.lock().await.as_ref() {
            let _ = engine.pipeline_tx.send(VoxEvent::SpeechStart {
                turn_id: turn,
                owner,
            });
        }
    }

    // Determine the owning window target
    let target = match owner {
        crate::core::state::InteractionOwner::Dictation => "tray",
        crate::core::state::InteractionOwner::MainWindow
        | crate::core::state::InteractionOwner::Ptt => "main",
        crate::core::state::InteractionOwner::Wizard => "wizard",
    };

    log::info!(
        "[PTT] >>> Recording started (turn: {}, target: {}, owner: {:?}, realtime: {})",
        turn,
        target,
        owner,
        is_realtime
    );
    let _ = app.emit_to(
        target,
        "ptt_status",
        json!({ "state": "RECORDING", "session_id": turn }),
    );

    // Update interaction state via centralized pipeline logic
    state.pipeline.update_interaction_state(
        crate::core::state::InteractionState::UserSpeaking,
        owner,
        &app,
    );

    Ok(())
}

#[tauri::command]
pub async fn ptt_stop(
    app: AppHandle,
    owner: Option<crate::core::state::InteractionOwner>,
) -> Result<(), String> {
    let state: State<'_, std::sync::Arc<AppState>> = app.state();

    if !state.ptt.is_recording.load(Ordering::SeqCst) {
        return Ok(());
    }

    let actual_owner = if let Some(o) = owner {
        state.owner.store(o as u32, Ordering::Relaxed);
        if let Some(engine) = state.engine.lock().await.as_ref() {
            let _ = engine
                .vad_tx
                .send(crate::core::state::VadCommand::UpdateOwner(o));
        }
        o
    } else {
        state.owner.load(Ordering::Relaxed).into()
    };

    let speech_detected = state.ptt.speech_detected.load(Ordering::Relaxed);
    let owner = actual_owner;
    let target = match owner {
        crate::core::state::InteractionOwner::Dictation => "tray",
        crate::core::state::InteractionOwner::MainWindow
        | crate::core::state::InteractionOwner::Ptt => "main",
        crate::core::state::InteractionOwner::Wizard => "wizard",
    };

    if !speech_detected {
        log::info!("[PTT] Silence only detected. Discarding PTT hold.");
        discard_ptt_hold_inner(&state.ptt);

        let _ = app.emit_to(target, "ptt_status", json!({ "state": "IDLE" }));
        state.pipeline.update_interaction_state(
            crate::core::state::InteractionState::Idle,
            owner,
            &app,
        );
        return Ok(());
    }

    let (turn, buffer_clone, is_realtime) = {
        let buffer = state.ptt.audio_buffer.lock();
        let turn = state.ptt.turn_id.load(Ordering::Relaxed);

        state.ptt.is_recording.store(false, Ordering::SeqCst);

        let settings = state.settings.read().unwrap();
        let is_realtime =
            settings.interaction.pipeline_mode == crate::core::settings::PipelineMode::Realtime;

        log::info!(
            "[PTT] <<< Recording stopped. Finalizing {} samples...",
            buffer.len()
        );
        (turn, buffer.clone(), is_realtime)
    };

    // Determine the owning window target
    let target = match owner {
        crate::core::state::InteractionOwner::Dictation => "tray",
        crate::core::state::InteractionOwner::MainWindow
        | crate::core::state::InteractionOwner::Ptt => "main",
        crate::core::state::InteractionOwner::Wizard => "wizard",
    };

    let _ = app.emit_to(
        target,
        "ptt_status",
        json!({ "state": "PROCESSING", "session_id": turn }),
    );

    // Update interaction state via centralized pipeline logic
    state.pipeline.update_interaction_state(
        crate::core::state::InteractionState::Thinking,
        owner,
        &app,
    );

    if is_realtime {
        let rt_guard = state.realtime_engine.lock().await;
        if let Some(ref rt_engine) = *rt_guard {
            rt_engine
                .activity_end()
                .map_err(|e| format!("Failed to signal activity_end: {}", e))?;
        }
    } else {
        // Send the full buffer to STT for finalization
        let engine_lock = state.engine.lock().await;
        if let Some(engine) = engine_lock.as_ref() {
            let _ = engine
                .stt_tx
                .send(SttCommand::Final(turn, owner, buffer_clone));
            log::info!(
                "[PTT] Sent final buffer to STT worker (turn: {}, owner: {:?})",
                turn,
                owner
            );
        } else {
            log::error!("[PTT] Engine not running, cannot finalize transcription.");
        }
    }

    Ok(())
}

#[tauri::command]
pub async fn ptt_cancel(
    app: AppHandle,
    owner: Option<crate::core::state::InteractionOwner>,
) -> Result<(), String> {
    let state: State<'_, std::sync::Arc<AppState>> = app.state();

    let actual_owner = if let Some(o) = owner {
        state.owner.store(o as u32, Ordering::Relaxed);
        if let Some(engine) = state.engine.lock().await.as_ref() {
            let _ = engine
                .vad_tx
                .send(crate::core::state::VadCommand::UpdateOwner(o));
        }
        o
    } else {
        state.owner.load(Ordering::Relaxed).into()
    };

    state.ptt.is_recording.store(false, Ordering::SeqCst);
    {
        let mut buffer = state.ptt.audio_buffer.lock();
        buffer.clear();
    }

    let owner = actual_owner;

    let is_realtime = {
        let settings = state.settings.read().unwrap();
        settings.interaction.pipeline_mode == crate::core::settings::PipelineMode::Realtime
    };

    if is_realtime {
        let rt_guard = state.realtime_engine.lock().await;
        if let Some(ref rt_engine) = *rt_guard {
            let engine_lock = state.engine.lock().await;
            if let Some(ref engine) = *engine_lock {
                rt_engine.barge_in(&engine.playback_engine);
            }
        }
    }

    // Determine the owning window target
    let target = match owner {
        crate::core::state::InteractionOwner::Dictation => "tray",
        crate::core::state::InteractionOwner::MainWindow
        | crate::core::state::InteractionOwner::Ptt => "main",
        crate::core::state::InteractionOwner::Wizard => "wizard",
    };

    log::info!(
        "[PTT] ❌ Recording cancelled (target: {}, owner: {:?})",
        target,
        owner
    );
    let _ = app.emit_to(target, "ptt_status", json!({ "state": "IDLE" }));

    // Update interaction state via centralized pipeline logic
    state.pipeline.update_interaction_state(
        crate::core::state::InteractionState::Idle,
        owner,
        &app,
    );

    Ok(())
}

// ─── Logic ───────────────────────────────────────────────────────────────────

/// Maximum samples allowed in PTT buffer (approx 10 minutes at 16kHz)
const MAX_PTT_SAMPLES: usize = 16000 * 60 * 10;

pub fn handle_ptt_audio_sync(app: &AppHandle, samples: &[f32]) {
    let state: State<'_, std::sync::Arc<AppState>> = app.state();

    if !state.ptt.is_recording.load(Ordering::SeqCst) {
        return;
    }

    let is_realtime = {
        let settings = state.settings.read().unwrap();
        settings.interaction.pipeline_mode == crate::core::settings::PipelineMode::Realtime
    };

    if is_realtime {
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        let start_ms = state.ptt.ptt_start_ms.load(Ordering::SeqCst);
        if start_ms > 0 && now_ms.saturating_sub(start_ms) > 30_000 {
            log::warn!("[PTT] Realtime PTT hold exceeded 30s. Auto-stopping.");
            let app_clone = app.clone();
            tauri::async_runtime::spawn(async move {
                let _ = ptt_stop(app_clone, None).await;
            });
            return;
        }

        // Waveform telemetry calculated on user audio
        let samples_waveform = state
            .ptt
            .samples_since_waveform
            .fetch_add(samples.len(), Ordering::SeqCst)
            + samples.len();
        if samples_waveform >= 960 {
            let sum_sq: f32 = samples.iter().map(|&s| s * s).sum();
            let rms = (sum_sq / samples.len() as f32).sqrt();
            let noise_gate = {
                let settings = state.settings.read().unwrap();
                settings.vad.ptt_noise_gate
            };
            let gated_energy = if rms > noise_gate {
                (rms * 8.0).min(1.0)
            } else {
                0.0
            };

            if let Ok(lock) = state.engine.try_lock() {
                if let Some(engine) = lock.as_ref() {
                    let _ = engine.telemetry_tx.send(
                        crate::monitoring::aggregator::TelemetryEvent::AudioEnergy {
                            energy: gated_energy,
                            vad_prob: 0.0,
                            low: gated_energy,
                            mid: gated_energy,
                            high: gated_energy,
                        },
                    );
                }
            }
            state.ptt.samples_since_waveform.store(0, Ordering::SeqCst);
        }
        return;
    }

    let mut buffer = state.ptt.audio_buffer.lock();

    // Capture ALL audio — the user's button press is the gate.
    if buffer.len() < MAX_PTT_SAMPLES {
        buffer.extend_from_slice(samples);
    } else {
        // Safety Cap: Stop recording if it exceeds 10 minutes (OOM protection)
        log::warn!(
            "[PTT] Hard limit reached ({} samples). Stopping recording.",
            MAX_PTT_SAMPLES
        );
        drop(buffer);
        let app_clone = app.clone();
        tauri::async_runtime::spawn(async move {
            let _ = ptt_stop(app_clone, None).await;
        });
        return;
    }

    // Advance partial counter only for appended samples (not silence)
    let samples_since = state
        .ptt
        .samples_since_partial
        .fetch_add(samples.len(), Ordering::SeqCst)
        + samples.len();

    // BACKGROUND STT: Every 800ms, send partial buffer to worker
    if samples_since >= 12800 {
        let turn = state.ptt.turn_id.load(Ordering::Relaxed);
        if let Ok(lock) = state.engine.try_lock() {
            if let Some(engine) = lock.as_ref() {
                // For partial transcripts, only send the last 15 seconds to keep CPU/Memory low
                // 15 seconds * 16,000 samples/sec = 240,000 samples
                let owner: crate::core::state::InteractionOwner =
                    state.owner.load(Ordering::Relaxed).into();
                let start_idx = buffer.len().saturating_sub(240000);
                let _ = engine.stt_tx.send(SttCommand::Partial(
                    turn,
                    owner,
                    buffer[start_idx..].to_vec(),
                ));
                log::debug!(
                    "[PTT] Sent partial buffer window ({} samples) to STT worker",
                    buffer[start_idx..].len()
                );
            }
        }
        state.ptt.samples_since_partial.store(0, Ordering::SeqCst);
    }

    // WAVEFORM THROTTLING: Only emit amplitude every 60ms (960 samples)
    let samples_waveform = state
        .ptt
        .samples_since_waveform
        .fetch_add(samples.len(), Ordering::SeqCst)
        + samples.len();

    if samples_waveform >= 960 {
        // Calculate RMS on the actual samples for live waveform feedback
        let sum_sq: f32 = samples.iter().map(|&s| s * s).sum();
        let rms = (sum_sq / samples.len() as f32).sqrt();

        let noise_gate = {
            let settings = state.settings.read().unwrap();
            settings.vad.ptt_noise_gate
        };

        // Apply noise gate and consistent 8.0x multiplier
        let gated_energy = if rms > noise_gate {
            (rms * 8.0).min(1.0)
        } else {
            0.0
        };

        if let Ok(lock) = state.engine.try_lock() {
            if let Some(engine) = lock.as_ref() {
                let _ = engine.telemetry_tx.send(
                    crate::monitoring::aggregator::TelemetryEvent::AudioEnergy {
                        energy: gated_energy,
                        vad_prob: 0.0,
                        low: gated_energy,
                        mid: gated_energy,
                        high: gated_energy,
                    },
                );
            }
        }
        state.ptt.samples_since_waveform.store(0, Ordering::SeqCst);
    }
}

pub fn reset_ptt_state_inner(ptt: &crate::core::state::PttState) {
    ptt.audio_buffer.lock().clear();
    ptt.samples_since_partial.store(0, Ordering::Relaxed);
    ptt.samples_since_waveform.store(0, Ordering::Relaxed);
    ptt.speech_detected.store(false, Ordering::SeqCst);
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;
    ptt.ptt_start_ms.store(now_ms, Ordering::SeqCst);
}

pub fn discard_ptt_hold_inner(ptt: &crate::core::state::PttState) {
    ptt.is_recording.store(false, Ordering::SeqCst);
    ptt.audio_buffer.lock().clear();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::state::PttState;
    use std::sync::atomic::AtomicU32;
    use std::sync::Arc;

    #[test]
    fn test_ptt_state_reset_and_discard() {
        let ptt = PttState {
            is_recording: std::sync::atomic::AtomicBool::new(true),
            turn_id: Arc::new(AtomicU32::new(5)),
            audio_buffer: parking_lot::Mutex::new(vec![1.0, 2.0, 3.0]),
            samples_since_partial: std::sync::atomic::AtomicUsize::new(100),
            samples_since_waveform: std::sync::atomic::AtomicUsize::new(200),
            speech_detected: std::sync::atomic::AtomicBool::new(true),
            ptt_start_ms: std::sync::atomic::AtomicU64::new(12345),
        };

        // Test reset_ptt_state_inner
        reset_ptt_state_inner(&ptt);
        assert_eq!(ptt.audio_buffer.lock().len(), 0);
        assert_eq!(ptt.samples_since_partial.load(Ordering::Relaxed), 0);
        assert_eq!(ptt.samples_since_waveform.load(Ordering::Relaxed), 0);
        assert!(!ptt.speech_detected.load(Ordering::SeqCst));
        assert!(ptt.ptt_start_ms.load(Ordering::SeqCst) > 12345);

        // Test discard_ptt_hold_inner
        ptt.is_recording.store(true, Ordering::SeqCst);
        ptt.audio_buffer.lock().extend_from_slice(&[1.0, 2.0]);
        discard_ptt_hold_inner(&ptt);
        assert!(!ptt.is_recording.load(Ordering::SeqCst));
        assert_eq!(ptt.audio_buffer.lock().len(), 0);
    }
}
