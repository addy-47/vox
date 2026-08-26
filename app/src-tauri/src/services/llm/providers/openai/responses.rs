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

/// Adapter for OpenAI `/v1/responses` API endpoints.
pub struct ResponsesAdapter {
    base_url: String,
    model: String,
    api_key: Option<String>,
    client: reqwest::Client,
    capabilities: ProviderCapabilities,
}

impl ResponsesAdapter {
    /// Creates a new Responses API provider adapter.
    pub fn new(
        base_url: &str,
        model: &str,
        api_key: Option<&str>,
        client: reqwest::Client,
    ) -> Self {
        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            model: model.to_string(),
            api_key: api_key.map(|s| s.to_string()),
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

    /// Returns static capabilities of the Responses API.
    pub fn capabilities(&self) -> &ProviderCapabilities {
        &self.capabilities
    }

    /// Injects authentication bearer header into the request.
    fn inject_headers(&self, mut builder: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        if let Some(ref key) = self.api_key {
            builder = builder.bearer_auth(key);
        }
        builder
    }

    /// Streams token generation from the remote OpenAI Responses API.
    pub fn generate<'a>(
        &'a self,
        request: GenerationRequest,
        turn_id: u32,
        cancel_flag: &'a Arc<AtomicBool>,
        tx: &'a mpsc::Sender<VoxEvent>,
    ) -> BoxFuture<'a, Result<(), LlmError>> {
        Box::pin(async move {
            let url = if self.base_url.ends_with("/responses") {
                self.base_url.clone()
            } else if self.base_url.ends_with("/v1") {
                format!("{}/responses", self.base_url)
            } else {
                format!("{}/v1/responses", self.base_url)
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

    /// Builds the serialized JSON payload for the Responses API.
    fn build_request_body(&self, request: &GenerationRequest) -> serde_json::Value {
        let input_messages: Vec<serde_json::Value> = request
            .input
            .messages
            .iter()
            .map(|m| serde_json::json!({ "role": m.role.to_string(), "content": m.content }))
            .collect();

        let mut req_map = serde_json::Map::new();
        req_map.insert("model".to_string(), serde_json::json!(self.model));
        req_map.insert("input".to_string(), serde_json::json!(input_messages));
        req_map.insert("stream".to_string(), serde_json::json!(true));

        if let Some(temp) = request.options.temperature {
            req_map.insert("temperature".to_string(), serde_json::json!(temp));
        }
        if let Some(max_tokens) = request.options.max_output_tokens {
            req_map.insert(
                "max_output_tokens".to_string(),
                serde_json::json!(max_tokens),
            );
        }

        match &request.output {
            OutputConstraint::Text => {}
            OutputConstraint::JsonObject => {
                req_map.insert(
                    "text".to_string(),
                    serde_json::json!({ "format": { "type": "json_object" } }),
                );
            }
            OutputConstraint::JsonSchema {
                name,
                schema,
                strict,
            } => {
                req_map.insert(
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

        serde_json::Value::Object(req_map)
    }

    /// Reads streaming Server-Sent Events from the response and sends tokens.
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
                    log::warn!("[ResponsesAPI] Send cancel error: {}", e);
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
                            log::error!("[ResponsesAPI] Stream read error: {}", e);
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
                            struct DeltaObj {
                                text: Option<String>,
                            }
                            #[derive(Deserialize)]
                            struct ResponsesChunk {
                                delta: Option<DeltaObj>,
                            }

                            if let Some(data) = trimmed.strip_prefix("data:") {
                                let d = data.trim();
                                if d == "[DONE]" {
                                    finished = true;
                                    break;
                                }
                                if let Ok(c) = serde_json::from_str::<ResponsesChunk>(d) {
                                    if let Some(delta) = c.delta {
                                        if let Some(token) = delta.text {
                                            if !token.is_empty() {
                                                if let Err(e) =
                                                    tx.send(VoxEvent::LlmToken { turn_id, token })
                                                {
                                                    log::warn!(
                                                        "[ResponsesAPI] Send token error: {}",
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
                log::warn!("[ResponsesAPI] Send cancel error: {}", e);
            }
        } else if let Err(e) = tx.send(VoxEvent::LlmFinished { turn_id }) {
            log::warn!("[ResponsesAPI] Send finished error: {}", e);
        }

        Ok(())
    }
}
