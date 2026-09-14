use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::{
    core::settings::ModelCapabilities,
    services::llm::transport::{CapabilitySource, TokenLimitField, TransportType},
};

/// Provenance tier identifying the source and confidence of a discovered capability.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityProvenance {
    /// Sourced directly from synchronized external catalog (models.dev / baseline).
    CatalogBaseline,
    /// Inferred from a matching model family when exact model ID is not listed.
    FamilyBaseline,
    /// Empirically verified or reported by the active runtime or provider endpoint.
    ProbedServer,
    /// Explicitly overridden by the user in settings.
    UserConfigured,
    /// Explicitly unobservable and unverified.
    #[default]
    Unknown,
}

impl CapabilityProvenance {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::CatalogBaseline => "catalog_baseline",
            Self::FamilyBaseline => "family_baseline",
            Self::ProbedServer => "probed_server",
            Self::UserConfigured => "user_configured",
            Self::Unknown => "unknown",
        }
    }
}

/// Normalized model specification from catalog or capability discovery.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ModelSpec {
    pub model_id: String,
    pub name: String,
    pub family: Option<String>,
    pub context_window: Option<u32>,
    pub max_output_tokens: Option<u32>,
    pub supports_tools: bool,
    pub supports_structured: bool,
    pub provenance: CapabilityProvenance,
}

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

/// Result of an empirical capability probe dispatched over IPC.
#[derive(Debug, Serialize, Deserialize)]
pub struct ModelProbeResult {
    pub capabilities: ModelCapabilities,
    pub validated_cap: Option<u32>,
    pub cached_map: HashMap<String, ModelCapabilities>,
}
