use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::{
    config::ActiveSelection,
    core::defaults::{
        DEFAULT_DEEPGRAM_MODEL, DEFAULT_DEEPGRAM_TEMP, DEFAULT_DEEPGRAM_VOICE,
        DEFAULT_GEMINI_REALTIME_LANG, DEFAULT_GEMINI_REALTIME_MODEL, DEFAULT_GEMINI_REALTIME_TEMP,
        DEFAULT_GEMINI_REALTIME_VOICE,
    },
};

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum RealtimeProviderKind {
    #[default]
    GeminiLive,
    OpenAiRealtime,
    DeepgramVoiceAgent,
    ElevenLabsConvai,
}

impl RealtimeProviderKind {
    pub fn from_provider_str(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "gemini_live" | "gemini" => Some(Self::GeminiLive),
            "deepgram_voice_agent" | "deepgram" => Some(Self::DeepgramVoiceAgent),
            "openai_realtime" | "openai" => Some(Self::OpenAiRealtime),
            "elevenlabs_convai" | "elevenlabs" => Some(Self::ElevenLabsConvai),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::GeminiLive => "gemini_live",
            Self::OpenAiRealtime => "openai_realtime",
            Self::DeepgramVoiceAgent => "deepgram_voice_agent",
            Self::ElevenLabsConvai => "elevenlabs_convai",
        }
    }
}

/// Realtime settings stored in settings.jsonc.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct RealtimeSettings {
    pub active: ActiveSelection,
    pub active_voice_id: String,
}

impl Default for RealtimeSettings {
    fn default() -> Self {
        Self {
            active: ActiveSelection {
                category: "cloud".to_string(),
                provider: Some("gemini_live".to_string()),
                model: DEFAULT_GEMINI_REALTIME_MODEL.to_string(),
            },
            active_voice_id: DEFAULT_GEMINI_REALTIME_VOICE.to_string(),
        }
    }
}

/// Provider credentials stored in providers.jsonc under `realtime`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(default)]
pub struct RealtimeProvidersConfig {
    pub api_keys: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct GeminiRealtimeConfig {
    pub api_key: String,
    pub model: String,
    pub voice_name: String,
    pub language_code: String,
    pub temperature: f32,
    pub enable_web_search: bool,
}

impl Default for GeminiRealtimeConfig {
    fn default() -> Self {
        Self {
            api_key: String::new(),
            model: DEFAULT_GEMINI_REALTIME_MODEL.to_string(),
            voice_name: DEFAULT_GEMINI_REALTIME_VOICE.to_string(),
            language_code: DEFAULT_GEMINI_REALTIME_LANG.to_string(),
            temperature: DEFAULT_GEMINI_REALTIME_TEMP,
            enable_web_search: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct DeepgramVoiceAgentConfig {
    pub api_key: String,
    pub model: String,
    pub voice: String,
    pub temperature: f32,
    pub agent_mode: bool,
}

impl Default for DeepgramVoiceAgentConfig {
    fn default() -> Self {
        Self {
            api_key: String::new(),
            model: DEFAULT_DEEPGRAM_MODEL.to_string(),
            voice: DEFAULT_DEEPGRAM_VOICE.to_string(),
            temperature: DEFAULT_DEEPGRAM_TEMP,
            agent_mode: false,
        }
    }
}
