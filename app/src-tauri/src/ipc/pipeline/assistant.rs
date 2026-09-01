use crate::core::error::VoxIpcError;
use crate::core::settings::{InteractionMode, PipelineMode};
use crate::core::state::{AppState, InteractionOwner, InteractionState};
use crate::pipeline::RoutingContext;
use crate::services::vad::VadCommand;
use crate::services::vad::VadOperationalMode;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use tauri::{AppHandle, Manager, State};

/// Launches and initializes the 3-tier audio engine.
#[tauri::command]
pub async fn launch_engine<R: tauri::Runtime>(app: AppHandle<R>) -> Result<(), VoxIpcError> {
    let state: State<'_, Arc<AppState>> = app.state();
    crate::core::start_audio_engine(&app, &state)
        .await
        .map_err(VoxIpcError::Engine)
}

/// Shuts down the 3-tier audio engine and unloads models.
#[tauri::command]
pub async fn stop_engine<R: tauri::Runtime>(app: AppHandle<R>) -> Result<(), VoxIpcError> {
    let state: State<'_, Arc<AppState>> = app.state();
    crate::core::stop_audio_engine(&state)
        .await
        .map_err(VoxIpcError::Engine)
}

/// Starts the voice assistant session with entry state validation and audio ownership initialization.
#[tauri::command]
pub async fn start_session<R: tauri::Runtime>(
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

    crate::core::start_audio_engine(&app, &state)
        .await
        .map_err(VoxIpcError::Engine)?;
    state
        .owner
        .store(InteractionOwner::Assistant as u32, Ordering::Relaxed);
    state.pipeline.cancel_flag.store(false, Ordering::Relaxed);

    let ctx = RoutingContext::from_app_state(&state);

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    let conv_id = now;
    state.conversation_id.store(conv_id, Ordering::Relaxed);

    {
        let persist_lock = state.persist_tx.lock();
        if let Some(ref tx) = *persist_lock {
            if let Err(e) = tx.try_send(
                crate::persistence::events::PersistenceEvent::SessionStarted {
                    id: conv_id,
                    timestamp_ms: now,
                },
            ) {
                log::warn!(
                    "[IPC::Assistant] Failed to send SessionStarted to persist: {}",
                    e
                );
            }
        }
    }

    {
        let mem_lock = state.memory_tx.lock();
        if let Some(ref tx) = *mem_lock {
            if let Err(e) = tx.try_send(
                crate::persistence::events::MemoryWorkerEvent::ActiveSessionChanged {
                    session_id: conv_id,
                },
            ) {
                log::trace!(
                    "[IPC::Assistant] Failed to send ActiveSessionChanged to memory worker: {}",
                    e
                );
            }
        }
    }

    log::info!(
        "[IPC::Assistant] Session initiated: id={} pipeline_mode={:?} interaction_mode={:?}",
        conv_id,
        ctx.pipeline_mode,
        ctx.interaction_mode
    );

    let prompt = {
        let settings = state.settings.read().unwrap_or_else(|p| p.into_inner());
        match ctx.pipeline_mode {
            PipelineMode::Modular => settings.persona.modular_prompt.clone(),
            PipelineMode::Realtime => settings.persona.realtime_prompt.clone(),
        }
    };

    crate::pipeline::init_new_session(&state, &prompt).await;

    let state_arc = state.inner().clone();
    crate::pipeline::spawn_idle_monitor(app.clone(), state_arc);

    match (&ctx.pipeline_mode, &ctx.interaction_mode) {
        (PipelineMode::Modular, InteractionMode::Passive) => {
            crate::pipeline::modular::passive::start_session(&app, &state)
                .await
                .map_err(VoxIpcError::Engine)
        }
        (PipelineMode::Modular, InteractionMode::PTT) => {
            crate::pipeline::modular::ptt::start_session(&app, &state)
                .await
                .map_err(VoxIpcError::Engine)
        }
        (PipelineMode::Realtime, InteractionMode::Passive) => {
            crate::pipeline::realtime::passive::start_session(&app, &state)
                .await
                .map_err(VoxIpcError::Engine)
        }
        (PipelineMode::Realtime, InteractionMode::PTT) => {
            crate::pipeline::realtime::ptt::start_session(&app, &state)
                .await
                .map_err(VoxIpcError::Engine)
        }
    }
}

