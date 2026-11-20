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
        TruncationPolicy::TailHeavy { tail_ratio: 0.8 }
    }

    fn execution_mode(&self, args: &Value) -> ToolExecutionMode {
        match args.get("action").and_then(|v| v.as_str()) {
            Some("list") | Some("status") => ToolExecutionMode::Parallel,
            _ => ToolExecutionMode::Exclusive,
        }
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
        _ctx: &'a ToolExecutionContext,
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

            Ok(ToolResult::new(format!("manage_task stub for action: {}", action)))
        }
        .boxed()
    }
}
