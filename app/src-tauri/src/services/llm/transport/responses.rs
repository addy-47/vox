use super::sse::SseDecoder;
use crate::core::events::VoxEvent;
use crate::services::llm::config::ConnectionConfig;
use crate::services::llm::{GenerationRequest, LlmError, OutputConstraint};
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use std::sync::mpsc;

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
}

/// Builds the HTTP POST request payload for the OpenAI Responses API.
pub fn build_request_body(
    config: &ConnectionConfig,
    request: &GenerationRequest,
) -> serde_json::Value {
    let mut system_instructions = None;
    let mut input_items = Vec::new();

    for msg in &request.input.messages {
        if msg.role == crate::services::memory::Role::System {
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
    if let Some(top_p) = request.options.top_p {
        body.insert("top_p".to_string(), serde_json::json!(top_p));
    }
    if let Some(max_tokens) = request.options.max_output_tokens {
        body.insert(
            "max_output_tokens".to_string(),
            serde_json::json!(max_tokens),
        );
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
    tx: &mpsc::Sender<VoxEvent>,
) -> Result<(), LlmError> {
    let url = resolve_url(&config.base_url);
    let req_body = build_request_body(config, request);

    let mut builder = client.post(&url).json(&req_body);
    builder = super::inject_auth_headers(builder, &config.auth);

    let response = tokio::select! {
        res = builder.send() => {
            res.map_err(|e| LlmError::Transport(e.to_string()))?
        }
        _ = cancel.cancelled() => {
            if let Err(e) = tx.send(VoxEvent::Cancelled { turn_id }) {
                log::warn!("[Responses] Failed to dispatch Cancelled: {}", e);
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

    loop {
        if cancel.is_cancelled() {
            if let Err(e) = tx.send(VoxEvent::Cancelled { turn_id }) {
                log::warn!("[Responses] Failed to dispatch Cancelled: {}", e);
            }
            return Ok(());
        }

        let chunk_opt = tokio::select! {
            chunk = byte_stream.next() => chunk,
            _ = cancel.cancelled() => {
                if let Err(e) = tx.send(VoxEvent::Cancelled { turn_id }) {
                    log::warn!("[Responses] Cancel dispatch error: {}", e);
                }
                return Ok(());
            }
        };

        match chunk_opt {
            Some(Ok(bytes)) => {
                let lines = decoder.decode_chunk(&bytes);
                for line in lines {
                    if line == "[DONE]" {
                        if let Err(e) = tx.send(VoxEvent::LlmFinished { turn_id }) {
                            log::warn!("[Responses] Send finished event error: {}", e);
                        }
                        return Ok(());
                    }

                    if let Ok(event) = serde_json::from_str::<ResponsesEvent>(&line) {
                        match event.event_type.as_deref() {
                            Some("response.output_text.delta") => {
                                if let Some(delta) = event.delta {
                                    if !delta.is_empty() {
                                        if let Err(e) = tx.send(VoxEvent::LlmToken {
                                            turn_id,
                                            token: delta,
                                        }) {
                                            log::warn!("[Responses] Send token error: {}", e);
                                        }
                                    }
                                }
                            }
                            Some("response.completed") => {
                                if let Err(e) = tx.send(VoxEvent::LlmFinished { turn_id }) {
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
                            let _ = tx.send(VoxEvent::LlmToken {
                                turn_id,
                                token: delta,
                            });
                        }
                    }
                }
            }
        }
    }

    if let Err(e) = tx.send(VoxEvent::LlmFinished { turn_id }) {
        log::warn!("[Responses] Send final finished event error: {}", e);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::llm::config::TokenLimitField;
    use crate::services::llm::{
        ConversationInput, GenerationOptions, GenerationPurpose, OutputConstraint,
    };
    use crate::services::memory::{ChatMessage as MemMsg, Role};

    #[test]
    fn test_responses_request_body_flattened_history() {
        let config = ConnectionConfig {
            transport: crate::services::llm::config::TransportType::Responses,
            base_url: "https://api.openai.com/v1".to_string(),
            model: "gpt-4o".to_string(),
            auth: crate::services::llm::config::AuthScheme::Bearer(Some("test".to_string())),
            token_limit_field: TokenLimitField::MaxOutputTokens,
            capability_source: crate::services::llm::config::CapabilitySource::ProbedGeneric,
            provider_preset: Some("openai".to_string()),
        };

        let request = GenerationRequest {
            input: ConversationInput {
                messages: vec![
                    MemMsg {
                        role: Role::System,
                        content: "You are a helpful voice assistant.".to_string(),
                        timestamp_ms: 100,
                    },
                    MemMsg {
                        role: Role::User,
                        content: "Hello!".to_string(),
                        timestamp_ms: 200,
                    },
                    MemMsg {
                        role: Role::Assistant,
                        content: "Hi there! How can I help?".to_string(),
                        timestamp_ms: 300,
                    },
                    MemMsg {
                        role: Role::User,
                        content: "What's the weather?".to_string(),
                        timestamp_ms: 400,
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
