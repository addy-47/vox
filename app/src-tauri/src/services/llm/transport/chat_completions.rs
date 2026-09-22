 use std::{collections::BTreeMap, sync::mpsc};

use futures_util::StreamExt;
use serde::Deserialize;

use super::{config::ConnectionConfig, sse::SseDecoder};
use crate::services::llm::{
    CanonicalToolCall, GenerationRequest, LlmError, OutputConstraint, ReasoningMode,
};

#[derive(Deserialize)]
struct ChatCompletionChunk {
    #[serde(default)]
    choices: Vec<ChunkChoice>,
}

#[derive(Deserialize)]
struct ChunkChoice {
    delta: ChunkDelta,
    #[serde(default)]
    finish_reason: Option<String>,
}

#[derive(Deserialize)]
struct ChunkDelta {
    #[serde(default)]
    content: Option<String>,
    #[serde(default)]
    tool_calls: Option<Vec<ChunkDeltaToolCall>>,
}

#[derive(Deserialize, Clone)]
struct ChunkDeltaToolCall {
    index: Option<usize>,
    id: Option<String>,
    #[serde(default)]
    function: Option<ChunkDeltaFunction>,
}

#[derive(Deserialize, Clone)]
struct ChunkDeltaFunction {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    arguments: Option<String>,
}

#[derive(Default)]
struct PendingToolCall {
    id: String,
    name: String,
    arguments_buffer: String,
}

impl PendingToolCall {
    fn apply_delta(&mut self, tc: ChunkDeltaToolCall) {
        if let Some(id) = tc.id {
            self.id = id;
        }
        if let Some(func) = tc.function {
            if let Some(name) = func.name {
                self.name.push_str(&name);
            }
            if let Some(args) = func.arguments {
                self.arguments_buffer.push_str(&args);
            }
        }
    }

    fn into_canonical(self) -> Option<CanonicalToolCall> {
        if self.name.is_empty() {
            return None;
        }
        let id = if self.id.is_empty() {
            format!("call_{}", uuid::Uuid::new_v4().simple())
        } else {
            self.id
        };
        let arguments = if self.arguments_buffer.trim().is_empty() {
            serde_json::json!({})
        } else {
            serde_json::from_str::<serde_json::Value>(&self.arguments_buffer).unwrap_or_else(|err| {
                log::warn!(
                    "[ChatCompletions] Failed to parse tool arguments as JSON: {}. Using raw string.",
                    err
                );
                serde_json::json!({ "raw": self.arguments_buffer })
            })
        };
        Some(CanonicalToolCall {
            id,
            name: self.name,
            arguments,
        })
    }
}

/// Builds the HTTP POST request payload for Chat Completions.
pub fn build_request_body(
    config: &ConnectionConfig,
    request: &GenerationRequest,
) -> serde_json::Value {
    let messages = serialize_messages(&request.input.messages);

    let mut body = serde_json::Map::new();
    body.insert("model".to_string(), serde_json::json!(config.model));
    body.insert("messages".to_string(), serde_json::json!(messages));
    body.insert("stream".to_string(), serde_json::json!(true));
    body.insert(
        "stream_options".to_string(),
        serde_json::json!({ "include_usage": true }),
    );

    populate_sampling_options(&mut body, config, request);
    populate_format_constraints(&mut body, request);
    populate_tools(&mut body, request);

    serde_json::Value::Object(body)
}

fn serialize_messages(
    messages: &[crate::services::harness::ChatMessage],
) -> Vec<serde_json::Value> {
    messages
        .iter()
        .map(|m| {
            let mut map = serde_json::Map::new();
            map.insert("role".to_string(), serde_json::json!(m.role.to_string()));

            if let Some(ref t_id) = m.tool_call_id {
                map.insert("tool_call_id".to_string(), serde_json::json!(t_id));
            }

            if let Some(ref calls) = m.tool_calls {
                let calls_json: Vec<serde_json::Value> = calls
                    .iter()
                    .map(|c| {
                        serde_json::json!({
                            "id": c.id,
                            "type": "function",
                            "function": {
                                "name": c.name,
                                "arguments": serde_json::to_string(&c.arguments).unwrap_or_else(|_| "{}".to_string())
                            }
                        })
                    })
                    .collect();
                map.insert("tool_calls".to_string(), serde_json::json!(calls_json));
            }

            if m.content.is_empty() && m.tool_calls.is_some() {
                map.insert("content".to_string(), serde_json::Value::Null);
            } else {
                map.insert("content".to_string(), serde_json::json!(m.content));
            }

            serde_json::Value::Object(map)
        })
        .collect()
}

