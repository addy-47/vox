use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::{
    config::{
        classify_known, get_setting_reload_policy, mutation::supported_keys_for_domain, secret_keys,
    },
    core::defaults::{DEFAULT_LLM_MAX_OUTPUT_TOKENS, DEFAULT_LLM_TEMPERATURE, DEFAULT_TTS_SPEED},
    services::llm::catalog::presets::BUNDLED_PROVIDERS_JSON,
    setup::manifest::{ModelGroup, VoxManifest},
};

pub const SUPPORTED_DOMAINS: &[&str] = &[
    "appearance",
    "vad",
    "stt",
    "llm",
    "tts",
    "interaction",
    "dictation",
    "working_memory",
    "persona",
    "realtime",
    "personal_memory",
    "system",
];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecretKeyContract {
    pub domain: String,
    pub key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmCloudProviderContract {
    pub id: String,
    pub name: String,
    pub base_url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key_placeholder: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub popular: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tagline: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub logo: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NumericDefaultsContract {
    pub temperature: f32,
    pub max_output_tokens: u32,
    #[serde(alias = "max_tokens")]
    pub max_tokens: u32,
    pub speed: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RealtimeProviderContract {
    pub id: String,
    pub name: String,
    pub url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key_placeholder: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tagline: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub logo: Option<String>,
    pub voice_field: String,
    pub supports_web_search: bool,
    pub supports_agent_mode: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SettingsContract {
    pub supported_keys: HashMap<String, Vec<String>>,
    pub supported_keys_by_domain: HashMap<String, Vec<String>>,
    pub reload_policies: HashMap<String, HashMap<String, String>>,
    pub secret_keys: Vec<SecretKeyContract>,
    pub llm_cloud_providers: Vec<LlmCloudProviderContract>,
    pub tts_providers: Vec<ModelGroup>,
    pub tts_models: Vec<ModelGroup>,
    pub realtime_providers: Vec<RealtimeProviderContract>,
    pub defaults: NumericDefaultsContract,
    pub numeric_defaults: NumericDefaultsContract,
}

#[derive(Debug, Deserialize)]
struct RawProviderPreset {
    id: String,
    name: String,
    base_url: String,
    #[serde(default)]
    key_placeholder: Option<String>,
    #[serde(default)]
    popular: Option<bool>,
    #[serde(default)]
    tagline: Option<String>,
    #[serde(default)]
    logo: Option<String>,
    #[serde(default)]
    auth_scheme: Option<String>,
}

/// Builds the canonical settings contract payload.
pub fn build_settings_contract() -> SettingsContract {
    let mut supported_keys = HashMap::new();
    let mut reload_policies = HashMap::new();

    for &domain in SUPPORTED_DOMAINS {
        let keys_slice = supported_keys_for_domain(domain);
        let keys_vec: Vec<String> = keys_slice.iter().map(|&s| s.to_string()).collect();
        supported_keys.insert(domain.to_string(), keys_vec);

        let mut domain_policies = HashMap::new();
        for &key in keys_slice {
            let policy = classify_known(domain, key)
                .unwrap_or_else(|| get_setting_reload_policy(domain, key));
            domain_policies.insert(key.to_string(), policy.as_str().to_string());
        }
        reload_policies.insert(domain.to_string(), domain_policies);
    }

    let secret_keys = secret_keys()
        .iter()
        .map(|(d, k)| SecretKeyContract {
            domain: d.to_string(),
            key: k.to_string(),
        })
        .collect();

    // Parse cloud providers from baseline_providers.json
    let raw_presets: Vec<RawProviderPreset> =
        serde_json::from_str(BUNDLED_PROVIDERS_JSON).unwrap_or_default();

    let mut llm_cloud_providers = Vec::new();
    let mut seen_ids = std::collections::HashSet::new();

    for preset in raw_presets {
        let is_cloud = preset.key_placeholder.is_some()
            || (preset.auth_scheme.as_deref() != Some("none") && preset.auth_scheme.is_some());

        if !is_cloud {
            continue;
        }

        let id = preset.id;
        if seen_ids.contains(&id) {
            continue;
        }
        seen_ids.insert(id.clone());

        llm_cloud_providers.push(LlmCloudProviderContract {
            id,
            name: preset.name,
            base_url: preset.base_url,
            key_placeholder: preset.key_placeholder,
            popular: preset.popular,
            tagline: preset.tagline,
            logo: preset.logo,
        });
    }

    // Source TTS models directly from models_manifest.json ModelGroup SSOT
    let manifest = VoxManifest::bundled();
    let tts_models: Vec<ModelGroup> = manifest
        .model_groups
        .into_iter()
        .filter(|g| g.category == "tts")
        .collect();

    let defaults = NumericDefaultsContract {
        temperature: DEFAULT_LLM_TEMPERATURE,
        max_output_tokens: DEFAULT_LLM_MAX_OUTPUT_TOKENS,
        max_tokens: DEFAULT_LLM_MAX_OUTPUT_TOKENS,
        speed: DEFAULT_TTS_SPEED,
    };

    let realtime_providers = vec![
        RealtimeProviderContract {
            id: "gemini_live".to_string(),
            name: "Gemini Live".to_string(),
            url: "https://aistudio.google.com/apikey".to_string(),
            key_placeholder: Some("AIzaSy...".to_string()),
            tagline: Some("Google's direct live speech model with search grounding".to_string()),
            logo: None,
            voice_field: "voice_name".to_string(),
            supports_web_search: true,
            supports_agent_mode: false,
        },
        RealtimeProviderContract {
            id: "deepgram_voice_agent".to_string(),
            name: "Deepgram Agent".to_string(),
            url: "https://console.deepgram.com/".to_string(),
            key_placeholder: Some("Token...".to_string()),
            tagline: Some("Deepgram's real-time conversational voice agent".to_string()),
            logo: None,
            voice_field: "voice".to_string(),
            supports_web_search: false,
            supports_agent_mode: true,
        },
    ];

    SettingsContract {
        supported_keys_by_domain: supported_keys.clone(),
        supported_keys,
        reload_policies,
        secret_keys,
        llm_cloud_providers,
        tts_providers: tts_models.clone(),
        tts_models,
        realtime_providers,
        defaults: defaults.clone(),
        numeric_defaults: defaults,
    }
}
