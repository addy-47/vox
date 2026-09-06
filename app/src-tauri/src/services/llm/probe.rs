use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, Instant},
};

use futures_util::StreamExt;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::{
    core::{
        settings::{LlmModelInfo, LlmProviderConfig, ModelCapabilities},
        state::AppState,
    },
    services::llm::{
        lookup_preset,
        transport::{
            inject_auth_headers, CapabilitySource, ConnectionConfig, TokenLimitField, TransportType,
        },
        EmbeddedProvider, LlmProvider, RemoteTransport, QWEN_MODEL_DIR,
    },
    utils::paths,
};

#[derive(Debug, Serialize, Deserialize)]
pub struct ModelProbeResult {
    pub capabilities: ModelCapabilities,
    pub validated_cap: Option<u32>,
    pub cached_map: HashMap<String, ModelCapabilities>,
}

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

/// Lists available models for the given LLM provider configuration or active state.
pub async fn list_models(
    state: &Arc<AppState>,
    provider: Option<LlmProviderConfig>,
) -> Result<Vec<LlmModelInfo>, String> {
    let config = match provider {
        Some(prov) => prov,
        None => {
            let settings = state.settings.read().map_err(|e| e.to_string())?;
            settings.llm.to_provider_config()
        }
    };

    match config {
        LlmProviderConfig::Embedded => {
            let llm_dir = paths::get().models.join(QWEN_MODEL_DIR);
            EmbeddedProvider::list_models_in_dir(&llm_dir).map_err(|e| e.to_string())
        }
        LlmProviderConfig::OpenAiCompat {
            base_url,
            model,
            api_key,
            provider_name,
        } => {
            let conn_cfg = ConnectionConfig::new(
                &base_url,
                &model,
                api_key.as_deref(),
                provider_name.as_deref(),
            );
            let transport = RemoteTransport::new(conn_cfg);
            transport.list_models().await.map_err(|e| e.to_string())
        }
    }
}

