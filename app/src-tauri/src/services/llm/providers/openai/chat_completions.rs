use crate::core::events::VoxEvent;
use crate::services::llm::types::{
    GenerationRequest, LlmError, OutputConstraint, ProviderCapabilities, Support,
};
use futures_util::future::BoxFuture;
use futures_util::StreamExt;
use serde::Deserialize;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::Arc;
use std::time::Duration;

/// Adapter for standard OpenAI `/v1/chat/completions` API endpoints.
pub struct ChatCompletionsAdapter {
    base_url: String,
    model: String,
    api_key: Option<String>,
    provider_name: Option<String>,
    client: reqwest::Client,
    capabilities: ProviderCapabilities,
}

impl ChatCompletionsAdapter {
    /// Creates a new chat completions adapter instance.
    pub fn new(
        base_url: &str,
        model: &str,
        api_key: Option<&str>,
        provider_name: Option<&str>,
        client: reqwest::Client,
    ) -> Self {
        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            model: model.to_string(),
            api_key: api_key.map(|s| s.to_string()),
            provider_name: provider_name.map(|s| s.to_string()),
            client,
            capabilities: ProviderCapabilities {
                temperature: Support::Supported,
                top_p: Support::Supported,
                top_k: Support::Unknown,
                max_output_tokens: Support::Supported,
                json_object: Support::Supported,
                json_schema: Support::Supported,
                streaming: Support::Supported,
                seed: Support::Supported,
            },
        }
    }

    /// Returns static capabilities of the chat completions endpoint.
    pub fn capabilities(&self) -> &ProviderCapabilities {
        &self.capabilities
    }

    /// Injects authentication and provider-specific headers into the request.
    fn inject_headers(&self, mut builder: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        if let Some(ref name) = self.provider_name {
            if name.to_lowercase() == "anthropic" {
                builder = builder.header("anthropic-version", "2023-06-01");
                if let Some(ref key) = self.api_key {
                    builder = builder.header("x-api-key", key);
                }
                return builder;
            }
        }
        if let Some(ref key) = self.api_key {
            builder = builder.bearer_auth(key);
        }
        builder
    }

    /// Streams token generation from the remote OpenAI chat completions endpoint.
    pub fn generate<'a>(
        &'a self,
        request: GenerationRequest,
        turn_id: u32,
        cancel_flag: &'a Arc<AtomicBool>,
        tx: &'a mpsc::Sender<VoxEvent>,
    ) -> BoxFuture<'a, Result<(), LlmError>> {
        Box::pin(async move {
            let url = if self.base_url.ends_with("/chat/completions") {
                self.base_url.clone()
            } else if self.base_url.ends_with("/v1") || self.base_url.ends_with("/openai") {
                format!("{}/chat/completions", self.base_url)
            } else {
                format!("{}/v1/chat/completions", self.base_url)
            };

            let req_body = self.build_request_body(&request);
            let mut builder = self.client.post(&url).json(&req_body);
            builder = self.inject_headers(builder);

            let response = builder
                .send()
                .await
                .map_err(|e| LlmError::Transport(e.to_string()))?;

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

            self.stream_response(response, turn_id, cancel_flag, tx)
                .await
        })
    }

    /// Builds the serialized JSON payload for chat completions.
    fn build_request_body(&self, request: &GenerationRequest) -> serde_json::Value {
        let messages: Vec<serde_json::Value> = request
            .input
            .messages
            .iter()
            .map(|m| serde_json::json!({ "role": m.role.to_string(), "content": m.content }))
            .collect();

        let mut req_map = serde_json::Map::new();
        req_map.insert("model".to_string(), serde_json::json!(self.model));
        req_map.insert("messages".to_string(), serde_json::json!(messages));
        req_map.insert("stream".to_string(), serde_json::json!(true));

        if let Some(temp) = request.options.temperature {
            req_map.insert("temperature".to_string(), serde_json::json!(temp));
        }
        if let Some(top_p) = request.options.top_p {
            req_map.insert("top_p".to_string(), serde_json::json!(top_p));
        }
        if let Some(max_tokens) = request.options.max_output_tokens {
            req_map.insert(
                "max_completion_tokens".to_string(),
                serde_json::json!(max_tokens),
            );
        }
        if !request.options.stop.is_empty() {
            req_map.insert("stop".to_string(), serde_json::json!(request.options.stop));
        }
        if let Some(seed) = request.options.seed {
            req_map.insert("seed".to_string(), serde_json::json!(seed));
        }

        match &request.output {
            OutputConstraint::Text => {}
            OutputConstraint::JsonObject => {
                req_map.insert(
                    "response_format".to_string(),
                    serde_json::json!({ "type": "json_object" }),
                );
            }
            OutputConstraint::JsonSchema {
                name,
                schema,
                strict,
            } => {
                req_map.insert(
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

        serde_json::Value::Object(req_map)
    }

    /// Reads streaming Server-Sent Events from the response and emits tokens.
    async fn stream_response(
        &self,
        response: reqwest::Response,
        turn_id: u32,
        cancel_flag: &Arc<AtomicBool>,
        tx: &mpsc::Sender<VoxEvent>,
    ) -> Result<(), LlmError> {
        let mut stream = response.bytes_stream();
        let mut buffer = Vec::new();
        let mut finished = false;

        loop {
            if cancel_flag.load(Ordering::Relaxed) {
                if let Err(e) = tx.send(VoxEvent::Cancelled { turn_id }) {
                    log::warn!("[ChatCompletions] Send cancel error: {}", e);
                }
                return Ok(());
            }

            let chunk_opt = match tokio::time::timeout(
                Duration::from_millis(
                    crate::services::llm::DEFAULT_STREAM_CHUNK_TIMEOUT_MS,
                ),
                stream.next(),
            )
            .await
            {
                    Ok(Some(chunk_result)) => match chunk_result {
                        Ok(c) => Some(c),
                        Err(e) => {
                            log::error!("[ChatCompletions] Stream read error: {}", e);
                            break;
                        }
                    },
                    Ok(None) => break,
                    Err(_) => None,
                };

            if let Some(chunk) = chunk_opt {
                buffer.extend_from_slice(&chunk);

                while let Some(pos) = buffer.iter().position(|&b| b == b'\n') {
                    let line_bytes = buffer.drain(..=pos).collect::<Vec<u8>>();
                    if let Ok(line) = String::from_utf8(line_bytes) {
                        let trimmed = line.trim();
                        if !trimmed.is_empty() {
                            #[derive(Deserialize)]
                            struct ChunkDelta {
                                content: Option<String>,
                            }
                            #[derive(Deserialize)]
                            struct ChunkChoice {
                                delta: ChunkDelta,
                            }
                            #[derive(Deserialize)]
                            struct Chunk {
                                choices: Vec<ChunkChoice>,
                            }

                            if let Some(data) = trimmed.strip_prefix("data:") {
                                let d = data.trim();
                                if d == "[DONE]" {
                                    finished = true;
                                    break;
                                }
                                if let Ok(c) = serde_json::from_str::<Chunk>(d) {
                                    if let Some(choice) = c.choices.first() {
                                        if let Some(token) = &choice.delta.content {
                                            if !token.is_empty() {
                                                if let Err(e) = tx.send(VoxEvent::LlmToken {
                                                    turn_id,
                                                    token: token.clone(),
                                                }) {
                                                    log::warn!(
                                                        "[ChatCompletions] Send token error: {}",
                                                        e
                                                    );
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                if finished {
                    break;
                }
            }
        }

        if !finished && cancel_flag.load(Ordering::Relaxed) {
            if let Err(e) = tx.send(VoxEvent::Cancelled { turn_id }) {
                log::warn!("[ChatCompletions] Send cancel error: {}", e);
            }
        } else if let Err(e) = tx.send(VoxEvent::LlmFinished { turn_id }) {
            log::warn!("[ChatCompletions] Send finished error: {}", e);
        }

        Ok(())
    }
}
