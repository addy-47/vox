use serde::{Deserialize, Serialize};

use crate::{
    config::{ActiveSelection, LastSelectedModels},
    core::defaults::{
        DEFAULT_TTS_AZURE_REGION, DEFAULT_TTS_AZURE_VOICE, DEFAULT_TTS_GOOGLE_VOICE,
        DEFAULT_TTS_VOICE_ID, DEFAULT_TTS_ZIPVOICE_GUIDANCE_SCALE,
    },
};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TtsVoiceSource {
    Catalog,
    Custom,
    Edge,
    Cloud,
    None,
}

/// Inclusive, quantised bounds a frontend control must respect.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct ParamRange {
    pub min: f32,
    pub max: f32,
    pub step: f32,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct ProviderCaps {
    pub voices: TtsVoiceSource,
    pub clone: bool,
    pub speed_range: ParamRange,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum TtsActiveProvider {
    #[default]
    EdgeTts,
    Supertonic,
    Kokoro,
    Chatterbox,
    ChatterboxRemote,
    Zipvoice,
    AzureTts,
    GoogleTts,
}

/// A user-configured custom TTS server instance in providers.jsonc.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(default)]
pub struct TtsCustomServer {
    pub id: String,
    pub name: String,
    pub url: String,
    pub remote_path: String,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub protocol: Option<String>,
    #[serde(default)]
    pub api_key: Option<String>,
}

impl Default for TtsCustomServer {
    fn default() -> Self {
        Self {
            id: String::new(),
            name: String::new(),
            url: String::new(),
            remote_path: "/opt/chatterbox".to_string(),
            model: None,
            protocol: None,
            api_key: None,
        }
    }
}

/// Cloud Azure Speech configuration.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(default)]
pub struct TtsAzureConfig {
    pub region_or_endpoint: String,
    pub voice: String,
    pub key: String,
    pub resource_id: Option<String>,
}

impl Default for TtsAzureConfig {
    fn default() -> Self {
        Self {
            region_or_endpoint: DEFAULT_TTS_AZURE_REGION.to_string(),
            voice: DEFAULT_TTS_AZURE_VOICE.to_string(),
            key: String::new(),
            resource_id: None,
        }
    }
}

/// Cloud Google Text-to-Speech configuration.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(default)]
pub struct TtsGoogleConfig {
    pub voice: String,
    pub project_override: Option<String>,
}

impl Default for TtsGoogleConfig {
    fn default() -> Self {
        Self {
            voice: DEFAULT_TTS_GOOGLE_VOICE.to_string(),
            project_override: None,
        }
    }
}

/// Custom TTS servers and cloud settings stored in providers.jsonc.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Default)]
#[serde(default)]
pub struct TtsProvidersConfig {
    pub servers: Vec<TtsCustomServer>,
    pub azure_tts: TtsAzureConfig,
    pub google_tts: TtsGoogleConfig,
}

/// Concrete runtime configuration used to instantiate a TTS provider engine.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TtsProviderConfig {
    Supertonic,
    #[default]
    Kokoro,
    Chatterbox {
        language: String,
        speed: f32,
        #[serde(default)]
        voice_id: Option<String>,
    },
    ChatterboxRemote {
        endpoint: String,
        language: String,
        speed: f32,
        remote_path: String,
        #[serde(default)]
        voice_id: Option<String>,
    },
    EdgeTts {
        #[serde(default)]
        voice: Option<String>,
    },
    Zipvoice {
        #[serde(default)]
        voice_id: Option<String>,
        guidance_scale: f32,
    },
    AzureTts {
        region_or_endpoint: String,
        voice: String,
        #[serde(default)]
        key: String,
        #[serde(default)]
        resource_id: Option<String>,
    },
    GoogleTts {
        voice: String,
        #[serde(default)]
        project_override: Option<String>,
    },
}

/// Clean TTS settings stored in settings.jsonc.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct TtsSettings {
    pub active: ActiveSelection,
    pub last_selected: LastSelectedModels,
    pub active_voice_id: String,
}

impl Default for TtsSettings {
    fn default() -> Self {
        Self {
            active: ActiveSelection {
                category: "embedded".to_string(),
                provider: None,
                model: "kokoro".to_string(),
            },
            last_selected: LastSelectedModels {
                embedded: Some("kokoro".to_string()),
                server: Some("chatterbox-gpu".to_string()),
                cloud: Some("edge_tts".to_string()),
            },
            active_voice_id: DEFAULT_TTS_VOICE_ID.to_string(),
        }
    }
}

