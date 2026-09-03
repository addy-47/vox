use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Manager};

use crate::core::settings::{InteractionMode, PipelineMode};
use crate::core::state::{AppState, InteractionOwner, InteractionState};
use crate::pipeline::{spawn_idle_monitor, transition, RoutingContext};
use crate::services::vad::{VadCommand, VadOperationalMode};

/// Configures and arms the modular speech-to-text, LLM, and TTS worker pipelines.
fn start_modular_session<R: tauri::Runtime + 'static>(
    app: &AppHandle<R>,
    state: &AppState,
    ctx: &RoutingContext,
) -> Result<(), String> {
    crate::core::engine::ensure_modular_workers_sync(app, state)?;

    let vad_mode = match ctx.interaction_mode {
        InteractionMode::Passive => VadOperationalMode::ContinuousSegmentation,
        InteractionMode::PTT => VadOperationalMode::WindowedValidation,
    };

    if let Ok(guard) = state.engine.try_lock() {
        if let Some(ref engine) = *guard {
            if let Err(e) = engine.vad_tx.send(VadCommand::SetOperationalMode(vad_mode)) {
                log::warn!(
                    "[Pipeline::Session] Failed to set VAD operational mode: {}",
                    e
                );
            }
        }
    }

    Ok(())
}

/// Connects to the real-time speech-to-speech provider and configures bidirectional audio streaming.
fn start_realtime_session<R: tauri::Runtime + 'static>(
    app: &AppHandle<R>,
    state: &AppState,
    ctx: &RoutingContext,
) -> Result<(), String> {
    let (vad_tx, pipeline_tx, playback_engine) = {
        let guard = state.engine.blocking_lock();
        let engine = guard.as_ref().ok_or("Audio engine not ready")?;
        (
            engine.vad_tx.clone(),
            engine.pipeline_tx.clone(),
            engine.playback_engine.clone(),
        )
    };

    let mut rt_guard = state.realtime_engine.blocking_lock();
    if let Some(mut old_rt) = rt_guard.take() {
        old_rt.stop();
        if let Err(e) = vad_tx.send(VadCommand::StopRealtime) {
            log::warn!("[Pipeline::Session] Failed to send StopRealtime: {}", e);
        }
    }

    let provider = crate::services::realtime::session::create_realtime_provider(state)?;
    let tokio_handle = crate::persistence::db::get_tokio_handle();
    let mut rt_actor = crate::services::realtime::RealtimeActor::new(provider, tokio_handle);

    rt_actor
        .start(
            ctx.interaction_mode.clone(),
            playback_engine,
            pipeline_tx,
            app.clone(),
        )
        .map_err(|e| format!("[Pipeline::Session] Realtime actor start failed: {}", e))?;

    let audio_tx = rt_actor
        .get_audio_sender()
        .ok_or("Failed to obtain realtime audio sender")?;

    let is_ptt = ctx.interaction_mode == InteractionMode::PTT;
    if let Err(e) = vad_tx.send(VadCommand::StartRealtime {
        tx: audio_tx,
        is_ptt,
    }) {
        log::warn!(
            "[Pipeline::Session] Failed to send StartRealtime to VAD: {}",
            e
        );
    }

    *rt_guard = Some(rt_actor);
    Ok(())
}

/// Restarts real-time provider session and wires audio streaming on resume.
fn resume_realtime<R: tauri::Runtime + 'static>(
    app: &AppHandle<R>,
    state: &AppState,
    ctx: &RoutingContext,
) -> Result<(), String> {
    let (vad_tx, playback_engine, pipeline_tx) = {
        let guard = state.engine.blocking_lock();
        let engine = guard.as_ref().ok_or("Audio engine not ready")?;
        (
            engine.vad_tx.clone(),
            engine.playback_engine.clone(),
            engine.pipeline_tx.clone(),
        )
    };

    let mut rt_guard = state.realtime_engine.blocking_lock();
    if let Some(ref mut rt_actor) = *rt_guard {
        rt_actor
            .start(
                ctx.interaction_mode.clone(),
                playback_engine,
                pipeline_tx,
                app.clone(),
            )
            .map_err(|e| format!("Realtime actor restart failed: {}", e))?;

        let audio_tx = rt_actor
            .get_audio_sender()
            .ok_or_else(|| "Failed to obtain realtime audio sender".to_string())?;

        let is_ptt = ctx.interaction_mode == InteractionMode::PTT;
        if let Err(e) = vad_tx.send(VadCommand::StartRealtime {
            tx: audio_tx,
            is_ptt,
        }) {
            log::warn!(
                "[Pipeline::Session] Failed to send StartRealtime on resume: {}",
                e
            );
        }

        Ok(())
    } else {
        drop(rt_guard);
        start_realtime_session(app, state, ctx)
    }
}

