use crate::core::settings::{LlmProviderConfig, ModelCapabilities};
use crate::services::llm::catalog::lookup_preset;
use crate::services::llm::config::{CapabilitySource, ConnectionConfig, TransportType};
use crate::services::llm::transport::inject_auth_headers;
use futures_util::StreamExt;
use reqwest::Client;
use serde::Deserialize;
use serde_json::json;
use std::time::{Duration, Instant};

#[derive(Default, Debug)]
struct EndpointMeta {
    supports_tools: bool,
    context_window: Option<u32>,
    server_has_gpu: bool,
    is_gpu_accelerated: bool,
    vram_bytes: Option<u64>,
    parameter_size: Option<String>,
    quantization: Option<String>,
    family: Option<String>,
}

#[derive(Deserialize)]
struct OllamaShowResponse {
    #[serde(default)]
    model_info: Option<serde_json::Value>,
    #[serde(default)]
    capabilities: Option<Vec<String>>,
    #[serde(default)]
    details: Option<OllamaModelDetails>,
}

#[derive(Deserialize)]
struct OllamaModelDetails {
    #[serde(default)]
    parameter_size: Option<String>,
    #[serde(default)]
    quantization_level: Option<String>,
    #[serde(default)]
    family: Option<String>,
}

#[derive(Deserialize)]
struct OllamaPsResponse {
    #[serde(default)]
    models: Vec<OllamaPsModel>,
}

#[derive(Deserialize)]
struct OllamaPsModel {
    name: String,
    #[serde(default)]
    size_vram: Option<u64>,
}

#[derive(Deserialize)]
struct ChatProbeChunk {
    #[serde(default)]
    choices: Vec<ChatProbeChoice>,
}

#[derive(Deserialize)]
struct ChatProbeChoice {
    delta: ChatProbeDelta,
}

#[derive(Deserialize)]
struct ChatProbeDelta {
    content: Option<String>,
}

/// Empirical Capability Discovery Engine.
pub struct CapabilityProbeEngine;

impl CapabilityProbeEngine {
    /// Executes capability probing for the specified provider configuration.
    pub async fn probe(
        config: &LlmProviderConfig,
    ) -> Result<ModelCapabilities, Box<dyn std::error::Error + Send + Sync>> {
        Self::probe_capabilities(config, None).await
    }

