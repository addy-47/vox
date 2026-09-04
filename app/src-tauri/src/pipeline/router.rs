//! Central non-blocking pipeline event router.

use super::{RoutingContext, ROUTER_THREAD_NAME};
use crate::core::events::VoxEvent;
use crate::core::state::{AppState, InteractionOwner};
use std::sync::Arc;
use tauri::{AppHandle, Manager};

/// Routes an incoming pipeline event to canonical handlers based on snapshot context.
fn route_event<R: tauri::Runtime + 'static>(app: &AppHandle<R>, state: &AppState, event: VoxEvent) {
    let ctx = RoutingContext::from_app_state(state);

    match event {
        // Session lifecycle — always routed to assistant track
        VoxEvent::SessionStart { owner } => {
            super::assistant::session::on_session_start(owner, app, state, &ctx);
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
        VoxEvent::LlmFinished { turn_id } => {
            super::assistant::llm::on_llm_finished(turn_id, state, &ctx);
        }
        VoxEvent::PlaybackStarted { turn_id } => {
            super::assistant::playback::on_playback_started(turn_id, app, state, &ctx);
        }
        VoxEvent::PlaybackFinished { turn_id } => {
            super::assistant::playback::on_playback_finished(turn_id, app, state, &ctx);
        }
        VoxEvent::Error {
            turn_id,
            message,
            source,
        } => {
            super::assistant::error::on_error(turn_id, message, source, app, state, &ctx);
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
    event_rx: std::sync::mpsc::Receiver<VoxEvent>,
) -> Result<std::thread::JoinHandle<()>, String> {
    std::thread::Builder::new()
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
