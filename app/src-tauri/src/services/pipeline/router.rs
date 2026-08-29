use super::{RoutingContext, ROUTER_THREAD_NAME};
use crate::core::events::VoxEvent;
use crate::core::settings::PipelineMode;
use crate::core::state::{AppState, InteractionOwner};
use crate::services::audio::PlaybackEngine;
use std::sync::Arc;
use tauri::{AppHandle, Manager};

/// Routes a pipeline event to the active domain handler based on snapshot context.
fn route_event<R: tauri::Runtime>(
    app: &AppHandle<R>,
    state: &AppState,
    playback: &Arc<PlaybackEngine>,
    event: VoxEvent,
) {
    match &event {
        VoxEvent::SpeechStart { .. } | VoxEvent::WarmUp => {
            let mem_lock = state.memory_tx.lock();
            if let Some(ref tx) = *mem_lock {
                if let Err(e) = tx.try_send(crate::persistence::memory_worker::MemoryWorkerEvent::PipelineActive) {
                    log::trace!("[Pipeline::Router] Failed to send PipelineActive to memory worker: {}", e);
                }
            }
        }
        VoxEvent::SpeechEnd { .. } | VoxEvent::PlaybackFinished { .. } | VoxEvent::Cancelled { .. } => {
            let mem_lock = state.memory_tx.lock();
            if let Some(ref tx) = *mem_lock {
                if let Err(e) = tx.try_send(crate::persistence::memory_worker::MemoryWorkerEvent::PipelineIdle) {
                    log::trace!("[Pipeline::Router] Failed to send PipelineIdle to memory worker: {}", e);
                }
            }
        }
        _ => {}
    }

    let ctx = RoutingContext::from_app_state(state);
    match ctx.owner {
        InteractionOwner::Dictation => {
            super::dictation::handle_event(app, state, event);
        }
        InteractionOwner::Assistant => match ctx.pipeline_mode {
            PipelineMode::Modular => {
                super::modular::handle_event(ctx.interaction_mode, app, state, playback, event);
            }
            PipelineMode::Realtime => {
                super::realtime::handle_event(ctx.interaction_mode, app, state, playback, event);
            }
        },
    }
}

/// Spawns the central non-blocking event pump thread for VoxEvent routing.
pub fn spawn_router<R: tauri::Runtime + 'static>(
    app: AppHandle<R>,
    event_rx: std::sync::mpsc::Receiver<VoxEvent>,
    playback: Arc<PlaybackEngine>,
) -> Result<std::thread::JoinHandle<()>, String> {
    std::thread::Builder::new()
        .name(ROUTER_THREAD_NAME.to_string())
        .spawn(move || {
            let app_state: tauri::State<'_, Arc<AppState>> = app.state();
            log::info!("[Router] Central VoxEvent router pump started");

            while let Ok(event) = event_rx.recv() {
                if let VoxEvent::Shutdown = event {
                    log::info!("[Router] Shutdown event received. Exiting router pump.");
                    break;
                }
                route_event(&app, &app_state, &playback, event);
            }

            log::info!("[Router] Central VoxEvent router pump terminated");
        })
        .map_err(|e| format!("[Router] Failed to spawn router thread: {}", e))
}
