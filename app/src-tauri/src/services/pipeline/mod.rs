// ─── Pipeline Subsystem Constants ────────────────────────────────────────────
pub const WINDOW_MAIN: &str = "main";
pub const WINDOW_TRAY: &str = "tray";

pub const EVENT_STATE_CHANGED: &str = "state_changed";
pub const EVENT_TRANSCRIPT_PARTIAL: &str = "transcript_partial";
pub const EVENT_TRANSCRIPT_FINAL: &str = "transcript_final";
pub const EVENT_LLM_TOKEN: &str = "llm_token";
pub const EVENT_PIPELINE_ERROR: &str = "pipeline_error";
pub const EVENT_DICTATION_STATE_CHANGED: &str = "dictation_state_changed";

pub const ROUTER_THREAD_NAME: &str = "vox-router";

pub mod dictation;
pub mod modular;
pub mod realtime;
pub mod router;

use crate::core::settings::{DictationInteractionMode, InteractionMode, PipelineMode};
use crate::core::state::{AppState, InteractionOwner, InteractionState};
use std::sync::atomic::Ordering;
use tauri::{AppHandle, Emitter};

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
        let owner: InteractionOwner = state
            .owner
            .load(Ordering::Relaxed)
            .into();
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

/// Resets conversational working memory and preloads active Identity facts.
pub async fn init_new_session(state: &AppState, base_prompt: &str) {
    state.conversation_manager.lock().new_session(base_prompt);
    let db_path = crate::utils::paths::db_path();
    if let Ok(conn) = crate::persistence::db::VoxDb::open_readonly(&db_path).await {
        if let Ok(active_identities) =
            crate::persistence::queries::fetch_all_active_identity(&conn).await
        {
            let facts = active_identities.into_iter().map(|f| f.fact).collect();
            state.conversation_manager.lock().set_identity_facts(facts);
        }
    }
}

pub use router::spawn_router;
