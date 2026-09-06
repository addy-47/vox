use serde::{Deserialize, Serialize};

use crate::services::llm::catalog::{lookup_preset, CatalogAuthScheme};

/// Supported remote transport wire formats.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum TransportType {
    #[default]
    ChatCompletions,
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

/// Declares the output-length field expected by the upstream endpoint.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum TokenLimitField {
    #[default]
    MaxTokens,
    MaxCompletionTokens,
    MaxOutputTokens,
    NumPredict,
}

impl TokenLimitField {
    /// Returns the JSON property key for this token limit field.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::MaxTokens => "max_tokens",
            Self::MaxCompletionTokens => "max_completion_tokens",
            Self::MaxOutputTokens => "max_output_tokens",
            Self::NumPredict => "num_predict",
        }
    }
}

/// Source of truth for model capability discovery.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum CapabilitySource {
    OllamaNative,
    #[default]
    ProbedGeneric,
}

/// Authoritative connection configuration for a remote LLM endpoint.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConnectionConfig {
    pub transport: TransportType,
    pub base_url: String,
    pub model: String,
    pub auth: AuthScheme,
    pub token_limit_field: TokenLimitField,
    pub capability_source: CapabilitySource,
    pub provider_preset: Option<String>,
}

impl ConnectionConfig {
    /// Constructs a connection config from explicit parameters, applying preset defaults if provided.
    pub fn new(
        base_url: &str,
        model: &str,
        api_key: Option<&str>,
        provider_preset: Option<&str>,
    ) -> Self {
        let preset_meta = provider_preset.and_then(lookup_preset);

        let resolved_base_url = if base_url.trim().is_empty() {
            preset_meta
                .map(|p| p.default_base_url.to_string())
                .unwrap_or_else(|| "http://127.0.0.1:11434".to_string())
        } else {
            base_url.trim_end_matches('/').to_string()
        };

        let auth = if let Some(key) = api_key.filter(|k| !k.trim().is_empty()) {
            if let Some(p) = preset_meta {
                match p.auth_scheme {
                    CatalogAuthScheme::AnthropicNative => {
                        AuthScheme::AnthropicNative(key.to_string())
                    }
                    CatalogAuthScheme::Bearer => AuthScheme::Bearer(Some(key.to_string())),
                    CatalogAuthScheme::None => AuthScheme::None,
                }
            } else {
                AuthScheme::Bearer(Some(key.to_string()))
            }
        } else {
            AuthScheme::None
        };

        let transport = preset_meta
            .map(|p| p.default_transport)
            .unwrap_or(TransportType::ChatCompletions);

        let token_limit_field = preset_meta
            .map(|p| p.default_token_limit_field)
            .unwrap_or(TokenLimitField::MaxTokens);

        let capability_source = preset_meta
            .map(|p| p.capability_source)
            .unwrap_or(CapabilitySource::ProbedGeneric);

        Self {
            transport,
            base_url: resolved_base_url,
            model: model.to_string(),
            auth,
            token_limit_field,
            capability_source,
            provider_preset: provider_preset.map(|s| s.to_string()),
        }
    }
}
