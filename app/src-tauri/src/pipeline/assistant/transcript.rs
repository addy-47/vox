use std::sync::{atomic::Ordering, Arc};

use tauri::{AppHandle, Manager};

use crate::{
    core::{
        events::{emit_ipc_to, AudioIntent, IpcEvent, ToastLevel, TranscriptPayload},
        settings::PipelineMode,
        state::{AppState, InteractionState},
    },
    pipeline::{target_window, transition, RoutingContext},
    toast::show_toast,
    services::{
        harness::{
            CompactionParams, CompactionPlugin, StreamRoutingHandles,
            TurnPreparation,
        },
        llm::actor::LlmCommand,
        translit::transliterate_if_hi,
        tts::actor::TtsCommand,
    },
};

/// Spawns the background asynchronous task to prepare conversational context and trigger LLM generation.
fn spawn_modular_llm_task<R: tauri::Runtime + 'static>(
    turn_id: u32,
    query: String,
    app: &AppHandle<R>,
    state: &AppState,
    ctx: &RoutingContext,
) {
    let settings = state
        .settings
        .read()
        .unwrap_or_else(|p| p.into_inner())
        .clone();
    let cancel = state.pipeline.turn_token();
    let cancel_flag = Arc::clone(&state.pipeline.cancel_flag);
    let pending_jobs = Arc::clone(&state.pipeline.pending_synthesis_jobs);
    let accumulator = Arc::clone(&state.pipeline_accumulator);
    let app_clone = app.clone();
    let ctx_clone = ctx.clone();
    let ctx_owner = ctx.owner;

    let app_state_arc: tauri::State<'_, Arc<AppState>> = app.state();
    let app_state = Arc::clone(app_state_arc.inner());
    let harness_arc = Arc::clone(&state.harness);
    let provider_arc = Arc::clone(&state.llm_provider);
    let session_id = state.conversation_id.load(Ordering::Relaxed) as i64;

    tauri::async_runtime::spawn(async move {
        let (tts_tx, llm_tx, pipeline_tx) = {
            let guard = app_state.engine.lock().await;
            guard
                .as_ref()
                .map(|e| {
                    (
                        e.tts_tx.clone(),
                        e.llm_tx.clone(),
                        Some(e.pipeline_tx.clone()),
                    )
                })
                .unwrap_or((None, None, None))
        };

        let prep = {
            let mut guard = harness_arc.lock();
            let Some(ref mut harness) = *guard else {
                log::error!("[Pipeline::Transcript] No active HarnessSession mounted");
                return;
            };
            harness.prepare_turn(&query, turn_id)
        };

        let request = match prep {
            TurnPreparation::DuplicateTurnIgnored => {
                log::info!(
                    "[Pipeline::Transcript] Duplicate turn ignored (turn {})",
                    turn_id
                );
                return;
            }
            TurnPreparation::Ready(req) => req,
            TurnPreparation::NeedsInlineCompaction {
                filler_phrase,
                uncompacted_slice,
            } => {
                log::info!(
                    "[Pipeline::Transcript] Context threshold >= 85%. Transitioning to Working."
                );
                transition(InteractionState::Working, &ctx_clone, &app_clone, &app_state);

                if let Some(ref t_tx) = tts_tx {
                    pending_jobs.fetch_add(1, Ordering::Relaxed);
                    if let Err(e) = t_tx.send(TtsCommand::Generate {
                        turn_id,
                        text: filler_phrase.to_string(),
                        intent: AudioIntent::InterimFiller,
                    }) {
                        log::warn!(
                            "[Pipeline::Transcript] Failed to dispatch filler to TTS: {}",
                            e
                        );
                    }
                }

                let from_turn = {
                    let guard = harness_arc.lock();
                    guard.as_ref().map(|h| h.from_turn_id()).unwrap_or(0)
                };

                let provider_opt = provider_arc.read().clone();
                if let Some(provider) = provider_opt {
                    let params = CompactionParams {
                        session_id,
                        trigger_kind: "inline",
                        from_turn_id: from_turn,
                        to_turn_id: turn_id,
                        history_messages: &uncompacted_slice,
                        llm_settings: Some(&settings.llm),
                        cancel: Some(&cancel),
                    };

                    let compaction_res = CompactionPlugin::run_and_persist(
                        provider.as_ref(),
                        &app_state.db,
                        params,
                    )
                    .await;

                    let mut guard = harness_arc.lock();
                    if let Some(ref mut harness) = *guard {
                        match compaction_res {
                            Ok(result) => {
                                harness.apply_compaction_summary(&result.session_context, &query);
                                harness.set_last_compacted_to_turn(turn_id);
                            }
                            Err(e) => {
                                log::warn!("[Pipeline::Transcript] Compaction error ({}). Falling back to FIFO.", e);
                                harness.fallback_fifo_shift();
                            }
                        }
                    }
                }

                let guard = harness_arc.lock();
                let Some(ref harness) = *guard else {
                    return;
                };
                harness.create_generation_request()
            }
        };

        if cancel.is_cancelled() {
            log::info!(
                "[Pipeline::Transcript] Turn {} cancelled before LLM dispatch",
                turn_id
            );
            return;
        }

        let (response_tx, response_rx) = std::sync::mpsc::channel();

        if let Some(ref tx) = llm_tx {
            if let Err(e) = tx.send(LlmCommand::Generate {
                request: Box::new(request),
                turn_id,
                cancel,
                response_tx,
            }) {
                log::warn!(
                    "[Pipeline::Transcript] Failed to send Generate to LLM: {}",
                    e
                );
                return;
            }
        }

        if let Some(p_tx) = pipeline_tx {
            let stream_plugin_snapshot = {
                let guard = harness_arc.lock();
                guard.as_ref().map(|h| h.clone_stream_plugin())
            };
            let Some(stream_plugin) = stream_plugin_snapshot else {
                return;
            };
            let handles = StreamRoutingHandles {
                turn_id,
                owner: ctx_owner,
                accumulator,
                tts_tx,
                pending_synthesis_jobs: pending_jobs,
                cancel: cancel_flag,
                event_tx: p_tx,
                app: app_clone,
            };
            let stream_result = tokio::task::spawn_blocking(move || {
                stream_plugin.route_stream(handles, response_rx)
            })
            .await;

            match stream_result {
                Ok(Ok(full_text)) => {
                    let mut guard = harness_arc.lock();
                    if let Some(ref mut harness) = *guard {
                        if !full_text.trim().is_empty() {
                            harness.commit_turn(full_text);
                        } else {
                            harness.rollback_user_turn();
                        }
                        harness.on_turn_completed(app_state, Arc::clone(&harness_arc));
                    }
                }
                Ok(Err(e)) => {
                    log::warn!("[Pipeline::Transcript] Stream routing failed: {}", e);
                    let mut guard = harness_arc.lock();
                    if let Some(ref mut harness) = *guard {
                        harness.rollback_user_turn();
                    }
                }
                Err(e) => {
                    log::warn!("[Pipeline::Transcript] Stream routing task join error: {}", e);
                    let mut guard = harness_arc.lock();
                    if let Some(ref mut harness) = *guard {
                        harness.rollback_user_turn();
                    }
                }
            }
        }
    });
}

