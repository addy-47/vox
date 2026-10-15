use serde::{Deserialize, Serialize};

use crate::{
    config::{ActiveSelection, LastSelectedModels},
    core::defaults::{
        DEFAULT_TTS_VOICE_ID, DEFAULT_TTS_ZIPVOICE_GUIDANCE_SCALE,
    },
};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TtsVoiceSource {
    Catalog,
    Custom,
    Edge,
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
}

/// A user-configured custom TTS server instance in providers.jsonc.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct TtsCustomServer {
    pub id: String,
    pub name: String,
    pub url: String,
    pub remote_path: String,
    #[serde(default)]
    pub api_key: Option<String>,
}

/// Custom TTS servers stored in providers.jsonc.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(default)]
pub struct TtsProvidersConfig {
    pub servers: Vec<TtsCustomServer>,
}

impl Default for TtsProvidersConfig {
    fn default() -> Self {
        Self {
            servers: vec![TtsCustomServer {
                id: "chatterbox-gpu".to_string(),
                name: "Chatterbox GPU Server".to_string(),
                url: "http://100.67.98.126:8000".to_string(),
                remote_path: "/opt/chatterbox".to_string(),
                api_key: None,
            }],
        }
    }
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
                    .find(|s| s.id == target_id)
                    .or_else(|| provs.servers.first());

                if let Some(srv) = server {
                    TtsProviderConfig::ChatterboxRemote {
                        endpoint: srv.url.clone(),
                        language: "en".to_string(),
                        speed,
                        remote_path: srv.remote_path.clone(),
                        voice_id: Some(self.active_voice_id.clone()),
                    }
                } else {
                    TtsProviderConfig::Kokoro
                }
            }
            "cloud" => TtsProviderConfig::EdgeTts {
                voice: Some(self.active_voice_id.clone()),
            },
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

        let prov = settings.to_provider_config(None, 1.2);
        match prov {
            TtsProviderConfig::ChatterboxRemote {
                endpoint,
                language,
                speed,
                remote_path,
                voice_id,
            } => {
                assert_eq!(endpoint, "http://100.67.98.126:8000");
                assert_eq!(language, "en");
                assert_eq!(speed, 1.2);
                assert_eq!(remote_path, "/opt/chatterbox");
                assert_eq!(voice_id, Some("en-US-AriaNeural".to_string()));
            }
            _ => panic!("Expected ChatterboxRemote"),
        }
    }
}
