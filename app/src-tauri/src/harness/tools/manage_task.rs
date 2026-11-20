use futures_util::future::{BoxFuture, FutureExt};
use serde_json::{json, Value};

use super::{
    ToolDefinition, ToolDomain, ToolError, ToolExecutionContext, ToolExecutionMode, ToolResult,
    TruncationPolicy,
};
use crate::core::events::PipelineMode;

/// Built-in tool that inspects and controls background tasks.
pub struct ManageTaskTool;

impl ToolDefinition for ManageTaskTool {
    fn name(&self) -> &str {
        "manage_task"
    }

    fn domain(&self) -> ToolDomain {
        ToolDomain::Modular
    }

    fn truncation_policy(&self) -> TruncationPolicy {
        TruncationPolicy::None
    }

    fn execution_mode(&self, args: &Value) -> ToolExecutionMode {
        match args.get("action").and_then(|v| v.as_str()) {
            Some("list") | Some("status") => ToolExecutionMode::Parallel,
            _ => ToolExecutionMode::Exclusive,
        }
    }

    fn handles_own_timeout(&self) -> bool {
        false
    }

    fn supports_background_promotion(&self) -> bool {
        false
    }

    fn description(&self, _mode: PipelineMode) -> &str {
        "Inspects or manages active and completed background tasks."
    }

    fn parameters_schema(&self, _mode: PipelineMode) -> Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["list", "status", "kill", "kill_all"],
                    "description": "Action to perform on background tasks."
                },
                "task_id": {
                    "type": "string",
                    "description": "Unique task identifier (required for 'status' and 'kill')."
                }
            },
            "required": ["action"]
        })
    }

    fn execute<'a>(
        &'a self,
        _mode: PipelineMode,
        args: Value,
        ctx: &'a ToolExecutionContext,
    ) -> BoxFuture<'a, Result<ToolResult, ToolError>> {
        async move {
            let action = args
                .get("action")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .trim()
                .to_string();

            if action.is_empty() {
                return Err(ToolError::InvalidArguments(
                    "Parameter 'action' must be a non-empty string".to_string(),
                ));
            }

            match action.as_str() {
                "list" => {
                    let tasks = ctx.app_state.tasks.list_tasks(Some(ctx.session_id));
                    let json_str =
                        serde_json::to_string(&tasks).unwrap_or_else(|_| "[]".to_string());
                    Ok(ToolResult::new(json_str))
                }
                "status" => {
                    let task_id = args
                        .get("task_id")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .trim()
                        .to_string();

                    if task_id.is_empty() {
                        return Err(ToolError::InvalidArguments(
                            "Parameter 'task_id' is required for action 'status'".to_string(),
                        ));
                    }

                    let log_path =
                        crate::utils::paths::task_log_path(ctx.session_id, &task_id);
                    let tail = ctx
                        .app_state
                        .tasks
                        .read_log_tail(&log_path, 50)
                        .await
                        .map_err(|e| {
                            ToolError::ExecutionFailed(format!(
                                "Failed to read task log tail: {}",
                                e
                            ))
                        })?;

                    let status = ctx
                        .app_state
                        .tasks
                        .task_status(&task_id, ctx.session_id)
                        .await;
                    let mut lines = vec![format!("Task: {}", task_id)];
                    match status {
                        Some(dto) => {
                            lines.push(format!("Status: {}", dto.status));
                            if let Some(exit_code) = dto.exit_code {
                                lines.push(format!("Exit code: {}", exit_code));
                            }
                            if let Some(error) = dto.error {
                                lines.push(format!("Error: {}", error));
                            }
                        }
                        None => lines.push("Status: unknown".to_string()),
                    }
                    lines.push("Tail:".to_string());
                    lines.push(tail);

                    Ok(ToolResult::new(lines.join("\n")))
                }
                "kill" => {
                    let task_id = args
                        .get("task_id")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .trim()
                        .to_string();

                    if task_id.is_empty() {
                        return Err(ToolError::InvalidArguments(
                            "Parameter 'task_id' is required for action 'kill'".to_string(),
                        ));
                    }

                    ctx.app_state
                        .tasks
                        .kill_task(&task_id)
                        .await
                        .map_err(|e| {
                            ToolError::ExecutionFailed(format!("Failed to kill task: {}", e))
                        })?;

                    let payload = json!({
                        "status": "killed",
                        "task_id": task_id
                    });
                    Ok(ToolResult::new(payload.to_string()))
                }
                "kill_all" => {
                    ctx.app_state.tasks.kill_all(ctx.session_id);
                    let payload = json!({
                        "status": "all_killed"
                    });
                    Ok(ToolResult::new(payload.to_string()))
                }
                other => Err(ToolError::InvalidArguments(format!(
                    "Unsupported action '{}' for manage_task",
                    other
                ))),
            }
        }
        .boxed()
    }
}
