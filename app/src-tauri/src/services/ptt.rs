use tauri::{AppHandle, Manager, Emitter, State};
use serde_json::json;
use std::sync::atomic::Ordering;
use crate::services::stt::SttCommand;
use crate::core::state::AppState;
use crate::core::events::VoxEvent;

// ─── Commands ────────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn ptt_start(app: AppHandle) -> Result<(), String> {
    let state: State<'_, std::sync::Arc<AppState>> = app.state();
    
    if state.ptt.is_recording.compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst).is_err() {
        log::warn!("[PTT] ptt_start called while already recording. Ignoring.");
        return Ok(());
    }
    
    let turn = {
        let mut buffer = state.ptt.audio_buffer.lock().unwrap();

        // Sync PTT turn with global pipeline turn
        let current_global = state.pipeline.turn_id.load(Ordering::Relaxed);
        state.ptt.turn_id.store(current_global, Ordering::Relaxed);
        let turn = current_global;

        buffer.clear();
        state.ptt.samples_since_partial.store(0, Ordering::Relaxed);
        state.ptt.samples_since_waveform.store(0, Ordering::Relaxed);
        turn
    };

    let owner: crate::core::state::InteractionOwner = state.owner.load(Ordering::Relaxed).into();

    // Phase 5: Notify pipeline to cancel any ongoing playback (barge-in)
    if let Some(engine) = state.engine.lock().await.as_ref() {
        let _ = engine.pipeline_tx.send(VoxEvent::SpeechStart { turn_id: turn, owner });
    }

    // Determine the owning window target
    let target = match owner {
        crate::core::state::InteractionOwner::Tray => "tray",
        crate::core::state::InteractionOwner::MainWindow | crate::core::state::InteractionOwner::Ptt => "main",
        crate::core::state::InteractionOwner::Wizard => "wizard",
    };

    log::info!("[PTT] >>> Recording started (turn: {}, target: {}, owner: {:?})", turn, target, owner);
    let _ = app.emit_to(target, "ptt_status", json!({ "state": "RECORDING", "session_id": turn }));
    
    // Update interaction state via centralized pipeline logic
    state.pipeline.update_interaction_state(crate::core::state::InteractionState::UserSpeaking, owner, &app);
    
    Ok(())
}

#[tauri::command]
pub async fn ptt_stop(app: AppHandle) -> Result<(), String> {
    let state: State<'_, std::sync::Arc<AppState>> = app.state();
    
    let (turn, owner, buffer_clone) = {
        if !state.ptt.is_recording.load(Ordering::SeqCst) {
            return Ok(());
        }

        let buffer = state.ptt.audio_buffer.lock().unwrap();
        let turn = state.ptt.turn_id.load(Ordering::Relaxed);

        state.ptt.is_recording.store(false, Ordering::SeqCst);
        log::info!("[PTT] <<< Recording stopped. Finalizing {} samples...", buffer.len());
        
        let owner: crate::core::state::InteractionOwner = state.owner.load(Ordering::Relaxed).into();
        (turn, owner, buffer.clone())
    }; 

    // Determine the owning window target
    let target = match owner {
        crate::core::state::InteractionOwner::Tray => "tray",
        crate::core::state::InteractionOwner::MainWindow | crate::core::state::InteractionOwner::Ptt => "main",
        crate::core::state::InteractionOwner::Wizard => "wizard",
    };

    let _ = app.emit_to(target, "ptt_status", json!({ "state": "PROCESSING", "session_id": turn }));

    // Update interaction state via centralized pipeline logic
    state.pipeline.update_interaction_state(crate::core::state::InteractionState::Thinking, owner, &app);

    // Send the full buffer to STT for finalization
    let engine_lock = state.engine.lock().await;
    if let Some(engine) = engine_lock.as_ref() {
        let _ = engine.stt_tx.send(SttCommand::Final(turn, owner, buffer_clone));
        log::info!("[PTT] Sent final buffer to STT worker (turn: {}, owner: {:?})", turn, owner);
    } else {
        log::error!("[PTT] Engine not running, cannot finalize transcription.");
    }
    
    Ok(())
}

