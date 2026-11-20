use std::path::Path;

use futures_util::future::{BoxFuture, FutureExt};
use serde_json::{json, Value};

use super::{
    ToolDefinition, ToolDomain, ToolError, ToolExecutionContext, ToolExecutionMode, ToolResult,
    TruncationPolicy,
};
use crate::{
    core::events::{PipelineMode, TaskStatus},
    services::tasks::PromotionOutcome,
};

/// Built-in tool that executes OS shell commands in an isolated process group.
pub struct BashTool;

impl ToolDefinition for BashTool {
    fn name(&self) -> &str {
        "bash"
    }

    fn domain(&self) -> ToolDomain {
        ToolDomain::Modular
    }

    fn truncation_policy(&self) -> TruncationPolicy {
        TruncationPolicy::TailHeavy { tail_ratio: 0.8 }
    }

    fn execution_mode(&self, _args: &Value) -> ToolExecutionMode {
        ToolExecutionMode::Exclusive
    }

    fn handles_own_timeout(&self) -> bool {
        true
    }

    fn supports_background_promotion(&self) -> bool {
        true
    }

    fn description(&self, _mode: PipelineMode) -> &str {
        "Executes a shell command in an isolated child process group."
    }

    fn parameters_schema(&self, _mode: PipelineMode) -> Value {
        json!({
            "type": "object",
            "properties": {
                "command": {
                    "type": "string",
                    "description": "The exact shell command line string to execute."
                },
                "run_in_background": {
                    "type": "boolean",
                    "description": "Whether this command can run in the background if it takes longer than 5 seconds. Defaults to true. Set to false only when you strictly require the output before proceeding in this turn.",
                    "default": true
                }
            },
            "required": ["command"]
        })
    }

    fn execute<'a>(
        &'a self,
        _mode: PipelineMode,
        args: Value,
        ctx: &'a ToolExecutionContext,
    ) -> BoxFuture<'a, Result<ToolResult, ToolError>> {
        async move {
            let command = args
                .get("command")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .trim()
                .to_string();

            if command.is_empty() {
                return Err(ToolError::InvalidArguments(
                    "Parameter 'command' must be a non-empty string".to_string(),
                ));
            }

            let run_in_background = args
                .get("run_in_background")
                .and_then(|v| v.as_bool())
                .unwrap_or(true);

            let handle = ctx
                .app_state
                .tasks
                .spawn_supervised_task(
                    ctx.session_id,
                    Some(ctx.step_id.clone()),
                    command.clone(),
                )
                .map_err(|e| ToolError::ExecutionFailed(format!("Failed to spawn task: {}", e)))?;

            let timeout_duration = if run_in_background {
                ctx.auto_promote_timeout
            } else {
                ctx.foreground_hard_timeout
            };

            let mut completion_rx = handle.completion_rx;
            let task_id = handle.task_id;
            let log_path = handle.log_path;

            tokio::select! {
                _ = ctx.cancel.cancelled() => {
                    log::info!("[BashTool] Turn cancelled; terminating process group for task {}", task_id);
                    let _ = ctx.app_state.tasks.kill_task(&task_id).await;
                    Err(ToolError::Cancelled)
                }
                status_res = &mut completion_rx => {
                    match status_res {
                        Ok(status) => {
                            if !matches!(status, TaskStatus::Killed { .. }) {
                                ctx.app_state.tasks.prune_unpromoted(&task_id);
                            }
                            terminal_result(ctx, &status, &log_path).await
                        }
                        Err(_) => {
                            ctx.app_state.tasks.prune_unpromoted(&task_id);
                            Err(ToolError::ExecutionFailed(
                                "Execution channel closed unexpectedly".to_string(),
                            ))
                        }
                    }
                }
                _ = tokio::time::sleep(timeout_duration) => {
                    let message = if run_in_background {
                        log::info!(
                            "[BashTool] Exceeded {:?}; auto-promoting task {}",
                            timeout_duration,
                            task_id
                        );
                        "Process running in background. Use manage_task to monitor or kill."
                    } else {
                        log::warn!(
                            "[BashTool] Exceeded {:?} foreground ceiling; promoting task {}",
                            timeout_duration,
                            task_id
                        );
                        "Process exceeded foreground ceiling and was promoted to background."
                    };

                    match ctx.app_state.tasks.mark_promoted(&task_id).await {
                        PromotionOutcome::Promoted => {
                            let payload = json!({
                                "status": "running_in_background",
                                "task_id": task_id,
                                "command": command,
                                "log_path": log_path.to_string_lossy(),
                                "message": message
                            });
                            Ok(ToolResult::new(payload.to_string()))
                        }
                        PromotionOutcome::AlreadyFinished(status) => {
                            ctx.app_state.tasks.prune_unpromoted(&task_id);
                            terminal_result(ctx, &status, &log_path).await
                        }
                        PromotionOutcome::Gone => Err(ToolError::ExecutionFailed(
                            "Task is no longer running".to_string(),
                        )),
                    }
                }
            }
        }
        .boxed()
    }
}

/// Formats a supervised child's terminal status as the tool observation.
async fn terminal_result(
    ctx: &ToolExecutionContext,
    status: &TaskStatus,
    log_path: &Path,
) -> Result<ToolResult, ToolError> {
    match status {
        TaskStatus::Completed { exit_code, .. } => {
            let tail = ctx
                .app_state
                .tasks
                .read_log_tail(log_path, 50)
                .await
                .unwrap_or_default();
            if *exit_code == 0 {
                Ok(ToolResult::new(format!("Exit code: 0\nOutput:\n{}", tail)))
            } else {
                Err(ToolError::ExecutionFailed(format!(
                    "Exit code: {}\nOutput:\n{}",
                    exit_code, tail
                )))
            }
        }
        TaskStatus::Failed { error, .. } => {
            Err(ToolError::ExecutionFailed(format!("Execution failed: {}", error)))
        }
        TaskStatus::Killed { .. } => {
            Err(ToolError::ExecutionFailed("Process was terminated".to_string()))
        }
        TaskStatus::Running { .. } => {
            Err(ToolError::ExecutionFailed("Task is still running".to_string()))
        }
    }
}