    /// Probes model capabilities for a specific model override or active configuration.
    pub async fn probe_capabilities(
        config: &LlmProviderConfig,
        target_model: Option<&str>,
    ) -> Result<ModelCapabilities, Box<dyn std::error::Error + Send + Sync>> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        match config {
            LlmProviderConfig::Embedded => {
                let model_id = target_model.unwrap_or("embedded-default.gguf");
                Ok(Self::probe_local_embedded(model_id, None, now))
            }
            LlmProviderConfig::OpenAiCompat {
                base_url,
                model,
                api_key,
                provider_name,
            } => {
                let model_id = target_model.unwrap_or(model);
                let conn_cfg = ConnectionConfig::new(
                    base_url,
                    model_id,
                    api_key.as_deref(),
                    provider_name.as_deref(),
                );
                let client = Client::builder()
                    .timeout(Duration::from_secs(super::DEFAULT_PROBE_TIMEOUT_SECS))
                    .build()?;

                Self::probe_remote_endpoint(&client, &conn_cfg, now).await
            }
        }
    }

    /// Builds capability matrix for local embedded GGUF model.
    pub fn probe_local_embedded(
        model_id: &str,
        ctx_window: Option<u32>,
        now: u64,
    ) -> ModelCapabilities {
        ModelCapabilities {
            model_id: model_id.to_string(),
            provider_kind: "embedded".to_string(),
            supports_tools: true,
            supports_latin: true,
            supports_devanagari: true,
            context_window: ctx_window,
            tps: None,
            ttft_ms: None,
            server_has_gpu: false,
            is_gpu_accelerated: false,
            gpu_status: "Local CPU / In-Process".to_string(),
            vram_bytes: None,
            parameter_size: None,
            quantization: None,
            family: None,
            tested_at_epoch: now,
        }
    }

    /// Probes remote endpoint using empirical observation and authoritative native endpoints.
    pub async fn probe_remote_endpoint(
        client: &Client,
        config: &ConnectionConfig,
        now: u64,
    ) -> Result<ModelCapabilities, Box<dyn std::error::Error + Send + Sync>> {
        let preset_meta = config.provider_preset.as_deref().and_then(lookup_preset);

        let (supports_latin, supports_devanagari, tps, ttft_ms) =
            Self::empirical_streaming_probe(client, config).await;

        let tool_probe_success = Self::empirical_tool_probe(client, config).await;

        let mut meta = EndpointMeta {
            supports_tools: tool_probe_success,
            context_window: preset_meta.and_then(|p| p.published_context_window),
            ..Default::default()
        };

        if config.capability_source == CapabilitySource::OllamaNative {
            Self::probe_ollama_metadata(client, &config.base_url, &config.model, &mut meta).await;
        }

        let gpu_status = if let Some(p) = preset_meta {
            p.display_label.to_string()
        } else if meta.vram_bytes.is_some() {
            "Local Daemon (GPU VRAM Allocated)".to_string()
        } else {
            "Server / Provider Managed".to_string()
        };

        Ok(ModelCapabilities {
            model_id: config.model.clone(),
            provider_kind: "open_ai_compat".to_string(),
            supports_tools: meta.supports_tools,
            supports_latin,
            supports_devanagari,
            context_window: meta.context_window,
            tps,
            ttft_ms,
            server_has_gpu: meta.server_has_gpu,
            is_gpu_accelerated: meta.is_gpu_accelerated,
            gpu_status,
            vram_bytes: meta.vram_bytes,
            parameter_size: meta.parameter_size,
            quantization: meta.quantization,
            family: meta.family,
            tested_at_epoch: now,
        })
    }

    /// Queries Ollama native endpoints (/api/show and /api/ps) for model details.
    async fn probe_ollama_metadata(
        client: &Client,
        base_url: &str,
        model_id: &str,
        meta: &mut EndpointMeta,
    ) {
        let show_url = format!("{}/api/show", base_url.trim_end_matches('/'));
        let ps_url = format!("{}/api/ps", base_url.trim_end_matches('/'));

        if let Ok(resp) = client
            .post(&show_url)
            .json(&json!({ "name": model_id }))
            .send()
            .await
        {
            if resp.status().is_success() {
                if let Ok(show_data) = resp.json::<OllamaShowResponse>().await {
                    if let Some(caps) = show_data.capabilities {
                        if caps.iter().any(|c| c == "tools") {
                            meta.supports_tools = true;
                        }
                    }
                    if let Some(details) = show_data.details {
                        meta.parameter_size = details.parameter_size;
                        meta.quantization = details.quantization_level;
                        meta.family = details.family;
                    }
                    if let Some(info) = show_data.model_info {
                        if let Some(ctx_val) =
                            info.get("general.context_length").and_then(|v| v.as_u64())
                        {
                            meta.context_window = Some(ctx_val as u32);
                        }
                    }
                }
            }
        }

        if let Ok(resp) = client.get(&ps_url).send().await {
            if resp.status().is_success() {
                if let Ok(ps_data) = resp.json::<OllamaPsResponse>().await {
                    if let Some(m) = ps_data.models.iter().find(|m| m.name.contains(model_id)) {
                        if let Some(vram) = m.size_vram {
                            if vram > 0 {
                                meta.server_has_gpu = true;
                                meta.is_gpu_accelerated = true;
                                meta.vram_bytes = Some(vram);
                            }
                        }
                    }
                }
            }
        }
    }

    /// Executes live streaming test to empirically measure TTFT, TPS, and multilingual output.
    async fn empirical_streaming_probe(
        client: &Client,
        config: &ConnectionConfig,
    ) -> (bool, bool, Option<f32>, Option<u32>) {
        let supports_latin = true;
        let mut supports_devanagari = false;
        let mut tps = None;
        let mut ttft_ms = None;

        let prompt = "Respond with: Hello / नमस्ते";

        let (url, payload) = if config.transport == TransportType::Responses {
            let url = super::transport::responses::resolve_url(&config.base_url);
            let payload = json!({
                "model": config.model,
                "input": [{"role": "user", "content": prompt}],
                "stream": true,
                "max_output_tokens": super::DEFAULT_PROBE_MAX_TOKENS,
                "temperature": super::DEFAULT_PROBE_TEMPERATURE
            });
            (url, payload)
        } else {
            let url = super::transport::chat_completions::resolve_url(&config.base_url);
            let mut p = json!({
                "model": config.model,
                "messages": [{"role": "user", "content": prompt}],
                "stream": true,
                "temperature": super::DEFAULT_PROBE_TEMPERATURE
            });
            if let Some(obj) = p.as_object_mut() {
                obj.insert(
                    config.token_limit_field.as_str().to_string(),
                    json!(super::DEFAULT_PROBE_MAX_TOKENS),
                );
            }
            (url, p)
        };

        let mut builder = client.post(&url).json(&payload);
        builder = inject_auth_headers(builder, &config.auth);

        let t_start = Instant::now();
        let mut first_token_time = None;
        let mut token_count = 0usize;
        let mut collected_text = String::new();

        if let Ok(resp) = builder.send().await {
            if resp.status().is_success() {
                let mut stream = resp.bytes_stream();
                let mut decoder = super::transport::sse::SseDecoder::new();

                while let Some(Ok(chunk)) = stream.next().await {
                    let lines = decoder.decode_chunk(&chunk);
                    for line in lines {
                        if line == "[DONE]" {
                            break;
                        }
                        if config.transport == TransportType::Responses {
                            #[derive(Deserialize)]
                            struct RespEvt {
                                #[serde(rename = "type")]
                                event_type: Option<String>,
                                delta: Option<String>,
                            }
                            if let Ok(evt) = serde_json::from_str::<RespEvt>(&line) {
                                if evt.event_type.as_deref() == Some("response.output_text.delta") {
                                    if let Some(ref text) = evt.delta {
                                        if !text.is_empty() {
                                            if first_token_time.is_none() {
                                                first_token_time = Some(t_start.elapsed());
                                            }
                                            token_count += 1;
                                            collected_text.push_str(text);
                                        }
                                    }
                                }
                            }
                        } else if let Ok(parsed) = serde_json::from_str::<ChatProbeChunk>(&line) {
                            if let Some(choice) = parsed.choices.first() {
                                if let Some(ref text) = choice.delta.content {
                                    if !text.is_empty() {
                                        if first_token_time.is_none() {
                                            first_token_time = Some(t_start.elapsed());
                                        }
                                        token_count += 1;
                                        collected_text.push_str(text);
                                    }
                                }
                            }
                        }
                    }
                    if t_start.elapsed().as_secs() >= super::DEFAULT_PROBE_TIMEOUT_SECS {
                        break;
                    }
                }
            }
        }

        if let Some(ttft) = first_token_time {
            ttft_ms = Some(ttft.as_millis() as u32);
            let generation_time = t_start.elapsed().saturating_sub(ttft).as_secs_f32();
            if generation_time > 0.05 && token_count > 1 {
                tps = Some(token_count as f32 / generation_time);
            }
        }

        if collected_text
            .chars()
            .any(|c| ('\u{0900}'..='\u{097F}').contains(&c))
        {
            supports_devanagari = true;
        }

        (supports_latin, supports_devanagari, tps, ttft_ms)
    }

    /// Executes live structured tool probe to empirically observe tool calling support.
    async fn empirical_tool_probe(client: &Client, config: &ConnectionConfig) -> bool {
        let url = super::transport::chat_completions::resolve_url(&config.base_url);
        let mut payload = json!({
            "model": config.model,
            "messages": [{"role": "user", "content": "What is the weather in Tokyo?"}],
            "tools": [{
                "type": "function",
                "function": {
                    "name": "get_weather",
                    "description": "Get current weather",
                    "parameters": {
                        "type": "object",
                        "properties": {
                            "location": { "type": "string" }
                        },
                        "required": ["location"]
                    }
                }
            }],
            "tool_choice": "auto"
        });
        if let Some(obj) = payload.as_object_mut() {
            obj.insert(
                config.token_limit_field.as_str().to_string(),
                json!(super::DEFAULT_TOOL_PROBE_MAX_TOKENS),
            );
        }

        let mut builder = client.post(&url).json(&payload);
        builder = inject_auth_headers(builder, &config.auth);

        if let Ok(resp) = builder.send().await {
            let status = resp.status();
            if status.is_success() {
                if let Ok(body) = resp.text().await {
                    if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&body) {
                        if let Some(choices) = parsed.get("choices").and_then(|c| c.as_array()) {
                            if let Some(first) = choices.first() {
                                let msg = first.get("message");
                                if let Some(tools) = msg.and_then(|m| m.get("tool_calls")) {
                                    return tools.as_array().is_some_and(|a| !a.is_empty());
                                }
                                if msg.and_then(|m| m.get("function_call")).is_some() {
                                    return true;
                                }
                            }
                        }
                    }
                    return body.contains("\"tool_calls\"") || body.contains("\"function_call\"");
                }
            } else if status.as_u16() == 400 {
                return false;
            }
        }
        false
    }

    /// Smoke validation for custom token caps without error-text scraping.
    pub async fn validate_token_cap(
        config: &LlmProviderConfig,
        target_model_id: Option<&str>,
        target_cap: u32,
    ) -> Result<Option<u32>, String> {
        let (base_url, model, api_key, provider_name) = match config {
            LlmProviderConfig::OpenAiCompat {
                base_url,
                model,
                api_key,
                provider_name,
            } => (base_url, model, api_key, provider_name),
            LlmProviderConfig::Embedded => return Ok(None),
        };

        let conn_cfg = ConnectionConfig::new(
            base_url,
            target_model_id.unwrap_or(model),
            api_key.as_deref(),
            provider_name.as_deref(),
        );
        let client = Client::builder()
            .timeout(Duration::from_secs(super::DEFAULT_VALIDATION_TIMEOUT_SECS))
            .build()
            .map_err(|e| e.to_string())?;

        let url = super::transport::chat_completions::resolve_url(&conn_cfg.base_url);
        let payload = json!({
            "model": conn_cfg.model,
            "messages": [{"role": "user", "content": "."}],
            "max_tokens": target_cap
        });

        let mut builder = client.post(&url).json(&payload);
        builder = inject_auth_headers(builder, &conn_cfg.auth);

        let resp = builder.send().await.map_err(|e| e.to_string())?;
        if resp.status().is_success() {
            Ok(None)
        } else {
            let status = resp.status().as_u16();
            let text = resp.text().await.unwrap_or_default();
            if status == 400 && text.to_lowercase().contains("context_length_exceeded") {
                Err(format!("Context length exceeded (HTTP 400): {}", text))
            } else {
                Err(format!("Endpoint returned HTTP {}: {}", status, text))
            }
        }
    }
}