impl TtsSettings {
    pub fn to_provider_config(
        &self,
        providers: Option<&TtsProvidersConfig>,
        speed: f32,
    ) -> TtsProviderConfig {
        match self.active.category.as_str() {
            "server" => {
                let default_prov = TtsProvidersConfig::default();
                let provs = providers.unwrap_or(&default_prov);
                let target_id = self.active.provider.as_deref().unwrap_or("chatterbox-gpu");
                let server = provs
                    .servers
                    .iter()
                    .find(|s| s.id == target_id);

                if let Some(srv) = server {
                    TtsProviderConfig::ChatterboxRemote {
                        endpoint: srv.url.clone(),
                        language: "en".to_string(),
                        speed,
                        remote_path: srv.remote_path.clone(),
                        voice_id: Some(self.active_voice_id.clone()),
                    }
                } else {
                    log::warn!(
                        "[TtsSettings] Target server ID '{}' not found in providers inventory. Using Kokoro fallback.",
                        target_id
                    );
                    TtsProviderConfig::Kokoro
                }
            }
            "cloud" => {
                let default_prov = TtsProvidersConfig::default();
                let provs = providers.unwrap_or(&default_prov);
                match self.active.model.as_str() {
                    "azure_tts" | "azure" => TtsProviderConfig::AzureTts {
                        region_or_endpoint: provs.azure_tts.region_or_endpoint.clone(),
                        voice: if !self.active_voice_id.is_empty() {
                            self.active_voice_id.clone()
                        } else {
                            provs.azure_tts.voice.clone()
                        },
                        key: provs.azure_tts.key.clone(),
                        resource_id: provs.azure_tts.resource_id.clone(),
                    },
                    "google_tts" | "google" => TtsProviderConfig::GoogleTts {
                        voice: if !self.active_voice_id.is_empty() {
                            self.active_voice_id.clone()
                        } else {
                            provs.google_tts.voice.clone()
                        },
                        project_override: provs.google_tts.project_override.clone(),
                    },
                    _ => TtsProviderConfig::EdgeTts {
                        voice: Some(self.active_voice_id.clone()),
                    },
                }
            }
            _ => match self.active.model.as_str() {
                "supertonic" => TtsProviderConfig::Supertonic,
                "chatterbox" => TtsProviderConfig::Chatterbox {
                    language: "en".to_string(),
                    speed,
                    voice_id: Some(self.active_voice_id.clone()),
                },
                "zipvoice" => TtsProviderConfig::Zipvoice {
                    voice_id: Some(self.active_voice_id.clone()),
                    guidance_scale: DEFAULT_TTS_ZIPVOICE_GUIDANCE_SCALE,
                },
                _ => TtsProviderConfig::Kokoro,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::defaults::DEFAULT_TTS_SPEED;

    #[test]
    fn test_tts_settings_defaults() {
        let settings = TtsSettings::default();
        assert_eq!(settings.active.category, "embedded");
        assert_eq!(settings.active.model, "kokoro");
        assert_eq!(settings.active_voice_id, DEFAULT_TTS_VOICE_ID);
        assert_eq!(settings.last_selected.embedded.as_deref(), Some("kokoro"));

        let provider_config = settings.to_provider_config(None, DEFAULT_TTS_SPEED);
        assert_eq!(provider_config, TtsProviderConfig::Kokoro);
    }

    #[test]
    fn test_tts_settings_server_and_cloud() {
        let mut settings = TtsSettings {
            active: ActiveSelection {
                category: "cloud".to_string(),
                provider: None,
                model: "edge_tts".to_string(),
            },
            active_voice_id: "en-US-AriaNeural".to_string(),
            ..Default::default()
        };

        let prov = settings.to_provider_config(None, 1.0);
        assert_eq!(
            prov,
            TtsProviderConfig::EdgeTts {
                voice: Some("en-US-AriaNeural".to_string()),
            }
        );

        settings.active = ActiveSelection {
            category: "server".to_string(),
            provider: Some("chatterbox-gpu".to_string()),
            model: "chatterbox".to_string(),
        };

        let test_provs = TtsProvidersConfig {
            servers: vec![TtsCustomServer {
                id: "chatterbox-gpu".to_string(),
                name: "Test GPU".to_string(),
                url: "http://localhost:8000".to_string(),
                remote_path: "/opt/chatterbox".to_string(),
                ..Default::default()
            }],
            ..Default::default()
        };

        let prov = settings.to_provider_config(Some(&test_provs), 1.2);
        match prov {
            TtsProviderConfig::ChatterboxRemote {
                endpoint,
                language,
                speed,
                remote_path,
                voice_id,
            } => {
                assert_eq!(endpoint, "http://localhost:8000");
                assert_eq!(language, "en");
                assert_eq!(speed, 1.2);
                assert_eq!(remote_path, "/opt/chatterbox");
                assert_eq!(voice_id, Some("en-US-AriaNeural".to_string()));
            }
            _ => panic!("Expected ChatterboxRemote"),
        }

        // Test Cloud Azure TTS
        settings.active = ActiveSelection {
            category: "cloud".to_string(),
            provider: None,
            model: "azure_tts".to_string(),
        };
        settings.active_voice_id = "en-US-AriaNeural".to_string();
        let mut azure_provs = TtsProvidersConfig::default();
        azure_provs.azure_tts.region_or_endpoint = "westeurope".to_string();
        azure_provs.azure_tts.key = "test-key".to_string();
        azure_provs.azure_tts.resource_id = Some("res".to_string());
        assert_eq!(
            settings.to_provider_config(Some(&azure_provs), 1.0),
            TtsProviderConfig::AzureTts {
                region_or_endpoint: "westeurope".to_string(),
                voice: "en-US-AriaNeural".to_string(),
                key: "test-key".to_string(),
                resource_id: Some("res".to_string()),
            }
        );

        // Test Cloud Google TTS
        settings.active = ActiveSelection {
            category: "cloud".to_string(),
            provider: None,
            model: "google_tts".to_string(),
        };
        settings.active_voice_id = "en-US-Chirp3-HD-Charon".to_string();
        let mut google_provs = TtsProvidersConfig::default();
        google_provs.google_tts.project_override = Some("my-project".to_string());
        assert_eq!(
            settings.to_provider_config(Some(&google_provs), 1.0),
            TtsProviderConfig::GoogleTts {
                voice: "en-US-Chirp3-HD-Charon".to_string(),
                project_override: Some("my-project".to_string()),
            }
        );
    }
}
