use std::{
    collections::HashMap,
    error::Error,
    sync::Arc,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use futures_util::StreamExt;
use reqwest::Client;
use serde::Deserialize;
use serde_json::json;

use super::{
    discovery::{discover_server_dialect, ServerDialect},
    gguf::read_gguf_facts,
    presets::lookup_preset,
    sync::get_baseline_spec,
    types::{
        CapabilityCacheRead, CapabilityProvenance, LlmModelInfo, ModelCapabilities,
        ModelProbeResult, ProbeCheck, ProbeOutcome, CAP_KIND_CLOUD, CAP_KIND_EMBEDDED,
        CAP_KIND_SERVER,
    },
};
use crate::{
    core::{events::PipelineMode, state::AppState},
    services::{
        harness::{
            stages::tools::{web_search::WebSearchTool, ToolDefinition},
            ChatMessage, Role,
        },
        llm::{
            provider::Support,
            transport::{
                chat_completions::{self, ToolCallAccumulator},
                inject_auth_headers,
                ollama::{self, parse_tool_calls_in_line},
                responses::{self, ResponsesToolAccumulator},
                sse::SseDecoder,
                ConnectionConfig, TransportType,
            },
            CanonicalToolCall, CanonicalToolDefinition, ConversationInput, EmbeddedProvider,
            GenerationOptions, GenerationPurpose, GenerationRequest, LlmProvider,
            LlmProviderConfig, OutputConstraint, RemoteTransport, GEMMA_MODEL_DIR, QWEN_MODEL_DIR,
        },
    },
    setup::manifest::VoxManifest,
    utils::paths,
};

pub const DEFAULT_PROBE_TIMEOUT_SECS: u64 = 12;
pub const DEFAULT_VALIDATION_TIMEOUT_SECS: u64 = 8;
pub const DEFAULT_PROBE_MAX_TOKENS: u32 = 40;
pub const DEFAULT_TOOL_PROBE_MAX_TOKENS: u32 = 80;
pub const DEFAULT_PROBE_TEMPERATURE: f32 = 0.1;

#[derive(Default, Debug)]
struct EndpointMeta {
    supports_tools: Support,
    context_window: Option<u32>,
    max_output_tokens: Option<u32>,
    provenance: CapabilityProvenance,
    server_has_gpu: bool,
    is_gpu_accelerated: bool,
    vram_bytes: Option<u64>,
    parameter_size: Option<String>,
    quantization: Option<String>,
    family: Option<String>,
}

/// Outcome of the streaming generation probe: a full measurement or a recorded failure.
/// Failure reasons live on the `streaming` probe check, not here.
enum StreamOutcome {
    Measured {
        latin: bool,
        devanagari: bool,
        tps: f32,
        ttft_ms: u32,
    },
    Failed,
}

/// Outcome of the tool-call probe against one declared production tool.
/// Failure reasons live on the `tool_calls` probe check, not here.
enum ToolOutcome {
    Supported,
    Unsupported,
    Failed,
}

/// Appends one probe check with its elapsed time to the report.
fn record_check(
    checks: &mut Vec<ProbeCheck>,
    id: &str,
    label: &str,
    outcome: ProbeOutcome,
    detail: Option<String>,
    started: Instant,
) {
    checks.push(ProbeCheck {
        id: id.to_string(),
        label: label.to_string(),
        outcome,
        detail,
        duration_ms: Some(started.elapsed().as_millis() as u32),
    });
}

/// Truncates an HTTP error body for embedding in a probe-check detail.
fn error_excerpt(text: &str) -> String {
    const LIMIT: usize = 300;
    let trimmed = text.trim();
    if trimmed.len() <= LIMIT {
        trimmed.to_string()
    } else {
        format!("{}…", &trimmed[..LIMIT])
    }
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
        LlmProviderConfig::Server {
            base_url,
            model,
            api_key,
            provider_name,
            ..
        } => {
            list_remote_models(
                &base_url,
                &model,
                api_key.as_deref(),
                provider_name.as_deref(),
                CAP_KIND_SERVER,
            )
            .await
        }
        LlmProviderConfig::Cloud {
            base_url,
            model,
            api_key,
            provider_name,
            ..
        } => {
            list_remote_models(
                &base_url,
                &model,
                api_key.as_deref(),
                provider_name.as_deref(),
                CAP_KIND_CLOUD,
            )
            .await
        }
    }
}

/// Lists models from a remote endpoint, stamping the catalog-side provider kind
/// the transport cannot distinguish on its own.
async fn list_remote_models(
    base_url: &str,
    model: &str,
    api_key: Option<&str>,
    provider_name: Option<&str>,
    kind: &str,
) -> Result<Vec<LlmModelInfo>, String> {
    let conn_cfg = ConnectionConfig::new(base_url, model, api_key, provider_name);
    let transport = RemoteTransport::new(conn_cfg);
    let mut models = transport.list_models().await.map_err(|e| e.to_string())?;
    for entry in &mut models {
        entry.provider_kind = kind.to_string();
    }
    Ok(models)
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
    let kind = match config {
        LlmProviderConfig::Embedded => CAP_KIND_EMBEDDED,
        LlmProviderConfig::Server { .. } => CAP_KIND_SERVER,
        LlmProviderConfig::Cloud { .. } => CAP_KIND_CLOUD,
    };

    let target = model_id.or(Some(active_model));

    let mut caps = CapabilityProbeEngine::probe_capabilities(&config, target.as_deref(), kind)
        .await
        .map_err(|e| e.to_string())?;

    let mut validated_cap: Option<u32> = None;
    if let Some(cap) = target_cap {
        let (validated, check) =
            CapabilityProbeEngine::validate_token_cap(&config, target.as_deref(), cap).await;
        validated_cap = validated;
        caps.checks.push(check);
    }

    let cache_dir = paths::get().cache.clone();
    let cache_file = cache_dir.join("model_capabilities.json");

    // A corrupt cache is quarantined aside (never silently dropped: its other
    // entries may still be recoverable) and the probe rebuilds from empty.
    // I/O notes are run-level facts: they are reported on the returned result
    // but never persisted into the cached entry, where they would read as
    // stale model evidence on every later view.
    let mut run_note: Option<(ProbeOutcome, String)> = None;
    let mut map: HashMap<String, ModelCapabilities> = if cache_file.exists() {
        match tokio::fs::read_to_string(&cache_file).await {
            Ok(content) => match serde_json::from_str(&content) {
                Ok(parsed) => parsed,
                Err(err) => {
                    let backup = quarantine_corrupt_cache(&cache_file).await;
                    run_note = Some((
                        ProbeOutcome::Failed,
                        format!(
                            "Capability cache was corrupt ({}); moved aside to {}. This probe rebuilt it.",
                            err, backup
                        ),
                    ));
                    HashMap::new()
                }
            },
            Err(err) => {
                run_note = Some((
                    ProbeOutcome::Failed,
                    format!("Capability cache is unreadable: {}", err),
                ));
                HashMap::new()
            }
        }
    } else {
        HashMap::new()
    };
    if let Some((_, ref detail)) = run_note {
        log::warn!("[Catalog::Probe] {}", detail);
    }

    let key = format!("{}:{}", caps.provider_kind, caps.model_id);
    map.insert(key, caps.clone());

    let write_started = Instant::now();
    let write_note: Option<(ProbeOutcome, String)> =
        match write_capability_cache(&cache_dir, &cache_file, &map).await {
            Ok(()) => None,
            Err(err) => {
                log::warn!("[Catalog::Probe] {}", err);
                Some((ProbeOutcome::Failed, err))
            }
        };

    if let Some((outcome, ref detail)) = run_note {
        record_check(
            &mut caps.checks,
            "cache_read",
            "Capability cache read",
            outcome,
            Some(detail.clone()),
            Instant::now(),
        );
    }
    match write_note.as_ref() {
        Some((outcome, detail)) => record_check(
            &mut caps.checks,
            "cache_write",
            "Capability cache write",
            *outcome,
            Some(detail.clone()),
            write_started,
        ),
        None => record_check(
            &mut caps.checks,
            "cache_write",
            "Capability cache write",
            ProbeOutcome::Measured,
            None,
            write_started,
        ),
    }

    let cache_error = match (&run_note, &write_note) {
        (Some((_, r)), Some((_, w))) => Some(format!("{}; {}", r, w)),
        (Some((_, r)), None) => Some(r.clone()),
        (None, Some((_, w))) => Some(w.clone()),
        (None, None) => None,
    };

    Ok(ModelProbeResult {
        capabilities: caps,
        validated_cap,
        cached_map: map,
        cache_error,
    })
}

