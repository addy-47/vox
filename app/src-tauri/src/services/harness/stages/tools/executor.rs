use std::{
    sync::Arc,
    time::{Instant, SystemTime, UNIX_EPOCH},
};

use crate::{
    persistence::PersistenceEvent,
    services::{
        harness::TOOL_EXECUTION_TIMEOUT,
        llm::{CanonicalToolCall, ToolFlow},
    },
};

use super::{
    registry::ToolRegistry, ToolDefinition, ToolExecutionContext, ToolResult,
};

/// Outcome record of a single executed tool invocation.
#[derive(Debug, Clone)]
pub struct ToolExecutionOutcome {
    pub call_id: String,
    pub tool_name: String,
    pub tool_flow: ToolFlow,
    pub result: ToolResult,
    pub is_error: bool,
    pub duration_ms: u64,
}

/// Dispatches asynchronous persistence write for completed tool invocation.
fn dispatch_persistence(
    ctx: &ToolExecutionContext,
    call: &CanonicalToolCall,
    flow: ToolFlow,
    result_text: &str,
    is_error: bool,
    duration_ms: u64,
) {
    let persist_lock = ctx.app_state.persist_tx.lock();
    if let Some(ref tx) = *persist_lock {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        let event = PersistenceEvent::ToolCallExecuted {
            id: call.id.clone(),
            session_id: ctx.session_id,
            turn_id: ctx.turn_id,
            tool_name: call.name.clone(),
            tool_flow: flow,
            arguments: call.arguments.clone(),
            result: result_text.to_string(),
            is_error,
            duration_ms,
            created_at: now,
        };
        if let Err(e) = tx.try_send(event) {
            log::warn!(
                "[ToolExecutor] Failed to queue ToolCallExecuted persistence event: {}",
                e
            );
        }
    }
}

/// Executes a tool future with strict timeout and cancellation races.
async fn run_with_guards(
    tool: &Arc<dyn ToolDefinition>,
    call: &CanonicalToolCall,
    ctx: &ToolExecutionContext,
) -> (ToolResult, bool) {
    tokio::select! {
        _ = ctx.cancel.cancelled() => {
            log::info!("[ToolExecutor] Tool {} cancelled", call.name);
            (ToolResult::new("Tool execution cancelled"), true)
        }
        res = tokio::time::timeout(TOOL_EXECUTION_TIMEOUT, tool.execute(call.arguments.clone(), ctx)) => {
            match res {
                Ok(Ok(result)) => (result, false),
                Ok(Err(err)) => {
                    log::warn!("[ToolExecutor] Tool {} returned error: {}", call.name, err);
                    (ToolResult::new(err.to_string()), true)
                }
                Err(_) => {
                    log::warn!("[ToolExecutor] Tool {} timed out after {:?}", call.name, TOOL_EXECUTION_TIMEOUT);
                    (ToolResult::new(format!("Tool execution timed out after {:?}", TOOL_EXECUTION_TIMEOUT)), true)
                }
            }
        }
    }
}

/// Runtime executor for invoking cognitive tools under isolation and persistence guarantees.
pub struct ToolExecutor;

impl ToolExecutor {
    /// Executes a single canonical tool call against the registry with timeout and persistence.
    pub async fn execute_tool(
        registry: &ToolRegistry,
        call: CanonicalToolCall,
        ctx: ToolExecutionContext,
    ) -> ToolExecutionOutcome {
        let start = Instant::now();
        let tool = match registry.get(&call.name) {
            Some(t) => t,
            None => {
                log::warn!("[ToolExecutor] Tool '{}' not found in registry", call.name);
                let duration_ms = start.elapsed().as_millis() as u64;
                let err_msg = format!("Tool '{}' not found in registry", call.name);
                dispatch_persistence(
                    &ctx,
                    &call,
                    ToolFlow::Terminal,
                    &err_msg,
                    true,
                    duration_ms,
                );
                return ToolExecutionOutcome {
                    call_id: call.id,
                    tool_name: call.name,
                    tool_flow: ToolFlow::Terminal,
                    result: ToolResult::new(err_msg),
                    is_error: true,
                    duration_ms,
                };
            }
        };

        let flow = tool.flow();
        let (result, is_error) = run_with_guards(&tool, &call, &ctx).await;
        let duration_ms = start.elapsed().as_millis() as u64;

        dispatch_persistence(
            &ctx,
            &call,
            flow,
            &result.content,
            is_error,
            duration_ms,
        );

        log::info!(
            "[ToolExecutor] Executed {} ({:?}) in {}ms (is_error: {})",
            call.name,
            flow,
            duration_ms,
            is_error
        );

        ToolExecutionOutcome {
            call_id: call.id,
            tool_name: call.name,
            tool_flow: flow,
            result,
            is_error,
            duration_ms,
        }
    }
}
