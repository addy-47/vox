use std::{
    sync::{atomic::Ordering, Arc},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use tauri::{AppHandle, Manager};

use crate::{
    core::{
        engine::ensure_modular_workers_sync,
        events::ToastLevel,
        settings::{DictationInteractionMode, InteractionMode, PipelineMode},
        state::{AppState, InteractionOwner, InteractionState},
        engine::stop_audio_engine_sync,
    },
    persistence::{
        compactions::{fetch_latest_compaction_run, fetch_turns_for_compaction},
        db::get_tokio_handle,
        PersistenceEvent,
    },
    pipeline::{init_new_session_sync, spawn_idle_monitor, transition, RoutingContext},
    services::{
        llm::actor::LlmCommand,
        memory::compaction::coordinator::CompactionCoordinator,
        realtime::{
            session::{create_realtime_provider, purge_session_cache},
            RealtimeActor,
        },
        vad::{VadCommand, VadOperationalMode},
    },
    toast::show_toast,
};

/// Configures and arms the modular speech-to-text, LLM, and TTS worker pipelines.
fn start_modular_session<R: tauri::Runtime + 'static>(
    app: &AppHandle<R>,
    state: &AppState,
    ctx: &RoutingContext,
) -> Result<(), String> {
    ensure_modular_workers_sync(app, state)?;

    let vad_mode = match ctx.interaction_mode {
        InteractionMode::Passive => VadOperationalMode::ContinuousSegmentation,
        InteractionMode::PTT => VadOperationalMode::WindowedValidation,
    };

    let prompt = {
        let settings = state.settings.read().unwrap_or_else(|p| p.into_inner());
        settings.persona.modular_prompt.clone()
    };

    if let Ok(guard) = state.engine.try_lock() {
        if let Some(ref engine) = *guard {
            if let Err(e) = engine.vad_tx.send(VadCommand::SetOperationalMode(vad_mode)) {
                log::warn!(
                    "[Pipeline::Session] Failed to set VAD operational mode: {}",
                    e
                );
            }
            if let Some(ref llm_tx) = engine.llm_tx {
                if let Err(e) = llm_tx.send(LlmCommand::Warmup {
                    system_prompt: prompt,
                }) {
                    log::warn!("[Pipeline::Session] Failed to dispatch LLM Warmup: {}", e);
                }
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

    let provider = create_realtime_provider(state)?;
    let tokio_handle = get_tokio_handle();
    let mut rt_actor = RealtimeActor::new(provider, tokio_handle);

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
        rt_actor.stop();
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
        if let Err(e) = tx.try_send(PersistenceEvent::SessionStarted {
            session_id: conv_id as i64,
            timestamp_ms: now,
        }) {
            log::warn!(
                "[Pipeline::Session] Failed to send SessionStarted to persistence: {}",
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

    init_new_session_sync(state, &prompt);

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

    // Unconditionally yield owner to Dictation without reading settings for ownership.
    // dictation_state (Ready vs Idle) governs whether dictation actually reacts.
    state
        .owner
        .store(InteractionOwner::Dictation as u32, Ordering::Relaxed);

    // Sync VAD operational mode to Dictation's configured interaction mode (Passive vs PTT)
    let dictation_mode = state
        .settings
        .read()
        .map(|s| s.dictation.interaction_mode.clone())
        .unwrap_or(DictationInteractionMode::Ptt);
    let vad_op_mode = match dictation_mode {
        DictationInteractionMode::Passive => VadOperationalMode::ContinuousSegmentation,
        DictationInteractionMode::Ptt => VadOperationalMode::WindowedValidation,
    };
    if let Ok(guard) = state.engine.try_lock() {
        if let Some(ref engine) = *guard {
            if let Err(e) = engine
                .vad_tx
                .send(VadCommand::SetOperationalMode(vad_op_mode))
            {
                log::warn!(
                    "[Pipeline::Session] Failed to set VAD operational mode for dictation on pause: {}",
                    e
                );
            }
        }
    }

    let assistant_ctx = RoutingContext {
        owner: InteractionOwner::Assistant,
        ..ctx.clone()
    };
    transition(InteractionState::Paused, &assistant_ctx, app, state);
    log::info!("[Pipeline::Session] Session paused");
}

/// Resumes a paused, sleeping, or error-state voice session, re-arming VAD and provider pipelines.
pub fn on_resume<R: tauri::Runtime>(app: &AppHandle<R>, state: &AppState, ctx: &RoutingContext) {
    let current_state = state.pipeline.state();
    if current_state != InteractionState::Paused
        && current_state != InteractionState::Sleeping
        && current_state != InteractionState::Error
    {
        log::warn!(
            "[Pipeline::Session] Cannot resume session: current state is {:?}, expected Paused, Sleeping, or Error",
            current_state
        );
        return;
    }

    state
        .owner
        .store(InteractionOwner::Assistant as u32, Ordering::Relaxed);
    state.pipeline.cancel_flag.store(false, Ordering::Relaxed);
    state.pipeline.rearm_turn_token();

    let assistant_ctx = RoutingContext {
        owner: InteractionOwner::Assistant,
        ..ctx.clone()
    };

    let resume_res = match assistant_ctx.pipeline_mode {
        PipelineMode::Modular => {
            let vad_mode = match assistant_ctx.interaction_mode {
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
        PipelineMode::Realtime => resume_realtime(app, state, &assistant_ctx),
    };

    if let Err(e) = resume_res {
        log::error!("[Pipeline::Session] Resumption failed: {}", e);
        transition(InteractionState::Error, &assistant_ctx, app, state);
        let toast_msg = format!(
            "Resumption failed: {}. Please end session and start a new session.",
            e
        );
        if let Err(toast_err) = show_toast(app, "Resumption failed", &toast_msg, ToastLevel::Error)
        {
            log::warn!(
                "[Pipeline::Session] Failed to show resume failure toast: {}",
                toast_err
            );
        }
        return;
    }

    transition(InteractionState::Ready, &assistant_ctx, app, state);
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
        if let Ok(mut rt_guard) = state.realtime_engine.try_lock() {
            if let Some(mut rt_actor) = rt_guard.take() {
                rt_actor.stop();
            }
        }
        purge_session_cache();
    }

    let conv_id = state.conversation_id.load(Ordering::Relaxed);
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);

    {
        let persist_lock = state.persist_tx.lock();
        if let Some(ref tx) = *persist_lock {
            if let Err(e) = tx.try_send(PersistenceEvent::SessionEnded {
                session_id: conv_id as i64,
                timestamp_ms: now,
            }) {
                log::warn!(
                    "[Pipeline::Session] Failed to send SessionEnded to persistence: {}",
                    e
                );
            }
        }
    }

    // Unconditionally yield owner to Dictation.
    state
        .owner
        .store(InteractionOwner::Dictation as u32, Ordering::Relaxed);

    // Stop CPAL engine only if dictation is also disabled, otherwise switch VAD to dictation mode.
    if state.pipeline.dictation_state() == InteractionState::Idle {
        if let Err(e) = stop_audio_engine_sync(state) {
            log::warn!("[Pipeline::Session] Error stopping audio engine: {}", e);
        }
    } else {
        let dictation_mode = state
            .settings
            .read()
            .map(|s| s.dictation.interaction_mode.clone())
            .unwrap_or(DictationInteractionMode::Ptt);
        let vad_op_mode = match dictation_mode {
            DictationInteractionMode::Passive => VadOperationalMode::ContinuousSegmentation,
            DictationInteractionMode::Ptt => VadOperationalMode::WindowedValidation,
        };
        if let Ok(guard) = state.engine.try_lock() {
            if let Some(ref engine) = *guard {
                if let Err(e) = engine
                    .vad_tx
                    .send(VadCommand::SetOperationalMode(vad_op_mode))
                {
                    log::warn!(
                        "[Pipeline::Session] Failed to set VAD operational mode for dictation on session end: {}",
                        e
                    );
                }
            }
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
    let db = state.db.clone();

    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(Duration::from_millis(500)).await;

        let conn = &db;
        let last_compacted = match fetch_latest_compaction_run(conn, session_id).await {
            Ok(Some(run)) if run.status == "completed" => run.to_turn_id,
            _ => 0,
        };

        if let Ok(turns) =
            fetch_turns_for_compaction(conn, session_id, last_compacted, u32::MAX).await
        {
            let uncompacted_count = turns.len() as u32;
            if uncompacted_count > 0 {
                if auto_compaction {
                    use tauri::Manager;
                    let state_handle: tauri::State<'_, Arc<AppState>> = app_handle.state();
                    let app_state: &Arc<AppState> = state_handle.inner();
                    if let Err(e) = CompactionCoordinator::notify_uncompacted_session(
                        &app_handle,
                        &db,
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
                    if let Err(e) = CompactionCoordinator::run_compaction_slice(
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
                            session_id,
                            e
                        );
                    }
                } else if let Err(e) = CompactionCoordinator::notify_uncompacted_session(
                    &app_handle,
                    &db,
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
    });
}
