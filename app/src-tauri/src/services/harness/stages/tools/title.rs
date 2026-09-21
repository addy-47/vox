use futures_util::future::{BoxFuture, FutureExt};
use serde_json::{json, Value};

use crate::{
    persistence::set_session_title,
    services::llm::ToolFlow,
};

use super::{ToolDefinition, ToolError, ToolExecutionContext, ToolResult};

/// Built-in terminal tool that sets the conversational session title and delivers spoken audio.
pub struct RespondAndSetTitleTool;

impl ToolDefinition for RespondAndSetTitleTool {
    fn name(&self) -> &str {
        "respond_and_set_title"
    }

    fn description(&self) -> &str {
        "Generates a concise 3-5 word title for the session based on the user's initial inquiry, and responds directly to the user in voice."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "title": {
                    "type": "string",
                    "description": "A concise, descriptive 3-5 word title summarizing the user's initial query."
                },
                "spoken_response": {
                    "type": "string",
                    "description": "The complete conversational spoken reply delivered directly to the user."
                }
            },
            "required": ["title", "spoken_response"]
        })
    }

    fn flow(&self) -> ToolFlow {
        ToolFlow::Terminal
    }

    fn execute<'a>(
        &'a self,
        args: Value,
        ctx: &'a ToolExecutionContext,
    ) -> BoxFuture<'a, Result<ToolResult, ToolError>> {
        async move {
            let title = args
                .get("title")
                .and_then(|v| v.as_str())
                .unwrap_or("Untitled Session")
                .trim()
                .to_string();

            let spoken_response = args
                .get("spoken_response")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .trim()
                .to_string();

            if title.is_empty() {
                return Err(ToolError::InvalidArguments(
                    "Parameter 'title' must be a non-empty string".to_string(),
                ));
            }

            let conn = match ctx.app_state.db.connect() {
                Ok(c) => c,
                Err(e) => {
                    log::warn!("[RespondAndSetTitleTool] Failed to connect to database: {}", e);
                    return Err(ToolError::ExecutionFailed(format!("Database connect error: {}", e)));
                }
            };

            if let Err(e) = set_session_title(&conn, ctx.session_id, &title).await {
                log::warn!("[RespondAndSetTitleTool] Failed to set session title: {}", e);
                return Err(ToolError::ExecutionFailed(format!("Database error: {}", e)));
            }

            if let Some(ref notify_cb) = ctx.on_sessions_changed {
                notify_cb();
            }

            log::info!(
                "[RespondAndSetTitleTool] Set session {} title to '{}'",
                ctx.session_id,
                title
            );

            Ok(ToolResult::new(format!("Session title set to '{}'", title))
                .with_spoken_response(spoken_response))
        }
        .boxed()
    }
}