/// Initializes voice session context, persists lifecycle start events, arms workers, and transitions to Ready.
pub fn on_session_start<R: tauri::Runtime + 'static>(
    owner: InteractionOwner,
    app: &AppHandle<R>,
    state: &AppState,
    ctx: &RoutingContext,
) {
    let current_state = state.pipeline.state();
    if current_state != InteractionState::Idle {
        log::warn!(
            "[Pipeline::Session] Cannot start session: pipeline state is {:?}, expected Idle",
            current_state
        );
        return;
    }

    state.owner.store(owner as u32, Ordering::Relaxed);
    state.pipeline.cancel_flag.store(false, Ordering::Relaxed);

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    let conv_id = now;
    state.conversation_id.store(conv_id, Ordering::Relaxed);

    let persist_lock = state.persist_tx.lock();
    if let Some(ref tx) = *persist_lock {
        if let Err(e) = tx.try_send(
            crate::persistence::events::PersistenceEvent::SessionStarted {
                id: conv_id,
                timestamp_ms: now,
            },
        ) {
            log::warn!(
                "[Pipeline::Session] Failed to send SessionStarted to persistence: {}",
                e
            );
        }
    }

    let mem_lock = state.memory_tx.lock();
    if let Some(ref tx) = *mem_lock {
        if let Err(e) = tx.try_send(
            crate::persistence::events::MemoryWorkerEvent::ActiveSessionChanged {
                session_id: conv_id,
            },
        ) {
            log::trace!(
                "[Pipeline::Session] Failed to send ActiveSessionChanged to memory: {}",
                e
            );
        }
    }

    let prompt = {
        let settings = state.settings.read().unwrap_or_else(|p| p.into_inner());
        match ctx.pipeline_mode {
            PipelineMode::Modular => settings.persona.modular_prompt.clone(),
            PipelineMode::Realtime => settings.persona.realtime_prompt.clone(),
        }
    };

    crate::pipeline::init_new_session_sync(state, &prompt);

    let state_arc: tauri::State<'_, Arc<AppState>> = app.state();
    spawn_idle_monitor(app.clone(), Arc::clone(state_arc.inner()));

    let start_res = match ctx.pipeline_mode {
        PipelineMode::Modular => start_modular_session(app, state, ctx),
        PipelineMode::Realtime => start_realtime_session(app, state, ctx),
    };

    if let Err(e) = start_res {
        log::error!("[Pipeline::Session] Session start failed: {}", e);
        transition(InteractionState::Error, ctx, app, state);
        return;
    }

    state.pipeline_accumulator.lock().clear();
    transition(InteractionState::Ready, ctx, app, state);
    log::info!(
        "[Pipeline::Session] Session started (ID: {}, mode: {:?})",
        conv_id,
        ctx.pipeline_mode
    );
}

/// Pauses the active voice session, silencing audio output and placing the state machine in Paused.
pub fn on_pause<R: tauri::Runtime>(app: &AppHandle<R>, state: &AppState, ctx: &RoutingContext) {
    let current_state = state.pipeline.state();
    if current_state == InteractionState::Idle || current_state == InteractionState::Paused {
        log::debug!(
            "[Pipeline::Session] Pause dropped: already in {:?}",
            current_state
        );
        return;
    }

    state.pipeline.cancel_flag.store(true, Ordering::Relaxed);
    state.pipeline.turn_token().cancel();
    state.pipeline_accumulator.lock().clear();

    if let Ok(guard) = state.engine.try_lock() {
        if let Some(ref engine) = *guard {
            engine.playback_engine.cancel();
            if ctx.pipeline_mode == PipelineMode::Realtime {
                if let Err(e) = engine.vad_tx.send(VadCommand::StopRealtime) {
                    log::warn!(
                        "[Pipeline::Session] Failed to send StopRealtime on pause: {}",
                        e
                    );
                }
            }
        }
    }

    let dictation_enabled = state
        .settings
        .read()
        .map(|s| s.dictation.enabled)
        .unwrap_or(false);

    if dictation_enabled {
        state
            .owner
            .store(InteractionOwner::Dictation as u32, Ordering::Relaxed);
        let dictation_mode = state
            .settings
            .read()
            .map(|s| s.dictation.interaction_mode.clone())
            .unwrap_or(crate::core::settings::DictationInteractionMode::Ptt);
        let vad_mode = match dictation_mode {
            crate::core::settings::DictationInteractionMode::Passive => {
                VadOperationalMode::ContinuousSegmentation
            }
            crate::core::settings::DictationInteractionMode::Ptt => {
                VadOperationalMode::WindowedValidation
            }
        };
        if let Ok(guard) = state.engine.try_lock() {
            if let Some(ref engine) = *guard {
                if let Err(e) = engine.vad_tx.send(VadCommand::SetOperationalMode(vad_mode)) {
                    log::warn!(
                        "[Pipeline::Session] Failed to set VAD mode for dictation on pause: {}",
                        e
                    );
                }
            }
        }
    }

    transition(InteractionState::Paused, ctx, app, state);
    log::info!("[Pipeline::Session] Session paused");
}