/// Handles finalized speech transcript, validating non-empty text and routing to LLM or idle recovery.
pub fn on_transcript_final<R: tauri::Runtime>(
    turn_id: u32,
    text: String,
    app: &AppHandle<R>,
    state: &AppState,
    ctx: &RoutingContext,
) {
    let current_state = state.pipeline.state();
    if current_state != InteractionState::Thinking {
        log::debug!(
            "[Pipeline::Transcript] Transcript dropped: state is {:?}, expected Thinking",
            current_state
        );
        return;
    }

    let trimmed = text.trim().to_string();
    if trimmed.is_empty() {
        log::debug!(
            "[Pipeline::Transcript] Dropping empty transcript for turn {}",
            turn_id
        );
        state.pipeline_accumulator.lock().clear();
        transition(InteractionState::Ready, ctx, app, state);
        if let Err(e) = show_toast(
            app,
            "Voice Assistant",
            "No speech recognized",
            ToastLevel::Info,
        ) {
            log::warn!("[Pipeline::Transcript] Failed to show info toast: {}", e);
        }
        return;
    }

    let transliterate_enabled = state
        .settings
        .read()
        .map(|s| s.stt.transliterate_enabled)
        .unwrap_or(false);
    let query = transliterate_if_hi(&trimmed, true, transliterate_enabled);
    log::info!(
        "[Pipeline::Transcript] Turn {}: User said: '{}'",
        turn_id,
        query
    );

    state
        .pipeline_accumulator
        .lock()
        .set_user_transcript(query.clone());

    let payload = TranscriptPayload {
        turn_id,
        text: query.clone(),
        owner: Some(ctx.owner),
    };
    if let Err(e) = emit_ipc_to(
        app,
        target_window(ctx.owner),
        IpcEvent::TranscriptFinal(payload),
    ) {
        log::warn!(
            "[Pipeline::Transcript] Failed to emit TranscriptFinal IPC: {}",
            e
        );
    }

    transition(InteractionState::Thinking, ctx, app, state);

    match ctx.pipeline_mode {
        PipelineMode::Modular => spawn_modular_llm_task(turn_id, query, app, state, ctx),
        PipelineMode::Realtime => {
            log::info!(
                "[Pipeline::Transcript] Turn {} transcript processed in Realtime mode",
                turn_id
            );
        }
    }
}
