use super::config::ConnectionConfig;
use super::sse::SseDecoder;
use crate::core::events::VoxEvent;
use crate::services::llm::{GenerationRequest, LlmError, OutputConstraint};
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use std::sync::mpsc;

#[derive(Serialize)]
struct ChatMessage {
    role: String,
    content: String,
}

#[derive(Deserialize)]
struct ChatCompletionChunk {
    #[serde(default)]
    choices: Vec<ChunkChoice>,
}

#[derive(Deserialize)]
struct ChunkChoice {
    delta: ChunkDelta,
}

#[derive(Deserialize)]
struct ChunkDelta {
    content: Option<String>,
}

/// Builds the HTTP POST request payload for Chat Completions.
pub fn build_request_body(
    config: &ConnectionConfig,
    request: &GenerationRequest,
) -> serde_json::Value {
    let messages: Vec<ChatMessage> = request
        .input
        .messages
        .iter()
        .map(|m| ChatMessage {
            role: m.role.to_string(),
            content: m.content.clone(),
        })
        .collect();

    let mut body = serde_json::Map::new();
    body.insert("model".to_string(), serde_json::json!(config.model));
    body.insert("messages".to_string(), serde_json::json!(messages));
    body.insert("stream".to_string(), serde_json::json!(true));
    body.insert(
        "stream_options".to_string(),
        serde_json::json!({ "include_usage": true }),
    );

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

    serde_json::Value::Object(body)
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

/// Streams token generation from a `/v1/chat/completions` endpoint.
pub async fn stream_chat_completions(
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
                log::warn!("[ChatCompletions] Failed to dispatch Cancelled: {}", e);
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
                log::warn!("[ChatCompletions] Failed to dispatch Cancelled: {}", e);
            }
            return Ok(());
        }

        let chunk_opt = tokio::select! {
            chunk = byte_stream.next() => chunk,
            _ = cancel.cancelled() => {
                if let Err(e) = tx.send(VoxEvent::Cancelled { turn_id }) {
                    log::warn!("[ChatCompletions] Cancel dispatch error: {}", e);
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
                            log::warn!("[ChatCompletions] Send finished event error: {}", e);
                        }
                        return Ok(());
                    }

                    if let Ok(chunk) = serde_json::from_str::<ChatCompletionChunk>(&line) {
                        if let Some(choice) = chunk.choices.first() {
                            if let Some(token) = &choice.delta.content {
                                if !token.is_empty() {
                                    if let Err(e) = tx.send(VoxEvent::LlmToken {
                                        turn_id,
                                        token: token.clone(),
                                    }) {
                                        log::warn!("[ChatCompletions] Send token error: {}", e);
                                    }
                                }
                            }
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
            if let Ok(chunk) = serde_json::from_str::<ChatCompletionChunk>(&line) {
                if let Some(choice) = chunk.choices.first() {
                    if let Some(token) = &choice.delta.content {
                        if !token.is_empty() {
                            let _ = tx.send(VoxEvent::LlmToken {
                                turn_id,
                                token: token.clone(),
                            });
                        }
                    }
                }
            }
        }
    }

    if let Err(e) = tx.send(VoxEvent::LlmFinished { turn_id }) {
        log::warn!("[ChatCompletions] Send final finished event error: {}", e);
    }
    Ok(())
}
