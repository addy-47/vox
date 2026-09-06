pub mod chat_completions;
pub mod config;
pub mod ollama;
pub mod responses;
pub mod sse;

use std::{
    sync::{mpsc, Arc},
    time::Duration,
};

pub use config::{AuthScheme, CapabilitySource, ConnectionConfig, TokenLimitField, TransportType};
use futures_util::future::BoxFuture;
use parking_lot::RwLock;
use serde::Deserialize;

use crate::{
    core::settings::LlmModelInfo,
    services::llm::{
        GenerationRequest, LlmError, ProviderCapabilities, ProviderKind, Support,
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
    active_token_limit_field: Arc<RwLock<TokenLimitField>>,
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

impl RemoteTransport {
    /// Creates a new `RemoteTransport` from explicit connection configuration.
    pub fn new(config: ConnectionConfig) -> Self {
        let initial_field = config.token_limit_field;
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
            active_token_limit_field: Arc::new(RwLock::new(initial_field)),
            capabilities,
        }
    }

    /// Returns the active connection configuration.
    pub fn config(&self) -> &ConnectionConfig {
        &self.config
    }
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
            cfg.token_limit_field = *self.active_token_limit_field.read();

            let res = if cfg.capability_source == CapabilitySource::OllamaNative
                && cfg.token_limit_field == TokenLimitField::NumPredict
            {
                ollama::stream_ollama(&self.client, &cfg, &request, turn_id, cancel, tx).await
            } else if cfg.transport == TransportType::Responses {
                responses::stream_responses(&self.client, &cfg, &request, turn_id, cancel, tx).await
            } else {
                chat_completions::stream_chat_completions(
                    &self.client,
                    &cfg,
                    &request,
                    turn_id,
                    cancel,
                    tx,
                )
                .await
            };

            // Negotiation: if HTTP 400 unsupported_parameter naming the token field, flip and retry once
            if let Err(LlmError::Provider {
                status: 400,
                ref message,
            }) = res
            {
                let msg_lower = message.to_lowercase();
                if msg_lower.contains("unsupported_parameter")
                    || msg_lower.contains("max_completion_tokens")
                    || msg_lower.contains("max_tokens")
                {
                    let next_field = match cfg.token_limit_field {
                        TokenLimitField::MaxTokens => TokenLimitField::MaxCompletionTokens,
                        TokenLimitField::MaxCompletionTokens => TokenLimitField::MaxTokens,
                        other => other,
                    };
                    if next_field != cfg.token_limit_field {
                        log::info!(
                            "[RemoteTransport] Provider rejected {:?}, negotiating to {:?}",
                            cfg.token_limit_field,
                            next_field
                        );
                        *self.active_token_limit_field.write() = next_field;
                        cfg.token_limit_field = next_field;

                        return if cfg.transport == TransportType::Responses {
                            responses::stream_responses(
                                &self.client,
                                &cfg,
                                &request,
                                turn_id,
                                cancel,
                                tx,
                            )
                            .await
                        } else {
                            chat_completions::stream_chat_completions(
                                &self.client,
                                &cfg,
                                &request,
                                turn_id,
                                cancel,
                                tx,
                            )
                            .await
                        };
                    }
                }
            }

            res
        })
    }

    fn health_check<'a>(&'a self) -> BoxFuture<'a, Result<(), LlmError>> {
        Box::pin(async move {
            let url = if self.config.capability_source == CapabilitySource::OllamaNative {
                format!("{}/api/tags", self.config.base_url.trim_end_matches('/'))
            } else if self.config.base_url.ends_with("/v1") {
                format!("{}/models", self.config.base_url.trim_end_matches('/'))
            } else {
                format!("{}/v1/models", self.config.base_url.trim_end_matches('/'))
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
            if self.config.capability_source == CapabilitySource::OllamaNative {
                let url = format!("{}/api/tags", self.config.base_url.trim_end_matches('/'));
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
