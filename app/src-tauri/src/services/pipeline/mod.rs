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
        InteractionOwner::Dictation => "tray",
        InteractionOwner::Assistant => "main",
    }
}

/// Transitions the pipeline turn state, updates atomic flags, and emits state_changed events.
pub fn transition(
    new_state: InteractionState,
    ctx: &RoutingContext,
    app: &AppHandle,
    state: &AppState,
) {
    state.pipeline.set_state(new_state);
    let target = target_window(ctx.owner);

    if let Err(e) = app.emit_to(target, "state_changed", new_state) {
        log::warn!(
            "[Pipeline] Failed to emit state_changed to {}: {}",
            target,
            e
        );
    }
}
