use serde::{Deserialize, Serialize};

use crate::{
    config::{ActiveSelection, LastSelectedModels},
    core::{
        compute::{self, Execution, Workload},
        defaults::{DEFAULT_ASR_MODEL, DEFAULT_ASR_TRANSLITERATE_ENABLED},
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

/// Cloud STT stub in providers.jsonc — coming soon.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Default)]
#[serde(default)]
pub struct SttProvidersConfig {}

/// Concrete runtime configuration used to instantiate an STT provider engine.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SttProviderConfig {
    #[serde(rename_all = "snake_case")]
    Embedded { model_type: String },
    #[serde(rename_all = "snake_case")]
    Cloud {
        provider: String,
        model: String,
        language: String,
        region: String,
        sample_rate: u32,
        channels: u16,
        interim_results: bool,
        punctuate: bool,
    },
}

impl Default for SttProviderConfig {
    fn default() -> Self {
        Self::Embedded {
            model_type: DEFAULT_ASR_MODEL.to_string(),
        }
    }
}

/// Clean STT settings stored in settings.jsonc.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct SttSettings {
    pub active: ActiveSelection,
    pub last_selected: LastSelectedModels,
    pub transliterate_enabled: bool,
}

impl Default for SttSettings {
    fn default() -> Self {
        Self {
            active: ActiveSelection {
                category: "embedded".to_string(),
                provider: None,
                model: DEFAULT_ASR_MODEL.to_string(),
            },
            last_selected: LastSelectedModels {
                embedded: Some(DEFAULT_ASR_MODEL.to_string()),
                server: None,
                cloud: None,
            },
            transliterate_enabled: DEFAULT_ASR_TRANSLITERATE_ENABLED,
        }
    }
}

impl SttSettings {
    /// Returns the active STT model identifier.
    pub fn active_model(&self) -> &str {
        &self.active.model
    }

    /// Resolves the effective worker thread count.
    ///
    /// The budget comes from the backend compute policy. Only a provider that
    /// runs on this machine owns local inference threads, so a cloud provider
    /// resolves to zero threads.
    pub fn effective_threads(&self) -> u32 {
        // The execution site follows the resolved provider config. The STT
        // backend builds an embedded engine for every category except
        // "cloud", so only a cloud provider runs off this machine.
        let execution = match self.to_provider_config() {
            SttProviderConfig::Embedded { .. } => Execution::Embedded,
            SttProviderConfig::Cloud { .. } => Execution::Remote,
        };
        compute::resolve_threads(Workload::Stt, execution)
    }

    /// Converts active settings into a concrete provider configuration variant.
    pub fn to_provider_config(&self) -> SttProviderConfig {
        match self.active.category.as_str() {
            "cloud" => SttProviderConfig::Cloud {
                provider: self
                    .active
                    .provider
                    .clone()
                    .unwrap_or_else(|| "nvidia".to_string()),
                model: self.active.model.clone(),
                language: "en-US".to_string(),
                region: "us".to_string(),
                sample_rate: SAMPLE_RATE,
                channels: 1,
                interim_results: true,
                punctuate: true,
            },
            _ => SttProviderConfig::Embedded {
                model_type: self.active.model.clone(),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// PURPOSE:  A fresh install must start on the embedded ASR model with transliteration enabled.
    /// SCENARIO: The default SttSettings are read and resolved.
    /// ASSERTS:  The category is "embedded", the model is DEFAULT_ASR_MODEL, transliteration matches its default, and the provider config is Embedded.
    /// CLASS:    happy-path
    /// REQUIRES: none
    /// MUTANT:   Change the default STT active category to "server".
    #[test]
    fn stt_settings_default_to_the_embedded_model() {
        let settings = SttSettings::default();
        assert_eq!(settings.active.category, "embedded");
        assert_eq!(settings.active_model(), DEFAULT_ASR_MODEL);
        assert_eq!(
            settings.last_selected.embedded.as_deref(),
            Some(DEFAULT_ASR_MODEL)
        );
        assert_eq!(
            settings.transliterate_enabled,
            DEFAULT_ASR_TRANSLITERATE_ENABLED
        );
        assert_eq!(
            settings.to_provider_config(),
            SttProviderConfig::Embedded {
                model_type: DEFAULT_ASR_MODEL.to_string()
            }
        );
    }
}
