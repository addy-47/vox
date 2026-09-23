use tauri::AppHandle;

use crate::{
    core::{
        events::{emit_ipc_to, IpcEvent, TranscriptPayload},
        settings::PipelineMode,
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

    // Auto-resume so typed input is never silently dropped while paused or sleeping.
    if current_state == InteractionState::Paused || current_state == InteractionState::Sleeping {
        log::info!(
            "[Pipeline::Text] TextInput while {:?}: auto-resuming before dispatch",
            current_state
        );
        super::session::on_resume(app, state, ctx);
    }

    let current_state = state.pipeline.state();
    let active_turn_id = if current_state == InteractionState::Thinking
        || current_state == InteractionState::Speaking
        || current_state == InteractionState::Working
    {
        super::interrupt::on_interrupt(app, state, ctx)
    } else if current_state == InteractionState::Ready {
        let (new_turn_id, _) = state.pipeline.next_turn();
        state.pipeline_accumulator.lock().clear();
        state.turn_metrics.start_turn(new_turn_id);
        state.turn_metrics.record_speech_end();
        new_turn_id
    } else {
        return;
    };

    let trimmed = text.trim().to_string();
    if trimmed.is_empty() {
        log::debug!(
            "[Pipeline::Text] Dropping empty text input for turn {}",
            active_turn_id
        );
        return;
    }

    let transliterate_enabled = state
        .settings
        .read()
        .map(|s| s.stt.transliterate_enabled)
        .unwrap_or(false);
    let query = transliterate_if_hi(&trimmed, true, transliterate_enabled);
    log::info!(
        "[Pipeline::Text] Turn {}: User typed: '{}'",
        active_turn_id,
        query
    );

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

    state.pipeline.set_turn_open(true);
    state.pipeline.clear_drained_while_open();
    transition(InteractionState::Thinking, ctx, app, state);

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
                log::warn!("[Pipeline::Text] Realtime engine unavailable to process text input");
            }
        }
    }
}
