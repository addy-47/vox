use serde::{Deserialize, Serialize};

use crate::services::llm::catalog::{lookup_preset, CatalogAuthScheme, ProviderPresetMeta};

/// Supported remote transport wire formats.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum TransportType {
    #[default]
    ChatCompletions,
    OllamaNative,
    Responses,
}

/// Authentication scheme for HTTP requests.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(tag = "type", content = "key", rename_all = "snake_case")]
pub enum AuthScheme {
    Bearer(Option<String>),
    AnthropicNative(String),
    #[default]
    None,
}

/// Authoritative connection configuration for a remote LLM endpoint.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ConnectionConfig {
    pub transport: TransportType,
    pub base_url: String,
    pub model: String,
    pub auth: AuthScheme,
    pub provider_preset: Option<String>,
    pub policy: ProviderPresetMeta,
}

impl ConnectionConfig {
    /// Constructs a connection config from explicit parameters, applying preset defaults if provided.
    pub fn new(
        base_url: &str,
        model: &str,
        api_key: Option<&str>,
        provider_preset: Option<&str>,
    ) -> Self {
        let preset = provider_preset
            .and_then(lookup_preset)
            .unwrap_or_default();

        let resolved_base_url = if base_url.trim().is_empty() {
            preset.base_url.to_string()
        } else {
            base_url.trim_end_matches('/').to_string()
        };

        let auth = if let Some(key) = api_key.filter(|k| !k.trim().is_empty()) {
            match preset.auth_scheme {
                CatalogAuthScheme::AnthropicNative => {
                    AuthScheme::AnthropicNative(key.to_string())
                }
                CatalogAuthScheme::Bearer => AuthScheme::Bearer(Some(key.to_string())),
                CatalogAuthScheme::None => AuthScheme::None,
            }
        } else if preset.auth_scheme == CatalogAuthScheme::Bearer {
            AuthScheme::Bearer(None)
        } else {
            AuthScheme::None
        };

        Self {
            transport: preset.transport,
            base_url: resolved_base_url,
            model: model.to_string(),
            auth,
            provider_preset: provider_preset.map(|s| s.to_string()),
            policy: preset,
        }
    }
}