/// Probes capabilities, validates optional token cap, and persists result to the capabilities cache.
pub async fn probe_capabilities(
    state: &Arc<AppState>,
    provider: Option<LlmProviderConfig>,
    model_id: Option<String>,
    target_cap: Option<u32>,
) -> Result<ModelProbeResult, String> {
    let (config, active_model) = {
        let settings = state.settings.read().map_err(|e| e.to_string())?;
        (
            provider.unwrap_or_else(|| settings.llm.to_provider_config()),
            settings.llm.active_model().to_string(),
        )
    };

    let target = model_id.or(Some(active_model));

    let caps = CapabilityProbeEngine::probe_capabilities(&config, target.as_deref())
        .await
        .map_err(|e| e.to_string())?;

    let validated_cap = if let Some(cap) = target_cap {
        CapabilityProbeEngine::validate_token_cap(&config, target.as_deref(), cap)
            .await
            .unwrap_or(None)
    } else {
        None
    };

    let cache_dir = paths::get().cache.clone();
    let cache_file = cache_dir.join("model_capabilities.json");
    let caps_clone = caps.clone();

    let mut map: HashMap<String, ModelCapabilities> = if cache_file.exists() {
        tokio::fs::read_to_string(&cache_file)
            .await
            .ok()
            .and_then(|c| serde_json::from_str(&c).ok())
            .unwrap_or_default()
    } else {
        HashMap::new()
    };

    let key = format!("{}:{}", caps_clone.provider_kind, caps_clone.model_id);
    map.insert(key, caps_clone);

    let map_clone = map.clone();
    tokio::spawn(async move {
        if let Err(e) = tokio::fs::create_dir_all(&cache_dir).await {
            log::warn!(
                "[Settings::Health] Failed to create cache directory {:?}: {}",
                cache_dir,
                e
            );
        }
        if let Ok(json) = serde_json::to_string_pretty(&map_clone) {
            let tmp = cache_file.with_extension("tmp");
            if tokio::fs::write(&tmp, json).await.is_ok() {
                if let Err(e) = tokio::fs::rename(&tmp, &cache_file).await {
                    log::warn!("[Settings::Health] Failed to rename temp cache file: {}", e);
                }
            }
        }
    });

    Ok(ModelProbeResult {
        capabilities: caps,
        validated_cap,
        cached_map: map,
    })
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
            ..Default::default()
        };

        match config.capability_source {
            CapabilitySource::OllamaNative => {
                Self::probe_ollama_metadata(client, config, &mut meta).await;
            }
            CapabilitySource::ProbedGeneric => {
                if let Some(meta_preset) = preset_meta {
                    if meta.context_window.is_none() {
                        meta.context_window = meta_preset.published_context_window;
                    }
                }
            }
        }

        let is_gpu = meta.is_gpu_accelerated || meta.server_has_gpu;
        let gpu_status = if is_gpu {
            if let Some(vram) = meta.vram_bytes {
                let mb = vram / (1024 * 1024);
                format!("GPU Active (VRAM: {} MB)", mb)
            } else {
                "GPU Active (Hardware Accelerated)".to_string()
            }
        } else {
            "CPU Inference / Standard".to_string()
        };

        Ok(ModelCapabilities {
            model_id: config.model.clone(),
            provider_kind: "openai_compat".to_string(),
            supports_tools: meta.supports_tools,
            supports_latin,
            supports_devanagari,
            context_window: meta.context_window,
            tps,
            ttft_ms,
            server_has_gpu: meta.server_has_gpu,
            is_gpu_accelerated: is_gpu,
            gpu_status,
            vram_bytes: meta.vram_bytes,
            parameter_size: meta.parameter_size,
            quantization: meta.quantization,
            family: meta.family,
            tested_at_epoch: now,
        })
    }

    async fn probe_ollama_metadata(
        client: &Client,
        config: &ConnectionConfig,
        meta: &mut EndpointMeta,
    ) {
        let base_url = config.base_url.trim_end_matches('/');

        let show_url = format!("{}/api/show", base_url);
        let show_payload = json!({ "name": config.model });
        let mut builder = client.post(&show_url).json(&show_payload);
        builder = inject_auth_headers(builder, &config.auth);

        if let Ok(resp) = builder.send().await {
            if let Ok(show) = resp.json::<OllamaShowResponse>().await {
                if let Some(caps) = show.capabilities {
                    if caps.iter().any(|c| c.eq_ignore_ascii_case("tools")) {
                        meta.supports_tools = true;
                    }
                }
                if let Some(info) = show.model_info {
                    if let Some(obj) = info.as_object() {
                        for (k, v) in obj {
                            if k.ends_with(".context_length") {
                                if let Some(len) = v.as_u64() {
                                    meta.context_window = Some(len as u32);
                                    break;
                                }
                            }
                        }
                    }
                }
                if let Some(details) = show.details {
                    meta.parameter_size = details.parameter_size;
                    meta.quantization = details.quantization_level;
                    meta.family = details.family;
                }
            }
        }

        let ps_url = format!("{}/api/ps", base_url);
        let mut ps_builder = client.get(&ps_url);
        ps_builder = inject_auth_headers(ps_builder, &config.auth);

        if let Ok(resp) = ps_builder.send().await {
            if let Ok(ps) = resp.json::<OllamaPsResponse>().await {
                for running in ps.models {
                    if running.name == config.model
                        || running.name.starts_with(&format!("{}:", config.model))
                    {
                        if let Some(vram) = running.size_vram {
                            if vram > 0 {
                                meta.server_has_gpu = true;
                                meta.is_gpu_accelerated = true;
                                meta.vram_bytes = Some(vram);
                            }
                        }
                        break;
                    }
                }
            }
        }
    }

    async fn empirical_streaming_probe(
        client: &Client,
        config: &ConnectionConfig,
    ) -> (bool, bool, Option<f32>, Option<u32>) {
        let is_ollama_native = config.capability_source == CapabilitySource::OllamaNative
            && config.token_limit_field == TokenLimitField::NumPredict;

        let (url, payload) = if is_ollama_native {
            (
                super::transport::ollama::resolve_url(&config.base_url),
                json!({
                    "model": config.model,
                    "messages": [
                        {"role": "user", "content": "Respond strictly with: Hello नमस्ते"}
                    ],
                    "stream": true,
                    "options": {
                        "temperature": super::DEFAULT_PROBE_TEMPERATURE,
                        "num_predict": super::DEFAULT_PROBE_MAX_TOKENS
                    }
                }),
            )
        } else if config.transport == TransportType::Responses {
            (
                super::transport::responses::resolve_url(&config.base_url),
                json!({
                    "model": config.model,
                    "input": [
                        {"role": "user", "content": "Respond strictly with: Hello नमस्ते"}
                    ],
                    "temperature": super::DEFAULT_PROBE_TEMPERATURE,
                    "max_output_tokens": super::DEFAULT_PROBE_MAX_TOKENS,
                    "stream": true
                }),
            )
        } else {
            (
                super::transport::chat_completions::resolve_url(&config.base_url),
                json!({
                    "model": config.model,
                    "messages": [
                        {"role": "user", "content": "Respond strictly with: Hello नमस्ते"}
                    ],
                    "temperature": super::DEFAULT_PROBE_TEMPERATURE,
                    "max_tokens": super::DEFAULT_PROBE_MAX_TOKENS,
                    "stream": true
                }),
            )
        };

        let mut builder = client.post(&url).json(&payload);
        builder = inject_auth_headers(builder, &config.auth);

        let t_start = Instant::now();
        let mut first_token_time = None;
        let mut token_count = 0usize;
        let mut accumulated_text = String::new();

        let response = match builder.send().await {
            Ok(res) if res.status().is_success() => res,
            _ => return (false, false, None, None),
        };

        let mut decoder = super::transport::sse::SseDecoder::new();
        let mut stream = response.bytes_stream();

        while let Some(item) = stream.next().await {
            if let Ok(bytes) = item {
                let lines = decoder.decode_chunk(&bytes);
                for line in lines {
                    if line == "[DONE]" {
                        break;
                    }
                    if let Ok(val) = serde_json::from_str::<serde_json::Value>(&line) {
                        let token_opt = if is_ollama_native {
                            val.get("message")
                                .and_then(|m| m.get("content"))
                                .and_then(|s| s.as_str())
                        } else if config.transport == TransportType::Responses {
                            val.get("delta").and_then(|s| s.as_str())
                        } else {
                            val.get("choices")
                                .and_then(|c| c.get(0))
                                .and_then(|c| c.get("delta"))
                                .and_then(|d| d.get("content"))
                                .and_then(|s| s.as_str())
                        };

                        if let Some(tok) = token_opt {
                            if !tok.is_empty() {
                                if first_token_time.is_none() {
                                    first_token_time = Some(t_start.elapsed());
                                }
                                token_count += 1;
                                accumulated_text.push_str(tok);
                            }
                        }
                    }
                }
            }
        }

        let ttft_ms = first_token_time.map(|d| d.as_millis() as u32);
        let tps = if let Some(ttft) = first_token_time {
            let total_dur = t_start.elapsed();
            if total_dur > ttft && token_count > 1 {
                let gen_secs = (total_dur - ttft).as_secs_f32();
                if gen_secs > 0.0 {
                    Some((token_count - 1) as f32 / gen_secs)
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        };

        let has_latin = accumulated_text.chars().any(|c| c.is_ascii_alphabetic());
        let has_devanagari = accumulated_text
            .chars()
            .any(|c| ('\u{0900}'..='\u{097F}').contains(&c));

        (
            has_latin || !accumulated_text.is_empty(),
            has_devanagari,
            tps,
            ttft_ms,
        )
    }

    async fn empirical_tool_probe(client: &Client, config: &ConnectionConfig) -> bool {
        let is_ollama_native = config.capability_source == CapabilitySource::OllamaNative
            && config.token_limit_field == TokenLimitField::NumPredict;

        let (url, payload) = if is_ollama_native {
            (
                super::transport::ollama::resolve_url(&config.base_url),
                json!({
                    "model": config.model,
                    "messages": [
                        {"role": "user", "content": "What is the weather in Tokyo?"}
                    ],
                    "tools": [{
                        "type": "function",
                        "function": {
                            "name": "get_weather",
                            "description": "Get current weather for location",
                            "parameters": {
                                "type": "object",
                                "properties": {
                                    "location": {"type": "string"}
                                },
                                "required": ["location"]
                            }
                        }
                    }],
                    "stream": false
                }),
            )
        } else {
            (
                super::transport::chat_completions::resolve_url(&config.base_url),
                json!({
                    "model": config.model,
                    "messages": [
                        {"role": "user", "content": "What is the weather in Tokyo?"}
                    ],
                    "tools": [{
                        "type": "function",
                        "function": {
                            "name": "get_weather",
                            "description": "Get current weather for location",
                            "parameters": {
                                "type": "object",
                                "properties": {
                                    "location": {"type": "string"}
                                },
                                "required": ["location"]
                            }
                        }
                    }],
                    "tool_choice": "auto",
                    "max_tokens": super::DEFAULT_TOOL_PROBE_MAX_TOKENS,
                    "temperature": super::DEFAULT_PROBE_TEMPERATURE
                }),
            )
        };

        let mut builder = client.post(&url).json(&payload);
        builder = inject_auth_headers(builder, &config.auth);

        if let Ok(resp) = builder.send().await {
            let status = resp.status();
            if status.is_success() {
                if let Ok(body) = resp.text().await {
                    if let Ok(val) = serde_json::from_str::<serde_json::Value>(&body) {
                        if let Some(choices) = val.get("choices").and_then(|c| c.as_array()) {
                            if let Some(first) = choices.first() {
                                if first
                                    .get("message")
                                    .and_then(|m| m.get("tool_calls"))
                                    .is_some()
                                {
                                    return true;
                                }
                            }
                        }
                        if let Some(msg) = val.get("message") {
                            if msg.get("tool_calls").is_some() {
                                return true;
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
