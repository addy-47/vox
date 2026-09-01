use super::transport::{CapabilitySource, TokenLimitField, TransportType};
use serde::{Deserialize, Serialize};

/// Catalog authentication scheme declaration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CatalogAuthScheme {
    Bearer,
    AnthropicNative,
    None,
}

/// Metadata and default parameters for a curated provider preset.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderPresetMeta {
    pub id: &'static str,
    pub name: &'static str,
    pub default_base_url: &'static str,
    pub auth_scheme: CatalogAuthScheme,
    pub default_transport: TransportType,
    pub supports_responses: bool,
    pub default_token_limit_field: TokenLimitField,
    pub capability_source: CapabilitySource,
    pub published_context_window: Option<u32>,
    pub display_label: &'static str,
}

/// Static curated catalog of supported LLM provider presets.
pub static PROVIDER_CATALOG: &[ProviderPresetMeta] = &[
    ProviderPresetMeta {
        id: "openai",
        name: "OpenAI",
        default_base_url: "https://api.openai.com/v1",
        auth_scheme: CatalogAuthScheme::Bearer,
        default_transport: TransportType::ChatCompletions,
        supports_responses: true,
        default_token_limit_field: TokenLimitField::MaxCompletionTokens,
        capability_source: CapabilitySource::ProbedGeneric,
        published_context_window: Some(128_000),
        display_label: "Cloud / Server-Managed (OpenAI API)",
    },
    ProviderPresetMeta {
        id: "openrouter",
        name: "OpenRouter",
        default_base_url: "https://openrouter.ai/api/v1",
        auth_scheme: CatalogAuthScheme::Bearer,
        default_transport: TransportType::ChatCompletions,
        supports_responses: false,
        default_token_limit_field: TokenLimitField::MaxTokens,
        capability_source: CapabilitySource::ProbedGeneric,
        published_context_window: None,
        display_label: "Cloud / Server-Managed (OpenRouter Gateway)",
    },
    ProviderPresetMeta {
        id: "groq",
        name: "Groq",
        default_base_url: "https://api.groq.com/openai/v1",
        auth_scheme: CatalogAuthScheme::Bearer,
        default_transport: TransportType::ChatCompletions,
        supports_responses: false,
        default_token_limit_field: TokenLimitField::MaxTokens,
        capability_source: CapabilitySource::ProbedGeneric,
        published_context_window: Some(131_072),
        display_label: "Cloud / Server-Managed (Groq LPU Accelerated)",
    },
    ProviderPresetMeta {
        id: "together",
        name: "Together AI",
        default_base_url: "https://api.together.xyz/v1",
        auth_scheme: CatalogAuthScheme::Bearer,
        default_transport: TransportType::ChatCompletions,
        supports_responses: false,
        default_token_limit_field: TokenLimitField::MaxTokens,
        capability_source: CapabilitySource::ProbedGeneric,
        published_context_window: None,
        display_label: "Cloud / Server-Managed (Together AI)",
    },
    ProviderPresetMeta {
        id: "deepseek",
        name: "DeepSeek",
        default_base_url: "https://api.deepseek.com/v1",
        auth_scheme: CatalogAuthScheme::Bearer,
        default_transport: TransportType::ChatCompletions,
        supports_responses: false,
        default_token_limit_field: TokenLimitField::MaxTokens,
        capability_source: CapabilitySource::ProbedGeneric,
        published_context_window: Some(64_000),
        display_label: "Cloud / Server-Managed (DeepSeek API)",
    },
    ProviderPresetMeta {
        id: "mistral",
        name: "Mistral AI",
        default_base_url: "https://api.mistral.ai/v1",
        auth_scheme: CatalogAuthScheme::Bearer,
        default_transport: TransportType::ChatCompletions,
        supports_responses: false,
        default_token_limit_field: TokenLimitField::MaxTokens,
        capability_source: CapabilitySource::ProbedGeneric,
        published_context_window: Some(128_000),
        display_label: "Cloud / Server-Managed (Mistral AI)",
    },
    ProviderPresetMeta {
        id: "nvidia_nim",
        name: "NVIDIA NIM",
        default_base_url: "https://integrate.api.nvidia.com/v1",
        auth_scheme: CatalogAuthScheme::Bearer,
        default_transport: TransportType::ChatCompletions,
        supports_responses: false,
        default_token_limit_field: TokenLimitField::MaxTokens,
        capability_source: CapabilitySource::ProbedGeneric,
        published_context_window: None,
        display_label: "Cloud / Server-Managed (NVIDIA NIM Accelerated)",
    },
    ProviderPresetMeta {
        id: "gemini",
        name: "Google Gemini",
        default_base_url: "https://generativelanguage.googleapis.com/v1beta/openai",
        auth_scheme: CatalogAuthScheme::Bearer,
        default_transport: TransportType::ChatCompletions,
        supports_responses: false,
        default_token_limit_field: TokenLimitField::MaxTokens,
        capability_source: CapabilitySource::ProbedGeneric,
        published_context_window: Some(1_048_576),
        display_label: "Cloud / Server-Managed (Google Gemini)",
    },
    ProviderPresetMeta {
        id: "anthropic",
        name: "Anthropic",
        default_base_url: "https://api.anthropic.com/v1",
        auth_scheme: CatalogAuthScheme::AnthropicNative,
        default_transport: TransportType::ChatCompletions,
        supports_responses: false,
        default_token_limit_field: TokenLimitField::MaxTokens,
        capability_source: CapabilitySource::ProbedGeneric,
        published_context_window: Some(200_000),
        display_label: "Cloud / Server-Managed (Anthropic Claude)",
    },
    ProviderPresetMeta {
        id: "ollama",
        name: "Ollama (Local)",
        default_base_url: "http://127.0.0.1:11434",
        auth_scheme: CatalogAuthScheme::None,
        default_transport: TransportType::ChatCompletions,
        supports_responses: false,
        default_token_limit_field: TokenLimitField::NumPredict,
        capability_source: CapabilitySource::OllamaNative,
        published_context_window: None,
        display_label: "Local Daemon (Ollama)",
    },
    ProviderPresetMeta {
        id: "lm_studio",
        name: "LM Studio (Local)",
        default_base_url: "http://127.0.0.1:1234/v1",
        auth_scheme: CatalogAuthScheme::None,
        default_transport: TransportType::ChatCompletions,
        supports_responses: false,
        default_token_limit_field: TokenLimitField::MaxTokens,
        capability_source: CapabilitySource::ProbedGeneric,
        published_context_window: None,
        display_label: "Local Daemon (LM Studio)",
    },
    ProviderPresetMeta {
        id: "vllm",
        name: "vLLM / llama.cpp (Local)",
        default_base_url: "http://127.0.0.1:8000/v1",
        auth_scheme: CatalogAuthScheme::None,
        default_transport: TransportType::ChatCompletions,
        supports_responses: false,
        default_token_limit_field: TokenLimitField::MaxTokens,
        capability_source: CapabilitySource::ProbedGeneric,
        published_context_window: None,
        display_label: "Local Daemon / Proxy",
    },
    ProviderPresetMeta {
        id: "self_hosted",
        name: "Self-Hosted / Custom",
        default_base_url: "http://127.0.0.1:8080/v1",
        auth_scheme: CatalogAuthScheme::None,
        default_transport: TransportType::ChatCompletions,
        supports_responses: false,
        default_token_limit_field: TokenLimitField::MaxTokens,
        capability_source: CapabilitySource::ProbedGeneric,
        published_context_window: None,
        display_label: "Self-Hosted / Custom Endpoint",
    },
];

/// Looks up a provider preset by identifier.
pub fn lookup_preset(id: &str) -> Option<&'static ProviderPresetMeta> {
    let lower = id.to_lowercase();
    PROVIDER_CATALOG
        .iter()
        .find(|p| p.id == lower || p.name.to_lowercase() == lower)
}

/// Returns the complete list of available provider presets.
pub fn list_presets() -> &'static [ProviderPresetMeta] {
    PROVIDER_CATALOG
}
