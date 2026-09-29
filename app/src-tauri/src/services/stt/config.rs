use serde::{Deserialize, Serialize};

use crate::{
    core::defaults::{
        DEFAULT_ASR_MODEL, DEFAULT_ASR_TRANSLITERATE_ENABLED, DEFAULT_STT_CLOUD_LANGUAGE,
        DEFAULT_STT_CLOUD_MODEL, DEFAULT_STT_CLOUD_PROVIDER, DEFAULT_STT_CLOUD_REGION,
        DEFAULT_STT_PARTIAL_THROTTLE_MS, DEFAULT_STT_THREADS,
    },
    services::audio::SAMPLE_RATE,
};

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum SttActiveProvider {
    #[default]
    Embedded,
    Cloud,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(default)]
pub struct SttEmbeddedConfig {
    pub model: String,
    pub partial_throttle_ms: u64,
    pub threads: u32,
}

impl Default for SttEmbeddedConfig {
    fn default() -> Self {
        Self {
            model: DEFAULT_ASR_MODEL.to_string(),
            partial_throttle_ms: DEFAULT_STT_PARTIAL_THROTTLE_MS,
            threads: DEFAULT_STT_THREADS,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(default)]
pub struct SttCloudConfig {
    pub provider: String,
    pub model: String,
    pub language: String,
    pub region: String,
    pub sample_rate: u32,
    pub channels: u16,
    pub interim_results: bool,
    pub punctuate: bool,
}

impl Default for SttCloudConfig {
    fn default() -> Self {
        Self {
            provider: DEFAULT_STT_CLOUD_PROVIDER.to_string(),
            model: DEFAULT_STT_CLOUD_MODEL.to_string(),
            language: DEFAULT_STT_CLOUD_LANGUAGE.to_string(),
            region: DEFAULT_STT_CLOUD_REGION.to_string(),
            sample_rate: SAMPLE_RATE,
            channels: 1,
            interim_results: true,
            punctuate: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SttProviderConfig {
    #[serde(rename_all = "snake_case")]
    Embedded {
        #[serde(default = "default_stt_model")]
        model_type: String,
    },
    #[serde(rename_all = "snake_case")]
    Cloud {
        #[serde(default = "default_cloud_provider")]
        provider: String,
        #[serde(default = "default_cloud_model")]
        model: String,
        #[serde(default = "default_cloud_language")]
        language: String,
        #[serde(default = "default_cloud_region")]
        region: String,
        #[serde(default = "default_sample_rate")]
        sample_rate: u32,
        #[serde(default = "default_channels")]
        channels: u16,
        #[serde(default = "default_true")]
        interim_results: bool,
        #[serde(default = "default_true")]
        punctuate: bool,
    },
}

impl Default for SttProviderConfig {
    fn default() -> Self {
        Self::Embedded {
            model_type: default_stt_model(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct SttSettings {
    pub active: SttActiveProvider,
    #[serde(alias = "transliterate")]
    pub transliterate_enabled: bool,
    pub embedded: SttEmbeddedConfig,
    pub cloud: SttCloudConfig,
}

impl Default for SttSettings {
    fn default() -> Self {
        Self {
            active: SttActiveProvider::default(),
            transliterate_enabled: DEFAULT_ASR_TRANSLITERATE_ENABLED,
            embedded: SttEmbeddedConfig::default(),
            cloud: SttCloudConfig::default(),
        }
    }
}

impl SttSettings {
    /// Returns the active STT model identifier.
    pub fn active_model(&self) -> &str {
        match self.active {
            SttActiveProvider::Embedded => &self.embedded.model,
            SttActiveProvider::Cloud => &self.cloud.model,
        }
    }

    /// Resolves the effective worker thread count, falling back to defaults if zero.
    pub fn effective_threads(&self) -> u32 {
        if self.embedded.threads == 0 {
            DEFAULT_STT_THREADS
        } else {
            self.embedded.threads
        }
    }

    /// Converts active settings into a concrete provider configuration variant.
    pub fn to_provider_config(&self) -> SttProviderConfig {
        match self.active {
            SttActiveProvider::Embedded => SttProviderConfig::Embedded {
                model_type: self.embedded.model.clone(),
            },
            SttActiveProvider::Cloud => SttProviderConfig::Cloud {
                provider: self.cloud.provider.clone(),
                model: self.cloud.model.clone(),
                language: self.cloud.language.clone(),
                region: self.cloud.region.clone(),
                sample_rate: self.cloud.sample_rate,
                channels: self.cloud.channels,
                interim_results: self.cloud.interim_results,
                punctuate: self.cloud.punctuate,
            },
        }
    }
}

fn default_stt_model() -> String {
    DEFAULT_ASR_MODEL.to_string()
}

fn default_cloud_provider() -> String {
    DEFAULT_STT_CLOUD_PROVIDER.to_string()
}

fn default_cloud_model() -> String {
    DEFAULT_STT_CLOUD_MODEL.to_string()
}

fn default_cloud_language() -> String {
    DEFAULT_STT_CLOUD_LANGUAGE.to_string()
}

fn default_cloud_region() -> String {
    DEFAULT_STT_CLOUD_REGION.to_string()
}

fn default_sample_rate() -> u32 {
    SAMPLE_RATE
}

fn default_channels() -> u16 {
    1
}

fn default_true() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stt_settings_defaults() {
        let settings = SttSettings::default();
        assert_eq!(settings.active, SttActiveProvider::Embedded);
        assert!(settings.transliterate_enabled);
        assert_eq!(settings.embedded.model, DEFAULT_ASR_MODEL);
        assert_eq!(settings.active_model(), DEFAULT_ASR_MODEL);
        assert_eq!(settings.effective_threads(), DEFAULT_STT_THREADS);

        let provider_cfg = settings.to_provider_config();
        match provider_cfg {
            SttProviderConfig::Embedded { model_type } => {
                assert_eq!(model_type, DEFAULT_ASR_MODEL);
            }
            SttProviderConfig::Cloud { .. } => panic!("Expected embedded provider"),
        }
    }

    #[test]
    fn test_stt_settings_cloud_active() {
        let settings = SttSettings {
            active: SttActiveProvider::Cloud,
            ..Default::default()
        };
        assert_eq!(settings.active_model(), DEFAULT_STT_CLOUD_MODEL);

        let provider_cfg = settings.to_provider_config();
        match provider_cfg {
            SttProviderConfig::Cloud {
                provider,
                model,
                language,
                region,
                sample_rate,
                channels,
                interim_results,
                punctuate,
            } => {
                assert_eq!(provider, DEFAULT_STT_CLOUD_PROVIDER);
                assert_eq!(model, DEFAULT_STT_CLOUD_MODEL);
                assert_eq!(language, DEFAULT_STT_CLOUD_LANGUAGE);
                assert_eq!(region, DEFAULT_STT_CLOUD_REGION);
                assert_eq!(sample_rate, SAMPLE_RATE);
                assert_eq!(channels, 1);
                assert!(interim_results);
                assert!(punctuate);
            }
            SttProviderConfig::Embedded { .. } => panic!("Expected cloud provider"),
        }
    }

    #[test]
    fn test_effective_threads_fallback() {
        let mut settings = SttSettings::default();
        settings.embedded.threads = 0;
        assert_eq!(settings.effective_threads(), DEFAULT_STT_THREADS);
        settings.embedded.threads = 8;
        assert_eq!(settings.effective_threads(), 8);
    }

    #[test]
    fn test_serde_roundtrip() {
        let original = SttSettings::default();
        let json = serde_json::to_string(&original).unwrap();
        let deserialized: SttSettings = serde_json::from_str(&json).unwrap();
        assert_eq!(original, deserialized);
    }

    #[test]
    fn test_transliterate_alias() {
        let json = r#"{"transliterate": false}"#;
        let deserialized: SttSettings = serde_json::from_str(json).unwrap();
        assert!(!deserialized.transliterate_enabled);
    }
}
