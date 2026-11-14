use futures_util::future::{BoxFuture, FutureExt};
use serde_json::{json, Value};

use super::{
    ToolDefinition, ToolDomain, ToolError, ToolExecutionContext, ToolExecutionMode, ToolResult,
};
use crate::{core::events::PipelineMode, persistence::PersistenceEvent};

/// Built-in tool that assigns a concise title to initialize a conversation session in Modular mode.
pub struct SetTitleTool;

impl ToolDefinition for SetTitleTool {
    fn name(&self) -> &str {
        "set_title"
    }

    fn domain(&self) -> ToolDomain {
        ToolDomain::Modular
    }

    fn description(&self, _mode: PipelineMode) -> &str {
        "Assigns a concise 3 to 5 word title summarizing the user's intent to initialize this new conversation session."
    }

    fn execution_mode(&self, _args: &Value) -> ToolExecutionMode {
        ToolExecutionMode::Parallel
    }

    fn parameters_schema(&self, _mode: PipelineMode) -> Value {
        json!({
            "type": "object",
            "properties": {
                "title": {
                    "type": "string",
                    "description": "A concise, descriptive 3-5 word title summarizing the user's initial query."
                }
            },
            "required": ["title"]
        })
    }

    fn execute<'a>(
        &'a self,
        _mode: PipelineMode,
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

            if title.is_empty() {
                return Err(ToolError::InvalidArguments(
                    "Parameter 'title' must be a non-empty string".to_string(),
                ));
            }

            apply_session_title(&title, ctx).await?;

            Ok(ToolResult::new(format!(
                "Session title set to '{}'",
                title
            )))
        }
        .boxed()
    }
}

/// Built-in tool that assigns a concise title to initialize a conversation session in Realtime S2S mode.
pub struct SetSessionTitleTool;

impl ToolDefinition for SetSessionTitleTool {
    fn name(&self) -> &str {
        "set_session_title"
    }

    fn domain(&self) -> ToolDomain {
        ToolDomain::Realtime
    }

    fn description(&self, _mode: PipelineMode) -> &str {
        "Assigns a concise 3-5 word title to initialize this conversation session."
    }

    fn execution_mode(&self, _args: &Value) -> ToolExecutionMode {
        ToolExecutionMode::Parallel
    }

    fn parameters_schema(&self, _mode: PipelineMode) -> Value {
        json!({
            "type": "object",
            "properties": {
                "title": {
                    "type": "string",
                    "description": "A concise, descriptive 3-5 word title summarizing the user's initial query."
                }
            },
            "required": ["title"]
        })
    }

    fn execute<'a>(
        &'a self,
        _mode: PipelineMode,
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

            if title.is_empty() {
                return Err(ToolError::InvalidArguments(
                    "Parameter 'title' must be a non-empty string".to_string(),
                ));
            }

            // Realtime execution guard: ensure title cannot be overwritten once assigned
            if ctx.is_title_already_set().await {
                log::info!(
                    "[SetSessionTitleTool] Session {} title is already set; ignoring update request for '{}'",
                    ctx.session_id,
                    title
                );
                return Ok(ToolResult::new(
                    "{\"status\": \"ignored\", \"message\": \"Session title has already been set and is locked for this session.\"}"
                ));
            }

            apply_session_title(&title, ctx).await?;

            Ok(ToolResult::new(format!(
                "{{\"status\": \"ok\", \"title\": \"{}\"}}",
                title
            )))
        }
        .boxed()
    }
}

/// Shared helper executing session title persistence and notification dispatch.
async fn apply_session_title(title: &str, ctx: &ToolExecutionContext) -> Result<(), ToolError> {
    let is_private = ctx
        .app_state
        .telemetry
        .is_private_mode
        .load(std::sync::atomic::Ordering::Relaxed);

    if !is_private {
        let persist_tx = ctx.app_state.persist_tx.lock().clone();
        match persist_tx {
            Some(tx) => {
                if let Err(e) = tx.try_send(PersistenceEvent::UpdateSessionMetadata {
                    session_id: ctx.session_id,
                    key: "title".to_string(),
                    value: title.to_string(),
                }) {
                    log::warn!("[SetTitle] Failed to queue title update: {}", e);
                    return Err(ToolError::ExecutionFailed(format!(
                        "Persistence queue error: {}",
                        e
                    )));
                }
            }
            None => {
                log::warn!(
                    "[SetTitle] No persistence worker; title not set for session {}",
                    ctx.session_id
                );
            }
        }

        if let Some(ref notify_cb) = ctx.on_sessions_changed {
            notify_cb();
        }

        log::info!(
            "[SetTitle] Queued title update for session {} to '{}'",
            ctx.session_id,
            title
        );
    } else {
        log::info!(
            "[SetTitle] Private mode active; skipping session title DB write for session {}",
            ctx.session_id
        );
    }

    Ok(())
}
