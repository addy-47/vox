use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::{
    config::{ActiveSelection, LastSelectedModels},
    core::defaults::{
        DEFAULT_LLM_CLOUD_MODEL, DEFAULT_LLM_CLOUD_PROVIDER_NAME, DEFAULT_LLM_MODEL,
        DEFAULT_LLM_SERVER_BASE_URL, DEFAULT_LLM_SERVER_MODEL, DEFAULT_LLM_SERVER_PROVIDER_NAME,
        MIN_LLM_CONTEXT_WINDOW,
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
#[serde(default)]
pub struct LlmCustomServer {
    pub id: String,
    pub name: String,
    pub url: String,
    pub model: String,
    pub protocol: String,
    #[serde(default)]
    pub api_key: Option<String>,
}

impl Default for LlmCustomServer {
    fn default() -> Self {
        Self {
            id: String::new(),
            name: String::new(),
            url: String::new(),
            model: DEFAULT_LLM_SERVER_MODEL.to_string(),
            protocol: "ollama".to_string(),
            api_key: None,
        }
    }
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
    /// Cloud provider id could not be resolved against the declarative manifest.
    /// RULE-PROV-01: never silently fall back to an unrelated endpoint.
    Unresolved { provider_id: String },
    #[serde(rename_all = "snake_case")]
    Server {
        base_url: String,
        #[serde(default)]
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
        #[serde(default)]
        model: String,
        #[serde(default)]
        provider_name: Option<String>,
        #[serde(default)]
        protocol: Option<String>,
        #[serde(default)]
        api_key: Option<String>,
    },
}

impl LlmRuntimeTarget {
    /// Short kind label for logs without leaking endpoint or credential fields.
    pub fn kind_label(&self) -> &'static str {
        match self {
            Self::Embedded => "embedded",
            Self::Server { .. } => "server",
            Self::Cloud { .. } => "cloud",
            Self::Unresolved { .. } => "unresolved",
        }
    }
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

    pub fn runtime_target(&self, providers: Option<&LlmProvidersConfig>) -> LlmRuntimeTarget {
        self.to_provider_config(providers)
    }

    pub fn to_provider_config(&self, providers: Option<&LlmProvidersConfig>) -> LlmRuntimeTarget {
        let default_prov = LlmProvidersConfig::default();
        let provs = providers.unwrap_or(&default_prov);

        match self.active.category.as_str() {
            "server" => {
                let target_id = self.active.provider.as_deref().unwrap_or("ollama");
                let server = provs.servers.iter().find(|s| s.id == target_id);

                if let Some(srv) = server {
                    let model = if !self.active.model.is_empty() {
                        self.active.model.clone()
                    } else {
                        srv.model.clone()
                    };
                    log::info!(
                        "[LlmSettings] Resolved server id='{}' url='{}' model='{}'",
                        srv.id,
                        srv.url,
                        model
                    );
                    LlmRuntimeTarget::Server {
                        base_url: srv.url.clone(),
                        model,
                        provider_name: Some(srv.name.clone()),
                        protocol: Some(srv.protocol.clone()),
                        api_key: srv.api_key.clone(),
                    }
                } else {
                    log::warn!(
                        "[LlmSettings] Target server ID '{}' not found in providers inventory ({} servers). Using default server fallback.",
                        target_id,
                        provs.servers.len()
                    );
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
                let provider_id = match self.active.provider.as_deref() {
                    Some(id) if !id.trim().is_empty() => id.trim(),
                    _ => DEFAULT_LLM_CLOUD_PROVIDER_NAME,
                };
                let api_key = provs.cloud_keys.get(provider_id).cloned();
                use crate::services::llm::catalog::lookup_preset;
                let preset = lookup_preset(provider_id);
                match preset {
                    Some(p) => LlmRuntimeTarget::Cloud {
                        base_url: p.base_url.to_string(),
                        model: self.active.model.clone(),
                        provider_name: Some(provider_id.to_string()),
                        protocol: Some("openai".to_string()),
                        api_key,
                    },
                    None => {
                        log::error!(
                            "[LlmSettings] Cloud provider id '{}' is not in the declarative wire manifest (RULE-PROV-01). Refusing to silently fall back to an unrelated endpoint.",
                            provider_id
                        );
                        LlmRuntimeTarget::Unresolved {
                            provider_id: provider_id.to_string(),
                        }
                    }
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
        assert_eq!(settings.runtime_target(None), LlmRuntimeTarget::Embedded);
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
                assert_eq!(base_url, "https://integrate.api.nvidia.com/v1");
                assert_eq!(model, "meta/llama-3.1-8b-instruct");
                assert_eq!(provider_name.as_deref(), Some("nvidia"));
                assert_eq!(api_key.as_deref(), Some("nv-secret-123"));
            }
            _ => panic!("Expected LlmRuntimeTarget::Cloud"),
        }
    }

    #[test]
    fn test_llm_settings_cloud_provider_resolution() {
        let mut settings = LlmSettings {
            active: ActiveSelection {
                category: "cloud".to_string(),
                provider: Some("anthropic".to_string()),
                model: "claude-3-5-sonnet-20241022".to_string(),
            },
            ..Default::default()
        };
        let mut provs = LlmProvidersConfig::default();
        provs
            .cloud_keys
            .insert("anthropic".to_string(), "sk-ant-test".to_string());

        // Anthropic resolution: must match anthropic endpoint, never NVIDIA
        match settings.runtime_target(Some(&provs)) {
            LlmRuntimeTarget::Cloud {
                base_url,
                model,
                provider_name,
                api_key,
                ..
            } => {
                assert_eq!(base_url, "https://api.anthropic.com/v1");
                assert_eq!(model, "claude-3-5-sonnet-20241022");
                assert_eq!(provider_name.as_deref(), Some("anthropic"));
                assert_eq!(api_key.as_deref(), Some("sk-ant-test"));
            }
            other => panic!("Expected Cloud target for anthropic, got {:?}", other),
        }

        // Gemini resolution
        settings.active.provider = Some("gemini".to_string());
        settings.active.model = "gemini-2.5-flash".to_string();
        match settings.runtime_target(None) {
            LlmRuntimeTarget::Cloud {
                base_url,
                model,
                provider_name,
                ..
            } => {
                assert_eq!(
                    base_url,
                    "https://generativelanguage.googleapis.com/v1beta/openai"
                );
                assert_eq!(model, "gemini-2.5-flash");
                assert_eq!(provider_name.as_deref(), Some("gemini"));
            }
            other => panic!("Expected Cloud target for gemini, got {:?}", other),
        }

        // OpenRouter resolution
        settings.active.provider = Some("openrouter".to_string());
        settings.active.model = "meta-llama/llama-3.3-70b-instruct".to_string();
        match settings.runtime_target(None) {
            LlmRuntimeTarget::Cloud {
                base_url,
                model,
                provider_name,
                ..
            } => {
                assert_eq!(base_url, "https://openrouter.ai/api/v1");
                assert_eq!(model, "meta-llama/llama-3.3-70b-instruct");
                assert_eq!(provider_name.as_deref(), Some("openrouter"));
            }
            other => panic!("Expected Cloud target for openrouter, got {:?}", other),
        }

        // Groq resolution
        settings.active.provider = Some("groq".to_string());
        settings.active.model = "llama-3.3-70b-versatile".to_string();
        match settings.runtime_target(None) {
            LlmRuntimeTarget::Cloud {
                base_url,
                model,
                provider_name,
                ..
            } => {
                assert_eq!(base_url, "https://api.groq.com/openai/v1");
                assert_eq!(model, "llama-3.3-70b-versatile");
                assert_eq!(provider_name.as_deref(), Some("groq"));
            }
            other => panic!("Expected Cloud target for groq, got {:?}", other),
        }

        // Together AI resolution
        settings.active.provider = Some("together".to_string());
        settings.active.model = "meta-llama/Llama-3-70b-chat-hf".to_string();
        match settings.runtime_target(None) {
            LlmRuntimeTarget::Cloud {
                base_url,
                model,
                provider_name,
                ..
            } => {
                assert_eq!(base_url, "https://api.together.xyz/v1");
                assert_eq!(model, "meta-llama/Llama-3-70b-chat-hf");
                assert_eq!(provider_name.as_deref(), Some("together"));
            }
            other => panic!("Expected Cloud target for together, got {:?}", other),
        }

        // DeepSeek resolution
        settings.active.provider = Some("deepseek".to_string());
        settings.active.model = "deepseek-chat".to_string();
        match settings.runtime_target(None) {
            LlmRuntimeTarget::Cloud {
                base_url,
                model,
                provider_name,
                ..
            } => {
                assert_eq!(base_url, "https://api.deepseek.com/v1");
                assert_eq!(model, "deepseek-chat");
                assert_eq!(provider_name.as_deref(), Some("deepseek"));
            }
            other => panic!("Expected Cloud target for deepseek, got {:?}", other),
        }

        // Mistral resolution
        settings.active.provider = Some("mistral".to_string());
        settings.active.model = "mistral-large-latest".to_string();
        match settings.runtime_target(None) {
            LlmRuntimeTarget::Cloud {
                base_url,
                model,
                provider_name,
                ..
            } => {
                assert_eq!(base_url, "https://api.mistral.ai/v1");
                assert_eq!(model, "mistral-large-latest");
                assert_eq!(provider_name.as_deref(), Some("mistral"));
            }
            other => panic!("Expected Cloud target for mistral, got {:?}", other),
        }

        // Unknown vendor resolution: MUST be Unresolved, NEVER fall back to NVIDIA
        settings.active.provider = Some("non_existent_provider_xyz".to_string());
        match settings.runtime_target(None) {
            LlmRuntimeTarget::Unresolved { provider_id } => {
                assert_eq!(provider_id, "non_existent_provider_xyz");
            }
            other => panic!(
                "Expected Unresolved target for unknown provider, got {:?}",
                other
            ),
        }
    }
}