#[tauri::command]
pub async fn ptt_cancel(app: AppHandle) -> Result<(), String> {
    let state: State<'_, std::sync::Arc<AppState>> = app.state();
    
    state.ptt.is_recording.store(false, Ordering::SeqCst);
    let mut buffer = state.ptt.audio_buffer.lock().unwrap();
    buffer.clear();

    let owner: crate::core::state::InteractionOwner = state.owner.load(Ordering::Relaxed).into();

    // Determine the owning window target
    let target = match owner {
        crate::core::state::InteractionOwner::Tray => "tray",
        crate::core::state::InteractionOwner::MainWindow | crate::core::state::InteractionOwner::Ptt => "main",
        crate::core::state::InteractionOwner::Wizard => "wizard",
    };

    log::info!("[PTT] ❌ Recording cancelled (target: {}, owner: {:?})", target, owner);
    let _ = app.emit_to(target, "ptt_status", json!({ "state": "IDLE" }));

    // Update interaction state via centralized pipeline logic
    state.pipeline.update_interaction_state(crate::core::state::InteractionState::Idle, owner, &app);
    
    Ok(())
}

// ─── Logic ───────────────────────────────────────────────────────────────────

/// Maximum samples allowed in PTT buffer (approx 10 minutes at 16kHz)
const MAX_PTT_SAMPLES: usize = 16000 * 60 * 10;

/// Appends audio samples to the PTT buffer unconditionally.
pub fn handle_ptt_audio_sync(app: &AppHandle, samples: &[f32]) {
    let state: State<'_, std::sync::Arc<AppState>> = app.state();
    
    if !state.ptt.is_recording.load(Ordering::SeqCst) { return; }

    let mut buffer = state.ptt.audio_buffer.lock().unwrap();
    
    // Capture ALL audio — the user's button press is the gate.
    if buffer.len() < MAX_PTT_SAMPLES {
        buffer.extend_from_slice(samples);
    } else {
        // Safety Cap: Stop recording if it exceeds 10 minutes (OOM protection)
        log::warn!("[PTT] Hard limit reached ({} samples). Stopping recording.", MAX_PTT_SAMPLES);
        drop(buffer);
        let app_clone = app.clone();
        tauri::async_runtime::spawn(async move {
            let _ = ptt_stop(app_clone).await;
        });
        return;
    }

    // Advance partial counter only for appended samples (not silence)
    let samples_since = state.ptt.samples_since_partial.fetch_add(samples.len(), Ordering::SeqCst) + samples.len();

    // BACKGROUND STT: Every 800ms, send partial buffer to worker
    if samples_since >= 12800 {
        let turn = state.ptt.turn_id.load(Ordering::Relaxed);
        if let Ok(lock) = state.engine.try_lock() {
            if let Some(engine) = lock.as_ref() {
                // For partial transcripts, only send the last 15 seconds to keep CPU/Memory low
                // 15 seconds * 16,000 samples/sec = 240,000 samples
                let owner: crate::core::state::InteractionOwner = state.owner.load(Ordering::Relaxed).into();
                let start_idx = buffer.len().saturating_sub(240000);
                let _ = engine.stt_tx.send(SttCommand::Partial(turn, owner, buffer[start_idx..].to_vec()));
                log::debug!("[PTT] Sent partial buffer window ({} samples) to STT worker", buffer[start_idx..].len());
            }
        }
        state.ptt.samples_since_partial.store(0, Ordering::SeqCst);
    }

    // WAVEFORM THROTTLING: Only emit amplitude every 60ms (960 samples)
    let samples_waveform = state.ptt.samples_since_waveform.fetch_add(samples.len(), Ordering::SeqCst) + samples.len();

    if samples_waveform >= 960 {
        // Calculate RMS on the actual samples for live waveform feedback
        let sum_sq: f32 = samples.iter().map(|&s| s * s).sum();
        let rms = (sum_sq / samples.len() as f32).sqrt();
        
        let noise_gate = {
            let settings = state.settings.read().unwrap();
            settings.vad.ptt_noise_gate
        };

        // Apply noise gate and consistent 8.0x multiplier
        let gated_energy = if rms > noise_gate { (rms * 8.0).min(1.0) } else { 0.0 };

        if let Ok(lock) = state.engine.try_lock() {
            if let Some(engine) = lock.as_ref() {
                let _ = engine.telemetry_tx.send(crate::monitoring::aggregator::TelemetryEvent::AudioEnergy {
                    energy: gated_energy,
                    vad_prob: 0.0,
                });
            }
        }
        state.ptt.samples_since_waveform.store(0, Ordering::SeqCst);
    }
}

