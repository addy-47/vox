use crate::core::settings::{InteractionMode, PipelineMode};
use crate::core::state::AppState;
use crate::services::pipeline::RoutingContext;
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
    crate::services::audio::start_audio_engine(&app, &state).await
}

/// Shuts down the 3-tier audio engine and unloads models.
#[tauri::command]
pub async fn stop_engine(app: AppHandle) -> Result<(), String> {
    let state: State<'_, Arc<AppState>> = app.state();
    crate::services::audio::stop_audio_engine(&state).await
}

/// Starts the voice assistant session based on current pipeline settings.
#[tauri::command]
pub async fn start_session(app: AppHandle, state: State<'_, Arc<AppState>>) -> Result<(), String> {
    let ctx = RoutingContext::from_app_state(&state);
    match (ctx.pipeline_mode, ctx.interaction_mode) {
        (PipelineMode::Modular, InteractionMode::Passive) => {
            crate::services::pipeline::modular_passive::start_session(&app, &state).await
        }
        (PipelineMode::Modular, InteractionMode::PTT) => {
            crate::services::pipeline::modular_ptt::start_session(&app, &state).await
        }
        (PipelineMode::Realtime, InteractionMode::Passive) => {
            crate::services::pipeline::realtime_passive::start_session(&app, &state).await
        }
        (PipelineMode::Realtime, InteractionMode::PTT) => {
            crate::services::pipeline::realtime_ptt::start_session(&app, &state).await
        }
    }
}

/// Ends the active voice assistant session.
#[tauri::command]
pub async fn end_session(app: AppHandle, state: State<'_, Arc<AppState>>) -> Result<(), String> {
    let ctx = RoutingContext::from_app_state(&state);
    match (ctx.pipeline_mode, ctx.interaction_mode) {
        (PipelineMode::Modular, InteractionMode::Passive) => {
            crate::services::pipeline::modular_passive::end_session(&app, &state).await
        }
        (PipelineMode::Modular, InteractionMode::PTT) => {
            crate::services::pipeline::modular_ptt::end_session(&app, &state).await
        }
        (PipelineMode::Realtime, InteractionMode::Passive) => {
            crate::services::pipeline::realtime_passive::end_session(&app, &state).await
        }
        (PipelineMode::Realtime, InteractionMode::PTT) => {
            crate::services::pipeline::realtime_ptt::end_session(&app, &state).await
        }
    }
}

/// Pauses the active voice assistant pipeline.
#[tauri::command]
pub async fn pause_session(app: AppHandle, state: State<'_, Arc<AppState>>) -> Result<(), String> {
    let ctx = RoutingContext::from_app_state(&state);
    match ctx.pipeline_mode {
        PipelineMode::Modular => {
            crate::services::pipeline::modular_passive::pause_session(&app, &state).await
        }
        PipelineMode::Realtime => {
            crate::services::pipeline::realtime_passive::pause_session(&app, &state).await
        }
    }
}

/// Resumes a paused voice assistant pipeline.
#[tauri::command]
pub async fn resume_session(app: AppHandle, state: State<'_, Arc<AppState>>) -> Result<(), String> {
    let ctx = RoutingContext::from_app_state(&state);
    match ctx.pipeline_mode {
        PipelineMode::Modular => {
            crate::services::pipeline::modular_passive::resume_session(&app, &state).await
        }
        PipelineMode::Realtime => {
            crate::services::pipeline::realtime_passive::resume_session(&app, &state).await
        }
    }
}

/// Initiates Push-To-Talk speech recording.
#[tauri::command]
pub async fn ptt_start(app: AppHandle, state: State<'_, Arc<AppState>>) -> Result<(), String> {
    let ctx = RoutingContext::from_app_state(&state);
    match ctx.pipeline_mode {
        PipelineMode::Modular => {
            crate::services::pipeline::modular_ptt::handle_ptt_start(&app, &state)
        }
        PipelineMode::Realtime => {
            crate::services::pipeline::realtime_ptt::handle_ptt_start(&app, &state)
        }
    }
}

/// Finalizes Push-To-Talk recording and triggers speech inference.
#[tauri::command]
pub async fn ptt_stop(app: AppHandle, state: State<'_, Arc<AppState>>) -> Result<(), String> {
    let ctx = RoutingContext::from_app_state(&state);
    match ctx.pipeline_mode {
        PipelineMode::Modular => {
            crate::services::pipeline::modular_ptt::handle_ptt_stop(&app, &state)
        }
        PipelineMode::Realtime => {
            crate::services::pipeline::realtime_ptt::handle_ptt_stop(&app, &state)
        }
    }
}

/// Cancels an in-progress Push-To-Talk recording.
#[tauri::command]
pub async fn ptt_cancel(app: AppHandle, state: State<'_, Arc<AppState>>) -> Result<(), String> {
    let ctx = RoutingContext::from_app_state(&state);
    match ctx.pipeline_mode {
        PipelineMode::Modular => {
            crate::services::pipeline::modular_ptt::handle_ptt_cancel(&app, &state)
        }
        PipelineMode::Realtime => {
            crate::services::pipeline::realtime_ptt::handle_ptt_cancel(&app, &state)
        }
    }
}
