use std::{
    collections::{BTreeMap, HashSet},
    sync::mpsc,
};

use futures_util::StreamExt;
use serde::{Deserialize, Serialize};

use super::{config::ConnectionConfig, sse::SseDecoder};
use crate::services::{
    harness::Role,
    llm::{CanonicalToolCall, GenerationRequest, LlmError, OutputConstraint, ReasoningMode},
};

#[derive(Serialize)]
struct ResponsesInputItem {
    role: String,
    content: String,
}

#[derive(Deserialize)]
struct ResponsesEvent {
    #[serde(rename = "type")]
    event_type: Option<String>,
    delta: Option<String>,
    #[serde(default)]
    output_index: Option<usize>,
    #[serde(default)]
    item: Option<ResponsesOutputItem>,
    #[serde(default)]
    response: Option<ResponsesCompletedBody>,
}

#[derive(Deserialize)]
struct ResponsesOutputItem {
    #[serde(rename = "type")]
    item_type: Option<String>,
    #[serde(default)]
    call_id: Option<String>,
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    arguments: Option<String>,
}

#[derive(Deserialize)]
struct ResponsesCompletedBody {
    #[serde(default)]
    output: Vec<ResponsesOutputItem>,
}

#[derive(Default)]
struct ResponsesPendingCall {
    call_id: String,
    name: String,
    arguments: String,
}

/// Converts one Responses `function_call` output item into a canonical call.
/// Rejects items with no name or with unparseable argument JSON.
fn responses_item_to_call(item: &ResponsesOutputItem) -> Option<CanonicalToolCall> {
    if item.item_type.as_deref() != Some("function_call") {
        return None;
    }
    let name = item.name.clone().filter(|n| !n.is_empty())?;
    let call_id = item
        .call_id
        .clone()
        .or_else(|| item.id.clone())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| format!("call_{}", uuid::Uuid::new_v4().simple()));
    let arguments = match item.arguments.as_deref().map(str::trim) {
        None | Some("") => serde_json::json!({}),
        Some(text) => match serde_json::from_str::<serde_json::Value>(text) {
            Ok(value) => value,
            Err(err) => {
                log::warn!(
                    "[ResponsesTransport] Rejecting invalid JSON tool arguments for {}: {}",
                    name,
                    err
                );
                return None;
            }
        },
    };
    Some(CanonicalToolCall {
        id: call_id,
        name,
        arguments,
    })
}

/// Accumulates Responses function-call events into canonical calls. Shared by the
/// transport and the capability probe. Deduplicates on (name, arguments): the
/// terminal `response.completed` payload repeats calls already emitted from
/// `response.output_item.done`, and executing the same call twice is worse
/// than dropping a genuinely duplicated emission.
#[derive(Default)]
pub(crate) struct ResponsesToolAccumulator {
    pending: BTreeMap<usize, ResponsesPendingCall>,
    emitted: HashSet<String>,
}

impl ResponsesToolAccumulator {
    /// Feeds one SSE line, returning calls completed by this line.
    pub(crate) fn feed_line(&mut self, line: &str) -> Vec<CanonicalToolCall> {
        let mut out = Vec::new();
        let event = match serde_json::from_str::<ResponsesEvent>(line) {
            Ok(event) => event,
            Err(_) => return out,
        };
        match event.event_type.as_deref() {
            Some("response.output_item.added") => {
                if let Some(item) = event.item {
                    if item.item_type.as_deref() == Some("function_call") {
                        let idx = event.output_index.unwrap_or(0);
                        self.pending.insert(
                            idx,
                            ResponsesPendingCall {
                                call_id: item
                                    .call_id
                                    .or(item.id)
                                    .filter(|s| !s.is_empty())
                                    .unwrap_or_default(),
                                name: item.name.unwrap_or_default(),
                                arguments: String::new(),
                            },
                        );
                    }
                }
            }
            Some("response.function_call_arguments.delta") => {
                if let Some(delta) = event.delta {
                    let idx = event.output_index.unwrap_or(0);
                    self.pending.entry(idx).or_default().arguments.push_str(&delta);
                }
            }
            Some("response.output_item.done") => {
                if let Some(item) = event.item {
                    if item.item_type.as_deref() == Some("function_call") {
                        let idx = event.output_index.unwrap_or(0);
                        if let Some(call) = responses_item_to_call(&item) {
                            self.pending.remove(&idx);
                            self.push_deduped(&mut out, call);
                        } else if let Some(pending) = self.pending.remove(&idx) {
                            self.push_pending(&mut out, pending);
                        }
                    }
                }
            }
            Some("response.completed") => {
                if let Some(body) = event.response {
                    for item in &body.output {
                        if let Some(call) = responses_item_to_call(item) {
                            self.push_deduped(&mut out, call);
                        }
                    }
                    self.pending.clear();
                }
            }
            _ => {}
        }
        out
    }