/// Ends the active voice assistant session, resets state to Idle, and manages audio engine ownership.
#[tauri::command]
pub async fn end_session<R: tauri::Runtime>(
    app: AppHandle<R>,
    state: State<'_, Arc<AppState>>,
) -> Result<(), VoxIpcError> {
    let current_state = state.pipeline.state();
    if current_state == InteractionState::Idle {
        log::info!("[IPC::Assistant] end_session called while already Idle; no-op");
        return Ok(());
    }

    let ctx = RoutingContext::from_app_state(&state);
    match (&ctx.pipeline_mode, &ctx.interaction_mode) {
        (PipelineMode::Modular, InteractionMode::Passive) => {
            crate::pipeline::modular::passive::end_session(&app, &state)
                .await
                .map_err(VoxIpcError::Engine)?;
        }
        (PipelineMode::Modular, InteractionMode::PTT) => {
            crate::pipeline::modular::ptt::end_session(&app, &state)
                .await
                .map_err(VoxIpcError::Engine)?;
        }
        (PipelineMode::Realtime, InteractionMode::Passive) => {
            crate::pipeline::realtime::passive::end_session(&app, &state)
                .await
                .map_err(VoxIpcError::Engine)?;
        }
        (PipelineMode::Realtime, InteractionMode::PTT) => {
            crate::pipeline::realtime::ptt::end_session(&app, &state)
                .await
                .map_err(VoxIpcError::Engine)?;
        }
    }

    state.pipeline.cancel_flag.store(true, Ordering::Relaxed);

    let conv_id = state.conversation_id.load(Ordering::Relaxed);
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);

    {
        let persist_lock = state.persist_tx.lock();
        if let Some(ref tx) = *persist_lock {
            if let Err(e) =
                tx.try_send(crate::persistence::events::PersistenceEvent::SessionEnded {
                    id: conv_id,
                    timestamp_ms: now,
                })
            {
                log::warn!(
                    "[IPC::Assistant] Failed to send SessionEnded to persist: {}",
                    e
                );
            }
        }
    }

    {
        let mem_lock = state.memory_tx.lock();
        if let Some(ref tx) = *mem_lock {
            if let Err(e) = tx.try_send(crate::persistence::events::MemoryWorkerEvent::SessionEnd {
                session_id: conv_id.to_string(),
                summary: String::new(),
            }) {
                log::trace!(
                    "[IPC::Assistant] Failed to send SessionEnd to memory worker: {}",
                    e
                );
            }
        }
    }

    crate::pipeline::transition(InteractionState::Idle, &ctx, &app, &state);

    let dictation_enabled = state
        .settings
        .read()
        .map(|s| s.dictation.enabled)
        .unwrap_or(false);
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
            crate::core::settings::DictationInteractionMode::Passive => {
                VadOperationalMode::ContinuousSegmentation
            }
            crate::core::settings::DictationInteractionMode::Ptt => {
                VadOperationalMode::WindowedValidation
            }
        };
        if let Ok(guard) = state.engine.try_lock() {
            if let Some(ref engine) = *guard {
                if let Err(e) = engine.vad_tx.send(VadCommand::SetOperationalMode(vad_mode)) {
                    log::warn!(
                        "[IPC::Assistant] Failed to set VAD mode for dictation: {}",
                        e
                    );
                }
            }
        }
    } else {
        crate::core::stop_audio_engine(&state)
            .await
            .map_err(VoxIpcError::Engine)?;
    }

    Ok(())
}

/// Pauses the active voice assistant pipeline if active.
#[tauri::command]
pub async fn pause_session<R: tauri::Runtime>(
    app: AppHandle<R>,
    state: State<'_, Arc<AppState>>,
) -> Result<(), VoxIpcError> {
    let current_state = state.pipeline.state();
    if current_state == InteractionState::Idle {
        return Err(VoxIpcError::InvalidState(
            "[IPC::Assistant] Cannot pause session: pipeline is Idle".to_string(),
        ));
    }
    if current_state == InteractionState::Paused {
        return Ok(());
    }

    let ctx = RoutingContext::from_app_state(&state);
    match ctx.pipeline_mode {
        PipelineMode::Modular => crate::pipeline::modular::passive::pause_session(&app, &state)
            .await
            .map_err(VoxIpcError::Engine),
        PipelineMode::Realtime => crate::pipeline::realtime::passive::pause_session(&app, &state)
            .await
            .map_err(VoxIpcError::Engine),
    }
}

