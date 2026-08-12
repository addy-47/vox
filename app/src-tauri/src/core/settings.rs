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
    /// Earshot — pure Rust, no ONNX dependency, embedded NN weights.
    /// ~20x faster than TenVAD. Default starting from Phase 8.
    Earshot,
    /// TenVAD — ONNX-based, requires ten_vad.onnx model file.
    /// Legacy option, kept for user preference.
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
        // Male voices (M1-M5)
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
        // Female voices (F1-F5)
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
// NOTE: This is CODE-SIDE metadata only. It is NEVER stored in settings.json.
// It informs the IPC layer what action is required after a setting changes.

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SettingReloadPolicy {
    /// Apply immediately with no side effects. No worker restart required.
    Hot,
    /// Send an update command to the affected worker thread via its existing channel.
    /// The worker updates its own local copy without restarting.
    WorkerCommand,
    /// Full app restart required. Model path changes, log level, output device.
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

/// Returns the reload policy for a given settings domain and key.
///
/// This is the single authoritative source of truth for what happens when
/// each setting changes at runtime. Frontend uses this to show appropriate UX.
pub fn reload_policy_for(domain: &str, key: &str) -> SettingReloadPolicy {
    match (domain, key) {
        // UI — all hot: theme and accent change instantly
        ("ui", _) => SettingReloadPolicy::Hot,

        // VAD — threshold and noise gate update via VadCommand channel
        ("vad", "threshold") => SettingReloadPolicy::WorkerCommand,
        ("vad", "ptt_noise_gate") => SettingReloadPolicy::WorkerCommand,
        // VAD backend switch requires full engine restart (different constructor path)
        ("vad", "vad_backend") => SettingReloadPolicy::Restart,

        // Audio output mode — update VAD mic ducking snapshot
        ("audio", "output_mode") => SettingReloadPolicy::WorkerCommand,
        ("audio", "input_device") => SettingReloadPolicy::Restart,

        // ASR — model change requires full pipeline restart
        ("asr", "model") => SettingReloadPolicy::Restart,
        ("asr", "provider") => SettingReloadPolicy::Restart,
        ("asr", "transliterate_enabled") => SettingReloadPolicy::Hot,

        // LLM — most require restart (model is loaded once)
        ("llm", "model") => SettingReloadPolicy::Restart,
        ("llm", "ctx_size") => SettingReloadPolicy::Restart,
        ("llm", "threads") => SettingReloadPolicy::Restart,
        ("llm", "provider") => SettingReloadPolicy::Restart,

        // TTS — provider and voice change require engine restart; quality/speed are hot-updated
        ("tts", "provider") => SettingReloadPolicy::Restart,
        ("tts", "voice") => SettingReloadPolicy::Restart,
        ("tts", "quality_steps") => SettingReloadPolicy::WorkerCommand,
        ("tts", "speed") => SettingReloadPolicy::WorkerCommand,

        // Interaction — sent as mode-changed event immediately
        ("interaction", "auto_sleep_timeout") => SettingReloadPolicy::Hot,
        ("interaction", "pipeline_mode") => SettingReloadPolicy::Restart,
        ("interaction", _) => SettingReloadPolicy::Hot,

        // Telemetry toggle — hot
        ("telemetry", "enabled") => SettingReloadPolicy::Hot,
        // Log level — requires subscriber restart
        ("telemetry", "log_level") => SettingReloadPolicy::Restart,

        // Persistence — limits are hot
        ("persistence", "private_mode") => SettingReloadPolicy::Hot,

        // Memory — all personal parameters hot
        ("memory", _) => SettingReloadPolicy::Hot,

        // Assistant — hot update
        ("assistant", "modular_prompt") => SettingReloadPolicy::Hot,
        ("assistant", "realtime_prompt") => SettingReloadPolicy::Hot,

        // Realtime — hot (applied on next session launch)
        ("realtime", _) => SettingReloadPolicy::Hot,

        // Unknown — conservative default
        _ => SettingReloadPolicy::Restart,
    }
}

// ─── Domain Settings ──────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(default)]
pub struct UiSettings {
    pub theme: String,
    /// Seed color for the theme engine. Frontend derives full palette dynamically.
    /// Store only the seed — NOT generated gradients, shades, or glow colors.
    pub accent_seed: String,

    // Tray HUD Aesthetics
    pub tray_enabled: bool,
    pub tray_blur_density: u32,
    pub tray_glass_tint: bool,
    pub tray_history_limit: u32,
}

impl Default for UiSettings {
    fn default() -> Self {
        Self {
            theme: crate::core::defaults::DEFAULT_UI_THEME.into(),
            accent_seed: crate::core::defaults::DEFAULT_UI_ACCENT_SEED.into(),
            tray_enabled: crate::core::defaults::DEFAULT_UI_TRAY_ENABLED,
            tray_blur_density: crate::core::defaults::DEFAULT_UI_TRAY_BLUR_DENSITY,
            tray_glass_tint: crate::core::defaults::DEFAULT_UI_TRAY_GLASS_TINT,
            tray_history_limit: crate::core::defaults::DEFAULT_UI_TRAY_HISTORY_LIMIT,
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
    /// Which VAD backend to use. Changing this requires an engine restart.
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SttProviderConfig {
    #[serde(rename_all = "snake_case")]
    Embedded {
        #[serde(default = "default_stt_model")]
        model_type: String,
    },
    Cloud {
        /// Which cloud provider: "google", "deepgram", "whisperflow", etc.
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
pub struct AsrSettings {
    pub model: String,
    pub transliterate_enabled: bool,
    #[serde(default)]
    pub provider: SttProviderConfig,
}

impl Default for AsrSettings {
    fn default() -> Self {
        Self {
            model: crate::core::defaults::DEFAULT_ASR_MODEL.to_string(),
            transliterate_enabled: crate::core::defaults::DEFAULT_ASR_TRANSLITERATE_ENABLED,
            provider: SttProviderConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[derive(Default)]
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


fn default_chat_temperature() -> f32 {
    crate::core::defaults::DEFAULT_LLM_CHAT_TEMPERATURE
}

fn default_compaction_temperature() -> f32 {
    crate::core::defaults::DEFAULT_LLM_COMPACTION_TEMPERATURE
}

fn default_max_output_tokens() -> u32 {
    crate::core::defaults::DEFAULT_LLM_MAX_OUTPUT_TOKENS
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct LlmSettings {
    pub model: String,
    pub ctx_size: u32,
    pub threads: u32,
    pub provider: LlmProviderConfig,
    #[serde(default = "default_chat_temperature")]
    pub chat_temperature: f32,
    #[serde(default = "default_compaction_temperature")]
    pub compaction_temperature: f32,
    #[serde(default = "default_max_output_tokens")]
    pub max_output_tokens: u32,
}

impl LlmSettings {
    /// Returns the effective context window token limit based on provider type.
    /// Embedded llama.cpp uses the explicit user-configured value (e.g. 2048, 4096).
    /// Non-embedded (Server & Cloud) models enforce a hard floor of 8192 tokens.
    pub fn effective_ctx_size(&self) -> u32 {
        match self.provider {
            LlmProviderConfig::Embedded => self.ctx_size,
            LlmProviderConfig::OpenAiCompat { .. } => {
                self.ctx_size.max(crate::services::llm::CTX_FLOOR_NON_EMBEDDED)
            }
        }
    }
}

impl Default for LlmSettings {
    fn default() -> Self {
        Self {
            model: crate::core::defaults::DEFAULT_LLM_MODEL.to_string(),
            ctx_size: crate::core::defaults::DEFAULT_LLM_CTX_SIZE,
            threads: crate::core::defaults::DEFAULT_LLM_THREADS,
            provider: LlmProviderConfig::default(),
            chat_temperature: crate::core::defaults::DEFAULT_LLM_CHAT_TEMPERATURE,
            compaction_temperature: crate::core::defaults::DEFAULT_LLM_COMPACTION_TEMPERATURE,
            max_output_tokens: crate::core::defaults::DEFAULT_LLM_MAX_OUTPUT_TOKENS,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TtsProviderConfig {
    Supertonic,
    /// Chatterbox Multilingual TTS via chatterbox-rs (GGML).
    Chatterbox {
        language: String,
        quality_steps: u32,
        speed: f32,
        /// UUID of a cloned voice from the voices table.
        /// `None` = use Chatterbox's built-in reference voice.
        #[serde(default)]
        voice_id: Option<String>,
    },
    /// Chatterbox Remote TTS offloaded to a GPU server.
    ChatterboxRemote {
        endpoint: String,
        language: String,
        quality_steps: u32,
        speed: f32,
        remote_path: String,
        /// UUID of a cloned voice. Reserved for Phase D (remote forwarding).
        #[serde(default)]
        voice_id: Option<String>,
    },
    /// Microsoft Edge TTS via WebSocket.
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
    pub provider: TtsProviderConfig,
    pub voice: i32,         // Supertonic voice index (0-9)
    pub quality_steps: u32, // Supertonic total_steps / Chatterbox cfm_steps (2-12, default 8)
    pub speed: f32,         // Speed factor (0.7-2.0, default 1.05)
}

impl Default for TtsSettings {
    fn default() -> Self {
        Self {
            provider: TtsProviderConfig::default(),
            voice: crate::core::defaults::DEFAULT_TTS_VOICE_INDEX,
            quality_steps: crate::core::defaults::DEFAULT_TTS_QUALITY_STEPS,
            speed: crate::core::defaults::DEFAULT_TTS_SPEED,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct InteractionSettings {
    pub main_app_mode: InteractionMode,
    pub tray_mode: InteractionMode,
    pub auto_sleep_timeout: u32,
    #[serde(default)]
    pub pipeline_mode: PipelineMode,
}

impl Default for InteractionSettings {
    fn default() -> Self {
        Self {
            main_app_mode: InteractionMode::Passive,
            tray_mode: InteractionMode::Passive,
            auto_sleep_timeout: crate::core::defaults::DEFAULT_AUTO_SLEEP_TIMEOUT,
            pipeline_mode: PipelineMode::Modular,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TelemetrySettings {
    pub enabled: bool,
    /// Controls log verbosity. Change requires subscriber restart.
    pub log_level: String,
}

impl Default for TelemetrySettings {
    fn default() -> Self {
        Self {
            enabled: crate::core::defaults::DEFAULT_TELEMETRY_ENABLED,
            log_level: crate::core::defaults::DEFAULT_TELEMETRY_LOG_LEVEL.into(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(default)]
#[derive(Default)]
pub struct PersistenceSettings {
    /// Disable all database writes for the current session.
    pub private_mode: bool,
}


#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(default)]
pub struct MemorySettings {
    /// Toggle 1: Controls whether retrieved memory is injected into live LLM turn prompts.
    pub context_retrieval_enabled: bool,

    /// Toggle 2: Controls whether the background worker thread processes queue items.
    pub pipeline_processing_enabled: bool,

    /// Hard context window budget cap (0.0 - 1.0, default 0.15 / 15% of total LLM context window).
    pub max_personal_memory_share: f32,

    /// Time window in hours for Context Chaining.
    pub context_chaining_window_hours: u32,

    /// Top-K facts limit per collection for vector retrieval (default 5).
    pub top_k_facts: u32,

    /// Maximum graph traversal expansion depth during Seed-and-Expand (default 2).
    pub max_hops: u32,

    /// Similarity Cutoff Floor (0.0 - 1.0, default 0.40 for MiniLM-L12).
    pub semantic_similarity_cutoff: f32,
}

impl Default for MemorySettings {
    fn default() -> Self {
        Self {
            context_retrieval_enabled: crate::core::defaults::DEFAULT_MEMORY_CONTEXT_RETRIEVAL_ENABLED,
            pipeline_processing_enabled: crate::core::defaults::DEFAULT_MEMORY_PIPELINE_PROCESSING_ENABLED,
            max_personal_memory_share: crate::core::defaults::DEFAULT_MEMORY_MAX_PERSONAL_SHARE,
            context_chaining_window_hours: crate::core::defaults::DEFAULT_MEMORY_CONTEXT_CHAINING_HOURS,
            top_k_facts: crate::core::defaults::DEFAULT_MEMORY_TOP_K_FACTS,
            max_hops: crate::core::defaults::DEFAULT_MEMORY_MAX_HOPS,
            semantic_similarity_cutoff: crate::core::defaults::DEFAULT_MEMORY_SEMANTIC_SIMILARITY_CUTOFF,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[derive(Default)]
pub struct SetupSettings {
    pub completed: bool,
}


#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(default)]
pub struct AssistantSettings {
    /// Prompt used when Devanagari (Hindi) input is detected.
    #[serde(alias = "hindi_prompt")]
    pub modular_prompt: String,
    /// Prompt used when English/other input is detected.
    #[serde(alias = "english_prompt")]
    pub realtime_prompt: String,
}

impl Default for AssistantSettings {
    fn default() -> Self {
        Self {
            modular_prompt: crate::core::constants::SYSTEM_PROMPT_MODULAR.into(),
            realtime_prompt: crate::core::constants::SYSTEM_PROMPT_REALTIME.into(),
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
    pub provider: RealtimeProviderKind,
    pub gemini: GeminiRealtimeConfig,
    pub openai: OpenAiRealtimeConfig,
    pub deepgram: DeepgramVoiceAgentConfig,
    pub elevenlabs: ElevenLabsConvaiConfig,
}

impl Default for RealtimeSettings {
    fn default() -> Self {
        Self {
            provider: RealtimeProviderKind::GeminiLive,
            gemini: GeminiRealtimeConfig::default(),
            openai: OpenAiRealtimeConfig::default(),
            deepgram: DeepgramVoiceAgentConfig::default(),
            elevenlabs: ElevenLabsConvaiConfig::default(),
        }
    }
}

// ─── Main Settings Struct ─────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(default)]
pub struct VoxSettings {
    pub ui: UiSettings,
    pub audio: AudioSettings,
    pub vad: VadSettings,
    pub asr: AsrSettings,
    pub llm: LlmSettings,
    pub tts: TtsSettings,
    pub interaction: InteractionSettings,
    pub telemetry: TelemetrySettings,
    pub persistence: PersistenceSettings,
    pub memory: MemorySettings,
    pub assistant: AssistantSettings,
    pub setup: SetupSettings,
    pub realtime: RealtimeSettings,
}

impl VoxSettings {
    /// Load settings from disk with corruption recovery.
    ///
    /// Recovery strategy:
    /// 1. Try to parse as current nested format
    /// 2. On corruption: rename `.json` → `.json.bak`, return defaults (NEVER panics)
    pub fn load() -> Self {
        let path = paths::get().settings.clone();

        if let Ok(content) = fs::read_to_string(&path) {
            // 1. Try current nested format
            if let Ok(settings) = serde_json::from_str::<Self>(&content) {
                log::info!("[Settings] Loaded configuration from {:?}", path);
                return settings;
            }

            // 2. Corruption recovery: rename to .bak, return defaults
            let bak = path.with_extension("json.bak");
            log::error!(
                "[Settings] Corrupt settings.json — backing up to {:?} and restoring defaults",
                bak
            );
            let _ = fs::rename(&path, &bak);
        }

        log::info!("[Settings] No valid settings.json found. Using system defaults.");
        let settings = Self::default();
        let _ = settings.save();
        settings
    }

    pub fn save(&self) -> Result<()> {
        let path = paths::get().settings.clone();

        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }

        let content = serde_json::to_string_pretty(self)?;

        // Atomic write strategy:
        // 1. Write to a .tmp file
        // 2. Atomically rename to .json
        let tmp_path = path.with_extension("tmp");
        fs::write(&tmp_path, content)?;
        fs::rename(tmp_path, path)?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memory_settings_defaults() {
        let settings = VoxSettings::default();
        assert!(settings.memory.context_retrieval_enabled);
        assert!(settings.memory.pipeline_processing_enabled);
        assert_eq!(settings.memory.top_k_facts, 5);
        assert_eq!(settings.memory.max_hops, 2);
    }

    #[test]
    fn test_memory_settings_reload_policy() {
        assert_eq!(
            reload_policy_for("memory", "context_retrieval_enabled"),
            SettingReloadPolicy::Hot
        );
        assert_eq!(
            reload_policy_for("memory", "pipeline_processing_enabled"),
            SettingReloadPolicy::Hot
        );
    }
}