fn populate_sampling_options(
    body: &mut serde_json::Map<String, serde_json::Value>,
    config: &ConnectionConfig,
    request: &GenerationRequest,
) {
    if let Some(temp) = request.options.temperature {
        body.insert("temperature".to_string(), serde_json::json!(temp));
    }
    if let Some(top_p) = request.options.top_p {
        body.insert("top_p".to_string(), serde_json::json!(top_p));
    }
    if let Some(top_k) = request.options.top_k {
        body.insert("top_k".to_string(), serde_json::json!(top_k));
    }
    if let Some(max_tokens) = request.options.max_output_tokens {
        body.insert(
            config.token_limit_field.as_str().to_string(),
            serde_json::json!(max_tokens),
        );
    }
    if !request.options.stop.is_empty() {
        body.insert("stop".to_string(), serde_json::json!(request.options.stop));
    }
    if let Some(seed) = request.options.seed {
        body.insert("seed".to_string(), serde_json::json!(seed));
    }
    if request.options.reasoning == ReasoningMode::Disabled {
        body.insert(
            "reasoning".to_string(),
            serde_json::json!({ "enabled": false }),
        );
    }
}

fn populate_format_constraints(
    body: &mut serde_json::Map<String, serde_json::Value>,
    request: &GenerationRequest,
) {
    match &request.output {
        OutputConstraint::Text => {}
        OutputConstraint::JsonObject => {
            body.insert(
                "response_format".to_string(),
                serde_json::json!({ "type": "json_object" }),
            );
        }
        OutputConstraint::JsonSchema {
            name,
            schema,
            strict,
        } => {
            body.insert(
                "response_format".to_string(),
                serde_json::json!({
                    "type": "json_schema",
                    "json_schema": {
                        "name": name,
                        "schema": schema,
                        "strict": strict
                    }
                }),
            );
        }
    }
}

fn populate_tools(
    body: &mut serde_json::Map<String, serde_json::Value>,
    request: &GenerationRequest,
) {
    if let Some(ref tools) = request.tools {
        if !tools.is_empty() {
            let tools_json: Vec<serde_json::Value> = tools
                .iter()
                .map(|t| {
                    serde_json::json!({
                        "type": "function",
                        "function": {
                            "name": t.name,
                            "description": t.description,
                            "parameters": t.parameters
                        }
                    })
                })
                .collect();
            body.insert("tools".to_string(), serde_json::json!(tools_json));
        }
    }
}

/// Resolves the canonical chat completions endpoint URL.
pub fn resolve_url(base_url: &str) -> String {
    let trimmed = base_url.trim_end_matches('/');
    if trimmed.ends_with("/chat/completions") {
        trimmed.to_string()
    } else if trimmed.ends_with("/v1") || trimmed.ends_with("/openai") {
        format!("{}/chat/completions", trimmed)
    } else {
        format!("{}/v1/chat/completions", trimmed)
    }
}

fn flush_pending_tool_calls(
    pending_map: &mut BTreeMap<usize, PendingToolCall>,
    tx: &mpsc::Sender<super::super::LlmStreamEvent>,
) {
    let keys: Vec<usize> = pending_map.keys().copied().collect();
    for key in keys {
        if let Some(pending) = pending_map.remove(&key) {
            if let Some(call) = pending.into_canonical() {
                log::info!(
                    "[ChatCompletions] Emitting tool call: {} (id: {})",
                    call.name,
                    call.id
                );
                if let Err(e) = tx.send(super::super::LlmStreamEvent::ToolCall(call)) {
                    log::warn!("[ChatCompletions] Failed to send ToolCall event: {}", e);
                }
            }
        }
    }
}

