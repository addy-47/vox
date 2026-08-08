//! ============================================================================
//! src/ipc/pipeline/lifecycle.rs — Engine startup, shutdown, engagement, and pause/resume
//! ============================================================================

use crate::core::events::VoxEvent;
use crate::core::state::{AppState, InteractionOwner, InteractionState};
use crate::services::stt::SttCommand;
use std::sync::atomic::Ordering;
use tauri::{AppHandle, Emitter, Manager, State};

pub use super::engine_launch::launch_engine;

#[tauri::command]
pub async fn check_engine_status(
    state: State<'_, std::sync::Arc<AppState>>,
) -> Result<bool, String> {
    let lock = state.engine.lock().await;
    Ok(lock.is_some())
}

#[tauri::command]
pub async fn stop_engine(app: AppHandle) -> Result<(), String> {
    let state: State<'_, std::sync::Arc<AppState>> = app.state();
    let mut lock = state.engine.lock().await;

    if let Some(mut engine) = lock.take() {
        log::info!("[PIPELINE] >>> Shutting down 3-Tier Audio Engine (Deterministic)...");

        // 1. Signal all threads to exit via Atomic flag (Secondary exit path)
        state
            .pipeline
            .engine_shutdown
            .store(true, Ordering::Relaxed);

        // Reset all loaded model atomic states immediately
        state.is_vad_loaded.store(false, Ordering::Relaxed);
        state.is_stt_loaded.store(false, Ordering::Relaxed);
        state.is_llm_loaded.store(false, Ordering::Relaxed);
        state.is_tts_loaded.store(false, Ordering::Relaxed);
        state
            .pipeline
            .current_state_atomic
            .store(InteractionState::Idle as u32, Ordering::Relaxed);
        *state.pipeline.state.lock() = InteractionState::Idle;

        // 2. Signal threads via channels (Primary exit path)
        let _ = engine.pipeline_tx.send(VoxEvent::Shutdown);
        let _ = engine.stt_tx.send(SttCommand::Shutdown);
        let _ = engine.vad_tx.send(crate::core::state::VadCommand::Shutdown);

        // Wait for threads to join
        if let Some(h) = engine.orchestrator_handle.take() {
            let _ = h.join();
        }
        if let Some(h) = engine.stt_handle.take() {
            let _ = h.join();
        }
        if let Some(h) = engine.vad_handle.take() {
            let _ = h.join();
        }
        log::info!("[PIPELINE] Audio Engine threads joined. Resources freed.");

        // 3. Gracefully shutdown Persistence Worker (Architect's requirement)
        // This closes the SQLite connection and flushes the WAL.
        {
            let mut persist_lock = state.persist_tx.lock();
            if let Some(tx) = persist_lock.take() {
                let _ = tx.send(crate::persistence::events::PersistenceEvent::Shutdown);
                log::info!("[PIPELINE] Persistence worker signaled to shutdown/flush.");
            }
        }
    }

    Ok(())
}