/// Resumes a paused or error-state voice session, re-arming VAD and provider pipelines.
pub fn on_resume<R: tauri::Runtime>(app: &AppHandle<R>, state: &AppState, ctx: &RoutingContext) {
    let current_state = state.pipeline.state();
    if current_state != InteractionState::Paused && current_state != InteractionState::Error {
        log::warn!(
            "[Pipeline::Session] Cannot resume session: current state is {:?}, expected Paused or Error",
            current_state
        );
        return;
    }

    state
        .owner
        .store(InteractionOwner::Assistant as u32, Ordering::Relaxed);
    state.pipeline.cancel_flag.store(false, Ordering::Relaxed);
    state.pipeline.renew_turn_token();

    let resume_res = match ctx.pipeline_mode {
        PipelineMode::Modular => {
            let vad_mode = match ctx.interaction_mode {
                InteractionMode::Passive => VadOperationalMode::ContinuousSegmentation,
                InteractionMode::PTT => VadOperationalMode::WindowedValidation,
            };

            if let Ok(guard) = state.engine.try_lock() {
                if let Some(ref engine) = *guard {
                    if let Err(e) = engine.vad_tx.send(VadCommand::SetOperationalMode(vad_mode)) {
                        log::warn!(
                            "[Pipeline::Session] Failed to set VAD mode on resume: {}",
                            e
                        );
                    }
                }
            }
            Ok(())
        }
        PipelineMode::Realtime => resume_realtime(app, state, ctx),
    };

    if let Err(e) = resume_res {
        log::error!("[Pipeline::Session] Resumption failed: {}", e);
        transition(InteractionState::Error, ctx, app, state);
        let toast_msg = format!(
            "Resumption failed: {}. Please end session and start a new session.",
            e
        );
        if let Err(toast_err) = crate::toast::show_toast(
            app,
            "Resumption failed",
            &toast_msg,
            crate::core::events::ToastLevel::Error,
        ) {
            log::warn!(
                "[Pipeline::Session] Failed to show resume failure toast: {}",
                toast_err
            );
        }
        return;
    }

    transition(InteractionState::Ready, ctx, app, state);
    log::info!("[Pipeline::Session] Session resumed -> Ready");
}

