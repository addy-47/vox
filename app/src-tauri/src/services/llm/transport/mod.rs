pub mod chat_completions;
pub mod config;
pub mod ollama;
pub mod responses;
pub mod sse;

use std::{
    sync::{mpsc, Arc},
    time::Duration,
};

pub use config::{AuthScheme, ConnectionConfig, TransportType};
use futures_util::future::BoxFuture;
use parking_lot::RwLock;
use serde::Deserialize;

use crate::{
    core::settings::LlmModelInfo,
    services::llm::{
        CanonicalToolDefinition, GenerationRequest, LlmError, LlmStreamEvent, OutputConstraint,
        ProviderCapabilities, ProviderKind, ReasoningMode, Support,
        DEFAULT_CLIENT_CONNECT_TIMEOUT_SECS, DEFAULT_CLIENT_REQUEST_TIMEOUT_SECS,
    },
};

#[derive(Deserialize)]
struct ModelListResponse {
    #[serde(default)]
    data: Vec<ModelListEntry>,
}

#[derive(Deserialize)]
struct ModelListEntry {
    id: String,
}

#[derive(Deserialize)]
struct OllamaTagsResponse {
    #[serde(default)]
    models: Vec<OllamaTagsEntry>,
}

#[derive(Deserialize)]
struct OllamaTagsEntry {
    name: String,
    #[serde(default)]
    size: Option<u64>,
}

/// Unified remote transport provider implementing `LlmProvider` via explicit `ConnectionConfig`.
pub struct RemoteTransport {
    config: ConnectionConfig,
    client: reqwest::Client,
    active_token_limit: Arc<RwLock<&'static str>>,
    capabilities: ProviderCapabilities,
}

/// Injects authentication headers into a request builder based on the explicit `AuthScheme`.
pub fn inject_auth_headers(
    mut builder: reqwest::RequestBuilder,
    auth: &AuthScheme,
) -> reqwest::RequestBuilder {
    match auth {
        AuthScheme::Bearer(Some(key)) => {
            if !key.trim().is_empty() {
                builder = builder.bearer_auth(key);
            }
        }
        AuthScheme::AnthropicNative(key) => {
            builder = builder
                .header("x-api-key", key)
                .header("anthropic-version", "2023-06-01");
        }
        AuthScheme::Bearer(None) | AuthScheme::None => {}
    }
    builder
}

/// Serializes canonical tool definitions into the shared OpenAI function envelope.
pub fn canonical_tools_json(tools: &[CanonicalToolDefinition]) -> serde_json::Value {
    serde_json::Value::Array(
        tools
            .iter()
            .map(|t| {
                serde_json::json!({
                    "type": "function",
                    "function": {
                        "name": t.name,
                        "description": t.description,
                        "parameters": t.parameters
                    }
                })
            })
            .collect(),
    )
}

/// Inserts a value at a dotted path, creating intermediate objects as needed.
pub fn insert_dotted(
    body: &mut serde_json::Map<String, serde_json::Value>,
    path: &str,
    value: serde_json::Value,
) {
    let mut split = path.splitn(2, '.');
    let (Some(first), second) = (split.next(), split.next()) else {
        return;
    };
    if first.is_empty() {
        return;
    }
    match second {
        None => {
            body.insert(first.to_string(), value);
        }
        Some(rest) => {
            let nested = body
                .entry(first.to_string())
                .or_insert_with(|| serde_json::json!({}));
            if !nested.is_object() {
                *nested = serde_json::json!({});
            }
            if let Some(obj) = nested.as_object_mut() {
                insert_dotted(obj, rest, value);
            }
        }
    }
}

impl RemoteTransport {
    /// Creates a new `RemoteTransport` from explicit connection configuration.
    pub fn new(config: ConnectionConfig) -> Self {
        let initial_limit = config.policy.token_limit;
        let client = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(DEFAULT_CLIENT_CONNECT_TIMEOUT_SECS))
            .timeout(Duration::from_secs(DEFAULT_CLIENT_REQUEST_TIMEOUT_SECS))
            .build()
            .unwrap_or_else(|e| {
                log::warn!(
                    "[LLM Transport] Failed to build tuned HTTP client ({}). Using default client.",
                    e
                );
                reqwest::Client::new()
            });