#[tauri::command]
pub async fn engage(
    state: State<'_, std::sync::Arc<AppState>>,
    app: AppHandle,
) -> Result<(), String> {
    let current = state.pipeline.is_engaged.load(Ordering::Relaxed);
    let new_state = !current;

    if new_state {
        log::info!("[Pipeline] Engaging main application pipeline. Starting User Session.");
        state.pipeline.is_engaged.store(true, Ordering::Relaxed);
        state.pipeline.cancel_flag.store(false, Ordering::Relaxed);
        state
            .owner
            .store(InteractionOwner::MainWindow as u32, Ordering::Relaxed);

        // Offload ONNX classifier asset loading and warmup inference off Tokio worker threads
        let scope_load_res = tokio::task::spawn_blocking(|| {
            crate::services::memory::ensure_scope_classifier_loaded()
        })
        .await;
        if let Err(e) = scope_load_res.unwrap_or(Ok(false)) {
            log::warn!(
                "[QueryScopeClassifier] Lazy load on pipeline engage skipped/failed: {}",
                e
            );
        }

        // Ensure engine is launched
        if state.engine.lock().await.is_none() {
            log::info!("[Pipeline] Engine not running. Launching for Engagement...");
            launch_engine(app.clone()).await?;
        }

        if let Some(engine) = state.engine.lock().await.as_ref() {
            // Generate conversation ID based on epoch ms
            let conv_id = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64;
            state.conversation_id.store(conv_id, Ordering::Relaxed);
            log::info!("[Session] >>> USER SESSION STARTED: id={}", conv_id);

            // Persist Session Start
            {
                let persist_tx = state.persist_tx.lock();
                if let Some(ref tx) = *persist_tx {
                    if let Err(_) = tx.try_send(
                        crate::persistence::events::PersistenceEvent::SessionStarted {
                            id: conv_id,
                            timestamp_ms: conv_id,
                        },
                    ) {
                        state
                            .dropped_persistence_events
                            .fetch_add(1, Ordering::Relaxed);
                    }
                }
            }

            // Notify Memory Worker of Active Session Change
            {
                let memory_tx = state.memory_tx.lock();
                if let Some(ref tx) = *memory_tx {
                    let _ = tx.try_send(
                        crate::persistence::memory_worker::MemoryWorkerEvent::ActiveSessionChanged {
                            session_id: conv_id,
                        },
                    );
                }
            }

            let _ = engine.pipeline_tx.send(VoxEvent::WarmUp);
            let _ = engine
                .vad_tx
                .send(crate::core::state::VadCommand::UpdateOwner(
                    InteractionOwner::MainWindow,
                ));
        }
    } else {
        log::info!("[Pipeline] Disengaging pipeline. Ending User Session.");
        state.pipeline.is_engaged.store(false, Ordering::Relaxed);
        state.pipeline.cancel_flag.store(true, Ordering::Relaxed);
        state.owner.store(
            crate::core::state::InteractionOwner::Tray as u32,
            Ordering::Relaxed,
        );

        let remote_tts_info = {
            let s = state.settings.read().ok();
            s.and_then(|settings| {
                if let crate::core::settings::TtsProviderConfig::ChatterboxRemote {
                    endpoint, ..
                } = &settings.tts.provider
                {
                    Some(endpoint.clone())
                } else {
                    None
                }
            })
        };

        if let Some(endpoint) = remote_tts_info {
            tauri::async_runtime::spawn(async move {
                log::info!(
                    "[Pipeline] Disengaged: Unloading remote Chatterbox models from {}",
                    endpoint
                );
                let client = reqwest::Client::new();
                let url = format!("{}/models/unload", endpoint.trim_end_matches('/'));
                if let Err(e) = client.post(&url).send().await {
                    log::warn!(
                        "[Pipeline] Failed to send unload command to remote server: {}",
                        e
                    );
                }
            });
        }

        if let Some(engine) = state.engine.lock().await.as_ref() {
            let turn_id = state.pipeline.turn_id.load(Ordering::Relaxed);
            let _ = engine.pipeline_tx.send(VoxEvent::Cancelled { turn_id });
            let _ = engine.stt_tx.send(SttCommand::ResetStream);
            engine.playback_engine.cancel();
            let _ = engine
                .vad_tx
                .send(crate::core::state::VadCommand::UpdateOwner(
                    InteractionOwner::Tray,
                ));

            let tray_enabled = {
                let s = state.settings.read().unwrap();
                s.ui.tray_enabled
            };
            if !tray_enabled {
                let _ = engine
                    .vad_tx
                    .send(crate::core::state::VadCommand::UpdateMode(
                        crate::core::settings::InteractionMode::PTT,
                    ));
            }

            // Persist Session End
            let conv_id = state.conversation_id.swap(0, Ordering::Relaxed);
            if conv_id != 0 {
                log::info!(
                    "[Session] <<< USER SESSION ENDED (User Disengaged): id={}",
                    conv_id
                );
                let persist_tx = state.persist_tx.lock();
                if let Some(ref tx) = *persist_tx {
                    let now = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_millis() as u64;
                    if let Err(_) =
                        tx.try_send(crate::persistence::events::PersistenceEvent::SessionEnded {
                            id: conv_id,
                            timestamp_ms: now,
                        })
                    {
                        state
                            .dropped_persistence_events
                            .fetch_add(1, Ordering::Relaxed);
                    }
                }

                // Trigger Memory SessionEnd consolidation
                let memory_tx = state.memory_tx.lock();
                if let Some(ref tx) = *memory_tx {
                    let summary = state.conversation_manager.lock().latest_summary();
                    let _ = tx.try_send(
                        crate::persistence::memory_worker::MemoryWorkerEvent::SessionEnd {
                            session_id: conv_id.to_string(),
                            summary,
                        },
                    );
                }
            }
        }

        state
            .owner
            .store(InteractionOwner::Tray as u32, Ordering::Relaxed);

        {
            let mut state_lock = state.pipeline.state.lock();
            *state_lock = InteractionState::Idle;
        }
        let _ = app.emit_to("main", "state_changed", InteractionState::Idle);
        let _ = app.emit_to("tray", "state_changed", InteractionState::Idle);
    }

    Ok(())
}

