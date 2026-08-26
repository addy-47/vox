use super::{RoutingContext, ROUTER_THREAD_NAME};
use crate::core::events::VoxEvent;
use crate::core::settings::{InteractionMode, PipelineMode};
use crate::core::state::{AppState, InteractionOwner};
use crate::services::audio::PlaybackEngine;
use std::sync::Arc;
use tauri::{AppHandle, Manager};

/// Routes a pipeline event to the active domain handler based on snapshot context.
fn route_event(app: &AppHandle, state: &AppState, playback: &Arc<PlaybackEngine>, event: VoxEvent) {
    let ctx = RoutingContext::from_app_state(state);
    match ctx.owner {
        InteractionOwner::Dictation => {
            super::dictation::handle_event(app, state, event);
        }
        InteractionOwner::Assistant => match (ctx.pipeline_mode, ctx.interaction_mode) {
            (PipelineMode::Modular, InteractionMode::Passive) => {
                super::modular_passive::handle_event(app, state, playback, event);
            }
            (PipelineMode::Modular, InteractionMode::PTT) => {
                super::modular_ptt::handle_event(app, state, playback, event);
            }
            (PipelineMode::Realtime, InteractionMode::Passive) => {
                super::realtime_passive::handle_event(app, state, playback, event);
            }
            (PipelineMode::Realtime, InteractionMode::PTT) => {
                super::realtime_ptt::handle_event(app, state, playback, event);
            }
        },
    }
}

/// Spawns the central non-blocking event pump thread for VoxEvent routing.
pub fn spawn_router(
    app: AppHandle,
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
