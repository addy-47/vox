use std::{
    collections::HashMap,
    fs,
    io::Write,
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::{
    core::defaults::{
        DEFAULT_ASR_MODEL, DEFAULT_ASR_TRANSLITERATE_ENABLED,
        DEFAULT_DEEPGRAM_MODEL, DEFAULT_DEEPGRAM_TEMP, DEFAULT_DEEPGRAM_VOICE,
        DEFAULT_DICTATION_ENABLED, DEFAULT_DICTATION_HOTKEY,
        DEFAULT_DICTATION_SILENCE_AUTO_STOP_MS, DEFAULT_GEMINI_REALTIME_LANG,
        DEFAULT_GEMINI_REALTIME_MODEL, DEFAULT_GEMINI_REALTIME_TEMP, DEFAULT_GEMINI_REALTIME_VOICE,
        DEFAULT_LLM_CLOUD_BASE_URL, DEFAULT_LLM_CLOUD_MODEL, DEFAULT_LLM_CLOUD_PROVIDER_NAME,
        DEFAULT_LLM_COMPACTION_TEMPERATURE, DEFAULT_LLM_CONTEXT_WINDOW,
        DEFAULT_LLM_MAX_OUTPUT_TOKENS, DEFAULT_LLM_MODEL, DEFAULT_LLM_SERVER_BASE_URL,
        DEFAULT_LLM_SERVER_MODEL, DEFAULT_LLM_SERVER_PROVIDER_NAME, DEFAULT_LLM_TEMPERATURE,
        DEFAULT_LLM_THREADS, DEFAULT_PERSONAL_MEMORY_CONSOLIDATION_CADENCE,
        DEFAULT_PERSONAL_MEMORY_CONSOLIDATION_TIME,
        DEFAULT_PERSONAL_MEMORY_CONTEXT_RETRIEVAL_ENABLED,
        DEFAULT_PERSONAL_MEMORY_PIPELINE_PROCESSING_ENABLED,
        DEFAULT_PERSONAL_MEMORY_SEMANTIC_SIMILARITY_CUTOFF, DEFAULT_PERSONAL_MEMORY_TOP_K_FACTS,
        DEFAULT_STT_CLOUD_LANGUAGE, DEFAULT_STT_CLOUD_MODEL, DEFAULT_STT_CLOUD_PROVIDER,
        DEFAULT_STT_CLOUD_REGION, DEFAULT_STT_PARTIAL_THROTTLE_MS, DEFAULT_STT_THREADS,
        DEFAULT_SYSTEM_PROMPT_MODULAR, DEFAULT_SYSTEM_PROMPT_REALTIME,
        DEFAULT_TTS_SPEED, DEFAULT_TTS_THREADS,
        DEFAULT_TTS_VOICE_INDEX, DEFAULT_TTS_ZIPVOICE_GUIDANCE_SCALE, DEFAULT_UI_ACCENT_SEED,
        DEFAULT_UI_THEME,
        DEFAULT_VAD_MAX_SPEECH_DURATION_S, DEFAULT_VAD_PTT_NOISE_GATE,
        DEFAULT_VAD_SILENCE_DURATION_MS, DEFAULT_VAD_SPEECH_ONSET_MS, DEFAULT_VAD_THRESHOLD,
        DEFAULT_WORKING_MEMORY_AUTO_COMPACTION, DEFAULT_WORKING_MEMORY_MAX_CONTEXT_SHARE,
        DEFAULT_WORKING_MEMORY_PRIVATE_MODE, DEFAULT_WORKING_MEMORY_WEB_SEARCH_ENABLED,
        MIN_LLM_CONTEXT_WINDOW,
    },
    utils::paths,
};

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Default)]
pub enum AudioOutputMode {
    #[default]
    Speaker,
    Headset,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Default)]
#[serde(rename_all = "snake_case")]
pub enum VadBackendOption {
    /// Earshot — pure Rust, no ONNX dependency, embedded neural weights.
    Earshot,
    /// TenVAD — ONNX-based standard VAD engine.
    TenVad,
    /// SileroVad — MIT-licensed ONNX VAD engine with superior noise immunity.
    #[default]
    SileroVad,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Default)]
pub enum InteractionMode {
    #[default]
    Passive,
    PTT,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Default)]
#[serde(rename_all = "snake_case")]
pub enum DictationInteractionMode {
    Passive,
    #[default]
    Ptt,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Default)]
