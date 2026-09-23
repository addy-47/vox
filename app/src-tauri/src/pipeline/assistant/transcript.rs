use std::sync::Arc;

use tauri::{AppHandle, Manager};

use crate::{
    core::{
        error::PipelineImpact,
        events::{emit_ipc_to, IpcEvent, Severity, TranscriptPayload},
        settings::PipelineMode,
        state::{AppState, InteractionState},
    },
    pipeline::{target_window, transition, RoutingContext},
    services::{
        self,
        harness::{Harness, TurnExecutionRequest, TurnOutcome},
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
    let cancel = state.pipeline.turn_token();
    let pending_jobs = Arc::clone(&state.pipeline.pending_synthesis_jobs);
    let accumulator = Arc::clone(&state.pipeline_accumulator);
    let app_clone = app.clone();
    let ctx_clone = ctx.clone();
    let ctx_owner = ctx.owner;

    let app_state_arc: tauri::State<'_, Arc<AppState>> = app.state();
    let app_state = Arc::clone(app_state_arc.inner());
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
            owner: ctx_owner,
            llm_tx,
            tts_tx,
            provider: provider_arc.read().clone(),
            db: Arc::clone(&app_state.db),
            pipeline_tx,
            accumulator,
            pending_synthesis_jobs: pending_jobs,
            app: app_clone.clone(),
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
            }
            TurnOutcome::DuplicateIgnored { turn_id } => {
                log::info!("[Pipeline::Transcript] Duplicate turn {} ignored", turn_id);
                app_state.pipeline.reset_turn_guards();
                transition(InteractionState::Ready, &ctx_clone, &app_clone, &app_state);
            }
            TurnOutcome::Cancelled { turn_id } => {
                log::info!("[Pipeline::Transcript] Turn {} cancelled", turn_id);
                app_state.pipeline.reset_turn_guards();
                transition(InteractionState::Ready, &ctx_clone, &app_clone, &app_state);
            }
            TurnOutcome::Error { turn_id, message } => {
                log::error!(
                    "[Pipeline::Transcript] Turn {} failed: {}",
                    turn_id,
                    message
                );
                app_state.pipeline.reset_turn_guards();
                transition(InteractionState::Ready, &ctx_clone, &app_clone, &app_state);
            }
        }
    });
}

/// Handles finalized speech transcripts from the STT engine, validating text and routing to the Harness.
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
            if let Err(e) = services::notifications::notify(&app_handle, &db, params).await {
                log::warn!(
                    "[Pipeline::Transcript] Failed to dispatch notification: {}",
                    e
                );
            }
        });
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

    state.turn_metrics.record_transcript_final();

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

    state.pipeline.set_turn_open(true);
    state.pipeline.clear_drained_while_open();
    transition(InteractionState::Thinking, ctx, app, state);

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
