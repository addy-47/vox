// ─── Pipeline Subsystem Constants ────────────────────────────────────────────
pub const WINDOW_MAIN: &str = "main";
pub const WINDOW_TRAY: &str = "tray";

pub const ROUTER_THREAD_NAME: &str = "vox-router";

pub mod dictation;
pub mod modular;
pub mod realtime;
pub mod router;

use crate::core::events::{emit_ipc_to, IpcEvent};
use crate::core::settings::{DictationInteractionMode, InteractionMode, PipelineMode};
use crate::core::state::{AppState, InteractionOwner, InteractionState};
use std::sync::atomic::Ordering;
use tauri::AppHandle;

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
    if state.pipeline.state() == new_state {
        return;
    }

    state.pipeline.set_state(new_state);
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
    };
    let payload = crate::core::events::StateChangedPayload {
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

/// Spawns an idle observer for the assistant pipeline that auto-pauses the session
/// after 7 continuous minutes in the Ready state.
pub fn spawn_idle_monitor<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: std::sync::Arc<AppState>,
) {
    tauri::async_runtime::spawn(async move {
        let mut state_rx = state.pipeline.state_rx.clone();
        loop {
            if *state_rx.borrow() == crate::core::state::InteractionState::Ready {
                tokio::select! {
                    _ = tokio::time::sleep(crate::services::realtime::REALTIME_IDLE_TIMEOUT) => {
                        if state.pipeline.state() == crate::core::state::InteractionState::Ready {
                            log::info!("[Pipeline] Auto-pausing session after 7 minutes of idle Ready state.");
                            let ctx = RoutingContext::from_app_state(&state);
                            match (&ctx.pipeline_mode, &ctx.interaction_mode) {
                                (PipelineMode::Modular, InteractionMode::Passive) => {
                                    let _ = crate::pipeline::modular::passive::pause_session(&app, &state).await;
                                }
                                (PipelineMode::Realtime, InteractionMode::Passive) => {
                                    let _ = crate::pipeline::realtime::passive::pause_session(&app, &state).await;
                                }
                                _ => {
                                    transition(crate::core::state::InteractionState::Paused, &ctx, &app, &state);
                                }
                            }
                        }
                    }
                    res = state_rx.changed() => {
                        if res.is_err() {
                            break;
                        }
                    }
                }
            } else {
                if state_rx.changed().await.is_err() {
                    break;
                }
                if state.pipeline.state() == crate::core::state::InteractionState::Idle {
                    break;
                }
            }
        }
    });
}

pub use router::spawn_router;
