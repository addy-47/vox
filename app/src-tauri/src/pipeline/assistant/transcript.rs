use std::sync::Arc;

use tauri::{AppHandle, Manager};

use crate::{
    core::{
        error::PipelineImpact,
        events::{emit_ipc_to, IpcEvent, PipelineMode, Severity, TranscriptPayload},
        state::{AppState, InteractionState},
    },
    harness::{Harness, Role, TurnExecutionRequest, TurnOutcome},
    pipeline::{target_window, transition, RoutingContext},
    services::{
        self,
        notifications::{Action, NotificationCategory, NotificationParams},
        translit::transliterate_if_hi,
    },
};

/// Spawns the background asynchronous task to execute a modular conversational turn through the Harness.
pub(crate) fn spawn_harness_turn_task<R: tauri::Runtime + 'static>(
    turn_id: u32,
    query: String,
    app: &AppHandle<R>,
    state: &AppState,
    ctx: &RoutingContext,
) {
    let cancel = state.pipeline.turn_cancel_token();
    let stream_cancel = state.pipeline.stream_cancel_token();
    let pending_jobs = Arc::clone(&state.pipeline.pending_tts_jobs);
    let accumulator = Arc::clone(&state.pipeline_accumulator);
    let app_clone = app.clone();
    let ctx_clone = ctx.clone();
    let ctx_owner = ctx.owner;

    let app_state = match app.try_state::<Arc<AppState>>() {
        Some(st) => Arc::clone(st.inner()),
        None => {
            log::warn!(
                "[Pipeline::Transcript] AppState not managed on AppHandle; cannot spawn turn task"
            );
            return;
        }
    };
    let harness_arc = Arc::clone(&state.harness);
    let provider_arc = Arc::clone(&state.llm_provider);

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

        let req = TurnExecutionRequest {
            query,
            turn_id,
            cancel,
            stream_cancel,
            owner: ctx_owner,
            llm_tx,
            tts_tx,
            provider: provider_arc.read().clone(),
            db: Arc::clone(&app_state.db),
            pipeline_tx,
            accumulator,
            pending_synthesis_jobs: pending_jobs,
            emitter: Arc::new(app_clone.clone()),
            routing_ctx: ctx_clone.clone(),
            app_state: Arc::clone(&app_state),
        };

        let outcome = Harness::execute_turn(&harness_arc, req).await;

        match outcome {
            TurnOutcome::Completed {
                turn_id,
                assistant_response,
            } => {
                log::info!(
                    "[Pipeline::Transcript] Turn {} completed (chars {})",
                    turn_id,
                    assistant_response.len()
                );
                let mut guard = harness_arc.lock();
                if let Some(ref mut harness) = *guard {
                    harness.on_turn_completed(Arc::clone(&app_state), Arc::clone(&harness_arc));
                }
                if turn_id == app_state.pipeline.peek_turn_id() {
                    app_state
                        .pipeline_accumulator
                        .lock()
                        .continuation_prefix
                        .clear();
                }
            }
            TurnOutcome::DuplicateIgnored { turn_id } => {
                log::info!("[Pipeline::Transcript] Duplicate turn {} ignored", turn_id);
                if turn_id == app_state.pipeline.peek_turn_id() {
                    transition(
                        InteractionState::Ready,
                        None,
                        &ctx_clone,
                        &app_clone,
                        &app_state,
                    );
                }
            }
            TurnOutcome::Cancelled { turn_id } => {
                log::info!("[Pipeline::Transcript] Turn {} cancelled", turn_id);
                if turn_id == app_state.pipeline.peek_turn_id() {
                    let current = app_state.pipeline.state();
                    if current != InteractionState::Listening
                        && current != InteractionState::Idle
                        && current != InteractionState::Ready
                    {
                        transition(
                            InteractionState::Ready,
                            None,
                            &ctx_clone,
                            &app_clone,
                            &app_state,
                        );
                    }
                }
            }
            TurnOutcome::Error { turn_id, message } => {
                log::error!(
                    "[Pipeline::Transcript] Turn {} failed: {}",
                    turn_id,
                    message
                );
                if turn_id == app_state.pipeline.peek_turn_id() {
                    transition(
                        InteractionState::Ready,
                        None,
                        &ctx_clone,
                        &app_clone,
                        &app_state,
                    );
                }
            }
        }
    });
}

