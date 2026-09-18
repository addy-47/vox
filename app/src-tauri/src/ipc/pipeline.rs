//! Strongly-typed IPC command handlers for assistant pipeline control.

use std::sync::{atomic::Ordering, Arc};

use tauri::{AppHandle, Manager, State};

use crate::core::{
    engine::{start_audio_engine, stop_audio_engine},
    error::VoxIpcError,
    events::VoxEvent,
    state::{AppState, InteractionOwner, InteractionState},
};

/// Launches and initializes the 3-tier audio engine.
#[tauri::command]
pub async fn launch_engine<R: tauri::Runtime>(app: AppHandle<R>) -> Result<(), VoxIpcError> {
    let state: State<'_, Arc<AppState>> = app.state();
    start_audio_engine(&app, &state)
        .await
        .map_err(VoxIpcError::Engine)
}

/// Shuts down the 3-tier audio engine and unloads models.
#[tauri::command]
pub async fn stop_engine<R: tauri::Runtime>(app: AppHandle<R>) -> Result<(), VoxIpcError> {
    let state: State<'_, Arc<AppState>> = app.state();
    stop_audio_engine(&state).await.map_err(VoxIpcError::Engine)
}

/// Starts the voice assistant session by booting audio engine and routing SessionStart.
#[tauri::command]
pub async fn start_session<R: tauri::Runtime>(
    session_id: Option<i64>,
    app: AppHandle<R>,
    state: State<'_, Arc<AppState>>,
) -> Result<(), VoxIpcError> {
    let current_state = state.pipeline.state();
    if current_state != InteractionState::Idle {
        return Err(VoxIpcError::InvalidState(format!(
            "[IPC::Assistant] Cannot start session: pipeline state is {:?}, expected Idle",
            current_state
        )));
    }

    start_audio_engine(&app, &state)
        .await
        .map_err(VoxIpcError::Engine)?;

    let event_tx = state
        .event_tx
        .lock()
        .clone()
        .ok_or_else(|| VoxIpcError::Engine("Event router is not active".into()))?;

    event_tx
        .send(VoxEvent::SessionStart {
            owner: InteractionOwner::Assistant,
            session_id,
        })
        .map_err(|e| VoxIpcError::Engine(format!("Failed to send SessionStart: {}", e)))?;

    Ok(())
}

/// Ends the active voice assistant session by routing EndSession to the pipeline router.
#[tauri::command]
pub async fn end_session<R: tauri::Runtime>(
    _app: AppHandle<R>,
    state: State<'_, Arc<AppState>>,
) -> Result<(), VoxIpcError> {
    let event_tx = state
        .event_tx
        .lock()
        .clone()
        .ok_or_else(|| VoxIpcError::Engine("Event router is not active".into()))?;

    event_tx
        .send(VoxEvent::EndSession)
        .map_err(|e| VoxIpcError::Engine(format!("Failed to send EndSession: {}", e)))?;

    Ok(())
}

/// Pauses the active voice assistant pipeline.
#[tauri::command]
pub async fn pause_session<R: tauri::Runtime>(
    _app: AppHandle<R>,
    state: State<'_, Arc<AppState>>,
) -> Result<(), VoxIpcError> {
    let event_tx = state
        .event_tx
        .lock()
        .clone()
        .ok_or_else(|| VoxIpcError::Engine("Event router is not active".into()))?;

    event_tx
        .send(VoxEvent::PauseSession)
        .map_err(|e| VoxIpcError::Engine(format!("Failed to send PauseSession: {}", e)))?;

    Ok(())
}

/// Resumes a paused voice assistant pipeline.
#[tauri::command]
pub async fn resume_session<R: tauri::Runtime>(
    _app: AppHandle<R>,
    state: State<'_, Arc<AppState>>,
) -> Result<(), VoxIpcError> {
    let event_tx = state
        .event_tx
        .lock()
        .clone()
        .ok_or_else(|| VoxIpcError::Engine("Event router is not active".into()))?;

    event_tx
        .send(VoxEvent::ResumeSession)
        .map_err(|e| VoxIpcError::Engine(format!("Failed to send ResumeSession: {}", e)))?;

    Ok(())
}

