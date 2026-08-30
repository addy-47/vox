use crate::core::settings::{InteractionMode, PipelineMode};
use crate::core::state::{AppState, InteractionOwner, InteractionState, VadCommand};
use crate::pipeline::RoutingContext;
use crate::services::vad::VadOperationalMode;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use tauri::{AppHandle, Manager, State};

/// Checks whether the background audio engine is currently active.
#[tauri::command]
pub async fn check_engine_status(state: State<'_, Arc<AppState>>) -> Result<bool, String> {
    let lock = state.engine.lock().await;
    Ok(lock.is_some())
}

/// Launches and initializes the 3-tier audio engine.
#[tauri::command]
pub async fn launch_engine(app: AppHandle) -> Result<(), String> {
    let state: State<'_, Arc<AppState>> = app.state();
    crate::core::start_audio_engine(&app, &state).await
}

/// Shuts down the 3-tier audio engine and unloads models.
#[tauri::command]
pub async fn stop_engine(app: AppHandle) -> Result<(), String> {
    let state: State<'_, Arc<AppState>> = app.state();
    crate::core::stop_audio_engine(&state).await
}

/// Starts the voice assistant session with entry state validation and audio ownership initialization.
#[tauri::command]
pub async fn start_session(app: AppHandle, state: State<'_, Arc<AppState>>) -> Result<(), String> {
    let current_state = state.pipeline.state();
    if current_state != InteractionState::Idle {
        return Err(format!(
            "[IPC::Assistant] Cannot start session: pipeline state is {:?}, expected Idle",
            current_state
        ));
    }

    crate::core::start_audio_engine(&app, &state).await?;
    state
        .owner
        .store(InteractionOwner::Assistant as u32, Ordering::Relaxed);

    let ctx = RoutingContext::from_app_state(&state);
    let vad_mode = match ctx.interaction_mode {
        InteractionMode::Passive => VadOperationalMode::ContinuousSegmentation,
        InteractionMode::PTT => VadOperationalMode::WindowedValidation,
    };

    if let Ok(guard) = state.engine.try_lock() {
        if let Some(ref engine) = *guard {
            if let Err(e) = engine.vad_tx.send(VadCommand::SetOperationalMode(vad_mode)) {
                log::warn!("[IPC::Assistant] Failed to set initial VAD operational mode: {}", e);
            }
        }
    }

    match (ctx.pipeline_mode, ctx.interaction_mode) {
        (PipelineMode::Modular, InteractionMode::Passive) => {
            crate::pipeline::modular::passive::start_session(&app, &state).await
        }
        (PipelineMode::Modular, InteractionMode::PTT) => {
            crate::pipeline::modular::ptt::start_session(&app, &state).await
        }
        (PipelineMode::Realtime, InteractionMode::Passive) => {
            crate::pipeline::realtime::passive::start_session(&app, &state).await
        }
        (PipelineMode::Realtime, InteractionMode::PTT) => {
            crate::pipeline::realtime::ptt::start_session(&app, &state).await
        }
    }
}

/// Ends the active voice assistant session, resets state to Idle, and manages audio engine ownership.
#[tauri::command]
pub async fn end_session(app: AppHandle, state: State<'_, Arc<AppState>>) -> Result<(), String> {
    let current_state = state.pipeline.state();
    if current_state == InteractionState::Idle {
        log::info!("[IPC::Assistant] end_session called while already Idle; no-op");
        return Ok(());
    }

    let ctx = RoutingContext::from_app_state(&state);
    match (ctx.pipeline_mode, ctx.interaction_mode) {
        (PipelineMode::Modular, InteractionMode::Passive) => {
            crate::pipeline::modular::passive::end_session(&app, &state).await?;
        }
        (PipelineMode::Modular, InteractionMode::PTT) => {
            crate::pipeline::modular::ptt::end_session(&app, &state).await?;
        }
        (PipelineMode::Realtime, InteractionMode::Passive) => {
            crate::pipeline::realtime::passive::end_session(&app, &state).await?;
        }
        (PipelineMode::Realtime, InteractionMode::PTT) => {
            crate::pipeline::realtime::ptt::end_session(&app, &state).await?;
        }
    }

    let dictation_enabled = state.settings.read().map(|s| s.dictation.enabled).unwrap_or(false);
    if dictation_enabled {
        state
            .owner
            .store(InteractionOwner::Dictation as u32, Ordering::Relaxed);
        let dictation_mode = state
            .settings
            .read()
            .map(|s| s.dictation.interaction_mode.clone())
            .unwrap_or(crate::core::settings::DictationInteractionMode::Ptt);
        let vad_mode = match dictation_mode {
            crate::core::settings::DictationInteractionMode::Passive => VadOperationalMode::ContinuousSegmentation,
            crate::core::settings::DictationInteractionMode::Ptt => VadOperationalMode::WindowedValidation,
        };
        if let Ok(guard) = state.engine.try_lock() {
            if let Some(ref engine) = *guard {
                if let Err(e) = engine.vad_tx.send(VadCommand::SetOperationalMode(vad_mode)) {
                    log::warn!("[IPC::Assistant] Failed to set VAD mode for dictation: {}", e);
                }
            }
        }
    } else {
        crate::core::stop_audio_engine(&state).await?;
    }

    Ok(())
}

