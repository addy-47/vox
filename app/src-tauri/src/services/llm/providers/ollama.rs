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

/// Adapter for Ollama `/api/chat` native daemon endpoints.
pub struct OllamaAdapter {
    base_url: String,
    model: String,
    client: reqwest::Client,
    capabilities: ProviderCapabilities,
}

impl OllamaAdapter {
    /// Creates a new Ollama provider adapter.
    pub fn new(base_url: &str, model: &str, client: reqwest::Client) -> Self {
        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            model: model.to_string(),
            client,
            capabilities: ProviderCapabilities {
                temperature: Support::Supported,
                top_p: Support::Supported,
                top_k: Support::Supported,
                max_output_tokens: Support::Supported,
                json_object: Support::Supported,
                json_schema: Support::Supported,
                streaming: Support::Supported,
                seed: Support::Supported,
            },
        }
    }

    /// Returns static capabilities of Ollama.
    pub fn capabilities(&self) -> &ProviderCapabilities {
        &self.capabilities
    }

    /// Streams token generation from the remote Ollama daemon.
    pub fn generate<'a>(
        &'a self,
        request: GenerationRequest,
        turn_id: u32,
        cancel_flag: &'a Arc<AtomicBool>,
        tx: &'a mpsc::Sender<VoxEvent>,
    ) -> BoxFuture<'a, Result<(), LlmError>> {
        Box::pin(async move {
            let url = format!("{}/api/chat", self.base_url);
            let req_body = self.build_request_body(&request);

            let response = tokio::select! {
                res = self.client.post(&url).json(&req_body).send() => {
                    res.map_err(|e| LlmError::Transport(e.to_string()))?
                }
                _ = async {
                    while !cancel_flag.load(Ordering::Relaxed) {
                        tokio::time::sleep(Duration::from_millis(50)).await;
                    }
                } => {
                    if let Err(e) = tx.send(VoxEvent::Cancelled { turn_id }) {
                        log::warn!("[Ollama] Failed to send Cancelled: {}", e);
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

            self.stream_response(response, turn_id, cancel_flag, tx)
                .await
        })
    }

    /// Builds the serialized JSON payload for Ollama `/api/chat`.
    fn build_request_body(&self, request: &GenerationRequest) -> serde_json::Value {
        let messages: Vec<serde_json::Value> = request
            .input
            .messages
            .iter()
            .map(|m| serde_json::json!({ "role": m.role.to_string(), "content": m.content }))
            .collect();

        let mut options_map = serde_json::Map::new();

        if let Some(temp) = request.options.temperature {
            options_map.insert("temperature".to_string(), serde_json::json!(temp));
        }
        if let Some(top_p) = request.options.top_p {
            options_map.insert("top_p".to_string(), serde_json::json!(top_p));
        }
        if let Some(top_k) = request.options.top_k {
            options_map.insert("top_k".to_string(), serde_json::json!(top_k));
        }
        if let Some(max_tokens) = request.options.max_output_tokens {
            options_map.insert("num_predict".to_string(), serde_json::json!(max_tokens));
        }
        if !request.options.stop.is_empty() {
            options_map.insert("stop".to_string(), serde_json::json!(request.options.stop));
        }
        if let Some(seed) = request.options.seed {
            options_map.insert("seed".to_string(), serde_json::json!(seed));
        }

        let mut req_body = serde_json::json!({
            "model": self.model,
            "messages": messages,
            "stream": true,
            "options": options_map
        });

        if let OutputConstraint::JsonObject = &request.output {
            if let Some(obj) = req_body.as_object_mut() {
                obj.insert("format".to_string(), serde_json::json!("json"));
            }
        }

        req_body
    }

    /// Reads streaming JSON chunks from the Ollama response stream.
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
                    log::warn!("[Ollama] Failed to send Cancelled: {}", e);
                }
                return Ok(());
            }

            let chunk_opt =
                match tokio::time::timeout(Duration::from_millis(150), stream.next()).await {
                    Ok(Some(chunk_result)) => match chunk_result {
                        Ok(c) => Some(c),
                        Err(e) => {
                            log::error!("[Ollama] Stream read error: {}", e);
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
                            struct OllamaMsg {
                                content: String,
                            }
                            #[derive(Deserialize)]
                            struct OllamaChunk {
                                message: Option<OllamaMsg>,
                                done: Option<bool>,
                            }

                            if let Ok(chunk) = serde_json::from_str::<OllamaChunk>(trimmed) {
                                if let Some(msg) = chunk.message {
                                    if !msg.content.is_empty() {
                                        if let Err(e) = tx.send(VoxEvent::LlmToken {
                                            turn_id,
                                            token: msg.content,
                                        }) {
                                            log::warn!("[Ollama] Send token error: {}", e);
                                        }
                                    }
                                }
                                if chunk.done.unwrap_or(false) {
                                    finished = true;
                                    break;
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
                log::warn!("[Ollama] Failed to send Cancelled: {}", e);
            }
        } else if let Err(e) = tx.send(VoxEvent::LlmFinished { turn_id }) {
            log::warn!("[Ollama] Failed to send LlmFinished: {}", e);
        }

        Ok(())
    }
}
