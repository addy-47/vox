use std::sync::Arc;

use tauri::AppHandle;

use super::{INACTIVITY_PAUSED_TIMEOUT, INACTIVITY_READY_TIMEOUT};
use crate::{
    core::{
        events::{emit_ipc_to, IpcEvent, StateChangedPayload, VoxEvent},
        state::{AppState, AppWindow, InteractionOwner, InteractionState},
    },
    services::{llm::actor::cool_down_llm, memory::trim_heap, tts::actor::cool_down_tts},
};

/// Spawns an idle observer for the assistant pipeline that auto-pauses after 7 minutes of Ready
/// and reclaims model RAM after 5 minutes of sustained Paused state.
pub fn spawn_idle_monitor<R: tauri::Runtime>(app: AppHandle<R>, state: Arc<AppState>) {
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
                                AppWindow::Main,
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
