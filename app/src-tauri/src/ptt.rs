use tauri::{AppHandle, Manager, Emitter, State};
use serde_json::json;
use crate::stt::SttCommand;
use crate::state::{AppState, InteractionMode};

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
    
    Ok(())
}

#[tauri::command]
pub async fn ptt_stop(app: AppHandle) -> Result<(), String> {
    let state: State<'_, AppState> = app.state();
    
    let mut recording = state.ptt.is_recording.lock().await;
    let buffer = state.ptt.audio_buffer.lock().await;
    let session = state.ptt.session_id.lock().await;
    let mut mode = state.interaction.lock().await;

    if !*recording {
        return Ok(());
    }

    *recording = false;
    *mode = InteractionMode::Passive;
    log::info!("[PTT] <<< Recording stopped. Finalizing {} samples...", buffer.len());
    
    let _ = app.emit("ptt_status", json!({ "state": "PROCESSING", "session_id": *session }));

    // Send the full buffer to STT for finalization
    let engine_lock = state.engine.lock().await;
    
    if let Some(engine) = engine_lock.as_ref() {
        let _ = engine.stt_tx.send(SttCommand::Final(*session, buffer.clone())).await;
        log::info!("[PTT] Sent final buffer to STT worker (session: {})", *session);
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
    
    Ok(())
}

// ─── Logic ───────────────────────────────────────────────────────────────────

/// Maximum samples allowed in PTT buffer (approx 10 minutes at 16kHz)
const MAX_PTT_SAMPLES: usize = 16000 * 60 * 10;

pub fn handle_ptt_audio_sync(app: &AppHandle, samples: &[f32], is_speech: bool) {
    let state: State<'_, AppState> = app.state();
    
    let recording = state.ptt.is_recording.blocking_lock();
    if !*recording { return; }

    let mut buffer = state.ptt.audio_buffer.blocking_lock();
    
    // Memory Safety: Discard silent chunks to prevent massive buffers 
    // and OOM if user forgets to release PTT.
    if is_speech {
        if buffer.len() < MAX_PTT_SAMPLES {
            buffer.extend_from_slice(samples);
        } else {
            // Cap reached: stop recording to prevent OOM
            drop(buffer);
            drop(recording);
            let app_clone = app.clone();
            tauri::async_runtime::spawn(async move {
                let _ = ptt_stop(app_clone).await;
            });
            return;
        }
    }

    let mut samples_since = state.ptt.samples_since_partial.blocking_lock();
    *samples_since += samples.len();

    // BACKGROUND STT: Every 800ms, send partial buffer to worker
    if *samples_since >= 12800 {
        let session = state.ptt.session_id.blocking_lock();
        if let Ok(lock) = state.engine.try_lock() {
            if let Some(engine) = lock.as_ref() {
                // For partial transcripts, only send the last 15 seconds to keep CPU/Memory low
                // 15 seconds * 16,000 samples/sec = 240,000 samples
                let start_idx = buffer.len().saturating_sub(240000);
                let _ = engine.stt_tx.try_send(SttCommand::Partial(*session, buffer[start_idx..].to_vec()));
                log::debug!("[PTT] Sent partial buffer window ({} samples) to STT worker", buffer[start_idx..].len());
            }
        }
        *samples_since = 0;
    }

    // WAVEFORM THROTTLING: Only emit amplitude every 60ms (960 samples)
    let mut samples_waveform = state.ptt.samples_since_waveform.blocking_lock();
    *samples_waveform += samples.len();

    if *samples_waveform >= 960 {
        // Calculate RMS for waveform
        let sum_sq: f32 = samples.iter().map(|&s| s * s).sum();
        let rms = (sum_sq / samples.len() as f32).sqrt();

        // Noise Gate: Use setting or default
        let gate = 0.005; // TODO: pull from settings if needed
        let amplitude = if rms < gate {
            0.0
        } else {
            (rms * 7.5).min(1.0)
        };

        let _ = app.emit("audio_amplitude", json!({ "amplitude": amplitude }));
        *samples_waveform = 0;
    }
}

