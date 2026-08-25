use crate::services::llm::providers::ProviderKind;
use crate::services::llm::types::Support;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

/// Provenance of a capability status observation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CapabilitySource {
    StaticProviderKnowledge,
    OpenApiSchema,
    ModelMetadata,
    ActiveProbe,
    UserOverride,
}

/// An individual capability status observation with full diagnostic metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapabilityObservation {
    pub support: Support,
    pub source: CapabilitySource,
    pub status_code: Option<u16>,
    pub detail: Option<String>,
}

impl CapabilityObservation {
    /// Creates a static observation marked as supported with documented details.
    pub fn static_supported(detail: &str) -> Self {
        Self {
            support: Support::Supported,
            source: CapabilitySource::StaticProviderKnowledge,
            status_code: Some(200),
            detail: Some(detail.to_string()),
        }
    }

    /// Creates a static observation marked as unsupported.
    pub fn static_unsupported(detail: &str) -> Self {
        Self {
            support: Support::Unsupported,
            source: CapabilitySource::StaticProviderKnowledge,
            status_code: None,
            detail: Some(detail.to_string()),
        }
    }

    /// Creates an unknown capability observation placeholder.
    pub fn unknown() -> Self {
        Self {
            support: Support::Unknown,
            source: CapabilitySource::StaticProviderKnowledge,
            status_code: None,
            detail: None,
        }
    }
}

/// Comprehensive capability matrix observation for a specific model/provider.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelCapabilities {
    pub temperature: CapabilityObservation,
    pub top_p: CapabilityObservation,
    pub top_k: CapabilityObservation,
    pub max_output_tokens: CapabilityObservation,
    pub json_object: CapabilityObservation,
    pub json_schema: CapabilityObservation,
    pub seed: CapabilityObservation,
}

impl ModelCapabilities {
    /// Constructs static baseline capabilities according to known provider specifications.
    pub fn default_for_kind(kind: ProviderKind) -> Self {
        match kind {
            ProviderKind::Embedded => Self {
                temperature: CapabilityObservation::static_supported("Embedded llama.cpp sampler"),
                top_p: CapabilityObservation::static_supported("Embedded llama.cpp sampler"),
                top_k: CapabilityObservation::static_supported("Embedded llama.cpp sampler"),
                max_output_tokens: CapabilityObservation::static_supported("Bound by ctx_size"),
                json_object: CapabilityObservation::static_supported("Constrained GGUF grammar"),
                json_schema: CapabilityObservation::static_supported("Constrained GGUF grammar"),
                seed: CapabilityObservation::static_supported("Supported"),
            },
            ProviderKind::OpenAiCompat => Self {
                temperature: CapabilityObservation::static_supported("OpenAI chat completions"),
                top_p: CapabilityObservation::static_supported("OpenAI chat completions"),
                top_k: CapabilityObservation::unknown(),
                max_output_tokens: CapabilityObservation::static_supported("max_completion_tokens"),
                json_object: CapabilityObservation::static_supported(
                    "response_format: json_object",
                ),
                json_schema: CapabilityObservation::static_supported(
                    "response_format: json_schema",
                ),
                seed: CapabilityObservation::static_supported("seed field"),
            },
        }
    }
}

/// In-memory thread-safe capability registry caching model capability observations.
#[derive(Debug, Default, Clone)]
pub struct CapabilityRegistry {
    cache: Arc<RwLock<HashMap<String, ModelCapabilities>>>,
}

impl CapabilityRegistry {
    /// Creates an empty thread-safe capability registry.
    pub fn new() -> Self {
        Self {
            cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Fetches capability matrix for key, initializing with kind baseline if missing.
    pub fn get_or_insert_default(&self, key: &str, kind: ProviderKind) -> ModelCapabilities {
        let read_guard = self.cache.read();
        if let Some(caps) = read_guard.get(key) {
            return caps.clone();
        }
        drop(read_guard);

        let default_caps = ModelCapabilities::default_for_kind(kind);
        let mut write_guard = self.cache.write();
        write_guard
            .entry(key.to_string())
            .or_insert_with(|| default_caps.clone());
        default_caps
    }

    /// Records an active probe observation for a given key and provider kind.
    pub fn update_observation(
        &self,
        key: &str,
        kind: ProviderKind,
        update_fn: impl FnOnce(&mut ModelCapabilities),
    ) {
        let mut write_guard = self.cache.write();
        let entry = write_guard
            .entry(key.to_string())
            .or_insert_with(|| ModelCapabilities::default_for_kind(kind));
        update_fn(entry);
    }
}
