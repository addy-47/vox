use std::{
    sync::{atomic::Ordering, Arc},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use tauri::{AppHandle, Manager};

use crate::{
    core::{
        engine::{ensure_modular_workers_sync, stop_audio_engine_sync},
        error::PipelineImpact,
        events::Severity,
        settings::{DictationInteractionMode, InteractionMode, PipelineMode},
        state::{AppState, InteractionOwner, InteractionState},
    },
    persistence::{
        compactions::{fetch_latest_compaction_run, fetch_turns_for_compaction},
        db::get_tokio_handle,
        personal_memory::get_personal_memory,
        sessions::fetch_session_continuation,
        PersistenceEvent,
    },
    pipeline::{spawn_idle_monitor, transition, RoutingContext},
    services::{
        self,
        harness::Harness,
        llm::actor::{cool_down_llm, LlmCommand},
        memory::{compaction::coordinator::CompactionCoordinator, trim_heap},
        notifications::{Action, ActionPayload, NotificationCategory, NotificationParams},
        realtime::{
            session::{create_realtime_provider, purge_session_cache},
            RealtimeActor,
        },
        tts::actor::cool_down_tts,
        vad::{VadCommand, VadOperationalMode},
    },
};

/// Configures and arms the modular speech-to-text, LLM, and TTS worker pipelines.
fn start_modular_session(state: &AppState, ctx: &RoutingContext) -> Result<(), String> {
    ensure_modular_workers_sync(state)?;

    let vad_mode = match ctx.interaction_mode {
        InteractionMode::Passive => VadOperationalMode::ContinuousSegmentation,
        InteractionMode::PTT => VadOperationalMode::WindowedValidation,
    };

    let prompt = state.resolve_base_prompt();

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
    session_id: Option<i64>,
    app: &AppHandle<R>,
    state: &AppState,
    _ctx: &RoutingContext,
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

    let session_ctx = RoutingContext::from_app_state(state);

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    let conv_id = session_id.map(|s| s as u64).unwrap_or(now);
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

    let start_res = match session_ctx.pipeline_mode {
        PipelineMode::Modular => start_modular_session(state, &session_ctx),
        PipelineMode::Realtime => start_realtime_session(app, state, &session_ctx),
    };

    if let Err(e) = start_res {
        log::error!("[Pipeline::Session] Session start failed: {}", e);
        transition(InteractionState::Error, &session_ctx, app, state);

        let app_handle = app.clone();
        let db = state.db.clone();
        let error_msg = format!("Session start failed: {}. Please check settings.", e);

        tauri::async_runtime::spawn(async move {
            let params = NotificationParams {
                category: NotificationCategory::Pipeline,
                severity: Severity::Critical,
                impact: Some(PipelineImpact::SessionHalted),
                action: Action::Interactive(ActionPayload::Navigate {
                    target: "settings/ai".to_string(),
                }),
                title: "Session Start Failed",
                message: &error_msg,
                group_key: Some("session:start_failed"),
                session_id: None,
                metadata: None,
                duration_ms: None,
            };
            if let Err(notify_err) = services::notifications::notify(&app_handle, &db, params).await
            {
                log::warn!(
                    "[Pipeline::Session] Failed to dispatch start failure notification: {}",
                    notify_err
                );
            }
        });
        return;
    }

    let settings = state
        .settings
        .read()
        .unwrap_or_else(|p| p.into_inner())
        .clone();
    let prompt = state.resolve_base_prompt();

    let tokio_handle = get_tokio_handle();
    let conn = state.db.connect().ok();
    let (personal_memory, summary, turns) =
        if let (Some(sid), Some(conn)) = (session_id, conn.as_ref()) {
            match tokio_handle.block_on(fetch_session_continuation(conn, sid)) {
                Ok(data) => (data.personal_memory, data.latest_summary, data.turns),
                Err(e) => {
                    log::warn!("[Pipeline::Session] Failed to fetch continuation: {}", e);
                    (None, None, Vec::new())
                }
            }
        } else if let Some(conn) = conn.as_ref() {
            let mem = tokio_handle
                .block_on(get_personal_memory(conn, None))
                .ok()
                .and_then(|r| {
                    if r.content.trim().is_empty() {
                        None
                    } else {
                        Some(r.content)
                    }
                });
            (mem, None, Vec::new())
        } else {
            (None, None, Vec::new())
        };

    match session_ctx.pipeline_mode {
        PipelineMode::Modular => {
            let llm_tx_opt = state
                .engine
                .try_lock()
                .ok()
                .and_then(|g| g.as_ref().and_then(|e| e.llm_tx.clone()));
            if let Some(llm_tx) = llm_tx_opt {
                let supports_tools = resolve_model_tool_support(app, state, &settings);
                let mut harness = Harness::new_modular(
                    session_id,
                    prompt,
                    personal_memory,
                    &settings,
                    llm_tx,
                    supports_tools,
                );
                if !turns.is_empty() || summary.is_some() {
                    harness.seed_continuation(summary, turns);
                }
                *state.harness.lock() = Some(harness);
            } else {
                log::warn!("[Pipeline::Session] No LLM tx available for Harness mount");
            }
        }
        PipelineMode::Realtime => {
            let mut harness = Harness::new_realtime(session_id, prompt, personal_memory, &settings);
            if !turns.is_empty() || summary.is_some() {
                harness.seed_continuation(summary, turns);
            }
            *state.harness.lock() = Some(harness);
        }
    }

    let state_arc: tauri::State<'_, Arc<AppState>> = app.state();
    spawn_idle_monitor(app.clone(), Arc::clone(state_arc.inner()));

    state.pipeline_accumulator.lock().clear();
    transition(InteractionState::Ready, &session_ctx, app, state);
    log::info!(
        "[Pipeline::Session] Session started (ID: {}, mode: {:?})",
        conv_id,
        session_ctx.pipeline_mode
    );
}

/// Pauses the active voice session, silencing audio output and placing the state machine in Paused.
pub fn on_pause<R: tauri::Runtime>(app: &AppHandle<R>, state: &AppState, ctx: &RoutingContext) {
    let current_state = state.pipeline.state();
    if current_state == InteractionState::Idle
        || current_state == InteractionState::Paused
        || current_state == InteractionState::Sleeping
    {
        log::debug!(
            "[Pipeline::Session] Pause dropped: already in {:?}",
            current_state
        );
        return;
    }

    state.pipeline.cancel_flag.store(true, Ordering::Relaxed);
    state.pipeline.turn_token().cancel();
    state.pipeline.reset_turn_guards();
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
pub fn on_resume<R: tauri::Runtime>(app: &AppHandle<R>, state: &AppState, _ctx: &RoutingContext) {
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

    let assistant_ctx = RoutingContext::from_app_state(state);

    let resume_res = match assistant_ctx.pipeline_mode {
        PipelineMode::Modular => {
            // Re-warm workers offloaded during sustained Paused/Sleeping so
            // resume never lands in Ready with dead LLM/TTS channels.
            if let Err(e) = ensure_modular_workers_sync(state) {
                Err(e)
            } else {
                let vad_mode = match assistant_ctx.interaction_mode {
                    InteractionMode::Passive => VadOperationalMode::ContinuousSegmentation,
                    InteractionMode::PTT => VadOperationalMode::WindowedValidation,
                };
                if let Ok(guard) = state.engine.try_lock() {
                    if let Some(ref engine) = *guard {
                        if let Err(e) = engine.vad_tx.send(VadCommand::SetOperationalMode(vad_mode))
                        {
                            log::warn!(
                                "[Pipeline::Session] Failed to set VAD mode on resume: {}",
                                e
                            );
                        }
                    }
                }
                Ok(())
            }
        }
        PipelineMode::Realtime => resume_realtime(app, state, &assistant_ctx),
    };

    if let Err(e) = resume_res {
        log::error!("[Pipeline::Session] Resumption failed: {}", e);
        transition(InteractionState::Error, &assistant_ctx, app, state);

        let app_handle = app.clone();
        let db = state.db.clone();
        let error_msg = format!(
            "Resumption failed: {}. Please check settings or start a new session.",
            e
        );

        tauri::async_runtime::spawn(async move {
            let params = NotificationParams {
                category: NotificationCategory::Pipeline,
                severity: Severity::Critical,
                impact: Some(PipelineImpact::SessionHalted),
                action: Action::Interactive(ActionPayload::Navigate {
                    target: "settings/ai".to_string(),
                }),
                title: "Resumption failed",
                message: &error_msg,
                group_key: Some("session:resume_failed"),
                session_id: None,
                metadata: None,
                duration_ms: None,
            };
            if let Err(notify_err) = services::notifications::notify(&app_handle, &db, params).await
            {
                log::warn!(
                    "[Pipeline::Session] Failed to dispatch resume failure notification: {}",
                    notify_err
                );
            }
        });
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
    state.pipeline.reset_turn_guards();
    state.pipeline_accumulator.lock().clear();
    state.harness.lock().take();

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

    // Stop CPAL engine only if dictation track is disabled (Idle).
    // Otherwise, dictation is active (Ready/Listening/Thinking): keep VAD + STT hot in memory,
    // switch VAD to dictation mode, and offload LLM and TTS since assistant session has ended.
    if state.pipeline.dictation_state() == InteractionState::Idle {
        if let Err(e) = stop_audio_engine_sync(state) {
            log::warn!("[Pipeline::Session] Error stopping audio engine: {}", e);
        }
    } else {
        if let Ok(mut guard) = state.engine.try_lock() {
            if let Some(ref mut engine) = *guard {
                cool_down_llm(&mut engine.llm_tx, Some(&state.llm_provider));
                cool_down_tts(&mut engine.tts_tx);
                let dictation_mode = state
                    .settings
                    .read()
                    .map(|s| s.dictation.interaction_mode.clone())
                    .unwrap_or(DictationInteractionMode::Ptt);
                let vad_op_mode = match dictation_mode {
                    DictationInteractionMode::Passive => VadOperationalMode::ContinuousSegmentation,
                    DictationInteractionMode::Ptt => VadOperationalMode::WindowedValidation,
                };
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
        trim_heap("session_end_dictation_standby");
    }

    transition(InteractionState::Idle, ctx, app, state);
    log::info!("[Pipeline::Session] Session ended -> Idle");

    // Check compaction for the completed session
    let auto_compaction = state
        .settings
        .read()
        .map(|s| s.working_memory.auto_compaction)
        .unwrap_or(false);

    let app_handle = app.clone();
    let session_id = conv_id as i64;
    let db = state.db.clone();

    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(Duration::from_millis(500)).await;

        let conn = match db.connect() {
            Ok(c) => c,
            Err(e) => {
                log::warn!(
                    "[Pipeline::Session] Failed to vend connection for post-session check: {}",
                    e
                );
                return;
            }
        };
        let last_compacted = match fetch_latest_compaction_run(&conn, session_id).await {
            Ok(Some(run)) if run.status == "completed" => run.to_turn_id,
            _ => 0,
        };

        if let Ok(turns) =
            fetch_turns_for_compaction(&conn, session_id, last_compacted, u32::MAX).await
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

/// Resolves tool calling capability for the active model from capability cache or 4s probe.
fn resolve_model_tool_support<R: tauri::Runtime + 'static>(
    app: &AppHandle<R>,
    state: &AppState,
    settings: &crate::core::settings::VoxSettings,
) -> bool {
    let active_model = settings.llm.active_model();
    let provider_kind = match settings.llm.active {
        crate::core::settings::LlmActiveProvider::Embedded => "embedded",
        crate::core::settings::LlmActiveProvider::Server => "server",
        crate::core::settings::LlmActiveProvider::Cloud => "cloud",
    };
    let key = format!("{}:{}", provider_kind, active_model);

    let cache_file = crate::paths::get().cache.join("model_capabilities.json");
    if cache_file.exists() {
        if let Ok(content) = std::fs::read_to_string(&cache_file) {
            if let Ok(map) = serde_json::from_str::<std::collections::HashMap<String, crate::core::settings::ModelCapabilities>>(&content) {
                if let Some(caps) = map.get(&key) {
                    log::info!(
                        "[Pipeline::Session] Cached capability for {}: supports_tools = {}",
                        key,
                        caps.supports_tools
                    );
                    if !caps.supports_tools {
                        emit_tool_unsupported_notification(app, state, active_model);
                    }
                    return caps.supports_tools;
                }
            }
        }
    }

    log::info!(
        "[Pipeline::Session] Model {} unprobed; spawning async background probe",
        key
    );
    let state_arc = app.state::<Arc<AppState>>().inner().clone();
    let app_handle = app.clone();
    let model_name = active_model.to_string();
    let is_cloud = provider_kind == "cloud";

    let tokio_handle = get_tokio_handle();
    tokio_handle.spawn(async move {
        let probe_res = tokio::time::timeout(
            Duration::from_secs(4),
            crate::services::llm::catalog::probe_capabilities(&state_arc, None, None, None),
        )
        .await;

        let supported = match probe_res {
            Ok(Ok(probe_result)) => {
                let s = probe_result.capabilities.supports_tools;
                log::info!(
                    "[Pipeline::Session] Background probe resolved for {}: supports_tools = {}",
                    key,
                    s
                );
                s
            }
            Ok(Err(err)) => {
                log::warn!(
                    "[Pipeline::Session] Background probe failed for {}: {}",
                    key,
                    err
                );
                false
            }
            Err(_) => {
                log::warn!(
                    "[Pipeline::Session] Background probe timed out (4s) for {}",
                    key
                );
                false
            }
        };

        let mut guard = state_arc.harness.lock();
        if let Some(ref mut harness) = *guard {
            harness.supports_tools = supported;
        }

        if !supported {
            emit_tool_unsupported_notification(&app_handle, &state_arc, &model_name);
        }
    });

    is_cloud
}

/// Dispatches a warning notification when tool calling is unavailable on the active model.
fn emit_tool_unsupported_notification<R: tauri::Runtime + 'static>(
    app: &AppHandle<R>,
    state: &AppState,
    model: &str,
) {
    let app_handle = app.clone();
    let db = state.db.clone();
    let model_name = model.to_string();

    tauri::async_runtime::spawn(async move {
        let msg = format!(
            "Active model '{}' does not support tool calling. Advanced agentic tools disabled.",
            model_name
        );
        let params = NotificationParams {
            category: NotificationCategory::Pipeline,
            severity: Severity::Warning,
            impact: Some(PipelineImpact::Degraded),
            action: Action::Interactive(ActionPayload::Navigate {
                target: "settings/ai".to_string(),
            }),
            title: "Tool Calling Unavailable",
            message: &msg,
            group_key: Some("model_tool_unsupported"),
            session_id: None,
            metadata: None,
            duration_ms: Some(5000),
        };
        if let Err(notify_err) = services::notifications::notify(&app_handle, &db, params).await {
            log::warn!(
                "[Pipeline::Session] Failed to dispatch tool unsupported notification: {}",
                notify_err
            );
        }
    });
}

