use std::sync::mpsc;

use futures_util::StreamExt;
use serde::{Deserialize, Serialize};

use super::{config::ConnectionConfig, sse::SseDecoder};
use crate::services::llm::{GenerationRequest, LlmError, OutputConstraint};

#[derive(Serialize)]
struct OllamaMessage {
    role: String,
    content: String,
}

#[derive(Deserialize)]
struct OllamaChatChunk {
    message: Option<OllamaChunkMessage>,
    done: Option<bool>,
}

#[derive(Deserialize)]
struct OllamaChunkMessage {
    content: Option<String>,
}

/// Builds the HTTP POST request payload for Ollama `/api/chat`.
pub fn build_request_body(
    config: &ConnectionConfig,
    request: &GenerationRequest,
) -> serde_json::Value {
    let messages: Vec<OllamaMessage> = request
        .input
        .messages
        .iter()
        .map(|m| OllamaMessage {
            role: m.role.to_string(),
            content: m.content.clone(),
        })
        .collect();

    let mut options = serde_json::Map::new();
    if let Some(temp) = request.options.temperature {
        options.insert("temperature".to_string(), serde_json::json!(temp));
    }
    if let Some(top_p) = request.options.top_p {
        options.insert("top_p".to_string(), serde_json::json!(top_p));
    }
    if let Some(top_k) = request.options.top_k {
        options.insert("top_k".to_string(), serde_json::json!(top_k));
    }
    if let Some(max_tokens) = request.options.max_output_tokens {
        options.insert("num_predict".to_string(), serde_json::json!(max_tokens));
    }
    if !request.options.stop.is_empty() {
        options.insert("stop".to_string(), serde_json::json!(request.options.stop));
    }
    if let Some(seed) = request.options.seed {
        options.insert("seed".to_string(), serde_json::json!(seed));
    }

    let mut body = serde_json::Map::new();
    body.insert("model".to_string(), serde_json::json!(config.model));
    body.insert("messages".to_string(), serde_json::json!(messages));
    body.insert("stream".to_string(), serde_json::json!(true));
    body.insert("options".to_string(), serde_json::Value::Object(options));

    match &request.output {
        OutputConstraint::Text => {}
        OutputConstraint::JsonObject | OutputConstraint::JsonSchema { .. } => {
            body.insert("format".to_string(), serde_json::json!("json"));
        }
    }

    serde_json::Value::Object(body)
}

/// Resolves the canonical Ollama chat endpoint URL.
pub fn resolve_url(base_url: &str) -> String {
    let trimmed = base_url.trim_end_matches('/');
    if trimmed.ends_with("/api/chat") {
        trimmed.to_string()
    } else {
        format!("{}/api/chat", trimmed)
    }
}

/// Streams token generation from an Ollama `/api/chat` endpoint.
pub async fn stream_ollama(
    client: &reqwest::Client,
    config: &ConnectionConfig,
    request: &GenerationRequest,
    turn_id: u32,
    cancel: &tokio_util::sync::CancellationToken,
    tx: &mpsc::Sender<super::super::LlmStreamEvent>,
) -> Result<(), LlmError> {
    let url = resolve_url(&config.base_url);
    log::debug!(
        "[OllamaTransport] Starting stream (turn: {}) to {}",
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
                log::warn!("[OllamaTransport] Failed to send Finished on cancel: {}", e);
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
            if let Err(e) = tx.send(super::super::LlmStreamEvent::Finished) {
                log::warn!(
                    "[OllamaTransport] Failed to send Finished on cancel during stream: {}",
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
                        "[OllamaTransport] Failed to send Finished on select cancel: {}",
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
                    if let Ok(chunk) = serde_json::from_str::<OllamaChatChunk>(&line) {
                        if let Some(msg) = chunk.message {
                            if let Some(content) = msg.content {
                                if !content.is_empty() {
                                    if let Err(e) =
                                        tx.send(super::super::LlmStreamEvent::Token(content))
                                    {
                                        log::warn!("[OllamaTransport] Send token error: {}", e);
                                    }
                                }
                            }
                        }
                        if chunk.done.unwrap_or(false) {
                            if let Err(e) = tx.send(super::super::LlmStreamEvent::Finished) {
                                log::warn!("[OllamaTransport] Send finished error: {}", e);
                            }
                            return Ok(());
                        }
                    }
                }
            }
            Some(Err(e)) => return Err(LlmError::Transport(e.to_string())),
            None => break,
        }
    }

    if let Some(line) = decoder.flush() {
        if let Ok(chunk) = serde_json::from_str::<OllamaChatChunk>(&line) {
            if let Some(msg) = chunk.message {
                if let Some(content) = msg.content {
                    if !content.is_empty() {
                        if let Err(e) = tx.send(super::super::LlmStreamEvent::Token(content)) {
                            log::warn!("[OllamaTransport] Send flush token error: {}", e);
                        }
                    }
                }
            }
        }
    }

    if let Err(e) = tx.send(super::super::LlmStreamEvent::Finished) {
        log::warn!("[OllamaTransport] Send final finished event error: {}", e);
    }
    Ok(())
}