        let capabilities = ProviderCapabilities {
            temperature: Support::Supported,
            top_p: Support::Supported,
            top_k: Support::Unknown,
            max_output_tokens: Support::Supported,
            json_object: Support::Supported,
            json_schema: Support::Supported,
            streaming: Support::Supported,
            seed: Support::Supported,
        };

        Self {
            config,
            client,
            active_token_limit: Arc::new(RwLock::new(initial_limit)),
            capabilities,
        }
    }

    /// Returns the active connection configuration.
    pub fn config(&self) -> &ConnectionConfig {
        &self.config
    }

    /// Routes a generation request to the configured wire transport.
    fn dispatch_stream<'a>(
        &'a self,
        cfg: &'a ConnectionConfig,
        request: &'a GenerationRequest,
        turn_id: u32,
        cancel: &'a tokio_util::sync::CancellationToken,
        tx: &'a mpsc::Sender<LlmStreamEvent>,
    ) -> BoxFuture<'a, Result<(), LlmError>> {
        Box::pin(async move {
            match cfg.transport {
                TransportType::OllamaNative => {
                    ollama::stream_ollama(&self.client, cfg, request, turn_id, cancel, tx).await
                }
                TransportType::Responses => {
                    responses::stream_responses(&self.client, cfg, request, turn_id, cancel, tx)
                        .await
                }
                TransportType::ChatCompletions => {
                    chat_completions::stream_chat_completions(
                        &self.client,
                        cfg,
                        request,
                        turn_id,
                        cancel,
                        tx,
                    )
                    .await
                }
            }
        })
    }

    /// Flips the token-limit path after a provider 400 rejection, returning true when flipped.
    fn flip_token_field(&self, cfg: &mut ConnectionConfig) -> bool {
        let next_limit = match cfg.policy.token_limit {
            "max_tokens" => "max_completion_tokens",
            "max_completion_tokens" => "max_tokens",
            _ => return false,
        };
        log::info!(
            "[RemoteTransport] Provider rejected {}, negotiating to {}",
            cfg.policy.token_limit,
            next_limit
        );
        *self.active_token_limit.write() = next_limit;
        cfg.policy.token_limit = next_limit;
        true
    }
}

/// Reports whether a 400 message names the token-limit field.
fn is_token_field_rejection(msg_lower: &str) -> bool {
    msg_lower.contains("unsupported_parameter")
        || msg_lower.contains("max_completion_tokens")
        || msg_lower.contains("max_tokens")
}

/// Strips provider-rejected format controls, returning true when the request was degraded.
fn degrade_request_on_unsupported(request: &mut GenerationRequest, msg_lower: &str) -> bool {
    if request.options.reasoning == ReasoningMode::Disabled
        && (msg_lower.contains("reasoning") || msg_lower.contains("think"))
    {
        log::warn!(
            "[RemoteTransport] Provider rejected reasoning controls; retrying without them."
        );
        request.options.reasoning = ReasoningMode::Enabled;
        return true;
    }
    if msg_lower.contains("response_format")
        || msg_lower.contains("json_schema")
        || msg_lower.contains("json_object")
        || msg_lower.contains("structured-outputs")
        || msg_lower.contains("structured outputs")
    {
        match &request.output {
            OutputConstraint::JsonSchema { .. } => {
                log::warn!("[RemoteTransport] Provider rejected strict schema; falling back to JSON-object.");
                request.output = OutputConstraint::JsonObject;
                return true;
            }
            OutputConstraint::JsonObject => {
                log::warn!("[RemoteTransport] Provider rejected JSON-object; falling back to prompt-only JSON.");
                request.output = OutputConstraint::Text;
                return true;
            }
            OutputConstraint::Text => {}
        }
    }
    false
}

