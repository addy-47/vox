pub mod assistant;
pub mod dictation;
pub mod router;
pub mod test;

pub use crate::core::constants::{WINDOW_MAIN, WINDOW_TOAST, WINDOW_TRAY, WINDOW_WIZARD};
use crate::core::events::{emit_ipc_to, IpcEvent};
use crate::core::settings::{DictationInteractionMode, InteractionMode, PipelineMode};
use crate::core::state::{AppState, InteractionOwner, InteractionState};
use std::sync::atomic::Ordering;
use std::time::Duration;
use tauri::AppHandle;

pub const ROUTER_THREAD_NAME: &str = "vox-router";
pub const INACTIVITY_READY_TIMEOUT: Duration = Duration::from_secs(420);
pub const INACTIVITY_PAUSED_TIMEOUT: Duration = Duration::from_secs(300); 

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
        InteractionState::Sleeping => "Sleeping",
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

/// Synchronous wrapper for init_new_session executed via the global Tokio runtime handle.
pub fn init_new_session_sync(state: &AppState, base_prompt: &str) {
    if let Ok(handle) = tokio::runtime::Handle::try_current() {
        if handle.runtime_flavor() == tokio::runtime::RuntimeFlavor::MultiThread {
            tokio::task::block_in_place(|| {
                handle.block_on(init_new_session(state, base_prompt));
            });
            return;
        }
    }
    let handle = crate::persistence::db::get_tokio_handle();
    if handle.runtime_flavor() == tokio::runtime::RuntimeFlavor::MultiThread {
        tokio::task::block_in_place(|| {
            handle.block_on(init_new_session(state, base_prompt));
        });
    } else {
        std::thread::scope(|s| {
            s.spawn(|| {
                handle.block_on(init_new_session(state, base_prompt));
            })
            .join()
            .expect("init_new_session worker panicked");
        });
    }
}

/// Spawns an idle observer for the assistant pipeline that auto-pauses after 7 minutes of Ready
/// and reclaims model RAM after 5 minutes of sustained Paused state.
pub fn spawn_idle_monitor<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: std::sync::Arc<AppState>,
) {
    tauri::async_runtime::spawn(async move {
        let mut state_rx = state.pipeline.subscribe_state();
        loop {
            let current = *state_rx.borrow_and_update();
            if current == crate::core::state::InteractionState::Idle {
                if state_rx.changed().await.is_err() {
                    break;
                }
                continue;
            }

            if current == crate::core::state::InteractionState::Ready {
                tokio::select! {
                    _ = tokio::time::sleep(INACTIVITY_READY_TIMEOUT) => {
                        if state.pipeline.state() == crate::core::state::InteractionState::Ready {
                            log::info!("[Pipeline] Auto-pausing session after 7 minutes of idle Ready state.");
                            let event_tx_opt = state.event_tx.lock().clone();
                            if let Some(tx) = event_tx_opt {
                                if let Err(e) = tx.send(crate::core::events::VoxEvent::PauseSession) {
                                    log::warn!("[Pipeline] Failed to send PauseSession from idle monitor: {}", e);
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
            } else if current == crate::core::state::InteractionState::Paused {
                tokio::select! {
                    _ = tokio::time::sleep(INACTIVITY_PAUSED_TIMEOUT) => {
                        if state.pipeline.state() == crate::core::state::InteractionState::Paused {
                            log::info!("[Pipeline] Offloading idle models after 5 minutes of sustained Paused state.");
                            let mut lock = state.engine.lock().await;
                            if let Some(ref mut engine) = *lock {
                                crate::services::llm::actor::cool_down_llm(
                                    &mut engine.llm_tx,
                                    Some(&state.llm_provider),
                                );
                                crate::services::tts::actor::cool_down_tts(&mut engine.tts_tx);
                            }
                            drop(lock);
                            crate::services::memory::trim_heap("secondary_paused_offload");

                            state.pipeline.set_state(crate::core::state::InteractionState::Sleeping);
                            let turn_id = state.pipeline.peek_turn_id();
                            let payload = crate::core::events::StateChangedPayload {
                                owner: crate::core::state::InteractionOwner::Assistant,
                                state: "Sleeping".to_string(),
                                turn_id,
                            };
                            if let Err(e) = crate::core::events::emit_ipc_to(
                                &app,
                                WINDOW_MAIN,
                                crate::core::events::IpcEvent::StateChanged(payload),
                            ) {
                                log::warn!("[Pipeline] Failed to emit Sleeping state_changed: {}", e);
                            }
                            log::info!("[Pipeline] Transitioned to Sleeping after model offload");
                        }
                    }
                    res = state_rx.changed() => {
                        if res.is_err() {
                            break;
                        }
                    }
                }
            } else if state_rx.changed().await.is_err() {
                break;
            }
        }
    });
}

pub use router::spawn_router;
