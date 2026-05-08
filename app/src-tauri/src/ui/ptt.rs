use tauri::{AppHandle, Manager, Emitter, State};
use serde_json::json;
use crate::services::stt::SttCommand;
use crate::core::state::{AppState, InteractionMode};

// ─── Commands ────────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn ptt_start(app: AppHandle) -> Result<(), String> {
    let state: State<'_, AppState> = app.state();
    
    let mut recording = state.ptt.is_recording.lock().await;
    let mut session = state.ptt.session_id.lock().await;
    let mut buffer = state.ptt.audio_buffer.lock().await;
    let mut samples_since = state.ptt.samples_since_partial.lock().await;
    let mut samples_waveform = state.ptt.samples_since_waveform.lock().await;
    let mut mode = state.interaction.lock().await;

    *recording = true;
    *mode = InteractionMode::Ptt;
    *session += 1;
    buffer.clear();
    *samples_since = 0;
    *samples_waveform = 0;

    log::info!("[PTT] >>> Recording started (session: {})", *session);
    let _ = app.emit("ptt_status", json!({ "state": "RECORDING", "session_id": *session }));
    
    // Phase 5: Update interaction state for unified UI
    {
        let state_atomic = &state.pipeline.state;
        let mut state_lock = state_atomic.lock().unwrap();
        *state_lock = crate::core::state::InteractionState::UserSpeaking;
    }
    let _ = app.emit("state_changed", crate::core::state::InteractionState::UserSpeaking);
    
    Ok(())
}

#[tauri::command]
pub async fn ptt_stop(app: AppHandle) -> Result<(), String> {
    let state: State<'_, AppState> = app.state();
    
    // Extract everything we need and drop the locks immediately to prevent 
    // pipeline freezes while waiting for the STT channel.
    let (session, owner, buffer_clone) = {
        let mut recording = state.ptt.is_recording.lock().await;
        if !*recording {
            return Ok(());
        }

        let buffer = state.ptt.audio_buffer.lock().await;
        let session = state.ptt.session_id.lock().await;
        let mut mode = state.interaction.lock().await;

        *recording = false;
        *mode = InteractionMode::Passive;
        log::info!("[PTT] <<< Recording stopped. Finalizing {} samples...", buffer.len());
        
        let owner = *state.owner.lock().await;
        (*session, owner, buffer.clone())
    }; 

    let _ = app.emit("ptt_status", json!({ "state": "PROCESSING", "session_id": session }));

    // Phase 5: Update interaction state for unified UI
    {
        let state_atomic = &state.pipeline.state;
        let mut state_lock = state_atomic.lock().unwrap();
        *state_lock = crate::core::state::InteractionState::Thinking;
    }
    let _ = app.emit("state_changed", crate::core::state::InteractionState::Thinking);

    // Send the full buffer to STT for finalization
    let engine_lock = state.engine.lock().await;
    if let Some(engine) = engine_lock.as_ref() {
        // We use .send().await here because it's the final buffer; we MUST ensure it's delivered.
        // Since we dropped the ptt locks above, the VAD thread can continue processing 
        // other tasks even if this send blocks temporarily.
        let _ = engine.stt_tx.send(SttCommand::Final(session, owner, buffer_clone));
        log::info!("[PTT] Sent final buffer to STT worker (session: {}, owner: {:?})", session, owner);
    } else {
        log::error!("[PTT] Engine not running, cannot finalize transcription.");
    }
    
    Ok(())
}

#[tauri::command]
pub async fn ptt_cancel(app: AppHandle) -> Result<(), String> {
    let state: State<'_, AppState> = app.state();
    
    let mut recording = state.ptt.is_recording.lock().await;
    let mut buffer = state.ptt.audio_buffer.lock().await;
    let mut mode = state.interaction.lock().await;

    *recording = false;
    *mode = InteractionMode::Passive;
    buffer.clear();

    log::info!("[PTT] ❌ Recording cancelled.");
    let _ = app.emit("ptt_status", json!({ "state": "IDLE" }));

    // Phase 5: Update interaction state for unified UI
    {
        let state_atomic = &state.pipeline.state;
        let mut state_lock = state_atomic.lock().unwrap();
        *state_lock = crate::core::state::InteractionState::Idle;
    }
    let _ = app.emit("state_changed", crate::core::state::InteractionState::Idle);
    
    Ok(())
}

// ─── Logic ───────────────────────────────────────────────────────────────────

/// Maximum samples allowed in PTT buffer (approx 10 minutes at 16kHz)
const MAX_PTT_SAMPLES: usize = 16000 * 60 * 10;

/// Appends audio samples to the PTT buffer unconditionally.
/// 
/// In PTT mode the user explicitly controls when recording starts/stops with 
/// a button. Unlike passive VAD mode, silence must NOT be discarded — doing so
/// causes onset frames (first ~300ms of speech) to be lost during VAD warm-up,
/// producing empty or truncated transcripts.
pub fn handle_ptt_audio_sync(app: &AppHandle, samples: &[f32]) {
    let state: State<'_, AppState> = app.state();
    
    let recording = state.ptt.is_recording.blocking_lock();
    if !*recording { return; }

    let mut buffer = state.ptt.audio_buffer.blocking_lock();
    
    // Capture ALL audio — the user's button press is the gate.
    if buffer.len() < MAX_PTT_SAMPLES {
        buffer.extend_from_slice(samples);
    } else {
        // Safety Cap: Stop recording if it exceeds 10 minutes (OOM protection)
        log::warn!("[PTT] Hard limit reached ({} samples). Stopping recording.", MAX_PTT_SAMPLES);
        drop(buffer);
        drop(recording);
        let app_clone = app.clone();
        tauri::async_runtime::spawn(async move {
            let _ = ptt_stop(app_clone).await;
        });
        return;
    }

    // Advance partial counter only for appended samples (not silence)
    let mut samples_since = state.ptt.samples_since_partial.blocking_lock();
    *samples_since += samples.len();

    // BACKGROUND STT: Every 800ms, send partial buffer to worker
    if *samples_since >= 12800 {
        let session = state.ptt.session_id.blocking_lock();
        if let Ok(lock) = state.engine.try_lock() {
            if let Some(engine) = lock.as_ref() {
                // For partial transcripts, only send the last 15 seconds to keep CPU/Memory low
                // 15 seconds * 16,000 samples/sec = 240,000 samples
                let owner = *state.owner.blocking_lock();
                let start_idx = buffer.len().saturating_sub(240000);
                let _ = engine.stt_tx.send(SttCommand::Partial(*session, owner, buffer[start_idx..].to_vec()));
                log::debug!("[PTT] Sent partial buffer window ({} samples) to STT worker", buffer[start_idx..].len());
            }
        }
        *samples_since = 0;
    }

    // WAVEFORM THROTTLING: Only emit amplitude every 60ms (960 samples)
    let mut samples_waveform = state.ptt.samples_since_waveform.blocking_lock();
    *samples_waveform += samples.len();

    if *samples_waveform >= 960 {
        // Calculate RMS on the actual samples for live waveform feedback
        let sum_sq: f32 = samples.iter().map(|&s| s * s).sum();
        let rms = (sum_sq / samples.len() as f32).sqrt();

        if let Ok(lock) = state.engine.try_lock() {
            if let Some(engine) = lock.as_ref() {
                let _ = engine.telemetry_tx.send(crate::core::state::TelemetryData {
                    energy: rms,
                    vad_prob: 0.0,
                });
            }
        }
        *samples_waveform = 0;
    }
}