/// Ends the active voice session, drains playback, flushes lifecycle events, and transitions to Idle.
pub fn on_end<R: tauri::Runtime>(app: &AppHandle<R>, state: &AppState, ctx: &RoutingContext) {
    let current_state = state.pipeline.state();
    if current_state == InteractionState::Idle {
        log::debug!("[Pipeline::Session] EndSession called while already Idle; no-op");
        return;
    }

    state.pipeline.cancel_flag.store(true, Ordering::Relaxed);
    state.pipeline.turn_token().cancel();
    state.pipeline_accumulator.lock().clear();

    if let Ok(guard) = state.engine.try_lock() {
        if let Some(ref engine) = *guard {
            engine.playback_engine.cancel();
            if ctx.pipeline_mode == PipelineMode::Realtime {
                if let Err(e) = engine.vad_tx.send(VadCommand::StopRealtime) {
                    log::warn!(
                        "[Pipeline::Session] Failed to send StopRealtime on end: {}",
                        e
                    );
                }
            }
        }
    }

    if ctx.pipeline_mode == PipelineMode::Realtime {
        let mut rt_guard = state.realtime_engine.blocking_lock();
        if let Some(mut rt_actor) = rt_guard.take() {
            rt_actor.stop();
        }
        crate::services::realtime::session::purge_session_cache();
    }

    let conv_id = state.conversation_id.load(Ordering::Relaxed);
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);

    let persist_lock = state.persist_tx.lock();
    if let Some(ref tx) = *persist_lock {
        if let Err(e) = tx.try_send(crate::persistence::events::PersistenceEvent::SessionEnded {
            id: conv_id,
            timestamp_ms: now,
        }) {
            log::warn!(
                "[Pipeline::Session] Failed to send SessionEnded to persistence: {}",
                e
            );
        }
    }

    let mem_lock = state.memory_tx.lock();
    if let Some(ref tx) = *mem_lock {
        if let Err(e) = tx.try_send(crate::persistence::events::MemoryWorkerEvent::SessionEnd {
            session_id: conv_id.to_string(),
            summary: String::new(),
        }) {
            log::trace!(
                "[Pipeline::Session] Failed to send SessionEnd to memory: {}",
                e
            );
        }
    }

    let dictation_enabled = state
        .settings
        .read()
        .map(|s| s.dictation.enabled)
        .unwrap_or(false);

    if dictation_enabled {
        state
            .owner
            .store(InteractionOwner::Dictation as u32, Ordering::Relaxed);
        let dictation_mode = state
            .settings
            .read()
            .map(|s| s.dictation.interaction_mode.clone())
            .unwrap_or(crate::core::settings::DictationInteractionMode::Ptt);
        let vad_mode = match dictation_mode {
            crate::core::settings::DictationInteractionMode::Passive => {
                VadOperationalMode::ContinuousSegmentation
            }
            crate::core::settings::DictationInteractionMode::Ptt => {
                VadOperationalMode::WindowedValidation
            }
        };
        if let Ok(guard) = state.engine.try_lock() {
            if let Some(ref engine) = *guard {
                if let Err(e) = engine.vad_tx.send(VadCommand::SetOperationalMode(vad_mode)) {
                    log::warn!(
                        "[Pipeline::Session] Failed to set VAD mode for dictation: {}",
                        e
                    );
                }
            }
        }
    } else {
        if let Err(e) = crate::core::stop_audio_engine_sync(state) {
            log::warn!("[Pipeline::Session] Error stopping audio engine: {}", e);
        }
    }

    transition(InteractionState::Idle, ctx, app, state);
    log::info!("[Pipeline::Session] Session ended -> Idle");

    // Check compaction for the completed session
    let auto_compaction = state
        .settings
        .read()
        .map(|s| s.history.auto_compaction)
        .unwrap_or(false);

    let app_handle = app.clone();
    let session_id = conv_id as i64;

    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;

        let db_path = crate::utils::paths::db_path();
        if let Ok(conn) = crate::persistence::db::VoxDb::open(&db_path).await {
            let last_compacted = match crate::persistence::compactions::fetch_latest_compaction_run(&conn, session_id).await {
                Ok(Some(run)) if run.status == "completed" => run.to_turn_id,
                _ => 0,
            };

            if let Ok(turns) = crate::persistence::compactions::fetch_turns_for_compaction(&conn, session_id, last_compacted).await {
                let uncompacted_count = turns.len() as u32;
                if uncompacted_count > 0 {
                    if auto_compaction {
                        use tauri::Manager;
                        let state_handle: tauri::State<'_, Arc<AppState>> = app_handle.state();
                        let app_state: &Arc<AppState> = state_handle.inner();
                        if let Err(e) = crate::services::memory::compaction::coordinator::CompactionCoordinator::run_compaction_slice(
                            &app_handle,
                            app_state,
                            session_id,
                            "auto",
                            None,
                        )
                        .await
                        {
                            log::warn!(
                                "[Pipeline::Session] Auto-compaction failed for session {}: {}",
                                session_id, e
                            );
                        }
                    } else if let Err(e) = crate::services::memory::compaction::coordinator::CompactionCoordinator::notify_uncompacted_session(
                        &app_handle,
                        session_id,
                        uncompacted_count,
                    )
                    .await
                    {
                        log::warn!(
                            "[Pipeline::Session] Failed to emit uncompacted notification for session {}: {}",
                            session_id, e
                        );
                    }
                }
            }
        }
    });
}
