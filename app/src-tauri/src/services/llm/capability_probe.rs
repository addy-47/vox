use anyhow::Result;
use reqwest::Client;
use serde::Deserialize;
use serde_json::json;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

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

impl CapabilityProbeEngine {
    /// Probes runtime model capabilities for local, remote (Ollama/vLLM/LM Studio), or cloud (OpenAI/Gemini/Anthropic) models.
    pub async fn probe_capabilities(
        config: &LlmProviderConfig,
        target_model_id: Option<&str>,
    ) -> Result<ModelCapabilities> {
        let client = Client::builder()
            .timeout(Duration::from_secs(10))
            .build()?;

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        match config {
            LlmProviderConfig::Embedded => {
                let model_id = target_model_id.unwrap_or("embedded_llama").to_string();
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
                    gpu_status: "CPU Only (Local Embedded)".to_string(),
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

                log::info!(
                    "[CapabilityProbe] Initiating capability probe for model '{}' on endpoint '{}' (provider_name: '{}')...",
                    model_id,
                    base_url,
                    prov_name
                );

                // 1. Cloud Provider Fast Path
                if prov_name == "gemini"
                    || prov_name == "openai"
                    || prov_name == "anthropic"
                    || base_url.contains("generativelanguage.googleapis.com")
                    || base_url.contains("api.openai.com")
                    || base_url.contains("anthropic.com")
                {
                    log::info!(
                        "[CapabilityProbe] Target is a managed Cloud provider. Using cloud metadata fast-path."
                    );
                    let (ctx, family) = Self::cloud_provider_metadata(&prov_name, &model_id);

                    let caps = ModelCapabilities {
                        model_id: model_id.clone(),
                        provider_kind: "open_ai_compat".to_string(),
                        supports_tools: true,
                        supports_latin: true,
                        supports_devanagari: true,
                        context_window: Some(ctx),
                        tps: None,
                        ttft_ms: None,
                        server_has_gpu: true,
                        is_gpu_accelerated: true,
                        gpu_status: "Cloud GPU/TPU Cluster".to_string(),
                        vram_bytes: None,
                        parameter_size: None,
                        quantization: None,
                        family: Some(family),
                        tested_at_epoch: now,
                    };

                    log::info!(
                        "[CapabilityProbe] Cloud model '{}' capabilities: ctx={}, tools=true, gpu=Cloud",
                        model_id,
                        ctx
                    );

                    return Ok(caps);
                }

                // 2. STEP 1 (CRITICAL): Execute OpenAI-Compatible Inference Probe FIRST
                // This forces lazy-loading servers (Ollama, LM Studio, vLLM) to load the model into VRAM/RAM!
                log::info!(
                    "[CapabilityProbe] Phase 1: Executing live inference prompt via /v1/chat/completions to wake up server and measure performance..."
                );

                let (supports_latin, supports_devanagari, tps, ttft_ms, tool_probe_success, headers_indicate_gpu) =
                    Self::functional_inference_probe(
                        &client,
                        base_url,
                        &model_id,
                        api_key.as_deref(),
                    )
                    .await;

                log::info!(
                    "[CapabilityProbe] Phase 1 Complete: ttft={:?}ms, tps={:?}, devanagari={}, latin={}, tool_probe={}",
                    ttft_ms,
                    tps,
                    supports_devanagari,
                    supports_latin,
                    tool_probe_success
                );

                // 3. STEP 2: Secondary Metadata Probe (/api/show and /api/ps now that model is loaded)
                log::info!(
                    "[CapabilityProbe] Phase 2: Inspecting server metadata and GPU offloading status..."
                );

                let show_url = format!("{}/api/show", base_url.trim_end_matches('/'));
                let ps_url = format!("{}/api/ps", base_url.trim_end_matches('/'));

                let mut supports_tools = tool_probe_success;
                let mut context_window: Option<u32> = None;
                let mut server_has_gpu = headers_indicate_gpu;
                let mut is_gpu_accelerated = false;
                let mut vram_bytes: Option<u64> = None;
                let mut family: Option<String> = None;
                let mut parameter_size: Option<String> = None;
                let mut quantization: Option<String> = None;

                // Query /api/show (Ollama native API)
                let show_res = client
                    .post(&show_url)
                    .json(&json!({ "name": model_id }))
                    .send()
                    .await;

                if let Ok(resp) = show_res {
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

                // Query /api/ps (Now that model has been loaded via Phase 1 inference request!)
                if let Ok(resp) = client.get(&ps_url).send().await {
                    if resp.status().is_success() {
                        if let Ok(ps_data) = resp.json::<OllamaPsResponse>().await {
                            if let Some(models) = ps_data.models {
                                if !models.is_empty() {
                                    server_has_gpu = true; // Server returned loaded models
                                }
                                for m in models {
                                    if m.name == model_id || m.name.starts_with(&model_id) {
                                        server_has_gpu = true;
                                        if let Some(vram) = m.size_vram {
                                            if vram > 0 {
                                                is_gpu_accelerated = true;
                                                vram_bytes = Some(vram);
                                                log::info!(
                                                    "[CapabilityProbe] Live model '{}' is GPU offloaded in VRAM: {} bytes",
                                                    model_id,
                                                    vram
                                                );
                                            } else {
                                                log::warn!(
                                                    "[CapabilityProbe] Server has GPU, but model '{}' size_vram is 0 (Model CPU-bound / User override).",
                                                    model_id
                                                );
                                            }
                                        }
                                        if context_window.is_none() && m.context_length.is_some() {
                                            context_window = m.context_length;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                // High TPS heuristic for non-Ollama servers (e.g., vLLM or LM Studio on CUDA)
                if !is_gpu_accelerated && tps.unwrap_or(0.0) > 20.0 {
                    server_has_gpu = true;
                    is_gpu_accelerated = true;
                    log::info!("[CapabilityProbe] TPS > 20.0 on remote endpoint indicates GPU acceleration.");
                }

                // Build human-readable GPU status string
                let gpu_status = if is_gpu_accelerated {
                    if let Some(vram) = vram_bytes {
                        format!("GPU Accelerated (VRAM: {} MB)", vram / (1024 * 1024))
                    } else {
                        "GPU Accelerated".to_string()
                    }
                } else if server_has_gpu {
                    "Server GPU Present (Model CPU-Bound)".to_string()
                } else {
                    "CPU Only".to_string()
                };

                let final_caps = ModelCapabilities {
                    model_id: model_id.clone(),
                    provider_kind: "open_ai_compat".to_string(),
                    supports_tools,
                    supports_latin,
                    supports_devanagari,
                    context_window: context_window.or(Some(4096)),
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

    fn cloud_provider_metadata(prov_name: &str, model_id: &str) -> (u32, String) {
        let lower = model_id.to_lowercase();
        if prov_name == "gemini" || lower.contains("gemini") {
            (1_048_576, "Gemini".to_string())
        } else if prov_name == "anthropic" || lower.contains("claude") {
            (200_000, "Claude".to_string())
        } else if lower.contains("gpt-4") {
            (128_000, "GPT-4".to_string())
        } else {
            (128_000, "Cloud".to_string())
        }
    }

    async fn functional_inference_probe(
        client: &Client,
        base_url: &str,
        model_id: &str,
        api_key: Option<&str>,
    ) -> (bool, bool, Option<f32>, Option<u32>, bool, bool) {
        let mut supports_latin = true;
        let mut supports_devanagari = false;
        let mut tps = None;
        let mut ttft_ms = None;
        let mut tool_probe_success = false;
        let mut headers_indicate_gpu = false;

        // --- Probe A: Script & Generation Probe ---
        let chat_url = format!("{}/v1/chat/completions", base_url.trim_end_matches('/'));
        let prompt_payload = json!({
            "model": model_id,
            "messages": [
                { "role": "user", "content": "Write 'नमस्ते' in Devanagari script and 'Hello' in English." }
            ],
            "max_tokens": 50,
            "temperature": 0.1
        });

        let mut req = client.post(&chat_url).json(&prompt_payload);
        if let Some(key) = api_key {
            if !key.is_empty() {
                req = req.header("Authorization", format!("Bearer {}", key));
            }
        }

        let start = Instant::now();
        if let Ok(resp) = req.send().await {
            let latency = start.elapsed();
            ttft_ms = Some(latency.as_millis() as u32);

            // Inspect HTTP Headers & Server Signatures
            let headers = resp.headers();
            if let Some(server_hdr) = headers.get("server").and_then(|v| v.to_str().ok()) {
                let s = server_hdr.to_lowercase();
                if s.contains("vllm") || s.contains("cuda") || s.contains("metal") {
                    headers_indicate_gpu = true;
                }
            }

            if resp.status().is_success() {
                if let Ok(body) = resp.json::<serde_json::Value>().await {
                    // Check system fingerprint if present (vLLM / LM Studio / OpenAI)
                    if let Some(fp) = body.get("system_fingerprint").and_then(|f| f.as_str()) {
                        let fp_lower = fp.to_lowercase();
                        if fp_lower.contains("cuda") || fp_lower.contains("vllm") || fp_lower.contains("gpu") {
                            headers_indicate_gpu = true;
                        }
                    }

                    if let Some(content) = body["choices"][0]["message"]["content"].as_str() {
                        supports_devanagari = content.chars().any(|c| ('\u{0900}'..='\u{097F}').contains(&c));
                        supports_latin = true;

                        // Check if response contains native server timing metrics (Ollama / vLLM / LM Studio)
                        if let (Some(eval_count), Some(eval_duration)) = (
                            body.get("eval_count").and_then(|v| v.as_f64()),
                            body.get("eval_duration").and_then(|v| v.as_f64()),
                        ) {
                            if eval_count > 0.0 && eval_duration > 0.0 {
                                let pure_eval_secs = eval_duration / 1_000_000_000.0;
                                let calc_tps = (eval_count / pure_eval_secs) as f32;
                                tps = Some(calc_tps);
                                log::info!(
                                    "[CapabilityProbe] Native server evaluation TPS: {:.2} (eval_count={}, pure_eval_duration={:.2}s)",
                                    calc_tps,
                                    eval_count,
                                    pure_eval_secs
                                );
                            }
                        }

                        // Fallback: Use usage["completion_tokens"] excluding cold-load time
                        if tps.is_none() {
                            if let Some(usage) = body.get("usage") {
                                if let Some(completion_tokens) = usage["completion_tokens"].as_f64() {
                                    let load_secs = body.get("load_duration").and_then(|v| v.as_f64()).unwrap_or(0.0) / 1_000_000_000.0;
                                    let prompt_secs = body.get("prompt_eval_duration").and_then(|v| v.as_f64()).unwrap_or(0.0) / 1_000_000_000.0;
                                    let pure_gen_secs = (latency.as_secs_f32() as f64 - load_secs - prompt_secs).max(0.05);

                                    if completion_tokens > 0.0 {
                                        let calc_tps = (completion_tokens / pure_gen_secs) as f32;
                                        tps = Some(calc_tps);
                                        log::info!(
                                            "[CapabilityProbe] Adjusted TPS: {:.2} (tokens={}, pure_gen_duration={:.2}s, cold_load_time={:.2}s)",
                                            calc_tps,
                                            completion_tokens,
                                            pure_gen_secs,
                                            load_secs
                                        );
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // --- Probe B: Tool Calling Functional Probe via /v1/chat/completions ---
        let tool_payload = json!({
            "model": model_id,
            "messages": [
                { "role": "user", "content": "What is the weather in Tokyo?" }
            ],
            "tools": [
                {
                    "type": "function",
                    "function": {
                        "name": "get_weather",
                        "description": "Get current weather for a location",
                        "parameters": {
                            "type": "object",
                            "properties": {
                                "location": { "type": "string" }
                            },
                            "required": ["location"]
                        }
                    }
                }
            ],
            "max_tokens": 60
        });

        let mut tool_req = client.post(&chat_url).json(&tool_payload);
        if let Some(key) = api_key {
            if !key.is_empty() {
                tool_req = tool_req.header("Authorization", format!("Bearer {}", key));
            }
        }

        if let Ok(resp) = tool_req.send().await {
            if resp.status().is_success() {
                if let Ok(body) = resp.json::<serde_json::Value>().await {
                    let msg = &body["choices"][0]["message"];
                    if msg.get("tool_calls").is_some() || msg.get("function_call").is_some() {
                        tool_probe_success = true;
                    }
                }
            }
        }

        (
            supports_latin,
            supports_devanagari,
            tps,
            ttft_ms,
            tool_probe_success,
            headers_indicate_gpu,
        )
    }
}
