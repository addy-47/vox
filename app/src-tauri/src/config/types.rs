use serde::{Deserialize, Serialize};

/// Identifies the active model selection across category, provider, and model identifier.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ActiveSelection {
    /// Primary runtime tier: "embedded" | "server" | "cloud"
    pub category: String,
    /// Provider name, server ID, or None for standard embedded models
    #[serde(default)]
    pub provider: Option<String>,
    /// Concrete model identifier
    pub model: String,
}

impl ActiveSelection {
    pub fn new(
        category: impl Into<String>,
        provider: Option<String>,
        model: impl Into<String>,
    ) -> Self {
        Self {
            category: category.into(),
            provider,
            model: model.into(),
        }
    }

    pub fn embedded(model: impl Into<String>) -> Self {
        Self {
            category: "embedded".to_string(),
            provider: None,
            model: model.into(),
        }
    }

    pub fn server(provider: impl Into<String>, model: impl Into<String>) -> Self {
        Self {
            category: "server".to_string(),
            provider: Some(provider.into()),
            model: model.into(),
        }
    }

    pub fn cloud(provider: impl Into<String>, model: impl Into<String>) -> Self {
        Self {
            category: "cloud".to_string(),
            provider: Some(provider.into()),
            model: model.into(),
        }
    }
}

/// Tracks the last selected model per category to enable instant roundtrip switching with zero state loss.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(default)]
pub struct LastSelectedModels {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub embedded: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub server: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cloud: Option<String>,
}
