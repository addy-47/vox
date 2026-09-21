use std::sync::Arc;

use crate::{
    core::{
        events::{emit_ipc_to, AudioIntent, IpcEvent},
        state::AppWindow,
    },
    services::{
        harness::{
            current_timestamp_ms,
            stages::streaming::TextNormalizer,
            ChatMessage, Role, ToolExecutionContext, ToolExecutor,
        },
        llm::CanonicalToolCall,
    },
};

use super::{
    loop_driver::TurnLoopContext,
    phase::{enter_non_terminal_phase, NonTerminalContext, NonTerminalPhase},
    TurnOutcome,
};

/// Handles Phase 6 Case B: Terminal tool execution (single pass, voice + action, commit).
pub async fn handle_terminal_tool<R: tauri::Runtime + 'static>(
    ctx: &TurnLoopContext<'_, R>,
    call: CanonicalToolCall,
) -> TurnOutcome {
    let turn_id = ctx.req.turn_id;
    let spoken_response = call
        .arguments
        .get("spoken_response")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_string();

    if !spoken_response.is_empty() {
        ctx.stream_stage.dispatch_spoken_response(
            &spoken_response,
            AudioIntent::TurnResponse,
            ctx.stream_handles,
        );
    }
    ctx.stream_stage.emit_finished(ctx.stream_handles);

    let app_clone = ctx.req.app.clone();
    let on_sessions_changed = Some(Arc::new(move || {
        if let Err(e) = emit_ipc_to(&app_clone, AppWindow::Main, IpcEvent::SessionsChanged) {
            log::warn!("[Harness::Tools] Failed to emit SessionsChanged IPC: {}", e);
        }
    }) as Arc<dyn Fn() + Send + Sync>);

    let tool_ctx = ToolExecutionContext {
        app_state: Arc::clone(&ctx.req.app_state),
        session_id: ctx.session_id,
        turn_id,
        cancel: ctx.req.cancel.clone(),
        on_sessions_changed,
    };

    let outcome = ToolExecutor::execute_tool(ctx.tool_registry, call, tool_ctx).await;
    let final_response = if !spoken_response.is_empty() {
        spoken_response
    } else {
        outcome.result.spoken_response.unwrap_or_default()
    };

    let mut guard = ctx.harness_arc.lock();
    if let Some(ref mut harness) = *guard {
        if outcome.tool_name == "respond_and_set_title" && !outcome.is_error {
            harness.title_set = true;
        }
        harness.history.push_assistant_turn(final_response.clone());
    }

    TurnOutcome::Completed {
        turn_id,
        assistant_response: final_response,
    }
}

/// Handles Phase 6 Case B: Non-terminal tool execution (interim filler, Working state, scratchpad).
pub async fn handle_non_terminal_tool<R: tauri::Runtime + 'static>(
    ctx: &TurnLoopContext<'_, R>,
    call: CanonicalToolCall,
    partial_text: &str,
    scratchpad: &mut Vec<ChatMessage>,
) {
    let turn_id = ctx.req.turn_id;
    let remainder = ctx.stream_handles.accumulator.lock().flush_chunker();
    if let Some(ref text) = remainder {
        let normalized = TextNormalizer::normalize_for_speech(text);
        if !normalized.is_empty() {
            ctx.stream_stage.dispatch_spoken_response(
                &normalized,
                AudioIntent::TurnResponse,
                ctx.stream_handles,
            );
        }
    }

    let spoken_filler = call
        .arguments
        .get("spoken_filler")
        .and_then(|v| v.as_str())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());

    let non_terminal = NonTerminalPhase::tool(&call.name, &call.id, spoken_filler);
    let non_term_ctx = NonTerminalContext {
        query: &ctx.req.query,
        turn_id,
        routing_ctx: &ctx.req.routing_ctx,
        app: &ctx.req.app,
        app_state: &ctx.req.app_state,
        tts_tx: ctx.req.tts_tx.as_ref(),
        pending_synthesis_jobs: &ctx.req.pending_synthesis_jobs,
    };
    enter_non_terminal_phase(&non_terminal, &non_term_ctx);

    let app_clone = ctx.req.app.clone();
    let on_sessions_changed = Some(Arc::new(move || {
        if let Err(e) = emit_ipc_to(&app_clone, AppWindow::Main, IpcEvent::SessionsChanged) {
            log::warn!("[Harness::Tools] Failed to emit SessionsChanged IPC: {}", e);
        }
    }) as Arc<dyn Fn() + Send + Sync>);

    let tool_ctx = ToolExecutionContext {
        app_state: Arc::clone(&ctx.req.app_state),
        session_id: ctx.session_id,
        turn_id,
        cancel: ctx.req.cancel.clone(),
        on_sessions_changed,
    };

    let outcome = ToolExecutor::execute_tool(ctx.tool_registry, call.clone(), tool_ctx).await;

    scratchpad.push(ChatMessage {
        role: Role::Assistant,
        content: partial_text.to_string(),
        timestamp_ms: current_timestamp_ms(),
        tool_call_id: None,
        tool_calls: Some(vec![call.clone()]),
    });

    scratchpad.push(ChatMessage {
        role: Role::Tool,
        content: outcome.result.content,
        timestamp_ms: current_timestamp_ms(),
        tool_call_id: Some(call.id),
        tool_calls: None,
    });
}
