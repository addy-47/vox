use serde::{Deserialize, Serialize};

use crate::core::defaults::{
    DEFAULT_TTS_SPEED, DEFAULT_TTS_THREADS, DEFAULT_TTS_VOICE_INDEX,
    DEFAULT_TTS_ZIPVOICE_GUIDANCE_SCALE,
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

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Default)]
#[serde(default)]
pub struct TtsEdgeConfig {
    pub voice: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Default)]
pub struct TtsSupertonicConfig {}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Default)]
pub struct TtsKokoroConfig {}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(default)]
pub struct TtsChatterboxConfig {
    pub language: String,
    pub voice_id: Option<String>,
}

impl Default for TtsChatterboxConfig {
    fn default() -> Self {
        Self {
            language: "en".to_string(),
            voice_id: None,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(default)]
pub struct TtsChatterboxRemoteConfig {
    pub endpoint: String,
    pub language: String,
    pub remote_path: String,
    pub voice_id: Option<String>,
}

impl Default for TtsChatterboxRemoteConfig {
    fn default() -> Self {
        Self {
            endpoint: String::new(),
            language: "en".to_string(),
            remote_path: "/opt/chatterbox".to_string(),
            voice_id: None,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(default)]
pub struct TtsZipvoiceConfig {
    pub voice_id: Option<String>,
    pub guidance_scale: f32,
}

impl Default for TtsZipvoiceConfig {
    fn default() -> Self {
        Self {
            voice_id: None,
            guidance_scale: DEFAULT_TTS_ZIPVOICE_GUIDANCE_SCALE,
        }
    }
}

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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct TtsSettings {
    pub active: TtsActiveProvider,
    #[serde(alias = "voice")]
    pub voice_index: i32,
    pub speed: f32,
    pub threads: u32,
    pub edge_tts: TtsEdgeConfig,
    pub supertonic: TtsSupertonicConfig,
    pub kokoro: TtsKokoroConfig,
    pub chatterbox: TtsChatterboxConfig,
    pub chatterbox_remote: TtsChatterboxRemoteConfig,
    pub zipvoice: TtsZipvoiceConfig,
}

impl Default for TtsSettings {
    fn default() -> Self {
        Self {
            active: TtsActiveProvider::EdgeTts,
            voice_index: DEFAULT_TTS_VOICE_INDEX,
            speed: DEFAULT_TTS_SPEED,
            threads: DEFAULT_TTS_THREADS,
            edge_tts: TtsEdgeConfig::default(),
            supertonic: TtsSupertonicConfig::default(),
            kokoro: TtsKokoroConfig::default(),
            chatterbox: TtsChatterboxConfig::default(),
            chatterbox_remote: TtsChatterboxRemoteConfig::default(),
            zipvoice: TtsZipvoiceConfig::default(),
        }
    }
}

impl TtsSettings {
    pub fn to_provider_config(&self) -> TtsProviderConfig {
        match self.active {
            TtsActiveProvider::EdgeTts => TtsProviderConfig::EdgeTts {
                voice: self.edge_tts.voice.clone(),
            },
            TtsActiveProvider::Supertonic => TtsProviderConfig::Supertonic,
            TtsActiveProvider::Kokoro => TtsProviderConfig::Kokoro,
            TtsActiveProvider::Chatterbox => TtsProviderConfig::Chatterbox {
                language: self.chatterbox.language.clone(),
                speed: self.speed,
                voice_id: self.chatterbox.voice_id.clone(),
            },
            TtsActiveProvider::ChatterboxRemote => TtsProviderConfig::ChatterboxRemote {
                endpoint: self.chatterbox_remote.endpoint.clone(),
                language: self.chatterbox_remote.language.clone(),
                speed: self.speed,
                remote_path: self.chatterbox_remote.remote_path.clone(),
                voice_id: self.chatterbox_remote.voice_id.clone(),
            },
            TtsActiveProvider::Zipvoice => TtsProviderConfig::Zipvoice {
                voice_id: self.zipvoice.voice_id.clone(),
                guidance_scale: self.zipvoice.guidance_scale,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tts_settings_defaults() {
        let settings = TtsSettings::default();
        assert_eq!(settings.active, TtsActiveProvider::EdgeTts);
        assert_eq!(settings.voice_index, DEFAULT_TTS_VOICE_INDEX);
        assert_eq!(settings.speed, DEFAULT_TTS_SPEED);
        assert_eq!(settings.threads, DEFAULT_TTS_THREADS);
        assert_eq!(settings.edge_tts, TtsEdgeConfig::default());
        assert_eq!(settings.supertonic, TtsSupertonicConfig::default());
        assert_eq!(settings.kokoro, TtsKokoroConfig::default());
        assert_eq!(settings.chatterbox, TtsChatterboxConfig::default());
        assert_eq!(
            settings.chatterbox_remote,
            TtsChatterboxRemoteConfig::default()
        );
        assert_eq!(settings.zipvoice, TtsZipvoiceConfig::default());
        assert_eq!(
            settings.to_provider_config(),
            TtsProviderConfig::EdgeTts { voice: None }
        );
    }

    #[test]
    fn test_to_provider_config_all_variants() {
        let mut settings = TtsSettings {
            active: TtsActiveProvider::Supertonic,
            ..Default::default()
        };
        assert_eq!(settings.to_provider_config(), TtsProviderConfig::Supertonic);

        settings.active = TtsActiveProvider::Kokoro;
        assert_eq!(settings.to_provider_config(), TtsProviderConfig::Kokoro);

        settings.active = TtsActiveProvider::Chatterbox;
        settings.chatterbox.language = "es".to_string();
        settings.chatterbox.voice_id = Some("voice-123".to_string());
        assert_eq!(
            settings.to_provider_config(),
            TtsProviderConfig::Chatterbox {
                language: "es".to_string(),
                speed: DEFAULT_TTS_SPEED,
                voice_id: Some("voice-123".to_string()),
            }
        );

        settings.active = TtsActiveProvider::ChatterboxRemote;
        settings.chatterbox_remote.endpoint = "http://remote:8080".to_string();
        settings.chatterbox_remote.language = "fr".to_string();
        settings.chatterbox_remote.remote_path = "/models".to_string();
        settings.chatterbox_remote.voice_id = Some("voice-456".to_string());
        assert_eq!(
            settings.to_provider_config(),
            TtsProviderConfig::ChatterboxRemote {
                endpoint: "http://remote:8080".to_string(),
                language: "fr".to_string(),
                speed: DEFAULT_TTS_SPEED,
                remote_path: "/models".to_string(),
                voice_id: Some("voice-456".to_string()),
            }
        );

        settings.active = TtsActiveProvider::Zipvoice;
        settings.zipvoice.voice_id = Some("zip-voice-1".to_string());
        settings.zipvoice.guidance_scale = 2.5;
        assert_eq!(
            settings.to_provider_config(),
            TtsProviderConfig::Zipvoice {
                voice_id: Some("zip-voice-1".to_string()),
                guidance_scale: 2.5,
            }
        );

        settings.active = TtsActiveProvider::EdgeTts;
        settings.edge_tts.voice = Some("en-US-AriaNeural".to_string());
        assert_eq!(
            settings.to_provider_config(),
            TtsProviderConfig::EdgeTts {
                voice: Some("en-US-AriaNeural".to_string()),
            }
        );
    }
}
