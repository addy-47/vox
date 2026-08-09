use super::{LlmProvider, ProviderKind};
use crate::core::events::VoxEvent;
use crate::core::settings::LlmModelInfo;
use crate::services::llm::types::{
    GenerationRequest, LlmError, OutputConstraint, ProviderCapabilities, Support,
};
use futures_util::future::BoxFuture;
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::Arc;
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LocalBackendKind {
    Ollama,
    LmStudio,
    StandardOpenAi,
}

pub struct OpenAiCompatProvider {
    base_url: String,
    model: String,
    api_key: Option<String>,
    provider_name: Option<String>,
    async_client: reqwest::Client,
    backend_kind: std::sync::OnceLock<LocalBackendKind>,
    capabilities: ProviderCapabilities,
}

impl OpenAiCompatProvider {
    pub fn new(
        base_url: &str,
        model: &str,
        api_key: Option<&str>,
        provider_name: Option<&str>,
    ) -> Self {
        let mut resolved_url = base_url.trim_end_matches('/').to_string();
        if let Some(p_name) = provider_name {
            let p_lower = p_name.to_lowercase();
            if resolved_url.is_empty()
                || resolved_url == "http://127.0.0.1:11434"
                || resolved_url == "http://localhost:11434"
                || resolved_url.contains("api.openai.com")
                || resolved_url.contains("generativelanguage.googleapis.com")
                || resolved_url.contains("api.anthropic.com")
                || resolved_url.contains("api.nvidia.com")
            {
                if p_lower == "openai" {
                    resolved_url = "https://api.openai.com".to_string();
                } else if p_lower == "gemini" {
                    resolved_url =
                        "https://generativelanguage.googleapis.com/v1beta/openai".to_string();
                } else if p_lower == "anthropic" {
                    resolved_url = "https://api.anthropic.com".to_string();
                } else if p_lower == "nvidia" {
                    resolved_url = "https://integrate.api.nvidia.com/v1".to_string();
                }
            }
        }

        let async_client = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(5))
            .timeout(Duration::from_secs(180))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());

        Self {
            base_url: resolved_url,
            model: model.to_string(),
            api_key: api_key.map(|s| s.trim_matches(|c| c == '"' || c == '\'').to_string()),
            provider_name: provider_name.map(|s| s.to_string()),
            async_client,
            backend_kind: std::sync::OnceLock::new(),
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

    pub fn inject_headers(&self, mut builder: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        if let Some(ref name) = self.provider_name {
            let name_lower = name.to_lowercase();
            if name_lower == "anthropic" {
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

    pub fn detect_backend_kind(&self) -> LocalBackendKind {
        *self.backend_kind.get_or_init(|| {
            let mut detected = LocalBackendKind::StandardOpenAi;

            // Try to probe Ollama
            let ollama_url = format!("{}/api/tags", self.base_url);
            let mut builder = self
                .async_client
                .get(&ollama_url)
                .timeout(Duration::from_secs(2));
            builder = self.inject_headers(builder);

            let is_ollama = block_on(async {
                match builder.send().await {
                    Ok(resp) => resp.status().is_success(),
                    Err(_) => false,
                }
            });

            if is_ollama {
                log::info!(
                    "[OpenAiCompat] Detected Ollama native backend at {}",
                    self.base_url
                );
                detected = LocalBackendKind::Ollama;
            } else {
                // Try to probe LM Studio
                let lms_url = format!("{}/v1/models", self.base_url);
                let mut builder = self
                    .async_client
                    .get(&lms_url)
                    .timeout(Duration::from_secs(2));
                builder = self.inject_headers(builder);

                let is_lms = block_on(async {
                    match builder.send().await {
                        Ok(resp) => resp.status().is_success(),
                        Err(_) => false,
                    }
                });

                if is_lms {
                    log::info!(
                        "[OpenAiCompat] Detected LM Studio native backend at {}",
                        self.base_url
                    );
                    detected = LocalBackendKind::LmStudio;
                } else {
                    log::info!(
                        "[OpenAiCompat] Fallback to standard OpenAI compatibility at {}",
                        self.base_url
                    );
                }
            }

            detected
        })
    }
}

#[derive(Serialize)]
struct ChatMessage {
    role: String,
    content: String,
}

#[derive(Deserialize)]
struct ChatCompletionChunk {
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

#[derive(Deserialize)]
struct ModelList {
    data: Vec<ModelEntry>,
}

#[derive(Deserialize)]
struct ModelEntry {
    id: String,
}

#[derive(Deserialize)]
struct OllamaTagsResponse {
    models: Vec<OllamaModelEntry>,
}

#[derive(Deserialize)]
struct OllamaModelEntry {
    name: String,
    size: u64,
    details: OllamaModelDetails,
}

#[derive(Deserialize)]
struct OllamaModelDetails {
    quantization_level: Option<String>,
    family: Option<String>,
}

impl LlmProvider for OpenAiCompatProvider {
    fn generate<'a>(
        &'a self,
        request: GenerationRequest,
        turn_id: u32,
        cancel_flag: &'a Arc<AtomicBool>,
        tx: &'a mpsc::Sender<VoxEvent>,
    ) -> BoxFuture<'a, Result<(), LlmError>> {
        Box::pin(async move {
            log::info!(
                "[OpenAiCompat] Starting generation for turn {} on model {} with url {} ({} messages in input)",
                turn_id,
                self.model,
                self.base_url,
                request.input.messages.len()
            );

            let last_user_text = request
                .input
                .messages
                .last()
                .map(|m| m.content.as_str())
                .unwrap_or("");
            if user_text_is_warmup(last_user_text) {
                log::info!("[OpenAiCompat] Warmup request received. Skipping remote LLM call.");
                return Ok(());
            }

            let messages: Vec<ChatMessage> = request
                .input
                .messages
                .iter()
                .map(|m| ChatMessage {
                    role: m.role.to_string(),
                    content: m.content.clone(),
                })
                .collect();

            let response_format = match &request.output {
                OutputConstraint::Text => None,
                OutputConstraint::JsonObject => Some(serde_json::json!({ "type": "json_object" })),
                OutputConstraint::JsonSchema {
                    name,
                    schema,
                    strict,
                } => Some(serde_json::json!({
                    "type": "json_schema",
                    "json_schema": {
                        "name": name,
                        "schema": schema,
                        "strict": strict
                    }
                })),
            };

            let kind = self.detect_backend_kind();

            let (url, req_body) = match kind {
                LocalBackendKind::Ollama => {
                    let url = format!("{}/api/chat", self.base_url);
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
                        options_map
                            .insert("num_predict".to_string(), serde_json::json!(max_tokens));
                    }
                    if !request.options.stop.is_empty() {
                        options_map
                            .insert("stop".to_string(), serde_json::json!(request.options.stop));
                    }
                    if let Some(seed) = request.options.seed {
                        options_map.insert("seed".to_string(), serde_json::json!(seed));
                    }

                    let req_body = serde_json::json!({
                        "model": self.model,
                        "messages": messages,
                        "stream": true,
                        "options": options_map
                    });
                    (url, req_body)
                }
                LocalBackendKind::LmStudio => {
                    let url = format!("{}/api/v1/chat", self.base_url);
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
                    if let Some(top_k) = request.options.top_k {
                        req_map.insert("top_k".to_string(), serde_json::json!(top_k));
                    }
                    if let Some(max_tokens) = request.options.max_output_tokens {
                        req_map.insert("max_tokens".to_string(), serde_json::json!(max_tokens));
                    }
                    if !request.options.stop.is_empty() {
                        req_map.insert("stop".to_string(), serde_json::json!(request.options.stop));
                    }
                    if let Some(rf) = &response_format {
                        req_map.insert("response_format".to_string(), rf.clone());
                    }

                    (url, serde_json::Value::Object(req_map))
                }
                LocalBackendKind::StandardOpenAi => {
                    let url = if self.base_url.ends_with("/chat/completions") {
                        self.base_url.clone()
                    } else if self.base_url.ends_with("/v1") || self.base_url.ends_with("/openai") {
                        format!("{}/chat/completions", self.base_url)
                    } else {
                        format!("{}/v1/chat/completions", self.base_url)
                    };

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
                    if let Some(rf) = &response_format {
                        req_map.insert("response_format".to_string(), rf.clone());
                    }

                    (url, serde_json::Value::Object(req_map))
                }
            };

            let mut builder = self.async_client.post(&url).json(&req_body);
            builder = self.inject_headers(builder);

            let response = tokio::select! {
                res = builder.send() => {
                    res.map_err(|e| LlmError::Transport(e.to_string()))?
                }
                _ = async {
                    while !cancel_flag.load(Ordering::Relaxed) {
                        tokio::time::sleep(Duration::from_millis(50)).await;
                    }
                } => {
                    log::info!("[OpenAiCompat] Generation cancelled during connect phase for turn {}", turn_id);
                    let _ = tx.send(VoxEvent::Cancelled { turn_id });
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

            let mut stream = response.bytes_stream();
            let mut buffer = Vec::new();
            let mut finished = false;

            loop {
                if cancel_flag.load(Ordering::Relaxed) {
                    log::info!("[OpenAiCompat] Generation cancelled for turn {}", turn_id);
                    let _ = tx.send(VoxEvent::Cancelled { turn_id });
                    return Ok(());
                }

                let chunk_opt =
                    match tokio::time::timeout(Duration::from_millis(150), stream.next()).await {
                        Ok(Some(chunk_result)) => match chunk_result {
                            Ok(c) => Some(c),
                            Err(e) => {
                                log::error!("[OpenAiCompat] Stream read error: {}", e);
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
                            process_line(trimmed, turn_id, tx, &mut finished);
                            if finished {
                                break;
                            }
                        }
                    }

                    if finished {
                        break;
                    }
                }
            }

            if !finished && !buffer.is_empty() {
                if let Ok(line) = String::from_utf8(buffer) {
                    let trimmed = line.trim();
                    process_line(trimmed, turn_id, tx, &mut finished);
                }
            }

            if !finished && cancel_flag.load(Ordering::Relaxed) {
                let _ = tx.send(VoxEvent::Cancelled { turn_id });
            } else {
                let _ = tx.send(VoxEvent::LlmFinished { turn_id });
            }

            Ok(())
        })
    }

    fn capabilities(&self) -> &ProviderCapabilities {
        &self.capabilities
    }

    fn health_check(&self) -> bool {
        let kind = self.detect_backend_kind();
        match kind {
            LocalBackendKind::Ollama | LocalBackendKind::LmStudio => true,
            LocalBackendKind::StandardOpenAi => {
                let url = if self.base_url.ends_with("/v1") || self.base_url.ends_with("/openai") {
                    format!("{}/models", self.base_url)
                } else {
                    format!("{}/v1/models", self.base_url)
                };
                let mut builder = self.async_client.get(&url).timeout(Duration::from_secs(3));
                builder = self.inject_headers(builder);

                block_on(async {
                    match builder.send().await {
                        Ok(resp) => resp.status().is_success(),
                        Err(_) => false,
                    }
                })
            }
        }
    }

    fn list_models(&self) -> Result<Vec<LlmModelInfo>, LlmError> {
        use crate::core::settings::LlmModelInfo;

        block_on(async {
            // Try Ollama-specific /api/tags first
            let ollama_url = format!("{}/api/tags", self.base_url);
            let mut builder = self
                .async_client
                .get(&ollama_url)
                .timeout(Duration::from_secs(3));
            builder = self.inject_headers(builder);

            if let Ok(resp) = builder.send().await {
                if resp.status().is_success() {
                    if let Ok(ollama_resp) = resp.json::<OllamaTagsResponse>().await {
                        let models = ollama_resp
                            .models
                            .into_iter()
                            .map(|m| {
                                let clean_name =
                                    m.name.replace([':', '_', '-'], " ");
                                LlmModelInfo {
                                    id: m.name.clone(),
                                    name: clean_name,
                                    size_bytes: Some(m.size),
                                    quantization: m.details.quantization_level,
                                    family: m.details.family.map(|f| {
                                        let mut c = f.chars();
                                        match c.next() {
                                            None => String::new(),
                                            Some(first) => {
                                                first.to_uppercase().collect::<String>()
                                                    + c.as_str()
                                            }
                                        }
                                    }),
                                    provider_kind: "open_ai_compat".to_string(),
                                    capabilities: None,
                                }
                            })
                            .collect();
                        return Ok(models);
                    }
                }
            }

            // Fallback: standard /v1/models
            let url = if self.base_url.ends_with("/v1") || self.base_url.ends_with("/openai") {
                format!("{}/models", self.base_url)
            } else {
                format!("{}/v1/models", self.base_url)
            };
            let mut builder = self.async_client.get(&url).timeout(Duration::from_secs(3));
            builder = self.inject_headers(builder);

            let resp = builder
                .send()
                .await
                .map_err(|e| LlmError::Transport(e.to_string()))?;
            if !resp.status().is_success() {
                return Err(LlmError::Provider {
                    status: resp.status().as_u16(),
                    message: "Failed listing models".to_string(),
                });
            }

            let model_list = resp
                .json::<ModelList>()
                .await
                .map_err(|e| LlmError::Parse(e.to_string()))?;
            let models = model_list
                .data
                .into_iter()
                .map(|m| {
                    let (clean_name, quantization, family) = parse_heuristic_metadata(&m.id);
                    LlmModelInfo {
                        id: m.id,
                        name: clean_name,
                        size_bytes: None,
                        quantization,
                        family,
                        provider_kind: "open_ai_compat".to_string(),
                        capabilities: None,
                    }
                })
                .collect();

            Ok(models)
        })
    }

    fn kind(&self) -> ProviderKind {
        ProviderKind::OpenAiCompat
    }
}

fn block_on<F: std::future::Future + Send>(future: F) -> F::Output
where
    F::Output: Send,
{
    match tokio::runtime::Handle::try_current() {
        Ok(handle) => {
            if handle.runtime_flavor() == tokio::runtime::RuntimeFlavor::MultiThread {
                tokio::task::block_in_place(|| handle.block_on(future))
            } else {
                std::thread::scope(|s| {
                    s.spawn(|| {
                        tokio::runtime::Builder::new_current_thread()
                            .enable_all()
                            .build()
                            .expect("Failed to build worker runtime")
                            .block_on(future)
                    })
                    .join()
                    .unwrap()
                })
            }
        }
        Err(_) => tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("Failed to build temporary tokio runtime")
            .block_on(future),
    }
}

fn user_text_is_warmup(text: &str) -> bool {
    text.is_empty() || text == "[WARMUP]"
}

fn process_line(line: &str, turn_id: u32, tx: &mpsc::Sender<VoxEvent>, finished: &mut bool) {
    if line.is_empty() {
        return;
    }

    if let Some(data) = line.strip_prefix("data:") {
        let data = data.trim();
        if data == "[DONE]" {
            *finished = true;
            return;
        }

        if let Ok(chunk) = serde_json::from_str::<ChatCompletionChunk>(data) {
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
    } else {
        // Try parsing as native Ollama chat stream event
        #[derive(Deserialize)]
        struct OllamaMessage {
            content: String,
        }
        #[derive(Deserialize)]
        struct OllamaChatChunk {
            message: Option<OllamaMessage>,
            done: Option<bool>,
        }
        if let Ok(chunk) = serde_json::from_str::<OllamaChatChunk>(line) {
            if let Some(msg) = chunk.message {
                if !msg.content.is_empty() {
                    let _ = tx.send(VoxEvent::LlmToken {
                        turn_id,
                        token: msg.content,
                    });
                }
            }
            if chunk.done.unwrap_or(false) {
                *finished = true;
            }
            return;
        }

        // Try parsing as raw JSON ChatCompletionChunk
        if let Ok(chunk) = serde_json::from_str::<ChatCompletionChunk>(line) {
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

fn parse_heuristic_metadata(id: &str) -> (String, Option<String>, Option<String>) {
    let id_lower = id.to_lowercase();
    let quantization =
        if id_lower.contains("q4_k_m") || id_lower.contains("q4_k") || id_lower.contains("q4") {
            Some("Q4_K_M".to_string())
        } else if id_lower.contains("q6_k") || id_lower.contains("q6") {
            Some("Q6_K".to_string())
        } else if id_lower.contains("q2_k") || id_lower.contains("q2") {
            Some("Q2_K".to_string())
        } else if id_lower.contains("fp16") {
            Some("FP16".to_string())
        } else {
            None
        };

    let family = if id_lower.contains("gemma") {
        Some("Gemma".to_string())
    } else if id_lower.contains("llama") {
        Some("Llama".to_string())
    } else {
        None
    };

    let clean_name = id.replace([':', '_', '-'], " ");

    (clean_name, quantization, family)
}
