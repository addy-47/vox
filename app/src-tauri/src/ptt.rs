use tauri::{AppHandle, Manager, Emitter, State};
use std::sync::Arc;
use tokio::sync::Mutex;
use serde_json::json;
use crate::stt::SttCommand;

// ─── Managed State ───────────────────────────────────────────────────────────

pub struct PttManager {
    pub is_recording: Arc<Mutex<bool>>,
    pub session_id: Arc<Mutex<u32>>,
    pub audio_buffer: Arc<Mutex<Vec<f32>>>,
    pub samples_since_partial: Arc<Mutex<usize>>,
    pub samples_since_waveform: Arc<Mutex<usize>>,
}

impl PttManager {
    pub fn new() -> Self {
        Self {
            is_recording: Arc::new(Mutex::new(false)),
            session_id: Arc::new(Mutex::new(0)),
            audio_buffer: Arc::new(Mutex::new(Vec::new())),
            samples_since_partial: Arc::new(Mutex::new(0)),
            samples_since_waveform: Arc::new(Mutex::new(0)),
        }
    }
}

// ─── Commands ────────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn ptt_start(app: AppHandle) -> Result<(), String> {
    let ptt: State<'_, PttManager> = app.state();
    let interaction: State<'_, crate::InteractionState> = app.state();
    
    let mut recording = ptt.is_recording.lock().await;
    let mut session = ptt.session_id.lock().await;
    let mut buffer = ptt.audio_buffer.lock().await;
    let mut samples_since = ptt.samples_since_partial.lock().await;
    let mut samples_waveform = ptt.samples_since_waveform.lock().await;
    let mut mode = interaction.0.lock().await;

    *recording = true;
    *mode = crate::InteractionMode::Ptt;
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
    let ptt: State<'_, PttManager> = app.state();
    let interaction: State<'_, crate::InteractionState> = app.state();
    
    let mut recording = ptt.is_recording.lock().await;
    let buffer = ptt.audio_buffer.lock().await;
    let session = ptt.session_id.lock().await;
    let mut mode = interaction.0.lock().await;

    if !*recording {
        return Ok(());
    }

    *recording = false;
    *mode = crate::InteractionMode::Passive;
    log::info!("[PTT] <<< Recording stopped. Finalizing {} samples...", buffer.len());
    
    let _ = app.emit("ptt_status", json!({ "state": "PROCESSING", "session_id": *session }));

    // Send the full buffer to STT for finalization
    let engine_state: State<'_, crate::EngineState> = app.state();
    let lock = engine_state.0.lock().await;
    
    if let Some(engine) = lock.as_ref() {
        let _ = engine.stt_tx.send(SttCommand::Final(*session, buffer.clone())).await;
        log::info!("[PTT] Sent final buffer to STT worker (session: {})", *session);
    } else {
        log::error!("[PTT] Engine not running, cannot finalize transcription.");
    }
    
    Ok(())
}

#[tauri::command]
pub async fn ptt_cancel(app: AppHandle) -> Result<(), String> {
    let ptt: State<'_, PttManager> = app.state();
    let interaction: State<'_, crate::InteractionState> = app.state();
    
    let mut recording = ptt.is_recording.lock().await;
    let mut buffer = ptt.audio_buffer.lock().await;
    let mut mode = interaction.0.lock().await;

    *recording = false;
    *mode = crate::InteractionMode::Passive;
    buffer.clear();

    log::info!("[PTT] ❌ Recording cancelled.");
    let _ = app.emit("ptt_status", json!({ "state": "IDLE" }));
    
    Ok(())
}

// ─── Logic ───────────────────────────────────────────────────────────────────

pub fn handle_ptt_audio_sync(app: &AppHandle, samples: &[f32]) {
    let ptt: State<'_, PttManager> = app.state();
    
    let recording = ptt.is_recording.blocking_lock();
    if !*recording { return; }

    let mut buffer = ptt.audio_buffer.blocking_lock();
    buffer.extend_from_slice(samples);

    let mut samples_since = ptt.samples_since_partial.blocking_lock();
    *samples_since += samples.len();

    // BACKGROUND STT: Every 800ms, send partial buffer to worker
    if *samples_since >= 12800 {
        let session = ptt.session_id.blocking_lock();
        let engine_state: State<'_, crate::EngineState> = app.state();
        if let Ok(lock) = engine_state.0.try_lock() {
            if let Some(engine) = lock.as_ref() {
                let _ = engine.stt_tx.try_send(SttCommand::Partial(*session, buffer.clone()));
                log::debug!("[PTT] Sent partial buffer ({} samples) to STT worker", buffer.len());
            }
        }
        *samples_since = 0;
    }

    // WAVEFORM THROTTLING: Only emit amplitude every 60ms (960 samples)
    let mut samples_waveform = ptt.samples_since_waveform.blocking_lock();
    *samples_waveform += samples.len();

    if *samples_waveform >= 960 {
        // Calculate RMS for waveform
        let sum_sq: f32 = samples.iter().map(|&s| s * s).sum();
        let rms = (sum_sq / samples.len() as f32).sqrt();

        // Noise Gate: Ignore background hum
        let amplitude = if rms < 0.005 {
            0.0
        } else {
            // Scale for UI (multiply by 7.5 for reduced sensitivity)
            (rms * 7.5).min(1.0)
        };

        let _ = app.emit("audio_amplitude", json!({ "amplitude": amplitude }));
        *samples_waveform = 0;
    }
}