fn process_sse_line(
    line: &str,
    pending_tool_calls: &mut BTreeMap<usize, PendingToolCall>,
    tx: &mpsc::Sender<super::super::LlmStreamEvent>,
) -> bool {
    if line == "[DONE]" {
        flush_pending_tool_calls(pending_tool_calls, tx);
        if let Err(e) = tx.send(super::super::LlmStreamEvent::Finished) {
            log::warn!("[ChatCompletions] Send finished event error: {}", e);
        }
        return true;
    }

    if let Ok(chunk) = serde_json::from_str::<ChatCompletionChunk>(line) {
        if let Some(choice) = chunk.choices.first() {
            if let Some(ref token) = choice.delta.content {
                if !token.is_empty() {
                    if let Err(e) = tx.send(super::super::LlmStreamEvent::Token(token.clone())) {
                        log::warn!("[ChatCompletions] Send token error: {}", e);
                    }
                }
            }
            if let Some(ref tc_list) = choice.delta.tool_calls {
                for tc in tc_list {
                    let idx = tc.index.unwrap_or(0);
                    let pending = pending_tool_calls.entry(idx).or_default();
                    pending.apply_delta(tc.clone());
                }
            }
            if let Some(ref reason) = choice.finish_reason {
                if reason == "tool_calls" {
                    flush_pending_tool_calls(pending_tool_calls, tx);
                }
            }
        }
    }
    false
}

/// Streams token generation from a `/v1/chat/completions` endpoint.
pub async fn stream_chat_completions(
    client: &reqwest::Client,
    config: &ConnectionConfig,
    request: &GenerationRequest,
    turn_id: u32,
    cancel: &tokio_util::sync::CancellationToken,
    tx: &mpsc::Sender<super::super::LlmStreamEvent>,
) -> Result<(), LlmError> {
    let url = resolve_url(&config.base_url);
    log::debug!(
        "[ChatCompletions] Starting stream (turn: {}) to {}",
        turn_id,
        url
    );
    let req_body = build_request_body(config, request);

    let mut builder = client.post(&url).json(&req_body);
    builder = super::inject_auth_headers(builder, &config.auth);

    let response = tokio::select! {
        res = builder.send() => {
            res.map_err(|e| LlmError::Transport(e.to_string()))?
        }
        _ = cancel.cancelled() => {
            if let Err(e) = tx.send(super::super::LlmStreamEvent::Finished) {
                log::warn!("[ChatCompletions] Failed to send Finished on cancel: {}", e);
            }
            return Ok(());
        }
    };

    if !response.status().is_success() {
        let status = response.status();
        let err_text = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());
        return Err(LlmError::Provider {
            status: status.as_u16(),
            message: err_text,
        });
    }

    let mut decoder = SseDecoder::new();
    let mut byte_stream = response.bytes_stream();
    let mut pending_tool_calls = BTreeMap::new();

    loop {
        if cancel.is_cancelled() {
            if let Err(e) = tx.send(super::super::LlmStreamEvent::Finished) {
                log::warn!(
                    "[ChatCompletions] Failed to send Finished on cancel during stream: {}",
                    e
                );
            }
            return Ok(());
        }

        let chunk_opt = tokio::select! {
            chunk = byte_stream.next() => chunk,
            _ = cancel.cancelled() => {
                if let Err(e) = tx.send(super::super::LlmStreamEvent::Finished) {
                    log::warn!(
                        "[ChatCompletions] Failed to send Finished on select cancel: {}",
                        e
                    );
                }
                return Ok(());
            }
        };

        match chunk_opt {
            Some(Ok(bytes)) => {
                let lines = decoder.decode_chunk(&bytes);
                for line in lines {
                    if process_sse_line(&line, &mut pending_tool_calls, tx) {
                        return Ok(());
                    }
                }
            }
            Some(Err(e)) => return Err(LlmError::Transport(e.to_string())),
            None => break,
        }
    }

    if let Some(line) = decoder.flush() {
        process_sse_line(&line, &mut pending_tool_calls, tx);
    }

    flush_pending_tool_calls(&mut pending_tool_calls, tx);

    if let Err(e) = tx.send(super::super::LlmStreamEvent::Finished) {
        log::warn!("[ChatCompletions] Send final finished event error: {}", e);
    }
    Ok(())
}