/// Builds the exact request production would send for a probe turn: same builders,
/// same policy gating, same envelopes. The probe never hand-crafts wire payloads.
fn probe_generation_request(
    prompt: &str,
    max_tokens: u32,
    tools: Option<Vec<CanonicalToolDefinition>>,
) -> GenerationRequest {
    GenerationRequest {
        input: ConversationInput {
            messages: vec![ChatMessage::new(Role::User, prompt.to_string())],
        },
        options: GenerationOptions {
            temperature: Some(DEFAULT_PROBE_TEMPERATURE),
            max_output_tokens: Some(max_tokens),
            ..Default::default()
        },
        output: OutputConstraint::Text,
        purpose: GenerationPurpose::Conversation,
        tools,
    }
}

/// Declares the single production tool the tool probe validates: web search,
/// chosen for the widest parameter schema. Byte-identical to harness declarations.
fn probe_tool_declaration() -> CanonicalToolDefinition {
    WebSearchTool.to_canonical(PipelineMode::Modular)
}

/// Decides the tool verdict from observed calls, returning the verdict with its
/// evidence string. A call naming the declared tool is support; other calls or
/// a clean empty stream are unsupported; only transport trouble is failure.
/// A matching call observed before a stream error still counts as support.
fn tool_verdict(
    observed: &[CanonicalToolCall],
    tool_name: &str,
    truncation: Option<&str>,
) -> (ToolOutcome, String) {
    if observed.iter().any(|c| c.name == tool_name) {
        return (
            ToolOutcome::Supported,
            format!("declared tool '{}' emitted with valid arguments", tool_name),
        );
    }
    if let Some(note) = truncation {
        if observed.is_empty() {
            return (ToolOutcome::Failed, note.to_string());
        }
    }
    if observed.is_empty() {
        return (
            ToolOutcome::Unsupported,
            "endpoint completed without tool calls".to_string(),
        );
    }
    let names: Vec<&str> = observed.iter().map(|c| c.name.as_str()).collect();
    (
        ToolOutcome::Unsupported,
        format!(
            "model emitted undeclared tool calls instead of '{}': {}",
            tool_name,
            names.join(", ")
        ),
    )
}

/// Reports whether one Ollama NDJSON line ends the stream.
fn ollama_stream_is_complete(line: &str) -> bool {
    serde_json::from_str::<serde_json::Value>(line)
        .ok()
        .and_then(|v| v.get("done").and_then(|d| d.as_bool()))
        .unwrap_or(false)
}

/// Reports whether one Responses SSE line ends the stream.
fn responses_stream_is_complete(line: &str) -> bool {
    serde_json::from_str::<serde_json::Value>(line)
        .ok()
        .and_then(|v| {
            v.get("type")
                .and_then(|t| t.as_str())
                .map(|t| t == "response.completed")
        })
        .unwrap_or(false)
}

/// Extracts generated text tokens from one stream line for the probe's transport.
fn extract_probe_text(line: &str, transport: TransportType) -> Vec<String> {
    let mut out = Vec::new();
    let value = match serde_json::from_str::<serde_json::Value>(line) {
        Ok(value) => value,
        Err(_) => return out,
    };
    match transport {
        TransportType::OllamaNative => {
            if let Some(text) = value
                .get("message")
                .and_then(|m| m.get("content"))
                .and_then(|s| s.as_str())
            {
                out.push(text.to_string());
            }
        }
        TransportType::Responses => {
            if value.get("type").and_then(|t| t.as_str()) == Some("response.output_text.delta") {
                if let Some(text) = value.get("delta").and_then(|d| d.as_str()) {
                    out.push(text.to_string());
                }
            }
        }
        TransportType::ChatCompletions => {
            if let Some(text) = value
                .get("choices")
                .and_then(|c| c.get(0))
                .and_then(|c0| c0.get("delta"))
                .and_then(|d| d.get("content"))
                .and_then(|s| s.as_str())
            {
                out.push(text.to_string());
            }
        }
    }
    out
}

/// Locates an embedded weight file by filename in the known model directories.
fn locate_embedded_weights(model_id: &str) -> Option<std::path::PathBuf> {
    let models = paths::get().models;
    [
        models.join(model_id),
        models.join(QWEN_MODEL_DIR).join(model_id),
        models.join(GEMMA_MODEL_DIR).join(model_id),
    ]
    .into_iter()
    .find(|candidate| candidate.is_file())
}

/// Finds the declared parameter count for an embedded model in the manifest.
/// Matches on file path, file id, group id, or group name; returns None when undeclared.
fn find_manifest_parameters(manifest: &VoxManifest, model_id: &str) -> Option<String> {
    for group in &manifest.model_groups {
        if group.id == model_id || group.name == model_id {
            return group.parameters.clone();
        }
        for file in &group.files {
            if file.path == model_id || file.id == model_id {
                return group.parameters.clone();
            }
        }
    }
    None
}

/// Moves a corrupt capability cache file aside to a timestamped backup so user
/// data is not silently deleted and the corrupt file can be inspected/recovered.
async fn quarantine_corrupt_cache(cache_file: &std::path::Path) -> String {
    let timestamp_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let backup_name = format!("model_capabilities.corrupt-{}.json", timestamp_ms);
    let backup_file = match cache_file.parent() {
        Some(dir) => dir.join(backup_name),
        None => std::path::PathBuf::from(backup_name),
    };
    match tokio::fs::rename(cache_file, &backup_file).await {
        Ok(()) => backup_file.display().to_string(),
        Err(err) => {
            log::warn!(
                "[Catalog::Probe] Failed to quarantine corrupt cache: {}",
                err
            );
            format!(
                "{} (quarantine rename failed: {})",
                cache_file.display(),
                err
            )
        }
    }
}

/// Atomically writes the capability cache map, reporting the first failure.
async fn write_capability_cache(
    cache_dir: &std::path::Path,
    cache_file: &std::path::Path,
    map: &HashMap<String, ModelCapabilities>,
) -> Result<(), String> {
    if let Err(err) = tokio::fs::create_dir_all(cache_dir).await {
        return Err(format!(
            "Failed to create capability cache dir {:?}: {}",
            cache_dir, err
        ));
    }
    let json = serde_json::to_string_pretty(map)
        .map_err(|err| format!("Failed to serialize capability cache: {}", err))?;
    let tmp = cache_file.with_extension("tmp");
    tokio::fs::write(&tmp, json)
        .await
        .map_err(|err| format!("Failed to write capability cache: {}", err))?;
    tokio::fs::rename(&tmp, cache_file)
        .await
        .map_err(|err| format!("Failed to replace capability cache: {}", err))?;
    Ok(())
}

