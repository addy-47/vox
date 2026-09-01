use crate::utils::paths;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fs;

// ─── Shared Enums ─────────────────────────────────────────────────────────────

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
    /// TenVAD — ONNX-based standard VAD engine. Default engine for Vox.
    #[default]
    TenVad,
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

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Default)]
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
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ModelCapabilities {
    pub model_id: String,
    pub provider_kind: String,
    pub supports_tools: bool,
    pub supports_latin: bool,
    pub supports_devanagari: bool,
    pub context_window: Option<u32>,
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

pub fn get_voice_profiles() -> Vec<VoiceProfile> {
    vec![
        VoiceProfile {
            id: 0,
            name: "James".to_string(),
        },
        VoiceProfile {
            id: 1,
            name: "David".to_string(),
        },
        VoiceProfile {
            id: 2,
            name: "Alex".to_string(),
        },
        VoiceProfile {
            id: 3,
            name: "Ryan".to_string(),
        },
        VoiceProfile {
            id: 4,
            name: "Ethan".to_string(),
        },
        VoiceProfile {
            id: 5,
            name: "Sophia".to_string(),
        },
        VoiceProfile {
            id: 6,
            name: "Olivia".to_string(),
        },
        VoiceProfile {
            id: 7,
            name: "Emma".to_string(),
        },
        VoiceProfile {
            id: 8,
            name: "Ava".to_string(),
        },
        VoiceProfile {
            id: 9,
            name: "Mia".to_string(),
        },
    ]
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

// ─── Reload Policy ────────────────────────────────────────────────────────────

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

pub fn get_setting_reload_policy(domain: &str, key: &str) -> SettingReloadPolicy {
    match domain {
        "appearance" | "memory" | "persona" | "history" | "realtime" => SettingReloadPolicy::Hot,
        "tts"
            if key == "quality_steps"
                || key == "speed"
                || key == "voice_index"
                || key == "voice" =>
        {
            SettingReloadPolicy::WorkerCommand
        }
        "llm"
            if key == "temperature"
                || key == "compaction_temperature"
                || key == "max_output_tokens" =>
        {
            SettingReloadPolicy::Hot
        }
        "vad" if key == "threshold" || key == "ptt_noise_gate" => SettingReloadPolicy::Hot,
        "stt" if key == "transliterate_enabled" => SettingReloadPolicy::Hot,
        "interaction" if key == "auto_sleep_timeout" => SettingReloadPolicy::Hot,
        "system" if key == "telemetry_enabled" || key == "setup_completed" => {
            SettingReloadPolicy::Hot
        }
        _ => SettingReloadPolicy::Restart,
    }
}

// ─── 1. Appearance Settings ───────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(default)]
pub struct AppearanceSettings {
    pub theme: String,
    pub accent_seed: String,
}

impl Default for AppearanceSettings {
    fn default() -> Self {
        Self {
            theme: crate::core::defaults::DEFAULT_UI_THEME.into(),
            accent_seed: crate::core::defaults::DEFAULT_UI_ACCENT_SEED.into(),
        }
    }
}

// ─── 2. Audio Settings ────────────────────────────────────────────────────────

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

// ─── 3. VAD Settings ──────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(default)]
pub struct VadSettings {
    pub threshold: f32,
    pub ptt_noise_gate: f32,
    pub vad_backend: VadBackendOption,
}

impl Default for VadSettings {
    fn default() -> Self {
        Self {
            threshold: crate::core::defaults::DEFAULT_VAD_THRESHOLD,
            ptt_noise_gate: crate::core::defaults::DEFAULT_VAD_PTT_NOISE_GATE,
            vad_backend: VadBackendOption::TenVad,
        }
    }
}

// ─── 4. STT Settings (Parallel Providers) ─────────────────────────────────────

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
}

impl Default for SttEmbeddedConfig {
    fn default() -> Self {
        Self {
            model: crate::core::defaults::DEFAULT_ASR_MODEL.to_string(),
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
    pub credentials_path: Option<String>,
    pub credentials_json: Option<String>,
    pub project_id: Option<String>,
    pub endpoint: Option<String>,
}

impl Default for SttCloudConfig {
    fn default() -> Self {
        Self {
            provider: crate::core::defaults::DEFAULT_STT_CLOUD_PROVIDER.to_string(),
            model: crate::core::defaults::DEFAULT_STT_CLOUD_MODEL.to_string(),
            language: crate::core::defaults::DEFAULT_STT_CLOUD_LANGUAGE.to_string(),
            region: crate::core::defaults::DEFAULT_STT_CLOUD_REGION.to_string(),
            credentials_path: None,
            credentials_json: None,
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
        credentials_path: Option<String>,
        #[serde(default)]
        credentials_json: Option<String>,
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
    crate::core::defaults::DEFAULT_ASR_MODEL.into()
}
fn default_cloud_model() -> String {
    crate::core::defaults::DEFAULT_STT_CLOUD_MODEL.into()
}
fn default_cloud_language() -> String {
    crate::core::defaults::DEFAULT_STT_CLOUD_LANGUAGE.into()
}
fn default_cloud_region() -> String {
    crate::core::defaults::DEFAULT_STT_CLOUD_REGION.into()
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
            transliterate_enabled: crate::core::defaults::DEFAULT_ASR_TRANSLITERATE_ENABLED,
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
                credentials_path: self.cloud.credentials_path.clone(),
                credentials_json: self.cloud.credentials_json.clone(),
                project_id: self.cloud.project_id.clone(),
                region: self.cloud.region.clone(),
                model: self.cloud.model.clone(),
                language: self.cloud.language.clone(),
                endpoint: self.cloud.endpoint.clone(),
            },
        }
    }
}

// ─── 5. LLM Settings (Parallel Providers) ─────────────────────────────────────

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
            model: crate::core::defaults::DEFAULT_LLM_MODEL.to_string(),
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
            base_url: crate::core::defaults::DEFAULT_LLM_SERVER_BASE_URL.to_string(),
            model: crate::core::defaults::DEFAULT_LLM_SERVER_MODEL.to_string(),
            api_key: None,
            provider_name: Some(
                crate::core::defaults::DEFAULT_LLM_SERVER_PROVIDER_NAME.to_string(),
            ),
        }
    }

    pub fn cloud_default() -> Self {
        Self {
            base_url: crate::core::defaults::DEFAULT_LLM_CLOUD_BASE_URL.to_string(),
            model: crate::core::defaults::DEFAULT_LLM_CLOUD_MODEL.to_string(),
            api_key: None,
            provider_name: Some(crate::core::defaults::DEFAULT_LLM_CLOUD_PROVIDER_NAME.to_string()),
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
    pub embedded: LlmEmbeddedConfig,
    pub server: LlmRemoteConfig,
    pub cloud: LlmRemoteConfig,
}

impl Default for LlmSettings {
    fn default() -> Self {
        Self {
            active: LlmActiveProvider::Embedded,
            temperature: crate::core::defaults::DEFAULT_LLM_TEMPERATURE,
            compaction_temperature: crate::core::defaults::DEFAULT_LLM_COMPACTION_TEMPERATURE,
            max_output_tokens: crate::core::defaults::DEFAULT_LLM_MAX_OUTPUT_TOKENS,
            context_window: crate::core::defaults::DEFAULT_LLM_CONTEXT_WINDOW,
            threads: crate::core::defaults::DEFAULT_LLM_THREADS,
            embedded: LlmEmbeddedConfig::default(),
            server: LlmRemoteConfig::server_default(),
            cloud: LlmRemoteConfig::cloud_default(),
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
        self.context_window
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

// ─── 6. TTS Settings (Parallel Providers) ─────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum TtsActiveProvider {
    #[default]
    EdgeTts,
    Supertonic,
    Kokoro,
    Chatterbox,
    ChatterboxRemote,
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TtsProviderConfig {
    Supertonic,
    Kokoro,
    Chatterbox {
        language: String,
        quality_steps: u32,
        speed: f32,
        #[serde(default)]
        voice_id: Option<String>,
    },
    ChatterboxRemote {
        endpoint: String,
        language: String,
        quality_steps: u32,
        speed: f32,
        remote_path: String,
        #[serde(default)]
        voice_id: Option<String>,
    },
    EdgeTts {
        #[serde(default)]
        voice: Option<String>,
    },
}

impl Default for TtsProviderConfig {
    fn default() -> Self {
        TtsProviderConfig::EdgeTts { voice: None }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct TtsSettings {
    pub active: TtsActiveProvider,
    #[serde(alias = "voice")]
    pub voice_index: i32,
    pub quality_steps: u32,
    pub speed: f32,
    pub edge_tts: TtsEdgeConfig,
    pub supertonic: TtsSupertonicConfig,
    pub kokoro: TtsKokoroConfig,
    pub chatterbox: TtsChatterboxConfig,
    pub chatterbox_remote: TtsChatterboxRemoteConfig,
}

impl Default for TtsSettings {
    fn default() -> Self {
        Self {
            active: TtsActiveProvider::EdgeTts,
            voice_index: crate::core::defaults::DEFAULT_TTS_VOICE_INDEX,
            quality_steps: crate::core::defaults::DEFAULT_TTS_QUALITY_STEPS,
            speed: crate::core::defaults::DEFAULT_TTS_SPEED,
            edge_tts: TtsEdgeConfig::default(),
            supertonic: TtsSupertonicConfig::default(),
            kokoro: TtsKokoroConfig::default(),
            chatterbox: TtsChatterboxConfig::default(),
            chatterbox_remote: TtsChatterboxRemoteConfig::default(),
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
                quality_steps: self.quality_steps,
                speed: self.speed,
                voice_id: self.chatterbox.voice_id.clone(),
            },
            TtsActiveProvider::ChatterboxRemote => TtsProviderConfig::ChatterboxRemote {
                endpoint: self.chatterbox_remote.endpoint.clone(),
                language: self.chatterbox_remote.language.clone(),
                quality_steps: self.quality_steps,
                speed: self.speed,
                remote_path: self.chatterbox_remote.remote_path.clone(),
                voice_id: self.chatterbox_remote.voice_id.clone(),
            },
        }
    }
}

// ─── 7. Interaction Settings ──────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(default)]
pub struct InteractionSettings {
    pub mode: InteractionMode,
    pub auto_sleep_timeout: u32,
    pub pipeline_mode: PipelineMode,
}

impl Default for InteractionSettings {
    fn default() -> Self {
        Self {
            mode: InteractionMode::Passive,
            auto_sleep_timeout: crate::core::defaults::DEFAULT_AUTO_SLEEP_TIMEOUT,
            pipeline_mode: PipelineMode::Modular,
        }
    }
}

// ─── 8. Dictation Settings ────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(default)]
pub struct DictationSettings {
    pub enabled: bool,
    pub interaction_mode: DictationInteractionMode,
    pub hotkey: String,
    pub output_mode: DictationOutputMode,
}

impl Default for DictationSettings {
    fn default() -> Self {
        Self {
            enabled: crate::core::defaults::DEFAULT_DICTATION_ENABLED,
            interaction_mode: DictationInteractionMode::Ptt,
            hotkey: crate::core::defaults::DEFAULT_DICTATION_HOTKEY.into(),
            output_mode: DictationOutputMode::Paste,
        }
    }
}

// ─── 9. History Settings ──────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(default)]
pub struct HistorySettings {
    pub private_mode: bool,
    pub tray_history_limit: u32,
}

impl Default for HistorySettings {
    fn default() -> Self {
        Self {
            private_mode: crate::core::defaults::DEFAULT_HISTORY_PRIVATE_MODE,
            tray_history_limit: crate::core::defaults::DEFAULT_HISTORY_TRAY_LIMIT,
        }
    }
}

// ─── 10. Memory Settings ──────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(default)]
pub struct MemorySettings {
    pub context_retrieval_enabled: bool,
    pub pipeline_processing_enabled: bool,
    pub max_context_share: f32,
    pub context_chaining_window_hours: u32,
    pub top_k_facts: u32,
    pub max_hops: u32,
    pub semantic_similarity_cutoff: f32,
}

impl Default for MemorySettings {
    fn default() -> Self {
        Self {
            context_retrieval_enabled:
                crate::core::defaults::DEFAULT_MEMORY_CONTEXT_RETRIEVAL_ENABLED,
            pipeline_processing_enabled:
                crate::core::defaults::DEFAULT_MEMORY_PIPELINE_PROCESSING_ENABLED,
            max_context_share: crate::core::defaults::DEFAULT_MEMORY_MAX_PERSONAL_SHARE,
            context_chaining_window_hours:
                crate::core::defaults::DEFAULT_MEMORY_CONTEXT_CHAINING_HOURS,
            top_k_facts: crate::core::defaults::DEFAULT_MEMORY_TOP_K_FACTS,
            max_hops: crate::core::defaults::DEFAULT_MEMORY_MAX_HOPS,
            semantic_similarity_cutoff:
                crate::core::defaults::DEFAULT_MEMORY_SEMANTIC_SIMILARITY_CUTOFF,
        }
    }
}

// ─── 11. Persona Settings ─────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(default)]
pub struct PersonaSettings {
    pub modular_prompt: String,
    pub realtime_prompt: String,
}

impl Default for PersonaSettings {
    fn default() -> Self {
        Self {
            modular_prompt: crate::core::constants::SYSTEM_PROMPT_MODULAR.into(),
            realtime_prompt: crate::core::constants::SYSTEM_PROMPT_REALTIME.into(),
        }
    }
}

// ─── 12. Realtime Settings ────────────────────────────────────────────────────

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
    pub resume_handle: Option<String>,
}

impl Default for GeminiRealtimeConfig {
    fn default() -> Self {
        Self {
            api_key: String::new(),
            model: crate::core::defaults::DEFAULT_GEMINI_REALTIME_MODEL.to_string(),
            voice_name: crate::core::defaults::DEFAULT_GEMINI_REALTIME_VOICE.to_string(),
            language_code: crate::core::defaults::DEFAULT_GEMINI_REALTIME_LANG.to_string(),
            temperature: crate::core::defaults::DEFAULT_GEMINI_REALTIME_TEMP,
            enable_web_search: true,
            resume_handle: None,
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
            model: crate::core::defaults::DEFAULT_DEEPGRAM_MODEL.to_string(),
            voice: crate::core::defaults::DEFAULT_DEEPGRAM_VOICE.to_string(),
            temperature: crate::core::defaults::DEFAULT_DEEPGRAM_TEMP,
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

// ─── 13. System Settings ──────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(default)]
pub struct SystemSettings {
    pub log_level: String,
    pub telemetry_enabled: bool,
    pub setup_completed: bool,
}

impl Default for SystemSettings {
    fn default() -> Self {
        Self {
            log_level: crate::core::defaults::DEFAULT_TELEMETRY_LOG_LEVEL.into(),
            telemetry_enabled: crate::core::defaults::DEFAULT_TELEMETRY_ENABLED,
            setup_completed: false,
        }
    }
}

// ─── Main Flat Settings Struct ────────────────────────────────────────────────

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
    pub history: HistorySettings,
    pub appearance: AppearanceSettings,
    pub memory: MemorySettings,
    pub persona: PersonaSettings,
    pub system: SystemSettings,
}

impl VoxSettings {
    pub fn load() -> Self {
        let path = paths::get().settings.clone();

        if let Ok(content) = fs::read_to_string(&path) {
            // Fast path: clean monolithic deserialization
            if let Ok(settings) = serde_json::from_str::<Self>(&content) {
                log::info!("[Settings] Loaded configuration from {:?}", path);
                return settings;
            }

            // Layered recovery path: attempt partial recovery section-by-section via serde_json::Value
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(&content) {
                log::warn!("[Settings] Partial corruption or schema drift detected in settings.json — attempting section recovery.");
                let mut settings = Self::default();
                if let Some(obj) = val.as_object() {
                    if let Some(v) = obj
                        .get("audio")
                        .and_then(|v| serde_json::from_value(v.clone()).ok())
                    {
                        settings.audio = v;
                    }
                    if let Some(v) = obj
                        .get("vad")
                        .and_then(|v| serde_json::from_value(v.clone()).ok())
                    {
                        settings.vad = v;
                    }
                    if let Some(v) = obj
                        .get("stt")
                        .and_then(|v| serde_json::from_value(v.clone()).ok())
                    {
                        settings.stt = v;
                    }
                    if let Some(v) = obj
                        .get("llm")
                        .and_then(|v| serde_json::from_value(v.clone()).ok())
                    {
                        settings.llm = v;
                    }
                    if let Some(v) = obj
                        .get("tts")
                        .and_then(|v| serde_json::from_value(v.clone()).ok())
                    {
                        settings.tts = v;
                    }
                    if let Some(v) = obj
                        .get("realtime")
                        .and_then(|v| serde_json::from_value(v.clone()).ok())
                    {
                        settings.realtime = v;
                    }
                    if let Some(v) = obj
                        .get("interaction")
                        .and_then(|v| serde_json::from_value(v.clone()).ok())
                    {
                        settings.interaction = v;
                    }
                    if let Some(v) = obj
                        .get("dictation")
                        .and_then(|v| serde_json::from_value(v.clone()).ok())
                    {
                        settings.dictation = v;
                    }
                    if let Some(v) = obj
                        .get("history")
                        .and_then(|v| serde_json::from_value(v.clone()).ok())
                    {
                        settings.history = v;
                    }
                    if let Some(v) = obj
                        .get("appearance")
                        .and_then(|v| serde_json::from_value(v.clone()).ok())
                    {
                        settings.appearance = v;
                    }
                    if let Some(v) = obj
                        .get("memory")
                        .and_then(|v| serde_json::from_value(v.clone()).ok())
                    {
                        settings.memory = v;
                    }
                    if let Some(v) = obj
                        .get("persona")
                        .and_then(|v| serde_json::from_value(v.clone()).ok())
                    {
                        settings.persona = v;
                    }
                    if let Some(v) = obj
                        .get("system")
                        .and_then(|v| serde_json::from_value(v.clone()).ok())
                    {
                        settings.system = v;
                    }
                }
                return settings;
            }

            // Total JSON parse failure: backup to timestamped file without clobbering prior backups
            let ts = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            let bak = path.with_file_name(format!("settings.corrupt.{}.json", ts));
            log::error!(
                "[Settings] Corrupt settings.json — backing up to {:?} and restoring in-memory defaults",
                bak
            );
            if let Err(e) = fs::rename(&path, &bak) {
                log::warn!("[Settings] Failed to backup corrupt settings file: {}", e);
            }
        }

        log::info!("[Settings] No valid settings.json found. Using in-memory system defaults.");
        Self::default()
    }

    pub fn save(&self) -> Result<()> {
        let path = paths::get().settings.clone();

        if let Some(parent) = path.parent() {
            if let Err(e) = fs::create_dir_all(parent) {
                log::warn!(
                    "[Settings] Failed to create settings parent directory: {}",
                    e
                );
            }
        }

        let content = serde_json::to_string_pretty(self)?;
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let tmp_path = path.with_file_name(format!("settings.{}.tmp", nanos));
        if let Err(e) = fs::write(&tmp_path, content) {
            if let Err(rm_err) = fs::remove_file(&tmp_path) {
                log::trace!(
                    "[Settings] Failed to remove temporary settings file: {}",
                    rm_err
                );
            }
            return Err(e.into());
        }
        if let Err(e) = fs::rename(&tmp_path, &path) {
            if let Err(rm_err) = fs::remove_file(&tmp_path) {
                log::trace!(
                    "[Settings] Failed to remove temporary settings file: {}",
                    rm_err
                );
            }
            return Err(e.into());
        }

        Ok(())
    }
}
