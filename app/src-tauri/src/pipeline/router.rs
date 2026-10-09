use std::{
    sync::{atomic::Ordering, mpsc, Arc},
    thread::{Builder, JoinHandle},
};

use tauri::{AppHandle, Manager};

use super::ROUTER_THREAD_NAME;
use crate::{
    core::{
        events::{
            emit_ipc_to, ActivityEnvelope, InteractionMode, IpcEvent, PipelineMode,
            StateChangedPayload, VoxEvent,
        },
        state::{AppState, AppWindow, InteractionOwner, InteractionState},
    },
    pipeline::dictation::DictationInteractionMode,
};

#[derive(Debug, Clone, PartialEq)]
pub struct RoutingContext {
    pub pipeline_mode: PipelineMode,
    pub interaction_mode: InteractionMode,
    pub owner: InteractionOwner,
}

impl RoutingContext {
    /// Snapshots the active routing context from settings and current owner with poison-safety.
    pub fn from_app_state(state: &AppState) -> Self {
        let settings = state.settings.read().unwrap_or_else(|p| p.into_inner());
        let owner: InteractionOwner = state.owner.load(Ordering::Relaxed).into();
        let (pipeline_mode, interaction_mode) = match owner {
            InteractionOwner::Dictation => {
                let im = match settings.dictation.interaction_mode {
                    DictationInteractionMode::Passive => InteractionMode::Passive,
                    DictationInteractionMode::Ptt => InteractionMode::PTT,
                };
                (settings.interaction.pipeline_mode, im)
            }
            InteractionOwner::Assistant => (
                settings.interaction.pipeline_mode,
                settings.interaction.mode,
            ),
        };

        Self {
            pipeline_mode,
            interaction_mode,
            owner,
        }
    }
}

/// Resolves the designated Tauri webview window target for a given interaction owner.
pub fn target_window(owner: InteractionOwner) -> AppWindow {
    match owner {
        InteractionOwner::Dictation => AppWindow::Tray,
        InteractionOwner::Assistant => AppWindow::Main,
    }
}

/// Transitions the pipeline turn state, updates atomic flags, and emits state_changed events.
pub fn transition<R: tauri::Runtime>(
    new_state: InteractionState,
    activity: Option<&ActivityEnvelope>,
    ctx: &RoutingContext,
    app: &AppHandle<R>,
    state: &AppState,
) {
    let activity = match (new_state, activity) {
        (InteractionState::Working, Some(act)) => Some(act),
        (InteractionState::Working, None) => None,
        (_, Some(act)) => {
            log::warn!(
                "[Pipeline] Discarding activity {:?} on non-Working state {:?}",
                act.name,
                new_state
            );
            None
        }
        (_, None) => None,
    };

    let previous = state.pipeline.state();
    if previous == new_state {
        let activity_unchanged = match (activity, state.pipeline.active_activity()) {
            (None, None) => true,
            (Some(next), Some(current)) => current.kind == next.kind && current.name == next.name,
            _ => false,
        };
        if activity_unchanged {
            return;
        }
        log::info!(
            "[Pipeline] Re-entering {:?} with new activity {:?}",
            new_state,
            activity.map(|a| a.name.as_str())
        );
    } else {
        state.pipeline.set_state(new_state);
    }
    state.pipeline.set_active_activity(activity);

    log::info!(
        "[Pipeline] State {:?} -> {:?} (owner {:?}, mode {:?}/{:?}, turn {}, activity {:?})",
        previous,
        new_state,
        ctx.owner,
        ctx.pipeline_mode,
        ctx.interaction_mode,
        state.pipeline.peek_turn_id(),
        activity.map(|a| a.name.as_str())
    );
    let target = target_window(ctx.owner);
    let turn_id = state.pipeline.peek_turn_id();
    let state_str = match new_state {
        InteractionState::Idle => "Idle",
        InteractionState::Ready => "Ready",
        InteractionState::Listening => "Listening",
        InteractionState::Thinking => "Thinking",
        InteractionState::Speaking => "Speaking",
        InteractionState::Paused => "Paused",
        InteractionState::Error => "Error",
        InteractionState::Sleeping => "Sleeping",
        InteractionState::Working => "Working",
    };
    let payload = StateChangedPayload {
        owner: ctx.owner,
        state: state_str.to_string(),
        turn_id,
        activity: activity.cloned(),
    };

    if let Err(e) = emit_ipc_to(app, target, IpcEvent::StateChanged(payload)) {
        log::warn!(
            "[Pipeline] Failed to emit state_changed to {}: {}",
            target,
            e
        );
    }
}

/// Routes an incoming pipeline event to canonical handlers based on snapshot context.
fn route_event<R: tauri::Runtime + 'static>(app: &AppHandle<R>, state: &AppState, event: VoxEvent) {
    let ctx = RoutingContext::from_app_state(state);
    log::debug!("[Router] Routing {:?}", event);
    log::info!(
        "[Pipeline::Router::Trace] event={:?} owner={:?} target={} dictation_state={:?} assistant_state={:?}",
        event,
        ctx.owner,
        target_window(ctx.owner),
        state.pipeline.dictation_state(),
        state.pipeline.state()
    );

    match event {
        // Dictation Track Dispatch
        VoxEvent::PttStart {
            owner: InteractionOwner::Dictation,
        }
        | VoxEvent::PttStop {
            owner: InteractionOwner::Dictation,
        }
        | VoxEvent::PttCancel {
            owner: InteractionOwner::Dictation,
        }
        | VoxEvent::SpeechStart {
            owner: InteractionOwner::Dictation,
        }
        | VoxEvent::SpeechEnd {
            owner: InteractionOwner::Dictation,
        }
        | VoxEvent::TranscriptFinal {
            owner: InteractionOwner::Dictation,
            ..
        }
        | VoxEvent::Cancelled {
            owner: InteractionOwner::Dictation,
            ..
        } => {
            super::dictation::handle_event(app, state, event);
        }

        // System
        VoxEvent::Error(err) => {
            if err.source.starts_with("Dictation") {
                super::dictation::handle_event(app, state, VoxEvent::Error(err));
            } else {
                super::assistant::error::on_error(err, app, state, &ctx);
            }
        }
        VoxEvent::Shutdown => {}

        // Assistant Domain Dispatch
        _ => {
            super::assistant::handle_event(app, state, &ctx, event);
        }
    }
}

/// Spawns the central non-blocking event pump thread for VoxEvent routing.
pub fn spawn_router<R: tauri::Runtime + 'static>(
    app: AppHandle<R>,
    event_rx: mpsc::Receiver<VoxEvent>,
) -> Result<JoinHandle<()>, String> {
    Builder::new()
        .name(ROUTER_THREAD_NAME.to_string())
        .spawn(move || {
            if let Err(e) =
                thread_priority::set_current_thread_priority(thread_priority::ThreadPriority::Max)
            {
                log::debug!("[Router] Could not elevate thread priority: {:?}", e);
            }

            let app_state: tauri::State<'_, Arc<AppState>> = app.state();
            log::info!("[Router] Central VoxEvent router pump started");

            while let Ok(event) = event_rx.recv() {
                if let VoxEvent::Shutdown = event {
                    log::info!("[Router] Shutdown event received. Exiting router pump.");
                    break;
                }
                route_event(&app, &app_state, event);
            }

            log::info!("[Router] Central VoxEvent router pump terminated");
        })
        .map_err(|e| format!("[Router] Failed to spawn router thread: {}", e))
}
