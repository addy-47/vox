use std::{
    sync::{atomic::Ordering, mpsc, Arc},
    thread::{Builder, JoinHandle},
};

use tauri::{AppHandle, Manager};

use super::ROUTER_THREAD_NAME;
use crate::core::{
    events::{emit_ipc_to, IpcEvent, StateChangedPayload, VoxEvent},
    settings::{DictationInteractionMode, InteractionMode, PipelineMode},
    state::{AppState, AppWindow, InteractionOwner, InteractionState},
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
                (settings.interaction.pipeline_mode.clone(), im)
            }
            InteractionOwner::Assistant => (
                settings.interaction.pipeline_mode.clone(),
                settings.interaction.mode.clone(),
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
    ctx: &RoutingContext,
    app: &AppHandle<R>,
    state: &AppState,
) {
    let previous = state.pipeline.state();
    if previous == new_state {
        return;
    }

    state.pipeline.set_state(new_state);
    log::info!(
        "[Pipeline] State {:?} -> {:?} (owner {:?}, mode {:?}/{:?}, turn {})",
        previous,
        new_state,
        ctx.owner,
        ctx.pipeline_mode,
        ctx.interaction_mode,
        state.pipeline.peek_turn_id()
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

    match event {
        // Session lifecycle — always routed to assistant track
        VoxEvent::SessionStart { owner, session_id } => {
            super::assistant::session::on_session_start(owner, session_id, app, state, &ctx);
        }
        VoxEvent::PauseSession => super::assistant::session::on_pause(app, state, &ctx),
        VoxEvent::ResumeSession => super::assistant::session::on_resume(app, state, &ctx),
        VoxEvent::EndSession => super::assistant::session::on_end(app, state, &ctx),

        // Speech/PTT/Transcript/Error — route by owner
        VoxEvent::SpeechStart
        | VoxEvent::SpeechEnd
        | VoxEvent::PttStart
        | VoxEvent::PttStop
        | VoxEvent::PttCancel
        | VoxEvent::TranscriptFinal { .. }
        | VoxEvent::Cancelled { .. }
        | VoxEvent::Error { .. }
            if ctx.owner == InteractionOwner::Dictation =>
        {
            super::dictation::handle_event(app, state, event);
        }

        // Remaining assistant events (playback, LLM, speech, PTT, transcript, error)
        VoxEvent::SpeechStart => super::assistant::speech::on_speech_start(app, state, &ctx),
        VoxEvent::SpeechEnd => super::assistant::speech::on_speech_end(app, state, &ctx),
        VoxEvent::TranscriptFinal { turn_id, text } => {
            super::assistant::transcript::on_transcript_final(turn_id, text, app, state, &ctx);
        }
        VoxEvent::TextInput { text } => {
            let current_state = state.pipeline.state();
            if current_state == InteractionState::Idle
                || current_state == InteractionState::Sleeping
            {
                log::debug!(
                    "[Pipeline::Router] TextInput dropped in {:?}",
                    current_state
                );
                return;
            }

            // Auto-resume so typed input is never silently dropped while paused.
            if current_state == InteractionState::Paused {
                log::info!(
                    "[Pipeline::Router] TextInput while Paused: auto-resuming before dispatch"
                );
                super::assistant::session::on_resume(app, state, &ctx);
            }

            let current_state = state.pipeline.state();
            let active_turn_id = if current_state == InteractionState::Thinking
                || current_state == InteractionState::Speaking
                || current_state == InteractionState::Working
            {
                super::assistant::interrupt::on_interrupt(app, state, &ctx)
            } else if current_state == InteractionState::Ready {
                let (new_turn_id, _) = state.pipeline.next_turn();
                state.pipeline_accumulator.lock().clear();
                new_turn_id
            } else {
                return;
            };

            transition(InteractionState::Thinking, &ctx, app, state);
            super::assistant::transcript::on_transcript_final(
                active_turn_id,
                text,
                app,
                state,
                &ctx,
            );
        }
        VoxEvent::LlmFinished { turn_id } => {
            super::assistant::llm::on_llm_finished(turn_id, state, &ctx);
        }
        VoxEvent::PlaybackStarted { turn_id, intent } => {
            super::assistant::playback::on_playback_started(turn_id, intent, app, state, &ctx);
        }
        VoxEvent::PlaybackFinished { turn_id, intent } => {
            super::assistant::playback::on_playback_finished(turn_id, intent, app, state, &ctx);
        }
        VoxEvent::Error(err) => {
            super::assistant::error::on_error(err, app, state, &ctx);
        }
        VoxEvent::Cancelled { turn_id } => {
            super::assistant::error::on_cancelled(turn_id, app, state, &ctx);
        }
        VoxEvent::PttStart => super::assistant::ptt::on_ptt_start(app, state, &ctx),
        VoxEvent::PttStop => super::assistant::ptt::on_ptt_stop(app, state, &ctx),
        VoxEvent::PttCancel => super::assistant::ptt::on_ptt_cancel(app, state, &ctx),
        VoxEvent::Shutdown => {}
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