#[tauri::command]
pub async fn pause_pipeline(
    app: AppHandle,
    state: State<'_, std::sync::Arc<AppState>>,
) -> Result<(), String> {
    log::info!("[IPC] pause_pipeline requested");

    let is_paused = state.pipeline.is_paused.load(Ordering::SeqCst);
    if is_paused {
        return Ok(());
    }

    state.pipeline.is_paused.store(true, Ordering::SeqCst);
    state.pipeline.cancel_flag.store(true, Ordering::SeqCst);

    let engine_guard = state.engine.lock().await;
    if let Some(engine) = &*engine_guard {
        engine.playback_engine.cancel();
    }

    let (pipeline_mode, owner) = {
        let settings = state.settings.read().unwrap();
        (
            settings.interaction.pipeline_mode.clone(),
            state.owner.load(Ordering::Relaxed).into(),
        )
    };

    if pipeline_mode == crate::core::settings::PipelineMode::Realtime {
        let rt_guard = state.realtime_engine.lock().await;
        if let Some(rt_engine) = &*rt_guard {
            let _ = rt_engine.activity_end();
        }
    } else {
        if let Some(engine) = &*engine_guard {
            let turn_id = state.pipeline.turn_id.load(Ordering::Relaxed);
            let _ = engine.pipeline_tx.send(VoxEvent::Cancelled { turn_id });
            let _ = engine.stt_tx.send(SttCommand::ResetStream);
        }
    }

    let target = match owner {
        InteractionOwner::Tray => "tray",
        InteractionOwner::MainWindow | InteractionOwner::Ptt => "main",
        InteractionOwner::Wizard => "wizard",
    };
    let _ = app.emit_to(target, "pipeline_paused", ());

    state
        .pipeline
        .update_interaction_state(InteractionState::Idle, owner, &app);

    Ok(())
}

#[tauri::command]
pub async fn resume_pipeline(
    app: AppHandle,
    state: State<'_, std::sync::Arc<AppState>>,
) -> Result<(), String> {
    log::info!("[IPC] resume_pipeline requested");

    let is_paused = state.pipeline.is_paused.load(Ordering::SeqCst);
    if !is_paused {
        return Ok(());
    }

    state.pipeline.is_paused.store(false, Ordering::SeqCst);

    let (pipeline_mode, interaction_mode, owner) = {
        let settings = state.settings.read().unwrap();
        let owner: InteractionOwner = state.owner.load(Ordering::Relaxed).into();
        let mode = match owner {
            InteractionOwner::Tray => settings.interaction.tray_mode.clone(),
            InteractionOwner::MainWindow | InteractionOwner::Ptt => {
                settings.interaction.main_app_mode.clone()
            }
            InteractionOwner::Wizard => crate::core::settings::InteractionMode::Passive,
        };
        (settings.interaction.pipeline_mode.clone(), mode, owner)
    };

    if pipeline_mode == crate::core::settings::PipelineMode::Modular {
        state.pipeline.cancel_flag.store(false, Ordering::SeqCst);
        let engine_guard = state.engine.lock().await;
        if let Some(engine) = &*engine_guard {
            let _ = engine.pipeline_tx.send(VoxEvent::WarmUp);
        }
    } else if pipeline_mode == crate::core::settings::PipelineMode::Realtime {
        let is_connected = {
            let rt_guard = state.realtime_engine.lock().await;
            if let Some(rt_engine) = &*rt_guard {
                rt_engine.is_connected()
            } else {
                false
            }
        };

        if !is_connected {
            log::info!("[IPC] Realtime S2S session is disconnected during pause. Reconnecting lazily on resume.");
            if let Err(e) = super::realtime::start_realtime_session_internal(&app, &state).await {
                log::error!("[IPC] Lazy S2S reconnection failed: {}", e);
                return Err(e);
            }
        }
    }

    let target = match owner {
        InteractionOwner::Tray => "tray",
        InteractionOwner::MainWindow | InteractionOwner::Ptt => "main",
        InteractionOwner::Wizard => "wizard",
    };
    let _ = app.emit_to(target, "pipeline_resumed", ());

    let next_state = match interaction_mode {
        crate::core::settings::InteractionMode::PTT => InteractionState::Idle,
        crate::core::settings::InteractionMode::Passive => InteractionState::Listening,
    };
    state
        .pipeline
        .update_interaction_state(next_state, owner, &app);

    Ok(())
}