/// Handles finalized speech transcripts from the STT engine (Stage 2 of RULE-EVT-08).
pub fn on_transcript_final<R: tauri::Runtime>(
    turn_id: u32,
    text: String,
    app: &AppHandle<R>,
    state: &AppState,
    ctx: &RoutingContext,
) {
    log::info!(
        "[Pipeline::Transcript::Trace] assistant_final_entry turn={} owner={:?} target={} chars={}",
        turn_id,
        ctx.owner,
        target_window(ctx.owner),
        text.chars().count()
    );

    let current_state = state.pipeline.state();
    let trimmed = text.trim().to_string();
    let is_held = {
        let guard = state.harness.lock();
        guard
            .as_ref()
            .map(|h| h.inbox().is_speech_held())
            .unwrap_or(false)
    };

    if is_held {
        if trimmed.is_empty() {
            let has_continuation = !state.pipeline_accumulator.lock().continuation_prefix.is_empty();
            if !has_continuation {
                // False barge-in: resume the active turn without cancelling it.
                {
                    let guard = state.harness.lock();
                    if let Some(ref harness) = *guard {
                        harness.inbox().release_speech_hold();
                    }
                }
                state.pipeline.rearm_hardware_cancel_flag();

                if state.pipeline.is_llm_done() {
                    transition(InteractionState::Ready, None, ctx, app, state);
                } else if state.pipeline.current_activity().is_some() {
                    transition(InteractionState::Working, None, ctx, app, state);
                } else {
                    state.pipeline.rearm_stream_cancel_token();
                    transition(InteractionState::Thinking, None, ctx, app, state);
                }
                log::info!(
                    "[Pipeline::Transcript] False barge-in on turn {} released speech hold",
                    turn_id
                );
                let app_handle = app.clone();
                let db = state.db.clone();
                tauri::async_runtime::spawn(async move {
                    let params = NotificationParams {
                        category: NotificationCategory::Pipeline,
                        severity: Severity::Info,
                        impact: Some(PipelineImpact::None),
                        action: Action::Transient,
                        title: "Voice Assistant",
                        message: "Speech detected, but no words recognized.",
                        group_key: Some("assistant:empty_speech"),
                        session_id: None,
                        metadata: None,
                        duration_ms: None,
                    };
                    let _ = services::notifications::notify(&app_handle, &db, params).await;
                });
                return;
            }

            // Empty transcript promotes stashed continuation_prefix for turn_id without rotating.
            {
                let guard = state.harness.lock();
                if let Some(ref harness) = *guard {
                    harness.inbox().release_speech_hold();
                }
            }
            let raw_query = {
                let mut acc = state.pipeline_accumulator.lock();
                let merged = acc.resolve_merged_transcript("");
                acc.continuation_prefix.clear();
                acc.awaiting_continuation = false;
                acc.bound_utterance_id = None;
                merged
            };
            let transliterate_enabled = state
                .settings
                .read()
                .map(|s| s.stt.transliterate_enabled)
                .unwrap_or(false);
            let query = transliterate_if_hi(&raw_query, true, transliterate_enabled);

            state.pipeline_accumulator.lock().clear();
            state
                .pipeline_accumulator
                .lock()
                .set_user_transcript(query.clone());
            state.turn_metrics.start_turn(turn_id);
            state.turn_metrics.record_transcript_final();
            let payload = TranscriptPayload {
                turn_id,
                text: query.clone(),
                owner: Some(ctx.owner),
            };
            let _ = emit_ipc_to(
                app,
                target_window(ctx.owner),
                IpcEvent::TranscriptFinal(payload),
            );
            transition(InteractionState::Thinking, None, ctx, app, state);
            spawn_harness_turn_task(turn_id, query, app, state, ctx);
            return;
        }

        // Real barge-in during an active turn. Voice never enters StepInbox.
        let committed_work = {
            let guard = state.harness.lock();
            guard
                .as_ref()
                .map(|h| {
                    h.messages()
                        .iter()
                        .any(|m| m.role == Role::Tool || m.tool_calls.is_some())
                })
                .unwrap_or(false)
        };
        let staged = {
            let mut guard = state.harness.lock();
            guard
                .as_mut()
                .and_then(|h| h.take_staged_query(turn_id))
        };

        // A final for a segment whose query is still in flight is stashed, not dispatched.
        let has_stashed_text = {
            let acc = state.pipeline_accumulator.lock();
            !acc.user_transcript.trim().is_empty() || !acc.continuation_prefix.is_empty()
        };
        if !committed_work && staged.is_none() && !has_stashed_text {
            state
                .pipeline_accumulator
                .lock()
                .append_continuation_prefix(&trimmed);
            return;
        }

        // RULE-EVT-08A merges only an uncommitted Step 0 query; Step K >= 1 stays standalone.
        let raw_query = if committed_work {
            trimmed.clone()
        } else if let Some(prefix) = staged.filter(|p| !p.trim().is_empty()) {
            format!("{} {}", prefix.trim(), trimmed)
        } else {
            let mut acc = state.pipeline_accumulator.lock();
            let merged = acc.resolve_merged_transcript(&trimmed);
            acc.continuation_prefix.clear();
            acc.awaiting_continuation = false;
            acc.bound_utterance_id = None;
            merged
        };

        let transliterate_enabled = state
            .settings
            .read()
            .map(|s| s.stt.transliterate_enabled)
            .unwrap_or(false);
        let query = transliterate_if_hi(&raw_query, true, transliterate_enabled);

        // Non-empty transcript on barge-in cancels the prior turn and advances to a new dialogue turn (RULE-EVT-08A / RULE-EVT-08 §2b).
        state.pipeline.cancel_active_turn();
        {
            let guard = state.harness.lock();
            if let Some(ref harness) = *guard {
                harness.inbox().release_speech_hold();
            }
        }
        let (new_turn_id, _) = state.pipeline.next_turn();
        state.pipeline_accumulator.lock().clear();
        state
            .pipeline_accumulator
            .lock()
            .set_user_transcript(query.clone());
        state.turn_metrics.start_turn(new_turn_id);
        state.turn_metrics.record_transcript_final();
        let payload = TranscriptPayload {
            turn_id: new_turn_id,
            text: query.clone(),
            owner: Some(ctx.owner),
        };
        let _ = emit_ipc_to(
            app,
            target_window(ctx.owner),
            IpcEvent::TranscriptFinal(payload),
        );
        transition(InteractionState::Thinking, None, ctx, app, state);
        spawn_harness_turn_task(new_turn_id, query, app, state, ctx);
        return;
    }

    // Late final from an older physical speech segment (RULE-EVT-01A).
    if current_state == InteractionState::Listening {
        let is_late = {
            let acc = state.pipeline_accumulator.lock();
            acc.awaiting_continuation
                && acc
                    .bound_utterance_id
                    .map(|bound| bound < state.pipeline.peek_utterance_id())
                    .unwrap_or(false)
        };
        if is_late {
            state
                .pipeline_accumulator
                .lock()
                .append_continuation_prefix(&text);
            log::info!(
                "[Pipeline::Transcript] Captured late final for bound utterance while in Listening"
            );
            return;
        }
    }

    if current_state != InteractionState::Thinking {
        log::debug!(
            "[Pipeline::Transcript] Transcript dropped: state is {:?}, expected Thinking",
            current_state
        );
        return;
    }

    let mut acc = state.pipeline_accumulator.lock();
    let raw_query = if trimmed.is_empty() {
        if !acc.continuation_prefix.is_empty() {
            log::info!(
                "[Pipeline::Transcript] Empty transcript for turn {}; promoting stashed continuation_prefix: '{}'",
                turn_id,
                acc.continuation_prefix
            );
            acc.resolve_merged_transcript("")
        } else {
            log::debug!(
                "[Pipeline::Transcript] Dropping empty transcript for turn {}",
                turn_id
            );
            acc.clear();
            drop(acc);
            transition(InteractionState::Ready, None, ctx, app, state);
            let app_handle = app.clone();
            let db = state.db.clone();
            tauri::async_runtime::spawn(async move {
                let params = NotificationParams {
                    category: NotificationCategory::Pipeline,
                    severity: Severity::Info,
                    impact: Some(PipelineImpact::None),
                    action: Action::Transient,
                    title: "Voice Assistant",
                    message: "Speech detected, but no words recognized.",
                    group_key: Some("assistant:empty_speech"),
                    session_id: None,
                    metadata: None,
                    duration_ms: None,
                };
                let _ = services::notifications::notify(&app_handle, &db, params).await;
            });
            return;
        }
    } else {
        acc.resolve_merged_transcript(&trimmed)
    };
    drop(acc);

    let transliterate_enabled = state
        .settings
        .read()
        .map(|s| s.stt.transliterate_enabled)
        .unwrap_or(false);
    let query = transliterate_if_hi(&raw_query, true, transliterate_enabled);

    if ctx.pipeline_mode == PipelineMode::Modular && is_duplicate_user_turn(state, &query) {
        log::info!(
            "[Pipeline::Transcript] Duplicate turn {} suppressed pre-dispatch ('{}')",
            turn_id,
            query
        );
        state.pipeline_accumulator.lock().clear();
        if turn_id == state.pipeline.peek_turn_id() {
            transition(InteractionState::Ready, None, ctx, app, state);
        }
        return;
    }

    log::info!(
        "[Pipeline::Transcript] Turn {}: User said: '{}'",
        turn_id,
        query
    );
    state
        .pipeline_accumulator
        .lock()
        .set_user_transcript(query.clone());
    state.turn_metrics.record_transcript_final();
    let payload = TranscriptPayload {
        turn_id,
        text: query.clone(),
        owner: Some(ctx.owner),
    };
    let _ = emit_ipc_to(
        app,
        target_window(ctx.owner),
        IpcEvent::TranscriptFinal(payload),
    );
    transition(InteractionState::Thinking, None, ctx, app, state);

    match ctx.pipeline_mode {
        PipelineMode::Modular => spawn_harness_turn_task(turn_id, query, app, state, ctx),
        PipelineMode::Realtime => {
            log::info!(
                "[Pipeline::Transcript] Turn {} voice transcript processed in Realtime mode",
                turn_id
            );
        }
    }
}

/// Returns true when the query matches the most recent user turn recorded in Harness history.
pub(super) fn is_duplicate_user_turn(state: &AppState, query: &str) -> bool {
    let guard = state.harness.lock();
    guard
        .as_ref()
        .map(|harness| harness.history.is_duplicate_user_turn(query))
        .unwrap_or(false)
}
