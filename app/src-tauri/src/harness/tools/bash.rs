use futures_util::future::{BoxFuture, FutureExt};
use serde_json::{json, Value};

use super::{
    ToolDefinition, ToolDomain, ToolError, ToolExecutionContext, ToolExecutionMode, ToolResult,
    TruncationPolicy,
};
use crate::core::events::PipelineMode;

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
        _ctx: &'a ToolExecutionContext,
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

            Ok(ToolResult::new(format!("Command output stub for: {}", command)))
        }
        .boxed()
    }
}
