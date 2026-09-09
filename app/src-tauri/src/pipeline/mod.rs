pub mod assistant;
pub mod dictation;
pub mod router;
pub mod test;

use std::{
    sync::{atomic::Ordering, Arc},
    thread::scope,
    time::Duration,
};

use tauri::AppHandle;

pub use crate::core::constants::{WINDOW_MAIN, WINDOW_TOAST, WINDOW_TRAY, WINDOW_WIZARD};
use crate::{
    core::{
        events::{emit_ipc_to, IpcEvent, StateChangedPayload, VoxEvent},
        settings::{DictationInteractionMode, InteractionMode, PipelineMode},
        state::{AppState, InteractionOwner, InteractionState},
    },
    persistence::{
        compactions::{fetch_latest_compaction_run, fetch_turns_for_compaction},
        db::get_tokio_handle,
        personal_memory::get_personal_memory,
    },
    services::{llm::actor::cool_down_llm, memory::trim_heap, tts::actor::cool_down_tts},
    utils::json::parse_unified_compaction_json,
};

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

/// Resets conversational working memory and preloads personal memory.
pub async fn init_new_session(state: &AppState, base_prompt: &str) {
    state.conversation_manager.lock().new_session(base_prompt);
    let (context_window, max_context_share) = {
        let settings = state.settings.read().unwrap_or_else(|p| p.into_inner());
        (
            settings.llm.context_window as usize,
            settings.memory.max_context_share,
        )
    };

    if let Ok(rec) = get_personal_memory(&state.db, None).await {
        if !rec.content.trim().is_empty() {
            state.conversation_manager.lock().set_personal_memory(
                Some(rec.content),
                context_window,
                max_context_share,
            );
        }
    }
}

/// Restores an existing session continuation context into working memory.
pub async fn resume_session(
    state: &AppState,
    base_prompt: &str,
    session_id: i64,
) -> anyhow::Result<()> {
    let (context_window, max_context_share) = {
        let settings = state.settings.read().unwrap_or_else(|p| p.into_inner());
        (
            settings.llm.context_window as usize,
            settings.memory.max_context_share,
        )
    };

    let personal_memory = match get_personal_memory(&state.db, None).await {
        Ok(rec) if !rec.content.trim().is_empty() => Some(rec.content),
        _ => None,
    };

    let (latest_summary, last_compacted) =
        match fetch_latest_compaction_run(&state.db, session_id).await? {
            Some(run) if run.status == "completed" => {
                let summary = parse_unified_compaction_json(&run.compaction_output).and_then(|p| {
                    if !p.context_summary.trim().is_empty() {
                        Some(p.context_summary.trim().to_string())
                    } else {
                        None
                    }
                });
                (summary, run.to_turn_id)
            }
            _ => (None, 0),
        };

    let turns =
        fetch_turns_for_compaction(&state.db, session_id, last_compacted + 1, u32::MAX).await?;

    let mut cm = state.conversation_manager.lock();
    cm.restore_session_continuation(
        base_prompt,
        personal_memory,
        latest_summary,
        turns,
        context_window,
        max_context_share,
    );

    log::info!(
        "[Pipeline] Restored continuation for session {}: loaded context summary + uncompacted turns",
        session_id
    );

    Ok(())
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
    let handle = get_tokio_handle();
    if handle.runtime_flavor() == tokio::runtime::RuntimeFlavor::MultiThread {
        tokio::task::block_in_place(|| {
            handle.block_on(init_new_session(state, base_prompt));
        });
    } else {
        scope(|s| {
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
pub fn spawn_idle_monitor<R: tauri::Runtime>(app: tauri::AppHandle<R>, state: Arc<AppState>) {
    tauri::async_runtime::spawn(async move {
        let mut state_rx = state.pipeline.subscribe_state();
        loop {
            let current = *state_rx.borrow_and_update();
            if current == InteractionState::Idle {
                if state_rx.changed().await.is_err() {
                    break;
                }
                continue;
            }

            if current == InteractionState::Ready {
                tokio::select! {
                    _ = tokio::time::sleep(INACTIVITY_READY_TIMEOUT) => {
                        if state.pipeline.state() == InteractionState::Ready {
                            log::info!("[Pipeline] Auto-pausing session after 7 minutes of idle Ready state.");
                            let event_tx_opt = state.event_tx.lock().clone();
                            if let Some(tx) = event_tx_opt {
                                if let Err(e) = tx.send(VoxEvent::PauseSession) {
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
            } else if current == InteractionState::Paused {
                tokio::select! {
                    _ = tokio::time::sleep(INACTIVITY_PAUSED_TIMEOUT) => {
                        if state.pipeline.state() == InteractionState::Paused {
                            log::info!("[Pipeline] Offloading idle models after 5 minutes of sustained Paused state.");
                            let mut lock = state.engine.lock().await;
                            if let Some(ref mut engine) = *lock {
                                cool_down_llm(&mut engine.llm_tx, Some(&state.llm_provider));
                                cool_down_tts(&mut engine.tts_tx);
                            }
                            drop(lock);
                            trim_heap("secondary_paused_offload");

                            state.pipeline.set_state(InteractionState::Sleeping);
                            let turn_id = state.pipeline.peek_turn_id();
                            let payload = StateChangedPayload {
                                owner: InteractionOwner::Assistant,
                                state: "Sleeping".to_string(),
                                turn_id,
                            };
                            if let Err(e) = emit_ipc_to(
                                &app,
                                WINDOW_MAIN,
                                IpcEvent::StateChanged(payload),
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
