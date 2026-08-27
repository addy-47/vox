// ─── Pipeline Subsystem Constants ────────────────────────────────────────────
pub const WINDOW_MAIN: &str = "main";
pub const WINDOW_TRAY: &str = "tray";

pub const EVENT_STATE_CHANGED: &str = "state_changed";
pub const EVENT_SESSION_STARTED: &str = "session_started";
pub const EVENT_SESSION_ENDED: &str = "session_ended";
pub const EVENT_PIPELINE_PAUSED: &str = "pipeline_paused";
pub const EVENT_PIPELINE_RESUMED: &str = "pipeline_resumed";
pub const EVENT_PTT_STATUS: &str = "ptt_status";
pub const EVENT_SPEECH_START: &str = "speech_start";
pub const EVENT_SPEECH_END: &str = "speech_end";
pub const EVENT_TRANSCRIPT_PARTIAL: &str = "transcript_partial";
pub const EVENT_TRANSCRIPT_FINAL: &str = "transcript_final";
pub const EVENT_LLM_TOKEN: &str = "llm_token";
pub const EVENT_LLM_FINISHED: &str = "llm_finished";
pub const EVENT_PLAYBACK_STARTED: &str = "playback_started";
pub const EVENT_PLAYBACK_FINISHED: &str = "playback_finished";
pub const EVENT_PIPELINE_ERROR: &str = "pipeline_error";

pub const STATUS_RECORDING: &str = "RECORDING";
pub const STATUS_PROCESSING: &str = "PROCESSING";
pub const STATUS_IDLE: &str = "IDLE";
pub const END_REASON_USER: &str = "user";
pub const OWNER_DICTATION: &str = "dictation";

pub const ROUTER_THREAD_NAME: &str = "vox-router";

pub mod dictation;
pub mod modular_passive;
pub mod modular_ptt;
pub mod realtime_passive;
pub mod realtime_ptt;
pub mod router;

pub use router::spawn_router;

use crate::core::settings::{DictationInteractionMode, InteractionMode, PipelineMode};
use crate::core::state::{AppState, InteractionOwner, InteractionState};
use tauri::{AppHandle, Emitter};

#[derive(Debug, Clone, PartialEq)]
pub struct RoutingContext {
    pub pipeline_mode: PipelineMode,
    pub interaction_mode: InteractionMode,
    pub owner: InteractionOwner,
}

impl RoutingContext {
    /// Snapshots the active routing context from settings and current owner.
    pub fn from_app_state(state: &AppState) -> Self {
        let settings = state.settings.read().unwrap();
        let owner: InteractionOwner = state
            .owner
            .load(std::sync::atomic::Ordering::Relaxed)
            .into();
        let interaction_mode = match owner {
            InteractionOwner::Dictation => match settings.dictation.interaction_mode {
                DictationInteractionMode::Passive => InteractionMode::Passive,
                DictationInteractionMode::Ptt => InteractionMode::PTT,
            },
            InteractionOwner::Assistant => settings.interaction.mode.clone(),
        };

        Self {
            pipeline_mode: settings.interaction.pipeline_mode.clone(),
            interaction_mode,
            owner,
        }
    }
}

/// Resolves the designated Tauri webview window target for a given interaction owner.
pub fn target_window(owner: InteractionOwner) -> &'static str {
    match owner {
        InteractionOwner::Dictation => WINDOW_TRAY,
        InteractionOwner::Assistant => WINDOW_MAIN,
    }
}

/// Transitions the pipeline turn state, updates atomic flags, and emits state_changed events.
pub fn transition<R: tauri::Runtime>(
    new_state: InteractionState,
    ctx: &RoutingContext,
    app: &AppHandle<R>,
    state: &AppState,
) {
    state.pipeline.set_state(new_state);
    let target = target_window(ctx.owner);

    if let Err(e) = app.emit_to(target, EVENT_STATE_CHANGED, new_state) {
        log::warn!(
            "[Pipeline] Failed to emit state_changed to {}: {}",
            target,
            e
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Tests that interaction owner maps strictly to the correct Tauri webview window label.
    #[test]
    fn test_target_window_routing() {
        assert_eq!(target_window(InteractionOwner::Dictation), WINDOW_TRAY);
        assert_eq!(target_window(InteractionOwner::Assistant), WINDOW_MAIN);
    }
}