/// Pauses the active voice assistant pipeline if active.
#[tauri::command]
pub async fn pause_session(app: AppHandle, state: State<'_, Arc<AppState>>) -> Result<(), String> {
    let current_state = state.pipeline.state();
    if current_state == InteractionState::Idle {
        return Err("[IPC::Assistant] Cannot pause session: pipeline is Idle".to_string());
    }
    if current_state == InteractionState::Paused {
        return Ok(());
    }

    let ctx = RoutingContext::from_app_state(&state);
    match ctx.pipeline_mode {
        PipelineMode::Modular => {
            crate::pipeline::modular::passive::pause_session(&app, &state).await
        }
        PipelineMode::Realtime => {
            crate::pipeline::realtime::passive::pause_session(&app, &state).await
        }
    }
}

/// Resumes a paused voice assistant pipeline if in Paused state.
#[tauri::command]
pub async fn resume_session(app: AppHandle, state: State<'_, Arc<AppState>>) -> Result<(), String> {
    let current_state = state.pipeline.state();
    if current_state != InteractionState::Paused {
        return Err(format!(
            "[IPC::Assistant] Cannot resume session: current state is {:?}, expected Paused",
            current_state
        ));
    }

    let ctx = RoutingContext::from_app_state(&state);
    match ctx.pipeline_mode {
        PipelineMode::Modular => {
            crate::pipeline::modular::passive::resume_session(&app, &state).await
        }
        PipelineMode::Realtime => {
            crate::pipeline::realtime::passive::resume_session(&app, &state).await
        }
    }
}

/// Initiates Push-To-Talk speech recording after validating Ready state.
#[tauri::command]
pub async fn ptt_start(app: AppHandle, state: State<'_, Arc<AppState>>) -> Result<(), String> {
    let current_state = state.pipeline.state();
    if current_state == InteractionState::Idle || current_state == InteractionState::Paused {
        return Err("[IPC::Assistant] Cannot start PTT: assistant session is not active".to_string());
    }

    let ctx = RoutingContext::from_app_state(&state);
    match ctx.pipeline_mode {
        PipelineMode::Modular => {
            crate::pipeline::modular::ptt::ptt_start(&app, &state)
        }
        PipelineMode::Realtime => {
            crate::pipeline::realtime::ptt::ptt_start(&app, &state)
        }
    }
}

/// Finalizes Push-To-Talk recording after validating Listening state.
#[tauri::command]
pub async fn ptt_stop(app: AppHandle, state: State<'_, Arc<AppState>>) -> Result<(), String> {
    let current_state = state.pipeline.state();
    if current_state != InteractionState::Listening {
        return Err(format!(
            "[IPC::Assistant] Cannot stop PTT: current state is {:?}, expected Listening",
            current_state
        ));
    }

    let ctx = RoutingContext::from_app_state(&state);
    match ctx.pipeline_mode {
        PipelineMode::Modular => {
            crate::pipeline::modular::ptt::ptt_stop(&app, &state).await
        }
        PipelineMode::Realtime => {
            crate::pipeline::realtime::ptt::ptt_stop(&app, &state).await
        }
    }
}

/// Cancels an in-progress Push-To-Talk recording if currently Listening.
#[tauri::command]
pub async fn ptt_cancel(app: AppHandle, state: State<'_, Arc<AppState>>) -> Result<(), String> {
    let current_state = state.pipeline.state();
    if current_state != InteractionState::Listening {
        return Ok(());
    }

    let ctx = RoutingContext::from_app_state(&state);
    match ctx.pipeline_mode {
        PipelineMode::Modular => {
            crate::pipeline::modular::ptt::ptt_cancel(&app, &state)
        }
        PipelineMode::Realtime => {
            crate::pipeline::realtime::ptt::ptt_cancel(&app, &state)
        }
    }
}
