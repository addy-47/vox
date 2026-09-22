use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::{core::settings::ModelCapabilities, services::llm::transport::TransportType};

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

/// Off-switch value emitted when reasoning is Disabled (boolean or level string).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum WireValue {
    Bool(bool),
    Str(&'static str),
}

/// Reasoning off-switch: the wire path plus the value that disables thinking.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReasoningOff {
    pub path: &'static str,
    pub value: WireValue,
}

/// Envelope selecting the `OutputConstraint` serialization per transport.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResponseEnvelope {
    Chat,
    Native,
    Text,
}

/// Stream shape the transport parser must handle for tool calls.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolStreamShape {
    OpenaiDelta,
    OllamaNdjson,
    OpenaiDeltaWithXmlFallback,
}

/// Metadata and wire mapping for a curated provider preset.
/// Plain data in plain words: every canonical intent maps to an exact wire path.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderPresetMeta {
    pub id: &'static str,
    pub name: &'static str,
    pub base_url: &'static str,
    pub auth_scheme: CatalogAuthScheme,
    pub transport: TransportType,
    pub token_limit: &'static str,
    pub reasoning_off: Option<ReasoningOff>,
    pub tool_choice: Option<&'static str>,
    pub stream_usage: bool,
    pub response_envelope: ResponseEnvelope,
    pub tool_stream: ToolStreamShape,
    pub top_k_field: Option<&'static str>,
    pub catalog: Option<&'static str>,
    pub context_window: Option<u32>,
    pub source: &'static str,
    pub checked: &'static str,
}

impl Default for ProviderPresetMeta {
    /// Documents current effective behavior for unknown presets (see spec §2.4).
    fn default() -> Self {
        Self {
            id: "unknown",
            name: "Unknown",
            base_url: "http://127.0.0.1:11434",
            auth_scheme: CatalogAuthScheme::None,
            transport: TransportType::ChatCompletions,
            token_limit: "max_tokens",
            reasoning_off: None,
            tool_choice: Some("auto"),
            stream_usage: true,
            response_envelope: ResponseEnvelope::Chat,
            tool_stream: ToolStreamShape::OpenaiDelta,
            top_k_field: None,
            catalog: None,
            context_window: None,
            source: "fallback default; no upstream citation",
            checked: "2026-09-22",
        }
    }
}

/// Result of an empirical capability probe dispatched over IPC.
#[derive(Debug, Serialize, Deserialize)]
pub struct ModelProbeResult {
    pub capabilities: ModelCapabilities,
    pub validated_cap: Option<u32>,
    pub cached_map: HashMap<String, ModelCapabilities>,
}