    /// Emits one accumulated pending call, parsing its argument buffer.
    fn push_pending(&mut self, out: &mut Vec<CanonicalToolCall>, pending: ResponsesPendingCall) {
        if pending.name.is_empty() {
            return;
        }
        let arguments = if pending.arguments.trim().is_empty() {
            serde_json::json!({})
        } else {
            match serde_json::from_str::<serde_json::Value>(&pending.arguments) {
                Ok(value) => value,
                Err(err) => {
                    log::warn!(
                        "[ResponsesTransport] Rejecting invalid JSON tool arguments for {}: {}",
                        pending.name,
                        err
                    );
                    return;
                }
            }
        };
        let call_id = if pending.call_id.is_empty() {
            format!("call_{}", uuid::Uuid::new_v4().simple())
        } else {
            pending.call_id
        };
        self.push_deduped(
            out,
            CanonicalToolCall {
                id: call_id,
                name: pending.name,
                arguments,
            },
        );
    }

    /// Emits a call unless an identical (name, arguments) call already went out.
    fn push_deduped(&mut self, out: &mut Vec<CanonicalToolCall>, call: CanonicalToolCall) {
        let key = format!("{}:{}", call.name, call.arguments);
        if self.emitted.insert(key) {
            out.push(call);
        } else {
            log::warn!(
                "[ResponsesTransport] Dropping duplicate function_call emission: {}",
                call.name
            );
        }
    }
}

/// Builds the HTTP POST request payload for the OpenAI Responses API.
pub fn build_request_body(
    config: &ConnectionConfig,
    request: &GenerationRequest,
) -> serde_json::Value {
    let mut system_instructions = None;
    let mut input_items = Vec::new();

    for msg in &request.input.messages {
        if msg.role == Role::System {
            system_instructions = Some(msg.content.clone());
        } else {
            input_items.push(ResponsesInputItem {
                role: msg.role.to_string(),
                content: msg.content.clone(),
            });
        }
    }

    let mut body = serde_json::Map::new();
    body.insert("model".to_string(), serde_json::json!(config.model));
    body.insert("input".to_string(), serde_json::json!(input_items));
    body.insert("stream".to_string(), serde_json::json!(true));

    if let Some(instructions) = system_instructions {
        body.insert("instructions".to_string(), serde_json::json!(instructions));
    }
    if let Some(temp) = request.options.temperature {
        body.insert("temperature".to_string(), serde_json::json!(temp));
    }
    if request.options.reasoning == ReasoningMode::Disabled {
        log::warn!("[ResponsesTransport] Reasoning disable requested but the Responses API has no off-switch; sending without reasoning parameters.");
    }
    if let Some(top_p) = request.options.top_p {
        body.insert("top_p".to_string(), serde_json::json!(top_p));
    }
    if let Some(max_tokens) = request.options.max_output_tokens {
        body.insert(
            "max_output_tokens".to_string(),
            serde_json::json!(max_tokens),
        );
    }
    if let Some(ref tools) = request.tools {
        if !tools.is_empty() {
            body.insert(
                "tools".to_string(),
                serde_json::Value::Array(
                    tools
                        .iter()
                        .map(|t| {
                            serde_json::json!({
                                "type": "function",
                                "name": t.name,
                                "description": t.description,
                                "parameters": t.parameters
                            })
                        })
                        .collect(),
                ),
            );
            if let Some(choice) = config.policy.tool_choice {
                body.insert("tool_choice".to_string(), serde_json::json!(choice));
            }
        }
    }

    match &request.output {
        OutputConstraint::Text => {}
        OutputConstraint::JsonObject => {
            body.insert(
                "text".to_string(),
                serde_json::json!({ "format": { "type": "json_object" } }),
            );
        }
        OutputConstraint::JsonSchema {
            name,
            schema,
            strict,
        } => {
            body.insert(
                "text".to_string(),
                serde_json::json!({
                    "format": {
                        "type": "json_schema",
                        "name": name,
                        "schema": schema,
                        "strict": strict
                    }
                }),
            );
        }
    }

    serde_json::Value::Object(body)
}

