use futures_util::StreamExt;
use reqwest::Client;
use serde::Deserialize;
use serde_json::json;
use std::time::Instant;

use crate::core::settings::{LlmProviderConfig, ModelCapabilities};

pub struct CapabilityProbeEngine;

#[derive(Debug, Deserialize)]
struct OllamaShowResponse {
    capabilities: Option<Vec<String>>,
    details: Option<OllamaShowDetails>,
    model_info: Option<serde_json::Map<String, serde_json::Value>>,
}

#[derive(Debug, Deserialize)]
struct OllamaShowDetails {
    family: Option<String>,
    parameter_size: Option<String>,
    quantization_level: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OllamaPsResponse {
    models: Option<Vec<OllamaPsModel>>,
}

#[derive(Debug, Deserialize)]
struct OllamaPsModel {
    name: String,
    size_vram: Option<u64>,
    context_length: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct ChatCompletionChunk {
    choices: Vec<ChunkChoice>,
}

#[derive(Debug, Deserialize)]
struct ChunkChoice {
    delta: ChunkDelta,
}

#[derive(Debug, Deserialize)]
struct ChunkDelta {
    content: Option<String>,
}

impl CapabilityProbeEngine {
    pub async fn probe_capabilities(
        config: &LlmProviderConfig,
        target_model_id: Option<&str>,
    ) -> Result<ModelCapabilities, Box<dyn std::error::Error + Send + Sync>> {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(12))
            .build()?;

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        match config {
            LlmProviderConfig::Embedded => {
                let model_id = target_model_id
                    .unwrap_or(crate::core::defaults::DEFAULT_LLM_MODEL)
                    .to_string();
                log::info!(
                    "[CapabilityProbe] Probing Local Embedded GGUF model '{}'...",
                    model_id
                );

                let (family, supports_tools, supports_devanagari) =
                    Self::heuristic_embedded_caps(&model_id);

                let caps = ModelCapabilities {
                    model_id: model_id.clone(),
                    provider_kind: "embedded".to_string(),
                    supports_tools,
                    supports_latin: true,
                    supports_devanagari,
                    context_window: Some(4096),
                    tps: None,
                    ttft_ms: None,
                    server_has_gpu: false,
                    is_gpu_accelerated: false,
                    gpu_status: "Local CPU / In-Process".to_string(),
                    vram_bytes: None,
                    parameter_size: None,
                    quantization: None,
                    family: Some(family),
                    tested_at_epoch: now,
                };

                log::info!(
                    "[CapabilityProbe] Completed local probe for '{}': tools={}, devanagari={}",
                    model_id,
                    caps.supports_tools,
                    caps.supports_devanagari
                );

                Ok(caps)
            }
            LlmProviderConfig::OpenAiCompat {
                base_url,
                model,
                api_key,
                provider_name,
            } => {
                let model_id = target_model_id.unwrap_or(model).to_string();
                let prov_name = provider_name.as_deref().unwrap_or("").to_lowercase();
                let is_cloud = Self::is_cloud_provider(base_url, &prov_name);

                log::info!(
                    "[CapabilityProbe] Initiating capability probe for model '{}' on endpoint '{}' (provider_name: '{}', is_cloud: {})...",
                    model_id,
                    base_url,
                    prov_name,
                    is_cloud
                );

                // Phase 1: Streaming Inference Probe for TTFT, TPS & Multi-lingual capabilities
                log::info!(
                    "[CapabilityProbe] Phase 1: Executing streaming inference probe via /v1/chat/completions..."
                );

                let (
                    supports_latin,
                    supports_devanagari,
                    tps,
                    ttft_ms,
                    headers_indicate_gpu,
                    header_context_window,
                ) = Self::streaming_inference_probe(
                    &client,
                    base_url,
                    &model_id,
                    api_key.as_deref(),
                )
                .await;

                log::info!(
                    "[CapabilityProbe] Phase 1 Complete: ttft={:?}ms, tps={:?}, devanagari={}, latin={}",
                    ttft_ms,
                    tps,
                    supports_devanagari,
                    supports_latin
                );

                // Phase 2: Structured Tool Calling Probe (lookup_user JSON schema with tool_choice: "auto")
                log::info!(
                    "[CapabilityProbe] Phase 2: Testing structured JSON tool/function calling capabilities..."
                );

                let tool_probe_success =
                    Self::structured_tool_probe(&client, base_url, &model_id, api_key.as_deref())
                        .await;

                log::info!(
                    "[CapabilityProbe] Phase 2 Complete: tool_probe_success={}",
                    tool_probe_success
                );

                // Phase 3: Metadata & GPU Offloading Verification
                let mut supports_tools = tool_probe_success;
                let mut context_window: Option<u32> = header_context_window;
                let mut server_has_gpu = is_cloud || headers_indicate_gpu;
                let mut is_gpu_accelerated = is_cloud;
                let mut vram_bytes: Option<u64> = None;
                let mut family: Option<String> = None;
                let mut parameter_size: Option<String> = None;
                let mut quantization: Option<String> = None;

                if is_cloud {
                    log::info!("[CapabilityProbe] Target is Cloud API. Preserving Cloud GPU Cluster designation.");
                } else {
                    // Local self-hosted endpoints (e.g. Ollama, LM Studio, vLLM on localhost)
                    log::info!("[CapabilityProbe] Target is local/self-hosted. Querying /api/show and /api/ps...");

                    let show_url = format!("{}/api/show", base_url.trim_end_matches('/'));
                    let ps_url = format!("{}/api/ps", base_url.trim_end_matches('/'));

                    // Query /api/show (Ollama)
                    if let Ok(resp) = client
                        .post(&show_url)
                        .json(&json!({ "name": model_id }))
                        .send()
                        .await
                    {
                        if resp.status().is_success() {
                            if let Ok(show_data) = resp.json::<OllamaShowResponse>().await {
                                log::info!("[CapabilityProbe] Successfully retrieved /api/show metadata for '{}'", model_id);
                                if let Some(caps) = show_data.capabilities {
                                    if caps.iter().any(|c| c.to_lowercase() == "tools") {
                                        supports_tools = true;
                                    }
                                }

                                if let Some(details) = show_data.details {
                                    family = details.family;
                                    parameter_size = details.parameter_size;
                                    quantization = details.quantization_level;
                                }

                                if let Some(ref info) = show_data.model_info {
                                    for (k, v) in info {
                                        if k.contains("context_length") {
                                            if let Some(ctx_val) = v.as_u64() {
                                                context_window = Some(ctx_val as u32);
                                                break;
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }

                    // Query /api/ps (Ollama)
                    if let Ok(resp) = client.get(&ps_url).send().await {
                        if resp.status().is_success() {
                            if let Ok(ps_data) = resp.json::<OllamaPsResponse>().await {
                                if let Some(models) = ps_data.models {
                                    if !models.is_empty() {
                                        server_has_gpu = true;
                                    }
                                    for m in models {
                                        if m.name == model_id || m.name.starts_with(&model_id) {
                                            server_has_gpu = true;
                                            if let Some(vram) = m.size_vram {
                                                if vram > 0 {
                                                    is_gpu_accelerated = true;
                                                    vram_bytes = Some(vram);
                                                    log::info!(
                                                        "[CapabilityProbe] Model '{}' is GPU offloaded in VRAM: {} bytes",
                                                        model_id,
                                                        vram
                                                    );
                                                }
                                            }
                                            if context_window.is_none()
                                                && m.context_length.is_some()
                                            {
                                                context_window = m.context_length;
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }

                    // TPS heuristic for non-Ollama local GPU inference (vLLM / LM Studio CUDA)
                    if !is_gpu_accelerated && tps.unwrap_or(0.0) > 20.0 {
                        server_has_gpu = true;
                        is_gpu_accelerated = true;
                        log::info!("[CapabilityProbe] TPS > 20.0 indicates GPU acceleration.");
                    }
                }

                // Build human-readable GPU status
                let gpu_status = if is_cloud {
                    if prov_name.contains("nvidia") || base_url.contains("nvidia") {
                        "NVIDIA Cloud GPU Cluster".to_string()
                    } else if prov_name.contains("groq") || base_url.contains("groq") {
                        "Groq LPU Cloud Acceleration".to_string()
                    } else if !prov_name.is_empty() {
                        format!("{} Cloud Cluster", Self::title_case(&prov_name))
                    } else {
                        "Cloud GPU/TPU Cluster".to_string()
                    }
                } else if is_gpu_accelerated {
                    if let Some(vram) = vram_bytes {
                        format!("GPU Accelerated (VRAM: {} MB)", vram / (1024 * 1024))
                    } else {
                        "GPU Accelerated".to_string()
                    }
                } else if server_has_gpu {
                    "Server GPU Present (Model CPU-Bound)".to_string()
                } else {
                    "Local Host CPU".to_string()
                };

                let final_caps = ModelCapabilities {
                    model_id: model_id.clone(),
                    provider_kind: "open_ai_compat".to_string(),
                    supports_tools,
                    supports_latin,
                    supports_devanagari,
                    context_window, // Exact verified or None (Endpoint Managed)
                    tps,
                    ttft_ms,
                    server_has_gpu,
                    is_gpu_accelerated,
                    gpu_status,
                    vram_bytes,
                    parameter_size,
                    quantization,
                    family,
                    tested_at_epoch: now,
                };

                log::info!(
                    "[CapabilityProbe] Probe finalized for model '{}': tools={}, devanagari={}, ctx={:?}, gpu_status='{}'",
                    model_id,
                    final_caps.supports_tools,
                    final_caps.supports_devanagari,
                    final_caps.context_window,
                    final_caps.gpu_status
                );

                Ok(final_caps)
            }
        }
    }

    fn resolve_chat_url(base_url: &str) -> String {
        let trimmed = base_url.trim_end_matches('/');
        if trimmed.ends_with("/chat/completions") {
            trimmed.to_string()
        } else if trimmed.ends_with("/v1") {
            format!("{}/chat/completions", trimmed)
        } else {
            format!("{}/v1/chat/completions", trimmed)
        }
    }

    /// Runtime smoke validation for custom user token caps.
    /// Sends a 1-token dummy prompt with the configured max_tokens cap.
    /// Returns Ok(None) if accepted, or Ok(Some(server_ceiling)) if rejected with HTTP 400.
    pub async fn validate_token_cap(
        config: &LlmProviderConfig,
        target_model_id: Option<&str>,
        target_cap: u32,
    ) -> Result<Option<u32>, String> {
        let (base_url, model, api_key) = match config {
            LlmProviderConfig::OpenAiCompat {
                base_url,
                model,
                api_key,
                ..
            } => (base_url, model, api_key),
            LlmProviderConfig::Embedded => return Ok(None),
        };

        let model_id = target_model_id.unwrap_or(model);
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(6))
            .build()
            .map_err(|e| e.to_string())?;

        let chat_url = Self::resolve_chat_url(base_url);
        let payload = json!({
            "model": model_id,
            "messages": [{"role": "user", "content": "."}],
            "max_tokens": target_cap
        });

        let mut req = client.post(&chat_url).json(&payload);
        if let Some(key) = api_key {
            if !key.is_empty() {
                req = req.header("Authorization", format!("Bearer {}", key));
            }
        }

        let resp = req.send().await.map_err(|e| e.to_string())?;
        let status = resp.status();

        if status.is_success() {
            return Ok(None); // Validated successfully
        }

        let err_text = resp.text().await.unwrap_or_default();
        log::warn!(
            "[CapabilityProbe] Token cap validation HTTP {}: {}",
            status,
            err_text
        );

        // Regex parsing to extract the server's true ceiling
        // Handles: "greater than maximum allowed 16384", "> 8192", "exceeds maximum supported output tokens (16384)"
        if let Some(ceiling) = Self::parse_token_ceiling_from_error(&err_text) {
            return Ok(Some(ceiling));
        }

        Err(format!(
            "Server returned HTTP {}: {}",
            status.as_u16(),
            err_text
        ))
    }

    fn is_cloud_provider(base_url: &str, provider_name: &str) -> bool {
        let b = base_url.to_lowercase();
        let p = provider_name.to_lowercase();

        p.contains("nvidia")
            || p.contains("nim")
            || p.contains("groq")
            || p.contains("openrouter")
            || p.contains("together")
            || p.contains("deepseek")
            || p.contains("mistral")
            || p.contains("openai")
            || p.contains("gemini")
            || p.contains("anthropic")
            || b.contains("nvidia")
            || b.contains("groq")
            || b.contains("openrouter")
            || b.contains("together")
            || b.contains("deepseek")
            || b.contains("mistral")
            || b.contains("openai")
            || b.contains("googleapis.com")
            || b.contains("anthropic")
    }

    fn title_case(s: &str) -> String {
        s.split_whitespace()
            .map(|word| {
                let mut c = word.chars();
                match c.next() {
                    None => String::new(),
                    Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
                }
            })
            .collect::<Vec<_>>()
            .join(" ")
    }

    fn heuristic_embedded_caps(model_id: &str) -> (String, bool, bool) {
        let lower = model_id.to_lowercase();
        if lower.contains("qwen") {
            ("Qwen".to_string(), true, true)
        } else if lower.contains("gemma") {
            ("Gemma".to_string(), true, true)
        } else if lower.contains("llama") {
            ("Llama".to_string(), true, true)
        } else {
            ("Unknown".to_string(), false, false)
        }
    }

    fn parse_token_ceiling_from_error(err_text: &str) -> Option<u32> {
        let lower = err_text.to_lowercase();

        let re_patterns = [
            r"(?:maximum(?: allowed)?|supported|limit|cap)[^\d]*(\d{3,7})",
            r">\s*(\d{3,7})",
            r"cannot exceed\s*(\d{3,7})",
            r"greater than (?:maximum allowed )?(\d{3,7})",
        ];

        for pat in re_patterns {
            if let Ok(re) = regex::Regex::new(pat) {
                if let Some(caps) = re.captures(&lower) {
                    if let Some(m) = caps.get(1) {
                        if let Ok(val) = m.as_str().parse::<u32>() {
                            if (256..=2_000_000).contains(&val) {
                                return Some(val);
                            }
                        }
                    }
                }
            }
        }
        None
    }

    async fn streaming_inference_probe(
        client: &Client,
        base_url: &str,
        model_id: &str,
        api_key: Option<&str>,
    ) -> (bool, bool, Option<f32>, Option<u32>, bool, Option<u32>) {
        let mut supports_latin = true;
        let mut supports_devanagari = false;
        let mut tps = None;
        let mut ttft_ms = None;
        let mut headers_indicate_gpu = false;
        let header_context_window = None;

        let chat_url = Self::resolve_chat_url(base_url);
        let prompt_payload = json!({
            "model": model_id,
            "messages": [
                { "role": "user", "content": "Write 'नमस्ते' in Devanagari script and 'Hello' in English." }
            ],
            "max_tokens": 40,
            "temperature": 0.1,
            "stream": true
        });

        let mut req = client.post(&chat_url).json(&prompt_payload);
        if let Some(key) = api_key {
            if !key.is_empty() {
                req = req.header("Authorization", format!("Bearer {}", key));
            }
        }

        let start = Instant::now();
        if let Ok(resp) = req.send().await {
            // Inspect HTTP Headers
            let headers = resp.headers();
            if let Some(server_hdr) = headers.get("server").and_then(|v| v.to_str().ok()) {
                let s = server_hdr.to_lowercase();
                if s.contains("vllm") || s.contains("cuda") || s.contains("metal") {
                    headers_indicate_gpu = true;
                }
            }

            if resp.status().is_success() {
                let mut bytes_stream = resp.bytes_stream();
                let mut full_text = String::new();
                let mut first_chunk_time: Option<Instant> = None;
                let mut token_chunks = 0;
                let mut line_buffer = Vec::new();
                let mut stream_done = false;

                while let Some(item) = bytes_stream.next().await {
                    if stream_done {
                        break;
                    }
                    if let Ok(bytes) = item {
                        line_buffer.extend_from_slice(&bytes);

                        while let Some(pos) = line_buffer.iter().position(|&b| b == b'\n') {
                            let line_bytes = line_buffer.drain(..=pos).collect::<Vec<u8>>();
                            if let Ok(line) = String::from_utf8(line_bytes) {
                                let trimmed = line.trim();
                                if trimmed.starts_with("data: ") {
                                    let json_str = trimmed.trim_start_matches("data: ").trim();
                                    if json_str == "[DONE]" {
                                        stream_done = true;
                                        break;
                                    }
                                    if let Ok(chunk) =
                                        serde_json::from_str::<ChatCompletionChunk>(json_str)
                                    {
                                        if let Some(choice) = chunk.choices.first() {
                                            if let Some(ref text) = choice.delta.content {
                                                if !text.is_empty() {
                                                    if first_chunk_time.is_none() {
                                                        let now = Instant::now();
                                                        first_chunk_time = Some(now);
                                                        ttft_ms = Some(
                                                            start.elapsed().as_millis() as u32
                                                        );
                                                    }
                                                    full_text.push_str(text);
                                                    token_chunks += 1;
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                // Check generated text
                if full_text
                    .chars()
                    .any(|c| ('\u{0900}'..='\u{097F}').contains(&c))
                {
                    supports_devanagari = true;
                }
                supports_latin = true;

                // Calculate streaming TPS
                if let Some(first_time) = first_chunk_time {
                    let gen_duration = first_time.elapsed().as_secs_f32();
                    if gen_duration > 0.05 && token_chunks > 1 {
                        tps = Some(token_chunks as f32 / gen_duration);
                    }
                }
            }
        }

        // Fallback for non-streaming server responses
        if ttft_ms.is_none() {
            let non_stream_payload = json!({
                "model": model_id,
                "messages": [
                    { "role": "user", "content": "Write 'नमस्ते' in Devanagari script and 'Hello' in English." }
                ],
                "max_tokens": 40,
                "temperature": 0.1
            });

            let mut req2 = client.post(&chat_url).json(&non_stream_payload);
            if let Some(key) = api_key {
                if !key.is_empty() {
                    req2 = req2.header("Authorization", format!("Bearer {}", key));
                }
            }

            let start2 = Instant::now();
            if let Ok(resp2) = req2.send().await {
                let latency = start2.elapsed();
                ttft_ms = Some(latency.as_millis() as u32);
                if resp2.status().is_success() {
                    if let Ok(body) = resp2.json::<serde_json::Value>().await {
                        if let Some(content) = body["choices"][0]["message"]["content"].as_str() {
                            supports_devanagari = content
                                .chars()
                                .any(|c| ('\u{0900}'..='\u{097F}').contains(&c));
                        }
                        if let Some(completion_tokens) = body
                            .get("usage")
                            .and_then(|u| u["completion_tokens"].as_f64())
                        {
                            if completion_tokens > 0.0 {
                                let secs = (latency.as_secs_f32() as f64).max(0.1);
                                tps = Some((completion_tokens / secs) as f32);
                            }
                        }
                    }
                }
            }
        }

        (
            supports_latin,
            supports_devanagari,
            tps,
            ttft_ms,
            headers_indicate_gpu,
            header_context_window,
        )
    }

    async fn structured_tool_probe(
        client: &Client,
        base_url: &str,
        model_id: &str,
        api_key: Option<&str>,
    ) -> bool {
        let chat_url = Self::resolve_chat_url(base_url);
        let tool_payload = json!({
            "model": model_id,
            "messages": [
                { "role": "user", "content": "Fetch database record for user ID 402." }
            ],
            "tools": [
                {
                    "type": "function",
                    "function": {
                        "name": "lookup_user",
                        "description": "Retrieves user record by integer ID",
                        "parameters": {
                            "type": "object",
                            "properties": {
                                "user_id": { "type": "integer" }
                            },
                            "required": ["user_id"]
                        }
                    }
                }
            ],
            "tool_choice": "auto",
            "max_tokens": 80
        });

        let mut req = client.post(&chat_url).json(&tool_payload);
        if let Some(key) = api_key {
            if !key.is_empty() {
                req = req.header("Authorization", format!("Bearer {}", key));
            }
        }

        if let Ok(resp) = req.send().await {
            if resp.status().is_success() {
                if let Ok(body) = resp.json::<serde_json::Value>().await {
                    let msg = &body["choices"][0]["message"];
                    if let Some(tool_calls) = msg.get("tool_calls").and_then(|tc| tc.as_array()) {
                        for tc in tool_calls {
                            if let Some(fn_obj) = tc.get("function") {
                                if let Some(name) = fn_obj.get("name").and_then(|n| n.as_str()) {
                                    if name == "lookup_user" || !name.is_empty() {
                                        return true;
                                    }
                                }
                            }
                        }
                    }
                    if msg.get("function_call").is_some() {
                        return true;
                    }
                }
            }
        }

        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_heuristic_embedded_caps_known_families() {
        // Qwen models
        let (family, tools, devanagari) =
            CapabilityProbeEngine::heuristic_embedded_caps("qwen-3.5-0.8b-q4_k_m.gguf");
        assert_eq!(family, "Qwen");
        assert!(tools);
        assert!(devanagari);

        let (family, tools, devanagari) =
            CapabilityProbeEngine::heuristic_embedded_caps("Qwen/Qwen2.5-Coder-7B-Instruct");
        assert_eq!(family, "Qwen");
        assert!(tools);
        assert!(devanagari);

        // Gemma models
        let (family, tools, devanagari) =
            CapabilityProbeEngine::heuristic_embedded_caps("gemma-4-e2b-q4_k_m.gguf");
        assert_eq!(family, "Gemma");
        assert!(tools);
        assert!(devanagari);

        let (family, tools, devanagari) =
            CapabilityProbeEngine::heuristic_embedded_caps("google/gemma-2-9b-it");
        assert_eq!(family, "Gemma");
        assert!(tools);
        assert!(devanagari);

        // Llama models
        let (family, tools, devanagari) =
            CapabilityProbeEngine::heuristic_embedded_caps("meta-llama/Llama-3.1-8B-Instruct");
        assert_eq!(family, "Llama");
        assert!(tools);
        assert!(devanagari);

        let (family, tools, devanagari) =
            CapabilityProbeEngine::heuristic_embedded_caps("LLAMA-2-7B");
        assert_eq!(family, "Llama");
        assert!(tools);
        assert!(devanagari);
    }

    #[test]
    fn test_heuristic_embedded_caps_unknown_models() {
        let (family, tools, devanagari) =
            CapabilityProbeEngine::heuristic_embedded_caps("mistralai/Mistral-7B-v0.1");
        assert_eq!(family, "Unknown");
        assert!(!tools);
        assert!(!devanagari);

        let (family, tools, devanagari) =
            CapabilityProbeEngine::heuristic_embedded_caps("microsoft/phi-3-mini");
        assert_eq!(family, "Unknown");
        assert!(!tools);
        assert!(!devanagari);

        let (family, tools, devanagari) = CapabilityProbeEngine::heuristic_embedded_caps("");
        assert_eq!(family, "Unknown");
        assert!(!tools);
        assert!(!devanagari);
    }

    #[test]
    fn test_is_cloud_provider_by_provider_name() {
        let cloud_providers = [
            "nvidia",
            "NVIDIA NIM",
            "NIM",
            "groq",
            "Groq Cloud",
            "openrouter",
            "together",
            "Together AI",
            "deepseek",
            "mistral",
            "openai",
            "OpenAI",
            "gemini",
            "Google Gemini",
            "anthropic",
            "Anthropic Claude",
        ];

        for name in cloud_providers {
            assert!(
                CapabilityProbeEngine::is_cloud_provider("http://localhost:8080/v1", name),
                "Expected provider '{}' to be recognized as cloud",
                name
            );
        }
    }

    #[test]
    fn test_is_cloud_provider_by_base_url() {
        let cloud_urls = [
            "https://integrate.api.nvidia.com/v1",
            "https://api.groq.com/openai/v1",
            "https://openrouter.ai/api/v1",
            "https://api.together.xyz/v1",
            "https://api.deepseek.com/v1",
            "https://api.mistral.ai/v1",
            "https://api.openai.com/v1",
            "https://generativelanguage.googleapis.com/v1beta/openai",
            "https://api.anthropic.com/v1",
            "HTTPS://API.OPENAI.COM/V1",
        ];

        for url in cloud_urls {
            assert!(
                CapabilityProbeEngine::is_cloud_provider(url, "custom"),
                "Expected url '{}' to be recognized as cloud",
                url
            );
        }
    }

    #[test]
    fn test_is_cloud_provider_local_and_custom() {
        let local_endpoints = [
            ("http://localhost:11434/v1", "ollama"),
            ("http://127.0.0.1:1234/v1", "lm-studio"),
            ("http://0.0.0.0:8000/v1", "vllm"),
            ("http://192.168.1.100:8080/v1", "local-llama"),
            ("http://localhost:5000/v1", "tabby"),
            ("", ""),
        ];

        for (url, name) in local_endpoints {
            assert!(
                !CapabilityProbeEngine::is_cloud_provider(url, name),
                "Expected ('{}', '{}') to be non-cloud / local",
                url,
                name
            );
        }
    }

    #[test]
    fn test_parse_token_ceiling_from_error_valid_patterns() {
        // Pattern 1: maximum allowed / supported / limit / cap
        assert_eq!(
            CapabilityProbeEngine::parse_token_ceiling_from_error(
                "max_tokens is greater than maximum allowed 16384"
            ),
            Some(16384)
        );
        assert_eq!(
            CapabilityProbeEngine::parse_token_ceiling_from_error(
                "exceeds maximum supported output tokens (16384)"
            ),
            Some(16384)
        );
        assert_eq!(
            CapabilityProbeEngine::parse_token_ceiling_from_error("The model token limit is 8192"),
            Some(8192)
        );
        assert_eq!(
            CapabilityProbeEngine::parse_token_ceiling_from_error(
                "Token cap reached: 32768 tokens maximum"
            ),
            Some(32768)
        );

        // Pattern 2: > N
        assert_eq!(
            CapabilityProbeEngine::parse_token_ceiling_from_error("max_tokens cannot be > 8192"),
            Some(8192)
        );
        assert_eq!(
            CapabilityProbeEngine::parse_token_ceiling_from_error("max_tokens >16384 not allowed"),
            Some(16384)
        );

        // Pattern 3: cannot exceed N
        assert_eq!(
            CapabilityProbeEngine::parse_token_ceiling_from_error(
                "Total response tokens cannot exceed 65536"
            ),
            Some(65536)
        );

        // Pattern 4: greater than N
        assert_eq!(
            CapabilityProbeEngine::parse_token_ceiling_from_error(
                "Parameter max_tokens is greater than 4096"
            ),
            Some(4096)
        );
        assert_eq!(
            CapabilityProbeEngine::parse_token_ceiling_from_error(
                "greater than maximum allowed 131072"
            ),
            Some(131072)
        );
    }

    #[test]
    fn test_parse_token_ceiling_bounds_and_edge_cases() {
        // Lower bound: exactly 256 is accepted
        assert_eq!(
            CapabilityProbeEngine::parse_token_ceiling_from_error("minimum cap 256"),
            Some(256)
        );
        // Below lower bound: 255 rejected
        assert_eq!(
            CapabilityProbeEngine::parse_token_ceiling_from_error("maximum 255"),
            None
        );

        // Upper bound: 2_000_000 accepted
        assert_eq!(
            CapabilityProbeEngine::parse_token_ceiling_from_error("supported limit 2000000"),
            Some(2_000_000)
        );
        // Above upper bound: 2_000_001 rejected
        assert_eq!(
            CapabilityProbeEngine::parse_token_ceiling_from_error("supported limit 2000001"),
            None
        );

        // Non-token or irrelevant errors
        assert_eq!(
            CapabilityProbeEngine::parse_token_ceiling_from_error("Internal server error 500"),
            None
        );
        assert_eq!(
            CapabilityProbeEngine::parse_token_ceiling_from_error(
                "Unauthorized: Invalid API key provided"
            ),
            None
        );
        assert_eq!(
            CapabilityProbeEngine::parse_token_ceiling_from_error(
                "Rate limit exceeded. Try again in 30 seconds."
            ),
            None
        );
        assert_eq!(
            CapabilityProbeEngine::parse_token_ceiling_from_error(""),
            None
        );
    }
}