/// Resumes a paused voice assistant pipeline if in Paused state.
#[tauri::command]
pub async fn resume_session<R: tauri::Runtime>(
    app: AppHandle<R>,
    state: State<'_, Arc<AppState>>,
) -> Result<(), VoxIpcError> {
    let current_state = state.pipeline.state();
    if current_state != InteractionState::Paused {
        return Err(VoxIpcError::InvalidState(format!(
            "[IPC::Assistant] Cannot resume session: current state is {:?}, expected Paused",
            current_state
        )));
    }

    let ctx = RoutingContext::from_app_state(&state);
    match ctx.pipeline_mode {
        PipelineMode::Modular => crate::pipeline::modular::passive::resume_session(&app, &state)
            .await
            .map_err(VoxIpcError::Engine),
        PipelineMode::Realtime => crate::pipeline::realtime::passive::resume_session(&app, &state)
            .await
            .map_err(VoxIpcError::Engine),
    }
}

/// Initiates Push-To-Talk speech recording after validating Ready state.
#[tauri::command]
pub async fn ptt_start<R: tauri::Runtime>(
    app: AppHandle<R>,
    state: State<'_, Arc<AppState>>,
) -> Result<(), VoxIpcError> {
    let current_state = state.pipeline.state();
    if current_state == InteractionState::Idle || current_state == InteractionState::Paused {
        return Err(VoxIpcError::InvalidState(
            "[IPC::Assistant] Cannot start PTT: assistant session is not active".to_string(),
        ));
    }

    let ctx = RoutingContext::from_app_state(&state);
    match ctx.pipeline_mode {
        PipelineMode::Modular => {
            crate::pipeline::modular::ptt::ptt_start(&app, &state).map_err(VoxIpcError::Engine)
        }
        PipelineMode::Realtime => {
            crate::pipeline::realtime::ptt::ptt_start(&app, &state).map_err(VoxIpcError::Engine)
        }
    }
}

/// Finalizes Push-To-Talk recording after validating Listening state.
#[tauri::command]
pub async fn ptt_stop<R: tauri::Runtime>(
    app: AppHandle<R>,
    state: State<'_, Arc<AppState>>,
) -> Result<(), VoxIpcError> {
    let current_state = state.pipeline.state();
    if current_state != InteractionState::Listening {
        return Err(VoxIpcError::InvalidState(format!(
            "[IPC::Assistant] Cannot stop PTT: current state is {:?}, expected Listening",
            current_state
        )));
    }

    let ctx = RoutingContext::from_app_state(&state);
    match ctx.pipeline_mode {
        PipelineMode::Modular => crate::pipeline::modular::ptt::ptt_stop(&app, &state)
            .await
            .map_err(VoxIpcError::Engine),
        PipelineMode::Realtime => crate::pipeline::realtime::ptt::ptt_stop(&app, &state)
            .await
            .map_err(VoxIpcError::Engine),
    }
}

/// Cancels an in-progress Push-To-Talk recording if currently Listening.
#[tauri::command]
pub async fn ptt_cancel<R: tauri::Runtime>(
    app: AppHandle<R>,
    state: State<'_, Arc<AppState>>,
) -> Result<(), VoxIpcError> {
    let current_state = state.pipeline.state();
    if current_state != InteractionState::Listening {
        return Ok(());
    }

    let ctx = RoutingContext::from_app_state(&state);
    match ctx.pipeline_mode {
        PipelineMode::Modular => {
            crate::pipeline::modular::ptt::ptt_cancel(&app, &state).map_err(VoxIpcError::Engine)
        }
        PipelineMode::Realtime => {
            crate::pipeline::realtime::ptt::ptt_cancel(&app, &state).map_err(VoxIpcError::Engine)
        }
    }
}