/// Resolves the canonical responses endpoint URL.
pub fn resolve_url(base_url: &str) -> String {
    let trimmed = base_url.trim_end_matches('/');
    if trimmed.ends_with("/responses") {
        trimmed.to_string()
    } else if trimmed.ends_with("/v1") {
        format!("{}/responses", trimmed)
    } else {
        format!("{}/v1/responses", trimmed)
    }
}

/// Streams token generation from a `/v1/responses` endpoint.
pub async fn stream_responses(
    client: &reqwest::Client,
    config: &ConnectionConfig,
    request: &GenerationRequest,
    turn_id: u32,
    cancel: &tokio_util::sync::CancellationToken,
    tx: &mpsc::Sender<super::super::LlmStreamEvent>,
) -> Result<(), LlmError> {
    let url = resolve_url(&config.base_url);
    log::debug!("[Responses] Starting stream (turn: {}) to {}", turn_id, url);
    let req_body = build_request_body(config, request);

    let mut builder = client.post(&url).json(&req_body);
    builder = super::inject_auth_headers(builder, &config.auth);

    let response = tokio::select! {
        res = builder.send() => {
            res.map_err(|e| LlmError::Transport(e.to_string()))?
        }
        _ = cancel.cancelled() => {
            if let Err(e) = tx.send(super::super::LlmStreamEvent::Finished) {
                log::warn!("[Responses] Failed to send Finished on cancel: {}", e);
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
    let mut tool_accumulator = ResponsesToolAccumulator::default();

    loop {
        if cancel.is_cancelled() {
            if let Err(e) = tx.send(super::super::LlmStreamEvent::Finished) {
                log::warn!(
                    "[Responses] Failed to send Finished on cancel during stream: {}",
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
                        "[Responses] Failed to send Finished on select cancel: {}",
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
                    if line == "[DONE]" {
                        if let Err(e) = tx.send(super::super::LlmStreamEvent::Finished) {
                            log::warn!("[Responses] Send finished event error: {}", e);
                        }
                        return Ok(());
                    }

                    for call in tool_accumulator.feed_line(&line) {
                        log::info!(
                            "[ResponsesTransport] Emitting tool call: {} (id: {})",
                            call.name,
                            call.id
                        );
                        if let Err(e) = tx.send(super::super::LlmStreamEvent::ToolCall(call)) {
                            log::warn!("[Responses] Send tool call error: {}", e);
                        }
                    }
                    if let Ok(event) = serde_json::from_str::<ResponsesEvent>(&line) {
                        match event.event_type.as_deref() {
                            Some("response.output_text.delta") => {
                                if let Some(delta) = event.delta {
                                    if !delta.is_empty() {
                                        if let Err(e) =
                                            tx.send(super::super::LlmStreamEvent::Token(delta))
                                        {
                                            log::warn!("[Responses] Send token error: {}", e);
                                        }
                                    }
                                }
                            }
                            Some("response.completed") => {
                                if let Err(e) = tx.send(super::super::LlmStreamEvent::Finished) {
                                    log::warn!("[Responses] Send finished error: {}", e);
                                }
                                return Ok(());
                            }
                            _ => {}
                        }
                    }
                }
            }
            Some(Err(e)) => return Err(LlmError::Transport(e.to_string())),
            None => break,
        }
    }

    if let Some(line) = decoder.flush() {
        if line != "[DONE]" {
            if let Ok(event) = serde_json::from_str::<ResponsesEvent>(&line) {
                if event.event_type.as_deref() == Some("response.output_text.delta") {
                    if let Some(delta) = event.delta {
                        if !delta.is_empty() {
                            if let Err(e) = tx.send(super::super::LlmStreamEvent::Token(delta)) {
                                log::warn!("[Responses] Send flush token error: {}", e);
                            }
                        }
                    }
                }
            }
        }
    }

    if let Err(e) = tx.send(super::super::LlmStreamEvent::Finished) {
        log::warn!("[Responses] Send final finished event error: {}", e);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::{
        harness::{ChatMessage as MemMsg, Role},
        llm::{
            catalog::ProviderPresetMeta, AuthScheme, ConversationInput, GenerationOptions,
            GenerationPurpose, OutputConstraint, TransportType,
        },
    };

    #[test]
    fn test_responses_request_body_flattened_history() {
        let config = ConnectionConfig {
            transport: TransportType::Responses,
            base_url: "https://api.openai.com/v1".to_string(),
            model: "gpt-4o".to_string(),
            auth: AuthScheme::Bearer(Some("test".to_string())),
            provider_preset: Some("openai".to_string()),
            policy: ProviderPresetMeta::default(),
        };

        let request = GenerationRequest {
            input: ConversationInput {
                messages: vec![
                    MemMsg {
                        role: Role::System,
                        content: "You are a helpful voice assistant.".to_string(),
                        timestamp_ms: 100,
                        tool_call_id: None,
                        tool_calls: None,
                    },
                    MemMsg {
                        role: Role::User,
                        content: "Hello!".to_string(),
                        timestamp_ms: 200,
                        tool_call_id: None,
                        tool_calls: None,
                    },
                    MemMsg {
                        role: Role::Assistant,
                        content: "Hi there! How can I help?".to_string(),
                        timestamp_ms: 300,
                        tool_call_id: None,
                        tool_calls: None,
                    },
                    MemMsg {
                        role: Role::User,
                        content: "What's the weather?".to_string(),
                        timestamp_ms: 400,
                        tool_call_id: None,
                        tool_calls: None,
                    },
                ],
            },
            options: GenerationOptions {
                max_output_tokens: Some(512),
                temperature: Some(0.7),
                ..Default::default()
            },
            output: OutputConstraint::Text,
            purpose: GenerationPurpose::Conversation,
            tools: None,
        };

        let body = build_request_body(&config, &request);
        assert_eq!(body["model"], "gpt-4o");
        assert_eq!(body["instructions"], "You are a helpful voice assistant.");
        assert_eq!(body["max_output_tokens"], 512);

        let input_items = body["input"].as_array().expect("input must be an array");
        assert_eq!(input_items.len(), 3);
        assert_eq!(input_items[0]["role"], "user");
        assert_eq!(input_items[0]["content"], "Hello!");
        assert_eq!(input_items[1]["role"], "assistant");
        assert_eq!(input_items[1]["content"], "Hi there! How can I help?");
        assert_eq!(input_items[2]["role"], "user");
        assert_eq!(input_items[2]["content"], "What's the weather?");
    }

    #[test]
    fn test_responses_typed_event_parsing() {
        let json_line = r#"{"type":"response.output_text.delta","delta":"Vox is ready."}"#;
        let event =
            serde_json::from_str::<ResponsesEvent>(json_line).expect("must parse typed event");
        assert_eq!(
            event.event_type.as_deref(),
            Some("response.output_text.delta")
        );
        assert_eq!(event.delta.as_deref(), Some("Vox is ready."));
    }
}
