use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::core::defaults::{
    DEFAULT_LLM_CLOUD_BASE_URL, DEFAULT_LLM_CLOUD_MODEL, DEFAULT_LLM_CLOUD_PROVIDER_NAME,
    DEFAULT_LLM_COMPACTION_TEMPERATURE, DEFAULT_LLM_CONTEXT_WINDOW, DEFAULT_LLM_MAX_OUTPUT_TOKENS,
    DEFAULT_LLM_MODEL, DEFAULT_LLM_SERVER_BASE_URL, DEFAULT_LLM_SERVER_MODEL,
    DEFAULT_LLM_SERVER_PROVIDER_NAME, DEFAULT_LLM_TEMPERATURE, DEFAULT_LLM_THREADS,
    MIN_LLM_CONTEXT_WINDOW,
};

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum LlmActiveProvider {
    #[default]
    Embedded,
    Server,
    Cloud,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(default)]
pub struct LlmEmbeddedConfig {
    pub model: String,
    pub n_gpu_layers: u32,
}

impl Default for LlmEmbeddedConfig {
    fn default() -> Self {
        Self {
            model: DEFAULT_LLM_MODEL.to_string(),
            n_gpu_layers: 0,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(default)]
pub struct LlmRemoteConfig {
    pub provider_name: Option<String>,
    pub base_url: String,
    pub model: String,
    pub protocol: Option<String>,
    pub api_key: Option<String>,
}

impl LlmRemoteConfig {
    pub fn server_default() -> Self {
        Self {
            provider_name: Some(DEFAULT_LLM_SERVER_PROVIDER_NAME.to_string()),
            base_url: DEFAULT_LLM_SERVER_BASE_URL.to_string(),
            model: DEFAULT_LLM_SERVER_MODEL.to_string(),
            protocol: None,
            api_key: None,
        }
    }

    pub fn cloud_default() -> Self {
        Self {
            provider_name: Some(DEFAULT_LLM_CLOUD_PROVIDER_NAME.to_string()),
            base_url: DEFAULT_LLM_CLOUD_BASE_URL.to_string(),
            model: DEFAULT_LLM_CLOUD_MODEL.to_string(),
            protocol: None,
            api_key: None,
        }
    }
}

impl Default for LlmRemoteConfig {
    fn default() -> Self {
        Self::server_default()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum LlmProviderConfig {
    #[default]
    Embedded,
    #[serde(rename_all = "snake_case")]
    Server {
        base_url: String,
        model: String,
        #[serde(default)]
        provider_name: Option<String>,
        #[serde(default)]
        protocol: Option<String>,
        #[serde(default)]
        api_key: Option<String>,
    },
    #[serde(rename_all = "snake_case")]
    Cloud {
        base_url: String,
        model: String,
        #[serde(default)]
        provider_name: Option<String>,
        #[serde(default)]
        protocol: Option<String>,
        #[serde(default)]
        api_key: Option<String>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct LlmSettings {
    pub active: LlmActiveProvider,
    pub temperature: f32,
    pub compaction_temperature: f32,
    pub max_output_tokens: u32,
    pub context_window: u32,
    pub threads: u32,
    pub reasoning_enabled: bool,
    pub embedded: LlmEmbeddedConfig,
    pub server: LlmRemoteConfig,
    pub cloud: LlmRemoteConfig,
    #[serde(default)]
    pub cloud_keys: HashMap<String, String>,
}

impl Default for LlmSettings {
    fn default() -> Self {
        Self {
            active: LlmActiveProvider::Embedded,
            temperature: DEFAULT_LLM_TEMPERATURE,
            compaction_temperature: DEFAULT_LLM_COMPACTION_TEMPERATURE,
            max_output_tokens: DEFAULT_LLM_MAX_OUTPUT_TOKENS,
            context_window: DEFAULT_LLM_CONTEXT_WINDOW,
            threads: DEFAULT_LLM_THREADS,
            reasoning_enabled: false,
            embedded: LlmEmbeddedConfig::default(),
            server: LlmRemoteConfig::server_default(),
            cloud: LlmRemoteConfig::cloud_default(),
            cloud_keys: HashMap::new(),
        }
    }
}

impl LlmSettings {
    pub fn active_model(&self) -> &str {
        match self.active {
            LlmActiveProvider::Embedded => &self.embedded.model,
            LlmActiveProvider::Server => &self.server.model,
            LlmActiveProvider::Cloud => &self.cloud.model,
        }
    }

    pub fn effective_ctx_size(&self) -> u32 {
        self.context_window.max(MIN_LLM_CONTEXT_WINDOW)
    }

    pub fn to_provider_config(&self) -> LlmProviderConfig {
        match self.active {
            LlmActiveProvider::Embedded => LlmProviderConfig::Embedded,
            LlmActiveProvider::Server => LlmProviderConfig::Server {
                base_url: self.server.base_url.clone(),
                model: self.server.model.clone(),
                provider_name: self.server.provider_name.clone(),
                protocol: self.server.protocol.clone(),
                api_key: self.server.api_key.clone(),
            },
            LlmActiveProvider::Cloud => {
                let api_key = self.cloud.api_key.clone().or_else(|| {
                    let provider = self.cloud.provider_name.as_deref().unwrap_or_default();
                    self.cloud_keys.get(provider).cloned()
                });
                LlmProviderConfig::Cloud {
                    base_url: self.cloud.base_url.clone(),
                    model: self.cloud.model.clone(),
                    provider_name: self.cloud.provider_name.clone(),
                    protocol: self.cloud.protocol.clone(),
                    api_key,
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_llm_settings_defaults() {
        let settings = LlmSettings::default();
        assert_eq!(settings.active, LlmActiveProvider::Embedded);
        assert_eq!(settings.temperature, DEFAULT_LLM_TEMPERATURE);
        assert_eq!(
            settings.compaction_temperature,
            DEFAULT_LLM_COMPACTION_TEMPERATURE
        );
        assert_eq!(settings.max_output_tokens, DEFAULT_LLM_MAX_OUTPUT_TOKENS);
        assert_eq!(settings.context_window, DEFAULT_LLM_CONTEXT_WINDOW);
        assert_eq!(settings.threads, DEFAULT_LLM_THREADS);
        assert!(!settings.reasoning_enabled);
        assert_eq!(settings.embedded.model, DEFAULT_LLM_MODEL);
        assert_eq!(settings.embedded.n_gpu_layers, 0);
        assert_eq!(settings.server.base_url, DEFAULT_LLM_SERVER_BASE_URL);
        assert_eq!(settings.server.model, DEFAULT_LLM_SERVER_MODEL);
        assert_eq!(
            settings.server.provider_name.as_deref(),
            Some(DEFAULT_LLM_SERVER_PROVIDER_NAME)
        );
        assert_eq!(settings.cloud.base_url, DEFAULT_LLM_CLOUD_BASE_URL);
        assert_eq!(settings.cloud.model, DEFAULT_LLM_CLOUD_MODEL);
        assert_eq!(
            settings.cloud.provider_name.as_deref(),
            Some(DEFAULT_LLM_CLOUD_PROVIDER_NAME)
        );
        assert!(settings.cloud_keys.is_empty());
        assert_eq!(settings.active_model(), DEFAULT_LLM_MODEL);
        assert_eq!(settings.effective_ctx_size(), DEFAULT_LLM_CONTEXT_WINDOW);

        let provider_config = settings.to_provider_config();
        assert_eq!(provider_config, LlmProviderConfig::Embedded);
    }

    #[test]
    fn test_llm_settings_server_and_cloud_active() {
        let mut settings = LlmSettings {
            active: LlmActiveProvider::Server,
            ..Default::default()
        };
        assert_eq!(settings.active_model(), DEFAULT_LLM_SERVER_MODEL);

        match settings.to_provider_config() {
            LlmProviderConfig::Server {
                base_url,
                model,
                provider_name,
                ..
            } => {
                assert_eq!(base_url, DEFAULT_LLM_SERVER_BASE_URL);
                assert_eq!(model, DEFAULT_LLM_SERVER_MODEL);
                assert_eq!(
                    provider_name.as_deref(),
                    Some(DEFAULT_LLM_SERVER_PROVIDER_NAME)
                );
            }
            _ => panic!("Expected LlmProviderConfig::Server"),
        }

        settings.active = LlmActiveProvider::Cloud;
        settings
            .cloud_keys
            .insert("nvidia".to_string(), "nv-secret-123".to_string());
        assert_eq!(settings.active_model(), DEFAULT_LLM_CLOUD_MODEL);

        match settings.to_provider_config() {
            LlmProviderConfig::Cloud {
                base_url,
                model,
                provider_name,
                api_key,
                ..
            } => {
                assert_eq!(base_url, DEFAULT_LLM_CLOUD_BASE_URL);
                assert_eq!(model, DEFAULT_LLM_CLOUD_MODEL);
                assert_eq!(
                    provider_name.as_deref(),
                    Some(DEFAULT_LLM_CLOUD_PROVIDER_NAME)
                );
                assert_eq!(api_key.as_deref(), Some("nv-secret-123"));
            }
            _ => panic!("Expected LlmProviderConfig::Cloud"),
        }
    }

    #[test]
    fn test_effective_ctx_size_clamping() {
        let mut settings = LlmSettings {
            context_window: 1024,
            ..Default::default()
        };
        assert_eq!(settings.effective_ctx_size(), MIN_LLM_CONTEXT_WINDOW);

        settings.context_window = 32768;
        assert_eq!(settings.effective_ctx_size(), 32768);
    }

    #[test]
    fn test_serde_roundtrip() {
        let original = LlmSettings::default();
        let json = serde_json::to_string(&original).unwrap();
        let deserialized: LlmSettings = serde_json::from_str(&json).unwrap();
        assert_eq!(original, deserialized);
    }
}