/// Reads the on-disk capability cache without contacting any endpoint.
pub async fn read_capabilities_cache() -> Result<CapabilityCacheRead, String> {
    let cache_file = paths::get().cache.join("model_capabilities.json");
    if !cache_file.exists() {
        return Ok(CapabilityCacheRead {
            cached_map: HashMap::new(),
            cache_error: None,
        });
    }
    match tokio::fs::read_to_string(&cache_file).await {
        Ok(content) => match serde_json::from_str(&content) {
            Ok(cached_map) => Ok(CapabilityCacheRead {
                cached_map,
                cache_error: None,
            }),
            Err(err) => {
                let backup = quarantine_corrupt_cache(&cache_file).await;
                Ok(CapabilityCacheRead {
                    cached_map: HashMap::new(),
                    cache_error: Some(format!(
                        "Capability cache was corrupt ({}); moved aside to {}.",
                        err, backup
                    )),
                })
            }
        },
        Err(err) => Ok(CapabilityCacheRead {
            cached_map: HashMap::new(),
            cache_error: Some(format!("Capability cache is unreadable: {}", err)),
        }),
    }
}

/// Empirical Capability Discovery Engine.
pub struct CapabilityProbeEngine;

impl CapabilityProbeEngine {
    /// Probes model capabilities for a specific model override or active configuration.
    pub async fn probe_capabilities(
        config: &LlmProviderConfig,
        target_model: Option<&str>,
        kind: &str,
    ) -> Result<ModelCapabilities, Box<dyn Error + Send + Sync>> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        match config {
            LlmProviderConfig::Embedded => {
                let model_id = target_model.unwrap_or("embedded-default.gguf");
                Ok(Self::probe_local_embedded(model_id, now))
            }
            LlmProviderConfig::Server {
                base_url,
                model,
                api_key,
                provider_name,
                ..
            }
            | LlmProviderConfig::Cloud {
                base_url,
                model,
                api_key,
                provider_name,
                ..
            } => {
                let model_id = target_model.unwrap_or(model);
                let conn_cfg = ConnectionConfig::new(
                    base_url,
                    model_id,
                    api_key.as_deref(),
                    provider_name.as_deref(),
                );
                let client = Client::builder()
                    .timeout(Duration::from_secs(DEFAULT_PROBE_TIMEOUT_SECS))
                    .build()?;

                Self::probe_remote_endpoint(&client, &conn_cfg, kind, now).await
            }
        }
    }

    /// Builds capability facts for a local embedded GGUF model from declared static sources.
    /// Only values present in the local models manifest or the GGUF file header are
    /// reported; everything else stays Unknown. The embedded engine implements no
    /// tool calls, so tool support is a declared No rather than a probe result.
    pub fn probe_local_embedded(model_id: &str, now: u64) -> ModelCapabilities {
        let started = Instant::now();
        let mut checks = Vec::new();
        let manifest_path = paths::get().models.join("models_manifest.json");
        let mut parameter_size: Option<String> = None;

        match std::fs::read_to_string(&manifest_path) {
            Ok(content) => match serde_json::from_str::<VoxManifest>(&content) {
                Ok(manifest) => {
                    parameter_size = find_manifest_parameters(&manifest, model_id);
                    record_check(
                        &mut checks,
                        "manifest_read",
                        "Embedded models manifest read",
                        ProbeOutcome::Measured,
                        parameter_size
                            .clone()
                            .map(|p| format!("declared parameters: {}", p)),
                        started,
                    );
                }
                Err(err) => {
                    let detail = format!("Embedded models manifest is corrupt: {}", err);
                    log::warn!("[Catalog::Probe] {}", detail);
                    record_check(
                        &mut checks,
                        "manifest_read",
                        "Embedded models manifest read",
                        ProbeOutcome::Failed,
                        Some(detail),
                        started,
                    );
                }
            },
            Err(err) => {
                let detail = format!("Embedded models manifest is unreadable: {}", err);
                log::warn!("[Catalog::Probe] {}", detail);
                record_check(
                    &mut checks,
                    "manifest_read",
                    "Embedded models manifest read",
                    ProbeOutcome::Failed,
                    Some(detail),
                    started,
                );
            }
        }

        let gguf_started = Instant::now();
        let (context_window, family) = match locate_embedded_weights(model_id) {
            Some(path) => match read_gguf_facts(&path) {
                Some(facts) => {
                    record_check(
                        &mut checks,
                        "gguf_header",
                        "GGUF weight header read",
                        ProbeOutcome::Measured,
                        Some(format!("header of {}", path.display())),
                        gguf_started,
                    );
                    (facts.context_length, facts.architecture)
                }
                None => {
                    let detail = format!(
                        "GGUF header of {} is unreadable or unsupported",
                        path.display()
                    );
                    log::warn!("[Catalog::Probe] {}", detail);
                    record_check(
                        &mut checks,
                        "gguf_header",
                        "GGUF weight header read",
                        ProbeOutcome::Failed,
                        Some(detail),
                        gguf_started,
                    );
                    (None, None)
                }
            },
            None => {
                let detail = format!("No weight file found for {}", model_id);
                log::warn!("[Catalog::Probe] {}", detail);
                record_check(
                    &mut checks,
                    "gguf_header",
                    "GGUF weight header read",
                    ProbeOutcome::Failed,
                    Some(detail),
                    gguf_started,
                );
                (None, None)
            }
        };

        record_check(
            &mut checks,
            "embedded_throughput",
            "Embedded throughput measurement",
            ProbeOutcome::Skipped,
            Some(
                "Throughput requires loading the model weights; run a benchmark for measured numbers."
                    .to_string(),
            ),
            Instant::now(),
        );

        ModelCapabilities {
            model_id: model_id.to_string(),
            provider_kind: CAP_KIND_EMBEDDED.to_string(),
            supports_tools: Support::Unsupported,
            supports_latin: Support::Unknown,
            supports_devanagari: Support::Unknown,
            context_window,
            max_output_tokens: None,
            provenance: CapabilityProvenance::DeclaredStatic,
            tps: None,
            ttft_ms: None,
            server_has_gpu: false,
            is_gpu_accelerated: false,
            gpu_status: "Local CPU / In-Process".to_string(),
            vram_bytes: None,
            parameter_size,
            quantization: None,
            family,
            tested_at_epoch: now,
            checks,
        }
    }

    /// Probes remote endpoint using empirical observation, baseline catalog, and native APIs.
    pub async fn probe_remote_endpoint(
        client: &Client,
        config: &ConnectionConfig,
        kind: &str,
        now: u64,
    ) -> Result<ModelCapabilities, Box<dyn Error + Send + Sync>> {
        let mut checks: Vec<ProbeCheck> = Vec::new();
        let preset_meta = config.provider_preset.as_deref().and_then(lookup_preset);
        let baseline_spec = get_baseline_spec(&config.model);
        // A baseline hit is recorded; a miss is normal (most models are not in
        // the catalog) and stays silent — provenance already tells the story.
        if baseline_spec.is_some() {
            record_check(
                &mut checks,
                "baseline_lookup",
                "Baseline catalog lookup",
                ProbeOutcome::Measured,
                Some(format!("catalog entry found for {}", config.model)),
                Instant::now(),
            );
        }

        let stream = Self::empirical_streaming_probe(client, config, &mut checks).await;
        let tool = Self::empirical_tool_probe(client, config, &mut checks).await;

        let (supports_latin, supports_devanagari, tps, ttft_ms) = match stream {
            StreamOutcome::Measured {
                latin,
                devanagari,
                tps,
                ttft_ms,
            } => (
                Support::from(latin),
                Support::from(devanagari),
                Some(tps),
                Some(ttft_ms),
            ),
            StreamOutcome::Failed => (Support::Unknown, Support::Unknown, None, None),
        };
        let mut supports_tools = match tool {
            ToolOutcome::Supported => Support::Supported,
            ToolOutcome::Unsupported => Support::Unsupported,
            ToolOutcome::Failed => Support::Unknown,
        };

        let mut meta = EndpointMeta {
            provenance: CapabilityProvenance::Unknown,
            ..Default::default()
        };

        // Seed from baseline catalog if available
        if let Some(ref base) = baseline_spec {
            meta.context_window = base.context_window;
            meta.max_output_tokens = base.max_output_tokens;
            meta.family = base.family.clone();
            meta.provenance = base.provenance;
        }

        match config.transport {
            TransportType::OllamaNative => {
                Self::probe_ollama_metadata(client, config, &mut meta, &mut checks).await;
            }
            TransportType::ChatCompletions | TransportType::Responses => {
                let dialect_started = Instant::now();
                let dialect = discover_server_dialect(client, &config.base_url, &config.auth).await;
                record_check(
                    &mut checks,
                    "dialect",
                    "Server dialect detection",
                    ProbeOutcome::Measured,
                    Some(format!("detected {}", dialect.display_name())),
                    dialect_started,
                );
                if matches!(dialect, ServerDialect::Ollama { .. }) {
                    Self::probe_ollama_metadata(client, config, &mut meta, &mut checks).await;
                }

                if let Some(meta_preset) = preset_meta {
                    if meta.context_window.is_none() {
                        meta.context_window = meta_preset.context_window;
                        if meta.provenance == CapabilityProvenance::Unknown {
                            meta.provenance = CapabilityProvenance::CatalogBaseline;
                        }
                    }
                }
            }
        }

        // A server-declared tool capability upgrades ignorance, never an empirical verdict.
        if supports_tools == Support::Unknown && meta.supports_tools == Support::Supported {
            supports_tools = Support::Supported;
        }

        // If empirical probe streamed successfully, upgrade provenance
        if tps.is_some() && meta.provenance != CapabilityProvenance::Unknown {
            meta.provenance = CapabilityProvenance::ProbedServer;
        }

        let is_gpu = meta.is_gpu_accelerated || meta.server_has_gpu;
        let gpu_check_failed = checks
            .iter()
            .any(|c| c.id == "ollama_gpu" && c.outcome == ProbeOutcome::Failed);
        let gpu_check_skipped = !checks.iter().any(|c| c.id == "ollama_gpu");
        let gpu_status = if is_gpu {
            if let Some(vram) = meta.vram_bytes {
                let mb = vram / (1024 * 1024);
                format!("GPU Active (VRAM: {} MB)", mb)
            } else {
                "GPU Active (Hardware Accelerated)".to_string()
            }
        } else if gpu_check_failed || gpu_check_skipped {
            "GPU state unknown".to_string()
        } else {
            "CPU Inference / Standard".to_string()
        };

        Ok(ModelCapabilities {
            model_id: config.model.clone(),
            provider_kind: kind.to_string(),
            supports_tools,
            supports_latin,
            supports_devanagari,
            context_window: meta.context_window,
            max_output_tokens: meta.max_output_tokens,
            provenance: meta.provenance,
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
            checks,
        })
    }

    async fn probe_ollama_metadata(
        client: &Client,
        config: &ConnectionConfig,
        meta: &mut EndpointMeta,
        checks: &mut Vec<ProbeCheck>,
    ) {
        let base_url = config.base_url.trim_end_matches('/');
        let root = base_url.strip_suffix("/v1").unwrap_or(base_url);
        let show_url = format!("{}/api/show", root);
        let show_payload = json!({ "name": config.model });
        let mut builder = client.post(&show_url).json(&show_payload);
        builder = inject_auth_headers(builder, &config.auth);

        let show_started = Instant::now();
        match builder.send().await {
            Err(err) => record_check(
                checks,
                "ollama_show",
                "Ollama model metadata",
                ProbeOutcome::Failed,
                Some(format!("request failed: {}", err)),
                show_started,
            ),
            Ok(resp) => {
                let status = resp.status();
                if !status.is_success() {
                    let excerpt = resp.text().await.unwrap_or_default();
                    record_check(
                        checks,
                        "ollama_show",
                        "Ollama model metadata",
                        ProbeOutcome::Failed,
                        Some(format!("HTTP {}: {}", status, error_excerpt(&excerpt))),
                        show_started,
                    );
                } else {
                    match resp.json::<OllamaShowResponse>().await {
                        Err(err) => record_check(
                            checks,
                            "ollama_show",
                            "Ollama model metadata",
                            ProbeOutcome::Failed,
                            Some(format!("response is not Ollama metadata: {}", err)),
                            show_started,
                        ),
                        Ok(show) => {
                            if let Some(caps) = show.capabilities {
                                if caps.iter().any(|c| c.eq_ignore_ascii_case("tools")) {
                                    meta.supports_tools = Support::Supported;
                                }
                            }
                            if let Some(info) = show.model_info {
                                if let Some(obj) = info.as_object() {
                                    for (k, v) in obj {
                                        if k.ends_with(".context_length") {
                                            if let Some(len) = v.as_u64() {
                                                meta.context_window = Some(len as u32);
                                                meta.provenance =
                                                    CapabilityProvenance::ProbedServer;
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
                            record_check(
                                checks,
                                "ollama_show",
                                "Ollama model metadata",
                                ProbeOutcome::Measured,
                                None,
                                show_started,
                            );
                        }
                    }
                }
            }
        }

        let ps_url = format!("{}/api/ps", root);
        let mut ps_builder = client.get(&ps_url);
        ps_builder = inject_auth_headers(ps_builder, &config.auth);

        let ps_started = Instant::now();
        match ps_builder.send().await {
            Err(err) => record_check(
                checks,
                "ollama_gpu",
                "Ollama GPU residency",
                ProbeOutcome::Failed,
                Some(format!("request failed: {}", err)),
                ps_started,
            ),
            Ok(resp) => {
                let status = resp.status();
                if !status.is_success() {
                    let excerpt = resp.text().await.unwrap_or_default();
                    record_check(
                        checks,
                        "ollama_gpu",
                        "Ollama GPU residency",
                        ProbeOutcome::Failed,
                        Some(format!("HTTP {}: {}", status, error_excerpt(&excerpt))),
                        ps_started,
                    );
                } else {
                    match resp.json::<OllamaPsResponse>().await {
                        Err(err) => record_check(
                            checks,
                            "ollama_gpu",
                            "Ollama GPU residency",
                            ProbeOutcome::Failed,
                            Some(format!("response is not Ollama process list: {}", err)),
                            ps_started,
                        ),
                        Ok(ps) => {
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
                                }
                            }
                            record_check(
                                checks,
                                "ollama_gpu",
                                "Ollama GPU residency",
                                ProbeOutcome::Measured,
                                None,
                                ps_started,
                            );
                        }
                    }
                }
            }
        }
    }

    async fn empirical_streaming_probe(
        client: &Client,
        config: &ConnectionConfig,
        checks: &mut Vec<ProbeCheck>,
    ) -> StreamOutcome {
        let started = Instant::now();
        let request = probe_generation_request(
            "Respond strictly with: Hello नमस्ते",
            DEFAULT_PROBE_MAX_TOKENS,
            None,
        );
        let (url, body) = match config.transport {
            TransportType::OllamaNative => (
                ollama::resolve_url(&config.base_url),
                ollama::build_request_body(config, &request),
            ),
            TransportType::Responses => (
                responses::resolve_url(&config.base_url),
                responses::build_request_body(config, &request),
            ),
            TransportType::ChatCompletions => (
                chat_completions::resolve_url(&config.base_url),
                chat_completions::build_request_body(config, &request),
            ),
        };

        let mut builder = client.post(&url).json(&body);
        builder = inject_auth_headers(builder, &config.auth);

        let fail = |detail: String, checks: &mut Vec<ProbeCheck>| {
            record_check(
                checks,
                "streaming",
                "Streaming generation probe",
                ProbeOutcome::Failed,
                Some(detail),
                started,
            );
            StreamOutcome::Failed
        };

        let response = match builder.send().await {
            Ok(res) => res,
            Err(err) => return fail(format!("request failed: {}", err), checks),
        };
        if !response.status().is_success() {
            let status = response.status();
            let excerpt = response.text().await.unwrap_or_default();
            return fail(
                format!("HTTP {}: {}", status, error_excerpt(&excerpt)),
                checks,
            );
        }

        let t_start = Instant::now();
        let mut first_token_time = None;
        let mut token_count = 0usize;
        let mut accumulated_text = String::new();

        let mut decoder = SseDecoder::new();
        let mut stream = response.bytes_stream();

        while let Some(item) = stream.next().await {
            let bytes = match item {
                Ok(bytes) => bytes,
                Err(err) => {
                    return fail(
                        format!("stream interrupted after {} tokens: {}", token_count, err),
                        checks,
                    )
                }
            };
            let lines = decoder.decode_chunk(&bytes);
            for line in lines {
                if line == "[DONE]" {
                    break;
                }
                for token in extract_probe_text(&line, config.transport) {
                    if first_token_time.is_none() {
                        first_token_time = Some(t_start.elapsed().as_millis() as u32);
                    }
                    token_count += 1;
                    accumulated_text.push_str(&token);
                }
            }
        }

        if token_count == 0 {
            return fail(
                "endpoint answered with an empty completion".to_string(),
                checks,
            );
        }

        let elapsed = t_start.elapsed().as_secs_f32();
        if elapsed <= 0.0 {
            return fail(
                "completion arrived with no measurable duration".to_string(),
                checks,
            );
        }
        let tps = token_count as f32 / elapsed;
        let ttft_ms = match first_token_time {
            Some(ttft) => ttft,
            None => return fail("no first token was observed".to_string(), checks),
        };

        let latin = accumulated_text.chars().any(|c| c.is_ascii_alphabetic());
        let devanagari = accumulated_text
            .chars()
            .any(|c| ('\u{0900}'..='\u{097F}').contains(&c));

        record_check(
            checks,
            "streaming",
            "Streaming generation probe",
            ProbeOutcome::Measured,
            Some(format!("{} tokens at {:.1} tps", token_count, tps)),
            started,
        );
        StreamOutcome::Measured {
            latin,
            devanagari,
            tps,
            ttft_ms,
        }
    }

    async fn empirical_tool_probe(
        client: &Client,
        config: &ConnectionConfig,
        checks: &mut Vec<ProbeCheck>,
    ) -> ToolOutcome {
        let started = Instant::now();
        let finish = |outcome: ToolOutcome, detail: String, checks: &mut Vec<ProbeCheck>| {
            let kind = match outcome {
                ToolOutcome::Supported => ProbeOutcome::Measured,
                ToolOutcome::Unsupported => ProbeOutcome::Unsupported,
                ToolOutcome::Failed => ProbeOutcome::Failed,
            };
            record_check(
                checks,
                "tool_calls",
                "Tool call probe",
                kind,
                Some(detail),
                started,
            );
            outcome
        };

        let tool_def = probe_tool_declaration();
        let tool_name = tool_def.name.clone();
        let request = probe_generation_request(
            "What is the current weather in Tokyo?",
            DEFAULT_TOOL_PROBE_MAX_TOKENS,
            Some(vec![tool_def]),
        );
        let (url, body) = match config.transport {
            TransportType::OllamaNative => (
                ollama::resolve_url(&config.base_url),
                ollama::build_request_body(config, &request),
            ),
            TransportType::Responses => (
                responses::resolve_url(&config.base_url),
                responses::build_request_body(config, &request),
            ),
            TransportType::ChatCompletions => (
                chat_completions::resolve_url(&config.base_url),
                chat_completions::build_request_body(config, &request),
            ),
        };

        let mut builder = client.post(&url).json(&body);
        builder = inject_auth_headers(builder, &config.auth);

        let response = match builder.send().await {
            Ok(res) => res,
            Err(err) => {
                return finish(
                    ToolOutcome::Failed,
                    format!("request failed: {}", err),
                    checks,
                )
            }
        };
        if !response.status().is_success() {
            let status = response.status();
            let excerpt = response.text().await.unwrap_or_default();
            return finish(
                ToolOutcome::Failed,
                format!("HTTP {}: {}", status, error_excerpt(&excerpt)),
                checks,
            );
        }

        let mut chat_accumulator = ToolCallAccumulator::default();
        let mut responses_accumulator = ResponsesToolAccumulator::default();
        let mut observed: Vec<CanonicalToolCall> = Vec::new();

        let mut decoder = SseDecoder::new();
        let mut stream = response.bytes_stream();

        while let Some(item) = stream.next().await {
            let bytes = match item {
                Ok(bytes) => bytes,
                Err(err) => {
                    let (outcome, detail) = tool_verdict(
                        &observed,
                        &tool_name,
                        Some(&format!("stream interrupted: {}", err)),
                    );
                    return finish(outcome, detail, checks);
                }
            };
            let lines = decoder.decode_chunk(&bytes);
            for line in lines {
                match config.transport {
                    TransportType::OllamaNative => {
                        observed.extend(parse_tool_calls_in_line(&line));
                        if ollama_stream_is_complete(&line) {
                            let (outcome, detail) = tool_verdict(&observed, &tool_name, None);
                            return finish(outcome, detail, checks);
                        }
                    }
                    TransportType::Responses => {
                        observed.extend(responses_accumulator.feed_line(&line));
                        if line == "[DONE]" || responses_stream_is_complete(&line) {
                            let (outcome, detail) = tool_verdict(&observed, &tool_name, None);
                            return finish(outcome, detail, checks);
                        }
                    }
                    TransportType::ChatCompletions => {
                        if line == "[DONE]" {
                            observed.extend(chat_accumulator.drain());
                            let (outcome, detail) = tool_verdict(&observed, &tool_name, None);
                            return finish(outcome, detail, checks);
                        }
                        chat_accumulator.feed_line(&line);
                    }
                }
            }
        }

        observed.extend(chat_accumulator.drain());
        let (outcome, detail) = tool_verdict(&observed, &tool_name, None);
        finish(outcome, detail, checks)
    }

    /// Smoke validation for custom token caps without error-text scraping.
    pub async fn validate_token_cap(
        config: &LlmProviderConfig,
        target_model_id: Option<&str>,
        target_cap: u32,
    ) -> (Option<u32>, ProbeCheck) {
        let started = Instant::now();
        let done = |outcome: ProbeOutcome, detail: Option<String>| ProbeCheck {
            id: "token_cap".to_string(),
            label: "Token cap validation".to_string(),
            outcome,
            detail,
            duration_ms: Some(started.elapsed().as_millis() as u32),
        };

        let (base_url, model, api_key, provider_name) = match config {
            LlmProviderConfig::Server {
                base_url,
                model,
                api_key,
                provider_name,
                ..
            }
            | LlmProviderConfig::Cloud {
                base_url,
                model,
                api_key,
                provider_name,
                ..
            } => (base_url, model, api_key, provider_name),
            LlmProviderConfig::Embedded => {
                return (
                    None,
                    done(
                        ProbeOutcome::Skipped,
                        Some("token-cap validation needs a remote endpoint".to_string()),
                    ),
                )
            }
        };

        let conn_cfg = ConnectionConfig::new(
            base_url,
            target_model_id.unwrap_or(model),
            api_key.as_deref(),
            provider_name.as_deref(),
        );
        let client = match Client::builder()
            .timeout(Duration::from_secs(DEFAULT_VALIDATION_TIMEOUT_SECS))
            .build()
        {
            Ok(client) => client,
            Err(err) => {
                return (
                    None,
                    done(
                        ProbeOutcome::Failed,
                        Some(format!("HTTP client failed to build: {}", err)),
                    ),
                )
            }
        };

        let url = chat_completions::resolve_url(&conn_cfg.base_url);
        let payload = json!({
            "model": conn_cfg.model,
            "messages": [{"role": "user", "content": "."}],
            "max_tokens": target_cap
        });

        let mut builder = client.post(&url).json(&payload);
        builder = inject_auth_headers(builder, &conn_cfg.auth);

        let resp = match builder.send().await {
            Ok(resp) => resp,
            Err(err) => {
                return (
                    None,
                    done(
                        ProbeOutcome::Failed,
                        Some(format!("request failed: {}", err)),
                    ),
                )
            }
        };
        if resp.status().is_success() {
            (
                Some(target_cap),
                done(
                    ProbeOutcome::Measured,
                    Some(format!("endpoint accepted max_tokens={}", target_cap)),
                ),
            )
        } else {
            let status = resp.status().as_u16();
            let text = resp.text().await.unwrap_or_default();
            if status == 400 && text.to_lowercase().contains("context_length_exceeded") {
                (
                    None,
                    done(
                        ProbeOutcome::Failed,
                        Some(format!(
                            "endpoint rejected the cap (context_length_exceeded): {}",
                            error_excerpt(&text)
                        )),
                    ),
                )
            } else {
                (
                    None,
                    done(
                        ProbeOutcome::Failed,
                        Some(format!(
                            "endpoint returned HTTP {}: {}",
                            status,
                            error_excerpt(&text)
                        )),
                    ),
                )
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{
        io::{Read, Write},
        net::TcpListener,
    };

    use super::*;

    /// Serves canned HTTP responses, one per connection, then exits.
    /// Every response carries `Connection: close` so no keep-alive pooling
    /// merges connections: one probe request always equals one accept.
    fn serve_canned(responses: Vec<(u16, String)>) -> (String, std::thread::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind mock server");
        let url = format!(
            "http://{}",
            listener.local_addr().expect("mock server addr")
        );
        let handle = std::thread::spawn(move || {
            for (status, body) in responses {
                let (mut stream, _) = listener.accept().expect("mock accept");
                stream
                    .set_read_timeout(Some(Duration::from_secs(5)))
                    .expect("mock read timeout");
                consume_request(&mut stream);
                let reason = if status == 200 { "OK" } else { "Error" };
                let head = format!(
                    "HTTP/1.1 {} {}\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    status,
                    reason,
                    body.len()
                );
                stream.write_all(head.as_bytes()).expect("mock write head");
                stream.write_all(body.as_bytes()).expect("mock write body");
            }
        });
        (url, handle)
    }

    /// Serves a response whose declared length exceeds the bytes sent, then
    /// closes the connection: the client observes a mid-stream transport error.
    fn serve_truncated(
        status: u16,
        claimed_len: usize,
        body: String,
    ) -> (String, std::thread::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind mock server");
        let url = format!(
            "http://{}",
            listener.local_addr().expect("mock server addr")
        );
        let handle = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("mock accept");
            stream
                .set_read_timeout(Some(Duration::from_secs(5)))
                .expect("mock read timeout");
            consume_request(&mut stream);
            let head = format!(
                "HTTP/1.1 {} Error\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                status, claimed_len
            );
            stream.write_all(head.as_bytes()).expect("mock write head");
            stream.write_all(body.as_bytes()).expect("mock write body");
        });
        (url, handle)
    }

    /// Consumes request headers plus body so the client never blocks on send.
    fn consume_request(stream: &mut std::net::TcpStream) {
        let mut buf = vec![0u8; 65536];
        let mut read = 0usize;
        let header_end = loop {
            if read >= buf.len() {
                return;
            }
            match stream.read(&mut buf[read..]) {
                Ok(0) => return,
                Ok(n) => {
                    read += n;
                    if let Some(pos) = find_header_end(&buf[..read]) {
                        break pos;
                    }
                }
                Err(_) => return,
            }
        };
        let headers = String::from_utf8_lossy(&buf[..header_end]);
        let mut remaining = 0usize;
        for line in headers.lines() {
            if let Some(value) = line.strip_prefix("content-length:") {
                remaining = value.trim().parse().unwrap_or(0);
            } else if let Some(value) = line.strip_prefix("Content-Length:") {
                remaining = value.trim().parse().unwrap_or(0);
            }
        }
        let body_read = read.saturating_sub(header_end);
        remaining = remaining.saturating_sub(body_read);
        let mut discard = vec![0u8; remaining.min(65536)];
        while remaining > 0 {
            let chunk = remaining.min(discard.len());
            match stream.read(&mut discard[..chunk]) {
                Ok(0) => return,
                Ok(n) => remaining -= n,
                Err(_) => return,
            }
        }
    }

    /// Locates the end of HTTP headers in a buffer.
    fn find_header_end(buf: &[u8]) -> Option<usize> {
        buf.windows(4)
            .position(|w| w == b"\r\n\r\n")
            .map(|pos| pos + 4)
    }

    /// Builds a probe client plus connection config against a mock server.
    fn mock_setup(base_url: &str, preset: Option<&str>) -> (Client, ConnectionConfig) {
        let client = Client::builder()
            .timeout(Duration::from_secs(5))
            .build()
            .expect("probe test client");
        let config = ConnectionConfig::new(base_url, "test-model", None, preset);
        (client, config)
    }

    /// Runs an async probe body under the mandatory hard timeout.
    async fn run_probed<F, T>(future: F) -> T
    where
        F: std::future::Future<Output = T>,
    {
        tokio::time::timeout(Duration::from_secs(10), future)
            .await
            .expect("probe test timed out")
    }

    #[test]
    fn tool_verdict_supports_declared_tool() {
        let observed = vec![CanonicalToolCall {
            id: "call_1".to_string(),
            name: "web_search".to_string(),
            arguments: serde_json::json!({"query": "x"}),
        }];
        let (outcome, _) = tool_verdict(&observed, "web_search", None);
        assert!(matches!(outcome, ToolOutcome::Supported));
    }

    #[test]
    fn tool_verdict_names_undeclared_tools() {
        let observed = vec![CanonicalToolCall {
            id: "call_1".to_string(),
            name: "get_weather".to_string(),
            arguments: serde_json::json!({}),
        }];
        let (outcome, detail) = tool_verdict(&observed, "web_search", None);
        match outcome {
            ToolOutcome::Unsupported => {
                assert!(detail.contains("get_weather"), "detail: {}", detail);
                assert!(detail.contains("web_search"), "detail: {}", detail);
            }
            _ => panic!("expected Unsupported"),
        }
    }

    #[test]
    fn tool_verdict_empty_clean_stream_is_unsupported() {
        let (outcome, detail) = tool_verdict(&[], "web_search", None);
        match outcome {
            ToolOutcome::Unsupported => {
                assert!(detail.contains("without tool calls"), "detail: {}", detail);
            }
            _ => panic!("expected Unsupported"),
        }
    }

    #[test]
    fn tool_verdict_truncation_without_calls_is_failure() {
        let (outcome, detail) = tool_verdict(&[], "web_search", Some("stream interrupted: reset"));
        match outcome {
            ToolOutcome::Failed => {
                assert!(detail.contains("reset"), "detail: {}", detail);
            }
            _ => panic!("expected Failed"),
        }
    }

    #[test]
    fn extract_probe_text_per_transport() {
        let chat = r#"{"choices":[{"delta":{"content":"Hello "}}]}"#;
        assert_eq!(
            extract_probe_text(chat, TransportType::ChatCompletions),
            vec!["Hello ".to_string()]
        );
        let ollama = r#"{"message":{"content":"Hello "}}"#;
        assert_eq!(
            extract_probe_text(ollama, TransportType::OllamaNative),
            vec!["Hello ".to_string()]
        );
        let responses = r#"{"type":"response.output_text.delta","delta":"Hello "}"#;
        assert_eq!(
            extract_probe_text(responses, TransportType::Responses),
            vec!["Hello ".to_string()]
        );
        assert!(extract_probe_text("not json", TransportType::ChatCompletions).is_empty());
        assert!(extract_probe_text(r#"{"choices":[]}"#, TransportType::ChatCompletions).is_empty());
    }

    #[test]
    fn stream_completion_markers() {
        assert!(ollama_stream_is_complete(r#"{"done":true}"#));
        assert!(!ollama_stream_is_complete(r#"{"done":false}"#));
        assert!(responses_stream_is_complete(
            r#"{"type":"response.completed"}"#
        ));
        assert!(!responses_stream_is_complete(
            r#"{"type":"response.output_text.delta"}"#
        ));
    }

    #[test]
    fn error_excerpt_truncates_long_bodies() {
        let long = "x".repeat(500);
        let excerpt = error_excerpt(&long);
        assert!(excerpt.len() <= 310, "len: {}", excerpt.len());
        assert_eq!(error_excerpt("  short  "), "short");
    }

    #[test]
    fn probe_tool_declaration_is_production_web_search() {
        use crate::services::llm::transport::canonical_tools_json;
        let def = probe_tool_declaration();
        assert_eq!(def.name, "web_search");
        let wire = canonical_tools_json(&[def]);
        let tool = &wire[0];
        assert_eq!(tool["type"], "function");
        assert_eq!(tool["function"]["name"], "web_search");
        let required = tool["function"]["parameters"]["required"]
            .as_array()
            .expect("parameters.required must be an array");
        let names: Vec<&str> = required.iter().filter_map(|v| v.as_str()).collect();
        assert!(names.contains(&"query"), "required: {:?}", names);
        assert!(names.contains(&"spoken_filler"), "required: {:?}", names);
    }

    #[test]
    fn accumulator_splits_parallel_index_zero_calls() {
        use crate::services::llm::transport::chat_completions::ToolCallAccumulator;
        let mut acc = ToolCallAccumulator::default();
        acc.feed_line(r#"{"choices":[{"delta":{"tool_calls":[{"index":0,"id":"call_1","function":{"name":"web_search","arguments":"{}"}}]}}]}"#);
        acc.feed_line(r#"{"choices":[{"delta":{"tool_calls":[{"index":0,"id":"call_2","function":{"name":"web_search","arguments":"{}"}}]}}]}"#);
        let calls = acc.drain();
        assert_eq!(calls.len(), 2, "index:0 reuse must not merge calls");
        assert_eq!(calls[0].id, "call_1");
        assert_eq!(calls[1].id, "call_2");
    }

    /// Finds a recorded probe check by id, failing the test when absent.
    fn find_check<'a>(checks: &'a [ProbeCheck], id: &str) -> &'a ProbeCheck {
        checks
            .iter()
            .find(|c| c.id == id)
            .unwrap_or_else(|| panic!("check {} was not recorded", id))
    }

    #[tokio::test]
    async fn streaming_probe_unauthorized_records_failed() {
        let (url, server) = serve_canned(vec![(401, "unauthorized".to_string())]);
        let (client, config) = mock_setup(&url, None);
        let mut checks = Vec::new();
        let outcome = run_probed(CapabilityProbeEngine::empirical_streaming_probe(
            &client,
            &config,
            &mut checks,
        ))
        .await;
        assert!(matches!(outcome, StreamOutcome::Failed));
        let check = find_check(&checks, "streaming");
        assert_eq!(check.outcome, ProbeOutcome::Failed);
        let detail = check
            .detail
            .as_ref()
            .expect("failed check must carry detail");
        assert!(detail.contains("401"), "detail: {}", detail);
        server.join().expect("mock server panicked");
    }

    #[tokio::test]
    async fn streaming_probe_empty_completion_is_failed() {
        let (url, server) = serve_canned(vec![(200, "data: [DONE]\n\n".to_string())]);
        let (client, config) = mock_setup(&url, None);
        let mut checks = Vec::new();
        let outcome = run_probed(CapabilityProbeEngine::empirical_streaming_probe(
            &client,
            &config,
            &mut checks,
        ))
        .await;
        assert!(matches!(outcome, StreamOutcome::Failed));
        let check = find_check(&checks, "streaming");
        let detail = check
            .detail
            .as_ref()
            .expect("failed check must carry detail");
        assert!(detail.contains("empty"), "detail: {}", detail);
        server.join().expect("mock server panicked");
    }

    #[tokio::test]
    async fn streaming_probe_measures_scripts_and_tps() {
        let body = "data: {\"choices\":[{\"delta\":{\"content\":\"Hello \"}}]}\n\ndata: {\"choices\":[{\"delta\":{\"content\":\"नमस्ते\"}}]}\n\ndata: [DONE]\n\n";
        let (url, server) = serve_canned(vec![(200, body.to_string())]);
        let (client, config) = mock_setup(&url, None);
        let mut checks = Vec::new();
        let outcome = run_probed(CapabilityProbeEngine::empirical_streaming_probe(
            &client,
            &config,
            &mut checks,
        ))
        .await;
        match outcome {
            StreamOutcome::Measured {
                latin,
                devanagari,
                tps,
                ttft_ms: _,
            } => {
                assert!(latin);
                assert!(devanagari);
                assert!(tps > 0.0);
            }
            StreamOutcome::Failed => panic!("expected Measured"),
        }
        server.join().expect("mock server panicked");
    }

    #[tokio::test]
    async fn streaming_probe_truncation_yields_no_tps() {
        let partial = "data: {\"choices\":[{\"delta\":{\"content\":\"Hello \"}}]}\n\n";
        let (url, server) = serve_truncated(200, partial.len() + 5000, partial.to_string());
        let (client, config) = mock_setup(&url, None);
        let mut checks = Vec::new();
        let outcome = run_probed(CapabilityProbeEngine::empirical_streaming_probe(
            &client,
            &config,
            &mut checks,
        ))
        .await;
        assert!(matches!(outcome, StreamOutcome::Failed));
        let check = find_check(&checks, "streaming");
        let detail = check
            .detail
            .as_ref()
            .expect("failed check must carry detail");
        assert!(detail.contains("interrupted"), "detail: {}", detail);
        server.join().expect("mock server panicked");
    }

    #[tokio::test]
    async fn tool_probe_supported_via_declared_tool_call() {
        let body = "data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"id\":\"call_1\",\"function\":{\"name\":\"web_search\",\"arguments\":\"{\\\"query\\\":\\\"weather in Tokyo\\\",\\\"spoken_filler\\\":\\\"Searching now\\\"}\"}}]}}]}\n\ndata: {\"choices\":[{\"delta\":{},\"finish_reason\":\"tool_calls\"}]}\n\ndata: [DONE]\n\n";
        let (url, server) = serve_canned(vec![(200, body.to_string())]);
        let (client, config) = mock_setup(&url, None);
        let mut checks = Vec::new();
        let outcome = run_probed(CapabilityProbeEngine::empirical_tool_probe(
            &client,
            &config,
            &mut checks,
        ))
        .await;
        assert!(matches!(outcome, ToolOutcome::Supported));
        server.join().expect("mock server panicked");
    }

    #[tokio::test]
    async fn tool_probe_text_only_is_unsupported() {
        let body = "data: {\"choices\":[{\"delta\":{\"content\":\"It is sunny.\"},\"finish_reason\":\"stop\"}]}\n\ndata: [DONE]\n\n";
        let (url, server) = serve_canned(vec![(200, body.to_string())]);
        let (client, config) = mock_setup(&url, None);
        let mut checks = Vec::new();
        let outcome = run_probed(CapabilityProbeEngine::empirical_tool_probe(
            &client,
            &config,
            &mut checks,
        ))
        .await;
        assert!(matches!(outcome, ToolOutcome::Unsupported));
        let check = find_check(&checks, "tool_calls");
        assert_eq!(check.outcome, ProbeOutcome::Unsupported);
        let detail = check.detail.as_ref().expect("check must carry detail");
        assert!(detail.contains("without tool calls"), "detail: {}", detail);
        server.join().expect("mock server panicked");
    }

    #[tokio::test]
    async fn tool_probe_rate_limit_is_failed_not_unsupported() {
        let (url, server) = serve_canned(vec![(429, "{\"error\":\"rate limited\"}".to_string())]);
        let (client, config) = mock_setup(&url, None);
        let mut checks = Vec::new();
        let outcome = run_probed(CapabilityProbeEngine::empirical_tool_probe(
            &client,
            &config,
            &mut checks,
        ))
        .await;
        assert!(matches!(outcome, ToolOutcome::Failed));
        let check = find_check(&checks, "tool_calls");
        assert_eq!(check.outcome, ProbeOutcome::Failed);
        let detail = check.detail.as_ref().expect("check must carry detail");
        assert!(detail.contains("429"), "detail: {}", detail);
        server.join().expect("mock server panicked");
    }

    #[tokio::test]
    async fn tool_probe_undeclared_tool_is_unsupported() {
        let body = "data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"id\":\"call_9\",\"function\":{\"name\":\"get_weather\",\"arguments\":\"{}\"}}]}}]}\n\ndata: [DONE]\n\n";
        let (url, server) = serve_canned(vec![(200, body.to_string())]);
        let (client, config) = mock_setup(&url, None);
        let mut checks = Vec::new();
        let outcome = run_probed(CapabilityProbeEngine::empirical_tool_probe(
            &client,
            &config,
            &mut checks,
        ))
        .await;
        assert!(matches!(outcome, ToolOutcome::Unsupported));
        let check = find_check(&checks, "tool_calls");
        let detail = check.detail.as_ref().expect("check must carry detail");
        assert!(detail.contains("get_weather"), "detail: {}", detail);
        server.join().expect("mock server panicked");
    }

    #[tokio::test]
    async fn tool_probe_ollama_native_uses_ndjson() {
        let body = "{\"message\":{\"role\":\"assistant\",\"tool_calls\":[{\"function\":{\"name\":\"web_search\",\"arguments\":{\"query\":\"weather in Tokyo\",\"spoken_filler\":\"Searching now\"}}}]},\"done\":false}\n{\"message\":{\"role\":\"assistant\",\"content\":\"\"},\"done\":true}\n";
        let (url, server) = serve_canned(vec![(200, body.to_string())]);
        let (client, config) = mock_setup(&url, Some("ollama"));
        assert_eq!(
            config.transport,
            TransportType::OllamaNative,
            "ollama preset must resolve to native transport"
        );
        let mut checks = Vec::new();
        let outcome = run_probed(CapabilityProbeEngine::empirical_tool_probe(
            &client,
            &config,
            &mut checks,
        ))
        .await;
        assert!(matches!(outcome, ToolOutcome::Supported));
        server.join().expect("mock server panicked");
    }

    #[tokio::test]
    async fn ollama_metadata_404_records_failed_checks() {
        let (url, server) = serve_canned(vec![
            (404, "not found".to_string()),
            (404, "not found".to_string()),
        ]);
        let (client, config) = mock_setup(&url, Some("ollama"));
        let mut meta = EndpointMeta::default();
        let mut checks = Vec::new();
        run_probed(CapabilityProbeEngine::probe_ollama_metadata(
            &client,
            &config,
            &mut meta,
            &mut checks,
        ))
        .await;
        assert_eq!(checks.len(), 2);
        assert!(checks.iter().all(|c| c.outcome == ProbeOutcome::Failed));
        assert!(checks
            .iter()
            .all(|c| c.detail.as_ref().is_some_and(|d| d.contains("404"))));
        assert_eq!(meta.supports_tools, Support::Unknown);
        server.join().expect("mock server panicked");
    }

    #[tokio::test]
    async fn validate_token_cap_accepts_working_cap() {
        let (url, server) = serve_canned(vec![(200, "{}".to_string())]);
        let provider = LlmProviderConfig::Server {
            base_url: url.clone(),
            model: "test-model".to_string(),
            api_key: None,
            provider_name: None,
            protocol: None,
        };
        let (validated, check) = run_probed(CapabilityProbeEngine::validate_token_cap(
            &provider, None, 4096,
        ))
        .await;
        assert_eq!(validated, Some(4096));
        assert_eq!(check.outcome, ProbeOutcome::Measured);
        server.join().expect("mock server panicked");
    }

    #[tokio::test]
    async fn validate_token_cap_rejects_overlong_cap() {
        let body = "{\"error\":{\"message\":\"context_length_exceeded: too long\",\"type\":\"invalid_request\"}}";
        let (url, server) = serve_canned(vec![(400, body.to_string())]);
        let provider = LlmProviderConfig::Server {
            base_url: url.clone(),
            model: "test-model".to_string(),
            api_key: None,
            provider_name: None,
            protocol: None,
        };
        let (validated, check) = run_probed(CapabilityProbeEngine::validate_token_cap(
            &provider, None, 999999,
        ))
        .await;
        assert_eq!(validated, None);
        assert_eq!(check.outcome, ProbeOutcome::Failed);
        let detail = check.detail.expect("failed check must carry detail");
        assert!(
            detail.contains("context_length_exceeded"),
            "detail: {}",
            detail
        );
        server.join().expect("mock server panicked");
    }
}
