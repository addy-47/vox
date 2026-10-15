use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::{
    config::{ActiveSelection, LastSelectedModels},
    core::defaults::{
        DEFAULT_LLM_CLOUD_BASE_URL, DEFAULT_LLM_CLOUD_MODEL, DEFAULT_LLM_CLOUD_PROVIDER_NAME,
        DEFAULT_LLM_MODEL, DEFAULT_LLM_SERVER_BASE_URL, DEFAULT_LLM_SERVER_MODEL,
        DEFAULT_LLM_SERVER_PROVIDER_NAME, MIN_LLM_CONTEXT_WINDOW,
    },
};

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum LlmActiveProvider {
    #[default]
    Embedded,
    Server,
    Cloud,
}

impl LlmActiveProvider {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Embedded => "embedded",
            Self::Server => "server",
            Self::Cloud => "cloud",
        }
    }
}

/// A user-configured custom server instance in providers.jsonc.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct LlmCustomServer {
    pub id: String,
    pub name: String,
    pub url: String,
    pub model: String,
    pub protocol: String,
    #[serde(default)]
    pub api_key: Option<String>,
}

/// Provider credentials, endpoints, and custom servers stored in providers.jsonc.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(default)]
pub struct LlmProvidersConfig {
    pub servers: Vec<LlmCustomServer>,
    pub cloud_keys: HashMap<String, String>,
}

impl Default for LlmProvidersConfig {
    fn default() -> Self {
        Self {
            servers: vec![LlmCustomServer {
                id: "ollama".to_string(),
                name: "Local Ollama".to_string(),
                url: DEFAULT_LLM_SERVER_BASE_URL.to_string(),
                model: DEFAULT_LLM_SERVER_MODEL.to_string(),
                protocol: "ollama".to_string(),
                api_key: None,
            }],
            cloud_keys: HashMap::new(),
        }
    }
}

/// Concrete runtime target used to instantiate an LLM provider transport.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum LlmRuntimeTarget {
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

/// Clean LLM settings stored in settings.jsonc.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct LlmSettings {
    pub active: ActiveSelection,
    pub last_selected: LastSelectedModels,
}

impl Default for LlmSettings {
    fn default() -> Self {
        Self {
            active: ActiveSelection {
                category: "embedded".to_string(),
                provider: None,
                model: DEFAULT_LLM_MODEL.to_string(),
            },
            last_selected: LastSelectedModels {
                embedded: Some(DEFAULT_LLM_MODEL.to_string()),
                server: Some(DEFAULT_LLM_SERVER_MODEL.to_string()),
                cloud: Some(DEFAULT_LLM_CLOUD_MODEL.to_string()),
            },
        }
    }
}

impl LlmSettings {
    pub fn active_model(&self) -> &str {
        &self.active.model
    }

    pub fn category(&self) -> &str {
        &self.active.category
    }

    pub fn effective_ctx_size(ctx: u32) -> u32 {
        ctx.max(MIN_LLM_CONTEXT_WINDOW)
    }

    pub fn to_provider_config(&self, providers: Option<&LlmProvidersConfig>) -> LlmRuntimeTarget {
        let default_prov = LlmProvidersConfig::default();
        let provs = providers.unwrap_or(&default_prov);

        match self.active.category.as_str() {
            "server" => {
                let target_id = self.active.provider.as_deref().unwrap_or("ollama");
                let server = provs
                    .servers
                    .iter()
                    .find(|s| s.id == target_id)
                    .or_else(|| provs.servers.first());

                if let Some(srv) = server {
                    let model = if !self.active.model.is_empty() {
                        self.active.model.clone()
                    } else {
                        srv.model.clone()
                    };
                    LlmRuntimeTarget::Server {
                        base_url: srv.url.clone(),
                        model,
                        provider_name: Some(srv.name.clone()),
                        protocol: Some(srv.protocol.clone()),
                        api_key: srv.api_key.clone(),
                    }
                } else {
                    LlmRuntimeTarget::Server {
                        base_url: DEFAULT_LLM_SERVER_BASE_URL.to_string(),
                        model: DEFAULT_LLM_SERVER_MODEL.to_string(),
                        provider_name: Some(DEFAULT_LLM_SERVER_PROVIDER_NAME.to_string()),
                        protocol: Some("ollama".to_string()),
                        api_key: None,
                    }
                }
            }
            "cloud" => {
                let provider_id = self
                    .active
                    .provider
                    .as_deref()
                    .unwrap_or(DEFAULT_LLM_CLOUD_PROVIDER_NAME);
                let api_key = provs.cloud_keys.get(provider_id).cloned();
                let base_url = match provider_id {
                    "openrouter" => "https://openrouter.ai/api/v1",
                    "groq" => "https://api.groq.com/openai/v1",
                    _ => DEFAULT_LLM_CLOUD_BASE_URL,
                };
                LlmRuntimeTarget::Cloud {
                    base_url: base_url.to_string(),
                    model: self.active.model.clone(),
                    provider_name: Some(provider_id.to_string()),
                    protocol: Some("openai".to_string()),
                    api_key,
                }
            }
            _ => LlmRuntimeTarget::Embedded,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_llm_settings_defaults() {
        let settings = LlmSettings::default();
        assert_eq!(settings.active.category, "embedded");
        assert_eq!(settings.active_model(), DEFAULT_LLM_MODEL);
        assert_eq!(
            settings.last_selected.embedded.as_deref(),
            Some(DEFAULT_LLM_MODEL)
        );

        let prov_config = settings.to_provider_config(None);
        assert_eq!(prov_config, LlmRuntimeTarget::Embedded);
    }

    #[test]
    fn test_llm_settings_server_and_cloud_active() {
        let mut settings = LlmSettings {
            active: ActiveSelection {
                category: "server".to_string(),
                provider: Some("ollama".to_string()),
                model: "gemma4:12b".to_string(),
            },
            ..Default::default()
        };

        match settings.to_provider_config(None) {
            LlmRuntimeTarget::Server {
                base_url,
                model,
                provider_name,
                ..
            } => {
                assert_eq!(base_url, DEFAULT_LLM_SERVER_BASE_URL);
                assert_eq!(model, "gemma4:12b");
                assert_eq!(provider_name.as_deref(), Some("Local Ollama"));
            }
            _ => panic!("Expected LlmRuntimeTarget::Server"),
        }

        settings.active = ActiveSelection {
            category: "cloud".to_string(),
            provider: Some("nvidia".to_string()),
            model: "meta/llama-3.1-8b-instruct".to_string(),
        };
        let mut provs = LlmProvidersConfig::default();
        provs
            .cloud_keys
            .insert("nvidia".to_string(), "nv-secret-123".to_string());

        match settings.to_provider_config(Some(&provs)) {
            LlmRuntimeTarget::Cloud {
                base_url,
                model,
                provider_name,
                api_key,
                ..
            } => {
                assert_eq!(base_url, DEFAULT_LLM_CLOUD_BASE_URL);
                assert_eq!(model, "meta/llama-3.1-8b-instruct");
                assert_eq!(provider_name.as_deref(), Some("nvidia"));
                assert_eq!(api_key.as_deref(), Some("nv-secret-123"));
            }
            _ => panic!("Expected LlmRuntimeTarget::Cloud"),
        }
    }
}
