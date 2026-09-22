use futures_util::future::{BoxFuture, FutureExt};
use serde_json::{json, Value};

use crate::services::llm::ToolFlow;

use super::{ToolDefinition, ToolExecutionContext, ToolError, ToolResult};

/// Non-terminal cognitive tool for searching personal memory documents and episodic turns.
pub struct MemorySearchTool;

impl ToolDefinition for MemorySearchTool {
    fn name(&self) -> &str {
        "search_memory"
    }

    fn description(&self) -> &str {
        "Searches personal cognitive memory for user facts, preferences, background context, or past conversational details."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "Semantic search query to locate relevant user facts or context in memory."
                },
                "spoken_filler": {
                    "type": "string",
                    "description": "A brief, natural 3-5 word spoken phrase delivered while memory search is evaluated."
                }
            },
            "required": ["query", "spoken_filler"]
        })
    }

    fn flow(&self) -> ToolFlow {
        ToolFlow::NonTerminal
    }

    fn execute<'a>(
        &'a self,
        args: Value,
        ctx: &'a ToolExecutionContext,
    ) -> BoxFuture<'a, Result<ToolResult, ToolError>> {
        async move {
            let query = args
                .get("query")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .trim()
                .to_string();

            let spoken_filler = args
                .get("spoken_filler")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .trim()
                .to_string();

            if query.is_empty() {
                return Err(ToolError::InvalidArguments(
                    "Parameter 'query' must be a non-empty string".to_string(),
                ));
            }

            log::info!(
                "[MemorySearchTool] Turn {}: searching memory for query: '{}' (filler: '{}')",
                ctx.turn_id,
                query,
                spoken_filler
            );

            // Phase 12.1 stub: vector search integration occurs in Batch 5
            let observation = format!(
                "Memory search completed for query '{}'. No relevant historical records found.",
                query
            );

            Ok(ToolResult::new(observation).with_spoken_filler(spoken_filler))
        }
        .boxed()
    }
}
