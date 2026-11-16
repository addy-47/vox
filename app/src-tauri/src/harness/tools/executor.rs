use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
    time::Instant,
};

use futures_util::future::join_all;

use super::{
    registry::ToolRegistry, ToolControlAction, ToolDefinition, ToolExecutionContext,
    ToolExecutionMode, ToolResult,
};
use crate::{
    core::events::PipelineMode, harness::TOOL_EXECUTION_TIMEOUT,
    services::llm::CanonicalToolCall,
};

/// Outcome record of a single executed tool invocation.
#[derive(Debug, Clone)]
pub struct ToolExecutionOutcome {
    pub call_id: String,
    pub tool_name: String,
    pub arguments: serde_json::Value,
    pub result: ToolResult,
    pub is_error: bool,
    pub duration_ms: u64,
    pub truncated: bool,
    pub spillover_path: Option<String>,
}

/// Runtime executor for invoking cognitive tools under isolation and persistence guarantees.
pub struct ToolExecutor;

impl ToolExecutor {
    /// Executes a single canonical tool call against the registry with timeout and metrics.
    pub async fn execute_tool(
        registry: &ToolRegistry,
        mode: PipelineMode,
        call: CanonicalToolCall,
        ctx: ToolExecutionContext,
    ) -> ToolExecutionOutcome {
        let start = Instant::now();
        ctx.app_state.turn_metrics.record_tool_start(&call.name);
        let tool = match registry.get(&call.name) {
            Some(t) => t,
            None => {
                log::warn!("[ToolExecutor] Tool '{}' not found in registry", call.name);
                let duration_ms = start.elapsed().as_millis() as u64;
                ctx.app_state
                    .turn_metrics
                    .record_tool_finish(duration_ms, true);
                let err_msg = format!("Tool '{}' not found in registry", call.name);
                return ToolExecutionOutcome {
                    call_id: call.id,
                    tool_name: call.name,
                    arguments: call.arguments,
                    result: ToolResult::new(err_msg),
                    is_error: true,
                    duration_ms,
                    truncated: false,
                    spillover_path: None,
                };
            }
        };

        if !tool.domain().matches(mode) {
            log::warn!(
                "[ToolExecutor] Tool '{}' does not support pipeline mode {:?}",
                call.name,
                mode
            );
            let duration_ms = start.elapsed().as_millis() as u64;
            ctx.app_state
                .turn_metrics
                .record_tool_finish(duration_ms, true);
            let err_msg = format!(
                "Tool '{}' does not support pipeline mode {:?}",
                call.name, mode
            );
            return ToolExecutionOutcome {
                call_id: call.id,
                tool_name: call.name,
                arguments: call.arguments,
                result: ToolResult::new(err_msg),
                is_error: true,
                duration_ms,
                truncated: false,
                spillover_path: None,
            };
        }

        let (mut result, is_error) = run_with_guards(&tool, mode, &call, &ctx).await;
        let duration_ms = start.elapsed().as_millis() as u64;

        let max_tokens = ctx.max_observation_tokens.unwrap_or(2000);
        let (preview_content, truncated, spillover_path) =
            crate::harness::stages::truncation::TruncationStage::apply(
                ctx.session_id,
                &call.id,
                &call.name,
                &result.content,
                max_tokens,
                tool.truncation_policy(),
            );
        result.content = preview_content;
        result.truncated = truncated;
        result.spillover_path = spillover_path.clone();

        ctx.app_state
            .turn_metrics
            .record_tool_finish(duration_ms, is_error);

        log::info!(
            "[ToolExecutor] Executed {} (mode: {:?}) in {}ms (is_error: {}, truncated: {})",
            call.name,
            mode,
            duration_ms,
            is_error,
            truncated
        );

        ToolExecutionOutcome {
            call_id: call.id,
            tool_name: call.name,
            arguments: call.arguments,
            result,
            is_error,
            duration_ms,
            truncated,
            spillover_path,
        }
    }

