use tauri::AppHandle;

use crate::{
    core::{
        events::{emit_ipc_to, IpcEvent, PipelineMode, TranscriptPayload},
        state::{AppState, InteractionState},
    },
    pipeline::{target_window, transition, RoutingContext},
    services::translit::transliterate_if_hi,
};

/// Orchestrates typed user query submission into modular or realtime conversational pipelines.
pub fn on_text_input<R: tauri::Runtime + 'static>(
    text: String,
    app: &AppHandle<R>,
    state: &AppState,
    ctx: &RoutingContext,
) {
    let current_state = state.pipeline.state();
    if current_state == InteractionState::Idle {
        log::debug!("[Pipeline::Text] TextInput dropped in {:?}", current_state);
        return;
    }

    let trimmed = text.trim().to_string();
    if trimmed.is_empty() {
        log::debug!("[Pipeline::Text] Dropping empty text input");
        return;
    }

    let transliterate_enabled = state
        .settings
        .read()
        .map(|s| s.stt.transliterate_enabled)
        .unwrap_or(false);
    let query = transliterate_if_hi(&trimmed, true, transliterate_enabled);

    // RULE-HARN-03: reject duplicate typed turns before allocating a turn ID, interrupting the
    // active turn, or emitting TranscriptFinal so ignored duplicates neither advance turn
    // counters nor render orphan UI bubbles.
    if ctx.pipeline_mode == PipelineMode::Modular
        && super::transcript::is_duplicate_user_turn(state, &query)
    {
        log::info!(
            "[Pipeline::Text] Duplicate typed turn suppressed pre-dispatch ('{}')",
            query
        );
        return;
    }

    // Auto-resume so typed input is never silently dropped while paused or sleeping.
    if current_state == InteractionState::Paused || current_state == InteractionState::Sleeping {
        log::info!(
            "[Pipeline::Text] TextInput while {:?}: auto-resuming before dispatch",
            current_state
        );
        super::session::on_resume(app, state, ctx);
    }

    let current_state = state.pipeline.state();
    let is_speech_held = state
        .harness
        .lock()
        .as_ref()
        .map(|h| h.inbox().is_speech_held())
        .unwrap_or(false);

    let active_turn_id = if matches!(
        current_state,
        InteractionState::Thinking | InteractionState::Speaking | InteractionState::Working
    ) || is_speech_held
    {
        // Typed input is semantically resolved on ingress, so rotate the turn explicitly.
        {
            let guard = state.harness.lock();
            if let Some(ref harness) = *guard {
                harness.inbox().release_speech_hold();
            }
        }
        state.pipeline.cancel_active_turn();
        let (new_turn_id, _) = state.pipeline.next_turn();
        state.pipeline_accumulator.lock().clear();
        new_turn_id
    } else if current_state == InteractionState::Ready {
        let (new_turn_id, _) = state.pipeline.next_turn();
        state.pipeline_accumulator.lock().clear();
        state.turn_metrics.start_turn(new_turn_id);
        state.turn_metrics.record_speech_end();
        new_turn_id
    } else {
        return;
    };

    log::info!(
        "[Pipeline::Text] Turn {}: User typed: '{}'",
        active_turn_id,
        query
    );

    // Stage the query on the router thread before the background task is spawned.
    {
        let mut guard = state.harness.lock();
        if let Some(ref mut harness) = *guard {
            harness.stage_user_query(active_turn_id, query.clone());
        }
    }
    state
        .pipeline_accumulator
        .lock()
        .set_user_transcript(query.clone());
    state.turn_metrics.record_transcript_final();

    let payload = TranscriptPayload {
        turn_id: active_turn_id,
        text: query.clone(),
        owner: Some(ctx.owner),
    };
    if let Err(e) = emit_ipc_to(
        app,
        target_window(ctx.owner),
        IpcEvent::TranscriptFinal(payload),
    ) {
        log::warn!("[Pipeline::Text] Failed to emit TranscriptFinal IPC: {}", e);
    }

    transition(InteractionState::Thinking, None, ctx, app, state);

    match ctx.pipeline_mode {
        PipelineMode::Modular => {
            super::transcript::spawn_harness_turn_task(active_turn_id, query, app, state, ctx);
        }
        PipelineMode::Realtime => {
            let rt_guard = state.realtime_engine.blocking_lock();
            if let Some(ref rt) = *rt_guard {
                if let Err(e) = rt.send_text(&query) {
                    log::warn!(
                        "[Pipeline::Text] Failed to dispatch text input to realtime provider: {}",
                        e
                    );
                } else {
                    log::info!(
                        "[Pipeline::Text] Dispatched typed input to realtime provider for turn {}",
                        active_turn_id
                    );
                }
            } else {
                log::warn!("Realtime engine unavailable to process text input");
            }
        }
    }
}