impl super::LlmProvider for RemoteTransport {
    fn generate<'a>(
        &'a self,
        request: GenerationRequest,
        turn_id: u32,
        cancel: &'a tokio_util::sync::CancellationToken,
        tx: &'a mpsc::Sender<super::LlmStreamEvent>,
    ) -> BoxFuture<'a, Result<(), LlmError>> {
        Box::pin(async move {
            let mut cfg = self.config.clone();
            cfg.policy.token_limit = *self.active_token_limit.read();
            let mut request = request;

            for _ in 0..3 {
                let res = self
                    .dispatch_stream(&cfg, &request, turn_id, cancel, tx)
                    .await;
                match res {
                    Err(LlmError::Provider { status, message }) if status == 400 => {
                        let msg_lower = message.to_lowercase();
                        if degrade_request_on_unsupported(&mut request, &msg_lower) {
                            continue;
                        }
                        if is_token_field_rejection(&msg_lower) && self.flip_token_field(&mut cfg) {
                            continue;
                        }
                        return Err(LlmError::Provider { status, message });
                    }
                    other => return other,
                }
            }

            Err(LlmError::Transport(
                "request negotiation exhausted after format fallbacks".to_string(),
            ))
        })
    }

    fn health_check<'a>(&'a self) -> BoxFuture<'a, Result<(), LlmError>> {
        Box::pin(async move {
            let base = self.config.base_url.trim_end_matches('/');
            let url = match self.config.transport {
                TransportType::OllamaNative => {
                    let root = base.strip_suffix("/v1").unwrap_or(base);
                    format!("{}/api/tags", root)
                }
                TransportType::ChatCompletions | TransportType::Responses => {
                    if base.ends_with("/v1") {
                        format!("{}/models", base)
                    } else {
                        format!("{}/v1/models", base)
                    }
                }
            };

            let mut builder = self.client.get(&url).timeout(Duration::from_secs(3));
            builder = inject_auth_headers(builder, &self.config.auth);

            let res = builder
                .send()
                .await
                .map_err(|e| LlmError::Transport(e.to_string()))?;

            if res.status().is_success() {
                Ok(())
            } else {
                Err(LlmError::Provider {
                    status: res.status().as_u16(),
                    message: format!("Health check failed with HTTP {}", res.status()),
                })
            }
        })
    }

    fn list_models<'a>(&'a self) -> BoxFuture<'a, Result<Vec<LlmModelInfo>, LlmError>> {
        Box::pin(async move {
            if self.config.transport == TransportType::OllamaNative {
                let base = self.config.base_url.trim_end_matches('/');
                let root = base.strip_suffix("/v1").unwrap_or(base);
                let url = format!("{}/api/tags", root);
                let mut builder = self.client.get(&url).timeout(Duration::from_secs(4));
                builder = inject_auth_headers(builder, &self.config.auth);

                if let Ok(resp) = builder.send().await {
                    if resp.status().is_success() {
                        if let Ok(tags) = resp.json::<OllamaTagsResponse>().await {
                            return Ok(tags
                                .models
                                .into_iter()
                                .map(|m| {
                                    let clean_name = m.name.replace([':', '_', '-'], " ");
                                    LlmModelInfo {
                                        id: m.name,
                                        name: clean_name,
                                        size_bytes: m.size,
                                        quantization: None,
                                        family: None,
                                        provider_kind: "open_ai_compat".to_string(),
                                        capabilities: None,
                                    }
                                })
                                .collect());
                        }
                    }
                }
            }

            let url = if self.config.base_url.ends_with("/v1") {
                format!("{}/models", self.config.base_url.trim_end_matches('/'))
            } else {
                format!("{}/v1/models", self.config.base_url.trim_end_matches('/'))
            };

            let mut builder = self.client.get(&url).timeout(Duration::from_secs(4));
            builder = inject_auth_headers(builder, &self.config.auth);

            let resp = builder
                .send()
                .await
                .map_err(|e| LlmError::Transport(e.to_string()))?;

            if !resp.status().is_success() {
                return Err(LlmError::Provider {
                    status: resp.status().as_u16(),
                    message: "Failed to list models from endpoint".to_string(),
                });
            }

            let list = resp
                .json::<ModelListResponse>()
                .await
                .map_err(|e| LlmError::Parse(e.to_string()))?;

            Ok(list
                .data
                .into_iter()
                .map(|m| {
                    let clean_name = m.id.replace([':', '_', '-'], " ");
                    LlmModelInfo {
                        id: m.id,
                        name: clean_name,
                        size_bytes: None,
                        quantization: None,
                        family: None,
                        provider_kind: "open_ai_compat".to_string(),
                        capabilities: None,
                    }
                })
                .collect())
        })
    }

    fn capabilities(&self) -> &ProviderCapabilities {
        &self.capabilities
    }

    fn kind(&self) -> ProviderKind {
        ProviderKind::OpenAiCompat
    }
}