/// Initiates Push-To-Talk speech recording.
#[tauri::command]
pub async fn ptt_start<R: tauri::Runtime>(
    _app: AppHandle<R>,
    state: State<'_, Arc<AppState>>,
) -> Result<(), VoxIpcError> {
    let event_tx = state
        .event_tx
        .lock()
        .clone()
        .ok_or_else(|| VoxIpcError::Engine("Event router is not active".into()))?;

    event_tx
        .send(VoxEvent::PttStart)
        .map_err(|e| VoxIpcError::Engine(format!("Failed to send PttStart: {}", e)))?;

    Ok(())
}

/// Finalizes Push-To-Talk speech recording.
#[tauri::command]
pub async fn ptt_stop<R: tauri::Runtime>(
    _app: AppHandle<R>,
    state: State<'_, Arc<AppState>>,
) -> Result<(), VoxIpcError> {
    let event_tx = state
        .event_tx
        .lock()
        .clone()
        .ok_or_else(|| VoxIpcError::Engine("Event router is not active".into()))?;

    event_tx
        .send(VoxEvent::PttStop)
        .map_err(|e| VoxIpcError::Engine(format!("Failed to send PttStop: {}", e)))?;

    Ok(())
}

/// Cancels an in-progress Push-To-Talk recording.
#[tauri::command]
pub async fn ptt_cancel<R: tauri::Runtime>(
    _app: AppHandle<R>,
    state: State<'_, Arc<AppState>>,
) -> Result<(), VoxIpcError> {
    let event_tx = state
        .event_tx
        .lock()
        .clone()
        .ok_or_else(|| VoxIpcError::Engine("Event router is not active".into()))?;

    event_tx
        .send(VoxEvent::PttCancel)
        .map_err(|e| VoxIpcError::Engine(format!("Failed to send PttCancel: {}", e)))?;

    Ok(())
}

/// Submits a typed user query into the assistant conversational pipeline.
#[tauri::command]
pub async fn submit_text_input<R: tauri::Runtime>(
    query: String,
    _app: AppHandle<R>,
    state: State<'_, Arc<AppState>>,
) -> Result<(), VoxIpcError> {
    let current_state = state.pipeline.state();
    if current_state == InteractionState::Idle {
        return Err(VoxIpcError::InvalidState(
            "Cannot submit text input: pipeline is not engaged".into(),
        ));
    }

    let event_tx = state
        .event_tx
        .lock()
        .clone()
        .ok_or_else(|| VoxIpcError::Engine("Event router is not active".into()))?;

    event_tx
        .send(VoxEvent::TextInput { text: query })
        .map_err(|e| VoxIpcError::Engine(format!("Failed to send TextInput event: {}", e)))?;

    Ok(())
}

/// Toggles speaker audio output muting at the CPAL output sink layer.
#[tauri::command]
pub async fn set_playback_muted<R: tauri::Runtime>(
    muted: bool,
    _app: AppHandle<R>,
    state: State<'_, Arc<AppState>>,
) -> Result<(), VoxIpcError> {
    state
        .pipeline
        .is_playback_muted
        .store(muted, Ordering::Relaxed);
    log::info!("[IPC::Pipeline] Playback mute set to: {}", muted);
    Ok(())
}

/// Toggles microphone audio input gating at the CPAL ingestion layer.
#[tauri::command]
pub async fn set_mic_muted<R: tauri::Runtime>(
    muted: bool,
    _app: AppHandle<R>,
    state: State<'_, Arc<AppState>>,
) -> Result<(), VoxIpcError> {
    state.pipeline.is_mic_muted.store(muted, Ordering::Relaxed);
    state.pipeline.update_ingestion_gate();
    log::info!("[IPC::Pipeline] Mic mute set to: {}", muted);
    Ok(())
}

/// Toggles ephemeral private mode for the current session in memory without modifying settings.json.
#[tauri::command]
pub async fn set_session_private_mode<R: tauri::Runtime>(
    enabled: bool,
    _app: AppHandle<R>,
    state: State<'_, Arc<AppState>>,
) -> Result<(), VoxIpcError> {
    state
        .telemetry
        .is_private_mode
        .store(enabled, Ordering::Relaxed);
    log::info!("[IPC::Pipeline] Session private mode set to: {}", enabled);
    Ok(())
}