#[serde(rename_all = "snake_case")]
pub enum DictationOutputMode {
    #[default]
    Paste,
    Clipboard,
    Tray,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum PipelineMode {
    #[default]
    Modular,
    Realtime,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct VoiceProfile {
    pub id: i32,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gender: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub accent: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
}

/// Canonical capability-cache provider-kind labels. Writer (probe) and reader (session boot) must agree.
pub const CAP_KIND_EMBEDDED: &str = "embedded";
pub const CAP_KIND_OPENAI_COMPAT: &str = "openai_compat";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ModelCapabilities {
    pub model_id: String,
    pub provider_kind: String,
    pub supports_tools: bool,
    pub supports_latin: bool,
    pub supports_devanagari: bool,
    pub context_window: Option<u32>,
    pub max_output_tokens: Option<u32>,
    pub provenance: Option<String>,
    pub tps: Option<f32>,
    pub ttft_ms: Option<u32>,
    pub server_has_gpu: bool,
    pub is_gpu_accelerated: bool,
    pub gpu_status: String,
    pub vram_bytes: Option<u64>,
    pub parameter_size: Option<String>,
    pub quantization: Option<String>,
    pub family: Option<String>,
    pub tested_at_epoch: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LlmModelInfo {
    pub id: String,   // e.g. "gemma4:31b"
    pub name: String, // display name derived from id
    pub size_bytes: Option<u64>,
    pub quantization: Option<String>, // e.g. "Q4_K_M"
    pub family: Option<String>,       // e.g. "Gemma"
    pub provider_kind: String,        // e.g. "open_ai_compat", "embedded"
    pub capabilities: Option<ModelCapabilities>,
}


#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SettingReloadPolicy {
    Hot,
    WorkerCommand,
    Restart,
}

impl SettingReloadPolicy {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Hot => "hot",
            Self::WorkerCommand => "worker_command",
            Self::Restart => "restart",
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(default)]
pub struct AppearanceSettings {
    pub theme: String,
    pub accent_seed: String,
}

impl Default for AppearanceSettings {
    fn default() -> Self {
        Self {
            theme: DEFAULT_UI_THEME.into(),
            accent_seed: DEFAULT_UI_ACCENT_SEED.into(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(default)]
pub struct AudioSettings {
    pub output_mode: AudioOutputMode,
    pub input_device: Option<String>,
}

impl Default for AudioSettings {
    fn default() -> Self {
        Self {
            output_mode: AudioOutputMode::Speaker,
            input_device: None,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(default)]
pub struct VadSettings {
    pub threshold: f32,
    pub ptt_noise_gate: f32,
    pub vad_backend: VadBackendOption,
    pub silence_duration_ms: u32,
    pub speech_onset_ms: u32,
    pub max_speech_duration_s: u32,
}

impl Default for VadSettings {
    fn default() -> Self {
        Self {
            threshold: DEFAULT_VAD_THRESHOLD,
            ptt_noise_gate: DEFAULT_VAD_PTT_NOISE_GATE,
            vad_backend: VadBackendOption::SileroVad,
            silence_duration_ms: DEFAULT_VAD_SILENCE_DURATION_MS,
            speech_onset_ms: DEFAULT_VAD_SPEECH_ONSET_MS,
            max_speech_duration_s: DEFAULT_VAD_MAX_SPEECH_DURATION_S,
        }
    }
}

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
    pub project_id: Option<String>,
    pub endpoint: Option<String>,
}

impl Default for SttCloudConfig {
    fn default() -> Self {
        Self {
            provider: DEFAULT_STT_CLOUD_PROVIDER.to_string(),
            model: DEFAULT_STT_CLOUD_MODEL.to_string(),
            language: DEFAULT_STT_CLOUD_LANGUAGE.to_string(),
            region: DEFAULT_STT_CLOUD_REGION.to_string(),
            project_id: None,
            endpoint: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SttProviderConfig {
    #[serde(rename_all = "snake_case")]
    Embedded {
        #[serde(default = "default_stt_model")]
        model_type: String,
    },
    Cloud {
        provider: String,
        #[serde(default)]
        project_id: Option<String>,
        #[serde(default = "default_cloud_region")]
        region: String,
        #[serde(default = "default_cloud_model")]
        model: String,
        #[serde(default = "default_cloud_language")]
        language: String,
        #[serde(default)]
        endpoint: Option<String>,
    },
}

impl Default for SttProviderConfig {
    fn default() -> Self {
        SttProviderConfig::Embedded {
            model_type: default_stt_model(),
        }
    }
}

fn default_stt_model() -> String {
    DEFAULT_ASR_MODEL.into()
}
fn default_cloud_model() -> String {
    DEFAULT_STT_CLOUD_MODEL.into()
}
fn default_cloud_language() -> String {
    DEFAULT_STT_CLOUD_LANGUAGE.into()
}
fn default_cloud_region() -> String {
    DEFAULT_STT_CLOUD_REGION.into()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct SttSettings {
    pub active: SttActiveProvider,
    pub transliterate_enabled: bool,
    pub embedded: SttEmbeddedConfig,
    pub cloud: SttCloudConfig,
}

impl Default for SttSettings {
    fn default() -> Self {
        Self {
            active: SttActiveProvider::Embedded,
            transliterate_enabled: DEFAULT_ASR_TRANSLITERATE_ENABLED,
            embedded: SttEmbeddedConfig::default(),
            cloud: SttCloudConfig::default(),
        }
    }
}

impl SttSettings {
    pub fn active_model(&self) -> &str {
        match self.active {
            SttActiveProvider::Embedded => &self.embedded.model,
            SttActiveProvider::Cloud => &self.cloud.model,
        }
    }

    pub fn to_provider_config(&self) -> SttProviderConfig {
        match self.active {
            SttActiveProvider::Embedded => SttProviderConfig::Embedded {
                model_type: self.embedded.model.clone(),
            },
            SttActiveProvider::Cloud => SttProviderConfig::Cloud {
                provider: self.cloud.provider.clone(),
                project_id: self.cloud.project_id.clone(),
                region: self.cloud.region.clone(),
                model: self.cloud.model.clone(),
                language: self.cloud.language.clone(),
                endpoint: self.cloud.endpoint.clone(),
            },
        }
    }
}

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
}

impl Default for LlmEmbeddedConfig {
    fn default() -> Self {
        Self {
            model: DEFAULT_LLM_MODEL.to_string(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(default)]
pub struct LlmRemoteConfig {
    pub base_url: String,
    pub model: String,
    pub api_key: Option<String>,
    pub provider_name: Option<String>,
}

impl LlmRemoteConfig {
    pub fn server_default() -> Self {
        Self {
            base_url: DEFAULT_LLM_SERVER_BASE_URL.to_string(),
            model: DEFAULT_LLM_SERVER_MODEL.to_string(),
            api_key: None,
            provider_name: Some(DEFAULT_LLM_SERVER_PROVIDER_NAME.to_string()),
        }
    }

    pub fn cloud_default() -> Self {
        Self {
            base_url: DEFAULT_LLM_CLOUD_BASE_URL.to_string(),
            model: DEFAULT_LLM_CLOUD_MODEL.to_string(),
            api_key: None,
            provider_name: Some(DEFAULT_LLM_CLOUD_PROVIDER_NAME.to_string()),
        }
    }
}

impl Default for LlmRemoteConfig {
    fn default() -> Self {
        Self::server_default()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum LlmProviderConfig {
    #[default]
    Embedded,
    OpenAiCompat {
        base_url: String,
        model: String,
        api_key: Option<String>,
        #[serde(default)]
        provider_name: Option<String>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct LlmSettings {
    pub active: LlmActiveProvider,
    pub temperature: f32,
    pub compaction_temperature: f32,
    pub max_output_tokens: u32,
    pub context_window: u32,
    pub threads: u32,
    /// Voice-native default is off; reserved for future user-driven agentic opt-in.
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
            LlmActiveProvider::Server => LlmProviderConfig::OpenAiCompat {
                base_url: self.server.base_url.clone(),
                model: self.server.model.clone(),
                api_key: self.server.api_key.clone(),
                provider_name: self.server.provider_name.clone(),
            },
            LlmActiveProvider::Cloud => LlmProviderConfig::OpenAiCompat {
                base_url: self.cloud.base_url.clone(),
                model: self.cloud.model.clone(),
                api_key: self.cloud.api_key.clone(),
                provider_name: self.cloud.provider_name.clone(),
            },
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum TtsActiveProvider {
    EdgeTts,
    Supertonic,
    #[default]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(default)]
pub struct InteractionSettings {
    pub mode: InteractionMode,
    pub pipeline_mode: PipelineMode,
}

impl Default for InteractionSettings {
    fn default() -> Self {
        Self {
            mode: InteractionMode::Passive,
            pipeline_mode: PipelineMode::Modular,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(default)]
pub struct DictationSettings {
    pub enabled: bool,
    pub interaction_mode: DictationInteractionMode,
    pub hotkey: String,
    pub output_mode: DictationOutputMode,
    pub silence_auto_stop_ms: u64,
}

impl Default for DictationSettings {
    fn default() -> Self {
        Self {
            enabled: DEFAULT_DICTATION_ENABLED,
            interaction_mode: DictationInteractionMode::Ptt,
            hotkey: DEFAULT_DICTATION_HOTKEY.into(),
            output_mode: DictationOutputMode::Paste,
            silence_auto_stop_ms: DEFAULT_DICTATION_SILENCE_AUTO_STOP_MS,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(default)]
pub struct WorkingMemorySettings {
    pub private_mode: bool,
    pub auto_compaction: bool,
    pub max_context_share: f32,
    pub web_search_enabled: bool,
}

impl Default for WorkingMemorySettings {
    fn default() -> Self {
        Self {
            private_mode: DEFAULT_WORKING_MEMORY_PRIVATE_MODE,
            auto_compaction: DEFAULT_WORKING_MEMORY_AUTO_COMPACTION,
            max_context_share: DEFAULT_WORKING_MEMORY_MAX_CONTEXT_SHARE,
            web_search_enabled: DEFAULT_WORKING_MEMORY_WEB_SEARCH_ENABLED,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(default)]
pub struct PersonalMemorySettings {
    pub context_retrieval_enabled: bool,
    pub pipeline_processing_enabled: bool,
    pub top_k_facts: u32,
    pub semantic_similarity_cutoff: f32,
    pub consolidation_cadence: String,
    pub consolidation_time: String,
}

impl Default for PersonalMemorySettings {
    fn default() -> Self {
        Self {
            context_retrieval_enabled: DEFAULT_PERSONAL_MEMORY_CONTEXT_RETRIEVAL_ENABLED,
            pipeline_processing_enabled: DEFAULT_PERSONAL_MEMORY_PIPELINE_PROCESSING_ENABLED,
            top_k_facts: DEFAULT_PERSONAL_MEMORY_TOP_K_FACTS,
            semantic_similarity_cutoff: DEFAULT_PERSONAL_MEMORY_SEMANTIC_SIMILARITY_CUTOFF,
            consolidation_cadence: DEFAULT_PERSONAL_MEMORY_CONSOLIDATION_CADENCE.to_string(),
            consolidation_time: DEFAULT_PERSONAL_MEMORY_CONSOLIDATION_TIME.to_string(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(default)]
pub struct PersonaSettings {
    pub modular_prompt: String,
    pub realtime_prompt: String,
}

impl Default for PersonaSettings {
    fn default() -> Self {
        Self {
            modular_prompt: DEFAULT_SYSTEM_PROMPT_MODULAR.into(),
            realtime_prompt: DEFAULT_SYSTEM_PROMPT_REALTIME.into(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum RealtimeProviderKind {
    #[default]
    GeminiLive,
    OpenAiRealtime,
    DeepgramVoiceAgent,
    ElevenLabsConvai,
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(default)]
pub struct OpenAiRealtimeConfig {
    pub api_key: String,
    pub model: String,
    pub voice: String,
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(default)]
pub struct ElevenLabsConvaiConfig {
    pub api_key: String,
    pub agent_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct RealtimeSettings {
    #[serde(alias = "provider")]
    pub active: RealtimeProviderKind,
    #[serde(alias = "gemini")]
    pub gemini_live: GeminiRealtimeConfig,
    #[serde(alias = "openai")]
    pub openai_realtime: OpenAiRealtimeConfig,
    #[serde(alias = "deepgram")]
    pub deepgram_voice_agent: DeepgramVoiceAgentConfig,
    #[serde(alias = "elevenlabs")]
    pub elevenlabs_convai: ElevenLabsConvaiConfig,
}

impl Default for RealtimeSettings {
    fn default() -> Self {
        Self {
            active: RealtimeProviderKind::GeminiLive,
            gemini_live: GeminiRealtimeConfig::default(),
            openai_realtime: OpenAiRealtimeConfig::default(),
            deepgram_voice_agent: DeepgramVoiceAgentConfig::default(),
            elevenlabs_convai: ElevenLabsConvaiConfig::default(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(default)]
pub struct SystemSettings {
    pub setup_completed: bool,
}
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(default)]
pub struct VoxSettings {
    pub audio: AudioSettings,
    pub vad: VadSettings,
    pub stt: SttSettings,
    pub llm: LlmSettings,
    pub tts: TtsSettings,
    pub realtime: RealtimeSettings,
    pub interaction: InteractionSettings,
    pub dictation: DictationSettings,
    pub working_memory: WorkingMemorySettings,
    pub appearance: AppearanceSettings,
    pub personal_memory: PersonalMemorySettings,
    pub persona: PersonaSettings,
    pub system: SystemSettings,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(default)]
pub struct SttWiringSettings {
    pub active: SttActiveProvider,
    pub transliterate_enabled: bool,
    pub embedded: SttEmbeddedConfig,
}

impl Default for SttWiringSettings {
    fn default() -> Self {
        Self {
            active: SttActiveProvider::Embedded,
            transliterate_enabled: DEFAULT_ASR_TRANSLITERATE_ENABLED,
            embedded: SttEmbeddedConfig::default(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(default)]
pub struct LlmWiringSettings {
    pub active: LlmActiveProvider,
    pub threads: u32,
    pub embedded: LlmEmbeddedConfig,
}

impl Default for LlmWiringSettings {
    fn default() -> Self {
        Self {
            active: LlmActiveProvider::Embedded,
            threads: DEFAULT_LLM_THREADS,
            embedded: LlmEmbeddedConfig::default(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(default)]
pub struct TtsWiringSettings {
    pub active: TtsActiveProvider,
    #[serde(alias = "voice")]
    pub voice_index: i32,
    pub speed: f32,
    pub threads: u32,
    pub supertonic: TtsSupertonicConfig,
    pub kokoro: TtsKokoroConfig,
}

impl Default for TtsWiringSettings {
    fn default() -> Self {
        Self {
            active: TtsActiveProvider::Kokoro,
            voice_index: DEFAULT_TTS_VOICE_INDEX,
            speed: DEFAULT_TTS_SPEED,
            threads: DEFAULT_TTS_THREADS,
            supertonic: TtsSupertonicConfig::default(),
            kokoro: TtsKokoroConfig::default(),
        }
    }
}

/// Tier 1: Hardware, Audio I/O, VAD, Theme, Active Engine Wiring (`config/settings.json`)
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(default)]
pub struct SettingsConfigFile {
    pub audio: AudioSettings,
    pub vad: VadSettings,
    pub appearance: AppearanceSettings,
    pub interaction: InteractionSettings,
    pub dictation: DictationSettings,
    pub system: SystemSettings,
    pub stt: SttWiringSettings,
    pub llm: LlmWiringSettings,
    pub tts: TtsWiringSettings,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(default)]
pub struct SttProvidersConfig {
    pub cloud: SttCloudConfig,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(default)]
pub struct LlmProvidersConfig {
    pub server: LlmRemoteConfig,
    pub cloud: LlmRemoteConfig,
    #[serde(default)]
    pub cloud_keys: HashMap<String, String>,
}

impl Default for LlmProvidersConfig {
    fn default() -> Self {
        Self {
            server: LlmRemoteConfig::server_default(),
            cloud: LlmRemoteConfig::cloud_default(),
            cloud_keys: HashMap::new(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(default)]
pub struct TtsProvidersConfig {
    pub edge_tts: TtsEdgeConfig,
    pub chatterbox: TtsChatterboxConfig,
    pub chatterbox_remote: TtsChatterboxRemoteConfig,
    pub zipvoice: TtsZipvoiceConfig,
}

/// Tier 2: Cloud/Server Endpoints, Model IDs, API Keys (`config/providers.json`, chmod 0600)
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(default)]
pub struct ProvidersConfigFile {
    pub llm: LlmProvidersConfig,
    pub stt: SttProvidersConfig,
    pub tts: TtsProvidersConfig,
    pub realtime: RealtimeSettings,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(default)]
pub struct CognitiveSettings {
    pub temperature: f32,
    pub compaction_temperature: f32,
    pub max_output_tokens: u32,
    pub context_window: u32,
    pub reasoning_enabled: bool,
}

impl Default for CognitiveSettings {
    fn default() -> Self {
        Self {
            temperature: DEFAULT_LLM_TEMPERATURE,
            compaction_temperature: DEFAULT_LLM_COMPACTION_TEMPERATURE,
            max_output_tokens: DEFAULT_LLM_MAX_OUTPUT_TOKENS,
            context_window: DEFAULT_LLM_CONTEXT_WINDOW,
            reasoning_enabled: false,
        }
    }
}

/// Tier 3: Cognitive Persona, Prompts, Tools, MCP, Memory Policy (`config/agent.json`)
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(default)]
pub struct AgentConfigFile {
    pub persona: PersonaSettings,
    pub working_memory: WorkingMemorySettings,
    pub personal_memory: PersonalMemorySettings,
    pub cognitive: CognitiveSettings,
}

pub fn get_setting_reload_policy(domain: &str, key: &str) -> SettingReloadPolicy {
    match domain {
        "appearance" | "working_memory" | "personal_memory" | "persona" | "realtime" => {
            SettingReloadPolicy::Hot
        }
        "tts" if key == "speed" || key == "voice_index" || key == "voice" => {
            SettingReloadPolicy::WorkerCommand
        }
        "llm"
            if key == "temperature"
                || key == "compaction_temperature"
                || key == "reasoning_enabled"
                || key == "max_output_tokens"
                || key == "cloud_keys" =>
        {
            SettingReloadPolicy::Hot
        }
        "vad"
            if key == "threshold"
                || key == "ptt_noise_gate"
                || key == "silence_duration_ms"
                || key == "speech_onset_ms"
                || key == "max_speech_duration_s" =>
        {
            SettingReloadPolicy::WorkerCommand
        }
        "stt" if key == "transliterate_enabled" || key == "partial_throttle_ms" => {
            SettingReloadPolicy::Hot
        }
        "stt" if key == "threads" => SettingReloadPolicy::Restart,
        "tts" if key == "threads" => SettingReloadPolicy::Restart,
        "dictation" => SettingReloadPolicy::Hot,
        "system" if key == "setup_completed" => {
            SettingReloadPolicy::Hot
        }
        _ => SettingReloadPolicy::Restart,
    }
}

fn atomic_write_json<T: Serialize>(path: &Path, value: &T, mode: Option<u32>) -> Result<()> {
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let content = serde_json::to_string_pretty(value)?;
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let tmp_path = path.with_file_name(format!(
        "{}.{}.tmp",
        path.file_name().and_then(|f| f.to_str()).unwrap_or("file"),
        nanos
    ));
    {
        let mut file = match fs::File::create(&tmp_path) {
            Ok(f) => f,
            Err(e) => {
                log::warn!("[Settings] Failed to create tmp file {:?}: {}", tmp_path, e);
                return Err(e.into());
            }
        };
        #[cfg(unix)]
        if let Some(m) = mode {
            use std::os::unix::fs::PermissionsExt;
            let _ = file.set_permissions(fs::Permissions::from_mode(m));
        }
        if let Err(e) = file.write_all(content.as_bytes()) {
            let _ = fs::remove_file(&tmp_path);
            return Err(e.into());
        }
        if let Err(e) = file.sync_all() {
            log::warn!("[Settings] Failed to fsync tmp file: {}", e);
        }
    }
    if let Err(e) = fs::rename(&tmp_path, path) {
        let _ = fs::remove_file(&tmp_path);
        return Err(e.into());
    }
    #[cfg(unix)]
    if let Some(m) = mode {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(path, fs::Permissions::from_mode(m));
    }
    Ok(())
}

impl VoxSettings {
    /// Strips the retired `zipvoice_voice_` id prefix from the stored voice
    /// selection. Schema v9 stores bare slugs; older files carry prefixes.
    fn normalize_retired_voice_ids(&mut self) {
        if let Some(id) = self.tts.zipvoice.voice_id.as_mut() {
            if let Some(slug) = id.strip_prefix("zipvoice_voice_").map(|s| s.to_string()) {
                *id = slug;
            }
        }
    }

    pub fn to_settings_config(&self) -> SettingsConfigFile {
        SettingsConfigFile {
            audio: self.audio.clone(),
            vad: self.vad.clone(),
            appearance: self.appearance.clone(),
            interaction: self.interaction.clone(),
            dictation: self.dictation.clone(),
            system: self.system.clone(),
            stt: SttWiringSettings {
                active: self.stt.active,
                transliterate_enabled: self.stt.transliterate_enabled,
                embedded: self.stt.embedded.clone(),
            },
            llm: LlmWiringSettings {
                active: self.llm.active,
                threads: self.llm.threads,
                embedded: self.llm.embedded.clone(),
            },
            tts: TtsWiringSettings {
                active: self.tts.active,
                voice_index: self.tts.voice_index,
                speed: self.tts.speed,
                threads: self.tts.threads,
                supertonic: self.tts.supertonic.clone(),
                kokoro: self.tts.kokoro.clone(),
            },
        }
    }

    pub fn to_providers_config(&self) -> ProvidersConfigFile {
        ProvidersConfigFile {
            llm: LlmProvidersConfig {
                server: self.llm.server.clone(),
                cloud: self.llm.cloud.clone(),
                cloud_keys: self.llm.cloud_keys.clone(),
            },
            stt: SttProvidersConfig {
                cloud: self.stt.cloud.clone(),
            },
            tts: TtsProvidersConfig {
                edge_tts: self.tts.edge_tts.clone(),
                chatterbox: self.tts.chatterbox.clone(),
                chatterbox_remote: self.tts.chatterbox_remote.clone(),
                zipvoice: self.tts.zipvoice.clone(),
            },
            realtime: self.realtime.clone(),
        }
    }

    pub fn to_agent_config(&self) -> AgentConfigFile {
        AgentConfigFile {
            persona: self.persona.clone(),
            working_memory: self.working_memory.clone(),
            personal_memory: self.personal_memory.clone(),
            cognitive: CognitiveSettings {
                temperature: self.llm.temperature,
                compaction_temperature: self.llm.compaction_temperature,
                max_output_tokens: self.llm.max_output_tokens,
                context_window: self.llm.context_window,
                reasoning_enabled: self.llm.reasoning_enabled,
            },
        }
    }

    pub fn merge_settings_config(&mut self, cfg: SettingsConfigFile) {
        self.audio = cfg.audio;
        self.vad = cfg.vad;
        self.appearance = cfg.appearance;
        self.interaction = cfg.interaction;
        self.dictation = cfg.dictation;
        self.system = cfg.system;
        self.stt.active = cfg.stt.active;
        self.stt.transliterate_enabled = cfg.stt.transliterate_enabled;
        self.stt.embedded = cfg.stt.embedded;
        self.llm.active = cfg.llm.active;
        self.llm.threads = cfg.llm.threads;
        self.llm.embedded = cfg.llm.embedded;
        self.tts.active = cfg.tts.active;
        self.tts.voice_index = cfg.tts.voice_index;
        self.tts.speed = cfg.tts.speed;
        self.tts.threads = cfg.tts.threads;
        self.tts.supertonic = cfg.tts.supertonic;
        self.tts.kokoro = cfg.tts.kokoro;
    }

    pub fn merge_providers_config(&mut self, cfg: ProvidersConfigFile) {
        self.llm.server = cfg.llm.server;
        self.llm.cloud = cfg.llm.cloud;
        self.llm.cloud_keys = cfg.llm.cloud_keys;
        self.stt.cloud = cfg.stt.cloud;
        self.tts.edge_tts = cfg.tts.edge_tts;
        self.tts.chatterbox = cfg.tts.chatterbox;
        self.tts.chatterbox_remote = cfg.tts.chatterbox_remote;
        self.tts.zipvoice = cfg.tts.zipvoice;
        self.realtime = cfg.realtime;
    }

    pub fn merge_agent_config(&mut self, cfg: AgentConfigFile) {
        self.persona = cfg.persona;
        self.working_memory = cfg.working_memory;
        self.personal_memory = cfg.personal_memory;
        self.llm.temperature = cfg.cognitive.temperature;
        self.llm.compaction_temperature = cfg.cognitive.compaction_temperature;
        self.llm.max_output_tokens = cfg.cognitive.max_output_tokens;
        self.llm.context_window = cfg.cognitive.context_window;
        self.llm.reasoning_enabled = cfg.cognitive.reasoning_enabled;
    }

    fn recover_settings_sections(&mut self, val: &serde_json::Value) {
        if let Some(obj) = val.as_object() {
            if let Some(v) = obj.get("audio").and_then(|v| serde_json::from_value(v.clone()).ok()) {
                self.audio = v;
            }
            if let Some(v) = obj.get("vad").and_then(|v| serde_json::from_value(v.clone()).ok()) {
                self.vad = v;
            }
            if let Some(v) = obj.get("appearance").and_then(|v| serde_json::from_value(v.clone()).ok()) {
                self.appearance = v;
            }
            if let Some(v) = obj.get("interaction").and_then(|v| serde_json::from_value(v.clone()).ok()) {
                self.interaction = v;
            }
            if let Some(v) = obj.get("dictation").and_then(|v| serde_json::from_value(v.clone()).ok()) {
                self.dictation = v;
            }
            if let Some(v) = obj.get("system").and_then(|v| serde_json::from_value(v.clone()).ok()) {
                self.system = v;
            }
            if let Some(v) = obj.get("stt").and_then(|v| serde_json::from_value::<SttWiringSettings>(v.clone()).ok()) {
                self.stt.active = v.active;
                self.stt.transliterate_enabled = v.transliterate_enabled;
                self.stt.embedded = v.embedded;
            }
            if let Some(v) = obj.get("llm").and_then(|v| serde_json::from_value::<LlmWiringSettings>(v.clone()).ok()) {
                self.llm.active = v.active;
                self.llm.threads = v.threads;
                self.llm.embedded = v.embedded;
            }
            if let Some(v) = obj.get("tts").and_then(|v| serde_json::from_value::<TtsWiringSettings>(v.clone()).ok()) {
                self.tts.active = v.active;
                self.tts.voice_index = v.voice_index;
                self.tts.speed = v.speed;
                self.tts.threads = v.threads;
                self.tts.supertonic = v.supertonic;
                self.tts.kokoro = v.kokoro;
            }
        }
    }

    fn recover_providers_sections(&mut self, val: &serde_json::Value) {
        if let Some(obj) = val.as_object() {
            if let Some(v) = obj.get("llm").and_then(|v| serde_json::from_value::<LlmProvidersConfig>(v.clone()).ok()) {
                self.llm.server = v.server;
                self.llm.cloud = v.cloud;
                self.llm.cloud_keys = v.cloud_keys;
            }
            if let Some(v) = obj.get("stt").and_then(|v| serde_json::from_value::<SttProvidersConfig>(v.clone()).ok()) {
                self.stt.cloud = v.cloud;
            }
            if let Some(v) = obj.get("tts").and_then(|v| serde_json::from_value::<TtsProvidersConfig>(v.clone()).ok()) {
                self.tts.edge_tts = v.edge_tts;
                self.tts.chatterbox = v.chatterbox;
                self.tts.chatterbox_remote = v.chatterbox_remote;
                self.tts.zipvoice = v.zipvoice;
            }
            if let Some(v) = obj.get("realtime").and_then(|v| serde_json::from_value(v.clone()).ok()) {
                self.realtime = v;
            }
        }
    }

    fn recover_agent_sections(&mut self, val: &serde_json::Value) {
        if let Some(obj) = val.as_object() {
            if let Some(v) = obj.get("persona").and_then(|v| serde_json::from_value(v.clone()).ok()) {
                self.persona = v;
            }
            if let Some(v) = obj.get("working_memory").and_then(|v| serde_json::from_value(v.clone()).ok()) {
                self.working_memory = v;
            }
            if let Some(v) = obj.get("personal_memory").and_then(|v| serde_json::from_value(v.clone()).ok()) {
                self.personal_memory = v;
            }
            if let Some(v) = obj.get("cognitive").and_then(|v| serde_json::from_value::<CognitiveSettings>(v.clone()).ok()) {
                self.llm.temperature = v.temperature;
                self.llm.compaction_temperature = v.compaction_temperature;
                self.llm.max_output_tokens = v.max_output_tokens;
                self.llm.context_window = v.context_window;
                self.llm.reasoning_enabled = v.reasoning_enabled;
            }
        }
    }

    fn recover_all_sections(&mut self, val: &serde_json::Value) {
        if let Some(obj) = val.as_object() {
            if let Some(v) = obj.get("audio").and_then(|v| serde_json::from_value(v.clone()).ok()) {
                self.audio = v;
            }
            if let Some(v) = obj.get("vad").and_then(|v| serde_json::from_value(v.clone()).ok()) {
                self.vad = v;
            }
            if let Some(v) = obj.get("stt").and_then(|v| serde_json::from_value(v.clone()).ok()) {
                self.stt = v;
            }
            if let Some(v) = obj.get("llm").and_then(|v| serde_json::from_value(v.clone()).ok()) {
                self.llm = v;
            }
            if let Some(v) = obj.get("tts").and_then(|v| serde_json::from_value(v.clone()).ok()) {
                self.tts = v;
            }
            if let Some(v) = obj.get("realtime").and_then(|v| serde_json::from_value(v.clone()).ok()) {
                self.realtime = v;
            }
            if let Some(v) = obj.get("interaction").and_then(|v| serde_json::from_value(v.clone()).ok()) {
                self.interaction = v;
            }
            if let Some(v) = obj.get("dictation").and_then(|v| serde_json::from_value(v.clone()).ok()) {
                self.dictation = v;
            }
            if let Some(v) = obj.get("working_memory").and_then(|v| serde_json::from_value(v.clone()).ok()) {
                self.working_memory = v;
            }
            if let Some(v) = obj.get("appearance").and_then(|v| serde_json::from_value(v.clone()).ok()) {
                self.appearance = v;
            }
            if let Some(v) = obj.get("personal_memory").and_then(|v| serde_json::from_value(v.clone()).ok()) {
                self.personal_memory = v;
            }
            if let Some(v) = obj.get("persona").and_then(|v| serde_json::from_value(v.clone()).ok()) {
                self.persona = v;
            }
            if let Some(v) = obj.get("system").and_then(|v| serde_json::from_value(v.clone()).ok()) {
                self.system = v;
            }
        }
    }

    fn backup_corrupt(path: &Path, prefix: &str) {
        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let bak = path.with_file_name(format!("{}.corrupt.{}.jsonc", prefix, ts));
        log::error!(
            "[Settings] Corrupt {} — backing up to {:?} and restoring in-memory defaults",
            path.display(),
            bak
        );
        if let Err(e) = fs::rename(path, &bak) {
            log::warn!("[Settings] Failed to backup corrupt file: {}", e);
        }
    }

    pub fn load() -> Self {
        let settings_path = paths::settings_path();
        let providers_path = paths::providers_path();
        let agent_path = paths::agent_path();

        // 1. If decomposed files exist (providers.jsonc or agent.jsonc)
        if providers_path.exists() || agent_path.exists() {
            let mut settings = Self::default();

            // Load settings.jsonc
            if let Ok(content) = fs::read_to_string(&settings_path) {
                if let Ok(cfg) = crate::utils::jsonc::from_jsonc_str::<SettingsConfigFile>(&content) {
                    settings.merge_settings_config(cfg);
                } else if let Ok(val) = crate::utils::jsonc::from_jsonc_str::<serde_json::Value>(&content) {
                    log::warn!("[Settings] Partial corruption in settings.jsonc — recovering sections.");
                    settings.recover_settings_sections(&val);
                } else {
                    Self::backup_corrupt(&settings_path, "settings");
                }
            }

            // Load providers.jsonc
            if let Ok(content) = fs::read_to_string(&providers_path) {
                if let Ok(cfg) = crate::utils::jsonc::from_jsonc_str::<ProvidersConfigFile>(&content) {
                    settings.merge_providers_config(cfg);
                } else if let Ok(val) = crate::utils::jsonc::from_jsonc_str::<serde_json::Value>(&content) {
                    log::warn!("[Settings] Partial corruption in providers.jsonc — recovering sections.");
                    settings.recover_providers_sections(&val);
                } else {
                    Self::backup_corrupt(&providers_path, "providers");
                }
            }

            // Load agent.jsonc
            if let Ok(content) = fs::read_to_string(&agent_path) {
                if let Ok(cfg) = crate::utils::jsonc::from_jsonc_str::<AgentConfigFile>(&content) {
                    settings.merge_agent_config(cfg);
                } else if let Ok(val) = crate::utils::jsonc::from_jsonc_str::<serde_json::Value>(&content) {
                    log::warn!("[Settings] Partial corruption in agent.jsonc — recovering sections.");
                    settings.recover_agent_sections(&val);
                } else {
                    Self::backup_corrupt(&agent_path, "agent");
                }
            }

            settings.normalize_retired_voice_ids();
            log::info!("[Settings] Loaded 3-way decomposed configuration from config/ (settings.jsonc, providers.jsonc, agent.jsonc)");
            return settings;
        }

        // 2. Monolithic legacy load or first-boot migration
        if let Ok(content) = fs::read_to_string(&settings_path) {
            // Clean monolithic deserialization
            if let Ok(mut settings) = crate::utils::jsonc::from_jsonc_str::<Self>(&content) {
                settings.normalize_retired_voice_ids();
                log::info!("[Settings] Loaded legacy monolithic configuration from {:?} — decomposing into 3-way split", settings_path);
                let _ = settings.save();
                return settings;
            }

            // Layered recovery path for monolithic
            if let Ok(val) = crate::utils::jsonc::from_jsonc_str::<serde_json::Value>(&content) {
                log::warn!("[Settings] Partial corruption or schema drift detected in settings.jsonc — attempting section recovery.");
                let mut settings = Self::default();
                settings.recover_all_sections(&val);
                settings.normalize_retired_voice_ids();
                let _ = settings.save();
                return settings;
            }

            // Total JSON parse failure
            Self::backup_corrupt(&settings_path, "settings");
        }

        log::info!("[Settings] No valid settings found. Using in-memory system defaults.");
        Self::default()
    }

    pub fn save(&self) -> Result<()> {
        let settings_path = paths::settings_path();
        let providers_path = paths::providers_path();
        let agent_path = paths::agent_path();

        let settings_cfg = self.to_settings_config();
        let providers_cfg = self.to_providers_config();
        let agent_cfg = self.to_agent_config();

        atomic_write_json(&settings_path, &settings_cfg, None)?;
        #[cfg(unix)]
        atomic_write_json(&providers_path, &providers_cfg, Some(0o600))?;
        #[cfg(not(unix))]
        atomic_write_json(&providers_path, &providers_cfg, None)?;
        atomic_write_json(&agent_path, &agent_cfg, None)?;

        Ok(())
    }
}

pub fn get_preset_colors() -> Vec<String> {
    vec![
        "#00DBE9".to_string(),
        "#8B5CF6".to_string(),
        "#EC4899".to_string(),
        "#F59E0B".to_string(),
        "#10B981".to_string(),
    ]
}