    /// Schedules and executes a batch of canonical tool calls using wave partitioning and exclusive barriers.
    pub async fn execute_batch(
        registry: &ToolRegistry,
        mode: PipelineMode,
        calls: Vec<CanonicalToolCall>,
        base_ctx: &ToolExecutionContext,
    ) -> (Vec<ToolExecutionOutcome>, Option<String>) {
        let mut outcomes = Vec::with_capacity(calls.len());
        let mut parallel_wave: Vec<CanonicalToolCall> = Vec::new();
        let mut suspended_question: Option<String> = None;
        let mut executed_ids: HashSet<String> = HashSet::new();

        for call in &calls {
            if base_ctx.cancel.is_cancelled() {
                break;
            }

            let exec_mode = registry
                .get(&call.name)
                .map(|t| t.execution_mode(&call.arguments))
                .unwrap_or(ToolExecutionMode::Exclusive);

            match exec_mode {
                ToolExecutionMode::Parallel => {
                    parallel_wave.push(call.clone());
                }
                ToolExecutionMode::Exclusive => {
                    // Drain any accumulated parallel wave before executing the exclusive barrier
                    if !parallel_wave.is_empty() {
                        let wave = std::mem::take(&mut parallel_wave);
                        let wave_outcomes =
                            Self::run_parallel_wave(registry, mode, wave, base_ctx).await;
                        for wo in &wave_outcomes {
                            executed_ids.insert(wo.call_id.clone());
                        }
                        outcomes.extend(wave_outcomes);
                    }

                    if base_ctx.cancel.is_cancelled() {
                        break;
                    }

                    let call_ctx = base_ctx.clone();
                    let outcome = Self::execute_tool(registry, mode, call.clone(), call_ctx).await;
                    executed_ids.insert(call.id.clone());
                    if let ToolControlAction::SuspendForUser { ref question, .. } =
                        outcome.result.control
                    {
                        suspended_question = Some(question.clone());
                    }
                    outcomes.push(outcome);

                    if suspended_question.is_some() {
                        break;
                    }
                }
            }
        }

        // Flush any trailing parallel wave
        if !parallel_wave.is_empty()
            && !base_ctx.cancel.is_cancelled()
            && suspended_question.is_none()
        {
            let wave = std::mem::take(&mut parallel_wave);
            let wave_outcomes = Self::run_parallel_wave(registry, mode, wave, base_ctx).await;
            for wo in &wave_outcomes {
                executed_ids.insert(wo.call_id.clone());
            }
            outcomes.extend(wave_outcomes);
        }

        // Synthesize fallback outcomes for any tool call not reached due to cancellation or suspension
        if outcomes.len() < calls.len() {
            for call in &calls {
                if !executed_ids.contains(&call.id) {
                    let reason = if base_ctx.cancel.is_cancelled() {
                        "Tool execution cancelled due to turn interruption"
                    } else {
                        "Tool execution skipped due to user interaction suspension"
                    };
                    outcomes.push(ToolExecutionOutcome {
                        call_id: call.id.clone(),
                        tool_name: call.name.clone(),
                        arguments: call.arguments.clone(),
                        result: ToolResult::new(reason),
                        is_error: true,
                        duration_ms: 0,
                        truncated: false,
                        spillover_path: None,
                    });
                }
            }
        }

        // Enforce model declaration order: sort outcomes to match calls sequence
        let call_id_to_index: HashMap<&str, usize> = calls
            .iter()
            .enumerate()
            .map(|(idx, c)| (c.id.as_str(), idx))
            .collect();
        outcomes.sort_by_key(|o| {
            call_id_to_index
                .get(o.call_id.as_str())
                .copied()
                .unwrap_or(usize::MAX)
        });

        (outcomes, suspended_question)
    }

    async fn run_parallel_wave(
        registry: &ToolRegistry,
        mode: PipelineMode,
        wave: Vec<CanonicalToolCall>,
        base_ctx: &ToolExecutionContext,
    ) -> Vec<ToolExecutionOutcome> {
        if wave.len() == 1 {
            let call = wave.into_iter().next().expect("non-empty wave");
            let call_ctx = base_ctx.clone();
            return vec![Self::execute_tool(registry, mode, call, call_ctx).await];
        }

        log::info!(
            "[ToolExecutor] Running parallel wave of {} read-only tools",
            wave.len()
        );
        let futs = wave.into_iter().map(|call| {
            let call_ctx = base_ctx.clone();
            Self::execute_tool(registry, mode, call, call_ctx)
        });
        join_all(futs).await
    }
}

/// Executes a tool future with strict timeout and cancellation races.
async fn run_with_guards(
    tool: &Arc<dyn ToolDefinition>,
    mode: PipelineMode,
    call: &CanonicalToolCall,
    ctx: &ToolExecutionContext,
) -> (ToolResult, bool) {
    if tool.handles_own_timeout() {
        tokio::select! {
            _ = ctx.cancel.cancelled() => {
                log::info!("[ToolExecutor] Tool {} cancelled", call.name);
                (ToolResult::new("Tool execution cancelled"), true)
            }
            res = tool.execute(mode, call.arguments.clone(), ctx) => {
                match res {
                    Ok(result) => (result, false),
                    Err(err) => {
                        log::warn!("[ToolExecutor] Tool {} returned error: {}", call.name, err);
                        (ToolResult::new(err.to_string()), true)
                    }
                }
            }
        }
    } else {
        tokio::select! {
            _ = ctx.cancel.cancelled() => {
                log::info!("[ToolExecutor] Tool {} cancelled", call.name);
                (ToolResult::new("Tool execution cancelled"), true)
            }
            res = tokio::time::timeout(TOOL_EXECUTION_TIMEOUT, tool.execute(mode, call.arguments.clone(), ctx)) => {
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
}
