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
#[serde(default)]
pub struct ModelMetadata {
    pub id: String,
    pub name: String,
    pub description: String,
    pub ram_usage: String,
    pub parameters: String,
    pub tradeoffs: String,
}

impl Default for ModelMetadata {
    fn default() -> Self {
        Self {
            id: String::new(),
            name: String::new(),
            description: String::new(),
            ram_usage: String::new(),
            parameters: String::new(),
            tradeoffs: String::new(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct VoiceProfile {
    pub id: i32,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RemoteModelInfo {
    pub id: String,   // e.g. "gemma4:31b"
    pub name: String, // display name derived from id
    pub size_bytes: Option<u64>,
    pub quantization: Option<String>, // e.g. "Q4_K_M"
    pub family: Option<String>,       // e.g. "Gemma"
    pub provider_kind: String,        // e.g. "open_ai_compat", "embedded"
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

pub fn get_llm_metadata() -> Vec<ModelMetadata> {
    vec![
        ModelMetadata {
            id: "gemma_4_reasoning".to_string(),
            name: "Gemma 4".to_string(),
            description: "Fast and smart core for general conversation and tasks.".to_string(),
            ram_usage: " ~1.4GB".to_string(),
            parameters: "2.4B (Q4_K_M)".to_string(),
            tradeoffs: "Agentic capabilities (function calling, tool use). Higher quality but slower TPS than Llama Q4. ~1.4GB RAM.".to_string(),
        },
        ModelMetadata {
            id: "llama_3_2_reasoning_q4".to_string(),
            name: "Llama 3.2 1B (Q4)".to_string(),
            description: "Fast, concise — optimized for low-latency responses.".to_string(),
            ram_usage: " ~750MB".to_string(),
            parameters: "1.2B (Q4_K_M)".to_string(),
            tradeoffs: "More concise, faster responses (~4.5 TPS). Slightly lower output quality than Q6. ~750MB RAM.".to_string(),
        },
        ModelMetadata {
            id: "llama_3_2_reasoning".to_string(),
            name: "Llama 3.2 1B (Q6)".to_string(),
            description: "Detailed, higher quality — maximises output fidelity.".to_string(),
            ram_usage: " ~1.0GB".to_string(),
            parameters: "1.2B (Q6_K)".to_string(),
            tradeoffs: "More elaborate, higher-quality responses. Slower TPS (~3.3). Higher RAM. ~1.0GB RAM.".to_string(),
        },
        ModelMetadata {
            id: "gemma_4_uncensored".to_string(),
            name: "Gemma 4 Uncensored".to_string(),
            description: "Unrestricted high-speed agent with ultra-quantized weights.".to_string(),
            ram_usage: " ~2.9GB".to_string(),
            parameters: "2.4B (Q2_K_P)".to_string(),
            tradeoffs: "Unrestricted output. Heavily quantized — may lose coherence on complex tasks. ~2.9GB RAM.".to_string(),
        },
    ]
}

pub fn get_asr_metadata() -> Vec<ModelMetadata> {
    vec![
        ModelMetadata {
            id: "qwen3_asr".to_string(),
            name: "Qwen3-ASR".to_string(),
            description: "Multi-lingual speech recognition.".to_string(),
            ram_usage: " ~800MB".to_string(),
            parameters: "Sherpa-ONNX".to_string(),
            tradeoffs: "Good multilingual ASR. Requires ~800MB. Standard ONNX engine.".to_string(),
        },
        ModelMetadata {
            id: "nvidia_nemotron".to_string(),
            name: "Nemotron-3.5 ASR".to_string(),
            description: "Streaming Automatic Speech Recognition (parakeet-rs).".to_string(),
            ram_usage: " ~2.5GB".to_string(),
            parameters: "0.6B".to_string(),
            tradeoffs: "Higher accuracy streaming ASR. Larger model — requires ~2.5GB RAM. Better for noisy environments.".to_string(),
        },
    ]
}

pub fn get_tts_metadata() -> Vec<ModelMetadata> {
    vec![ModelMetadata {
        id: "supertonic_tts".to_string(),
        name: "Supertonic 3".to_string(),
        description: "Multilingual TTS with 31 languages, flow-matching architecture.".to_string(),
        ram_usage: " ~400MB".to_string(),
        parameters: "99M".to_string(),
        tradeoffs: "31 languages, 10 voices. INT8 quantized — fast inference. ~400MB RAM."
            .to_string(),
    }]
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
        ("asr", "transliterate_enabled") => SettingReloadPolicy::Hot,

        // LLM — most require restart (model is loaded once)
        ("llm", "model") => SettingReloadPolicy::Restart,
        ("llm", "ctx_size") => SettingReloadPolicy::Restart,
        ("llm", "threads") => SettingReloadPolicy::Restart,
        ("llm", "provider") => SettingReloadPolicy::Restart,

        // TTS — voice change requires engine restart; quality/speed are hot-updated
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
        ("persistence", "max_sessions") => SettingReloadPolicy::Hot,
        ("persistence", "retention_days") => SettingReloadPolicy::Hot,

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
            theme: "dark".into(),
            accent_seed: "#00DBE9".into(), // Default Cyan
            tray_enabled: true,
            tray_blur_density: 40,
            tray_glass_tint: true,
            tray_history_limit: 5,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
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
pub struct VadSettings {
    pub threshold: f32,
    pub ptt_noise_gate: f32,
    /// Which VAD backend to use. Changing this requires an engine restart.
    pub vad_backend: VadBackendOption,
}

impl Default for VadSettings {
    fn default() -> Self {
        Self {
            threshold: 0.5, // Earshot recommends 0.5 as general default, TenVAD also optimized to 0.5
            ptt_noise_gate: 0.005,
            vad_backend: VadBackendOption::TenVad,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AsrSettings {
    pub model: String, // e.g., "nvidia_nemotron"
    pub transliterate_enabled: bool,
}

impl Default for AsrSettings {
    fn default() -> Self {
        Self {
            model: "nvidia_nemotron".to_string(),
            transliterate_enabled: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum LlmProviderConfig {
    Embedded,
    OpenAiCompat {
        base_url: String,
        model: String,
        api_key: Option<String>,
        #[serde(default)]
        provider_name: Option<String>,
    },
}

impl Default for LlmProviderConfig {
    fn default() -> Self {
        LlmProviderConfig::Embedded
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmSettings {
    pub model: String, // e.g., "llama_3_2_reasoning"
    pub ctx_size: u32,
    pub threads: u32,
    pub provider: LlmProviderConfig,
}

impl Default for LlmSettings {
    fn default() -> Self {
        Self {
            model: "llama_3_2_reasoning_q4".to_string(),
            ctx_size: 2048,
            threads: 4,
            provider: LlmProviderConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct TtsSettings {
    pub voice: i32,         // Supertonic voice index (0-9)
    pub quality_steps: u32, // Supertonic total_steps (2-12, default 8)
    pub speed: f32,         // Supertonic speed factor (0.7-2.0, default 1.05)
}

impl Default for TtsSettings {
    fn default() -> Self {
        Self {
            voice: 0,
            quality_steps: 12,
            speed: 1.05,
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
            auto_sleep_timeout: 400,
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
            enabled: true,
            log_level: "info".into(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PersistenceSettings {
    /// Disable all database writes for the current session.
    pub private_mode: bool,
    /// Maximum sessions retained. Older entries pruned at next startup.
    pub max_sessions: u32,
    /// Days to retain sessions. 0 = keep forever.
    pub retention_days: u32,
}

impl Default for PersistenceSettings {
    fn default() -> Self {
        Self {
            private_mode: false,
            max_sessions: 500,
            retention_days: 30,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SetupSettings {
    pub completed: bool,
}

impl Default for SetupSettings {
    fn default() -> Self {
        Self { completed: false }
    }
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
    pub voice_name: String,    // default "Aoede"
    pub language_code: String, // BCP-47, default "en-US"
    pub temperature: f32, // default 0.2
    pub enable_web_search: bool,
    pub resume_handle: Option<String>,
}

impl Default for GeminiRealtimeConfig {
    fn default() -> Self {
        Self {
            api_key: String::new(),
            model: "gemini-3.1-flash-live-preview".to_string(),
            voice_name: "Aoede".to_string(),
            language_code: "en-US".to_string(),
            temperature: 0.2,
            enable_web_search: false,
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(default)]
pub struct DeepgramVoiceAgentConfig {
    pub api_key: String,
    pub model: String,
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
    pub assistant: AssistantSettings,
    pub setup: SetupSettings,
    pub realtime: RealtimeSettings,
}

impl VoxSettings {
    /// Load settings from disk with full corruption recovery.
    ///
    /// Recovery strategy:
    /// 1. Try to parse as current nested format
    /// 2. Try Phase 6.0 flat migration
    /// 3. On corruption: rename `.json` → `.json.bak`, return defaults (NEVER panics)
    pub fn load() -> Self {
        let path = paths::get().settings.clone();

        if let Ok(content) = fs::read_to_string(&path) {
            // 1. Try current nested format
            if let Ok(settings) = serde_json::from_str::<Self>(&content) {
                log::info!("[Settings] Loaded configuration from {:?}", path);
                return settings;
            }

            // 2. Try Phase 6.0 flat migration
            if let Ok(legacy) = serde_json::from_str::<serde_json::Value>(&content) {
                if legacy.is_object()
                    && !legacy
                        .as_object()
                        .map(|o| o.contains_key("ui"))
                        .unwrap_or(false)
                {
                    log::warn!("[Settings] Phase 6.0 legacy config detected. Migrating...");
                    return Self::migrate_from_v6_0(legacy);
                }
            }

            // 3. Corruption recovery: rename to .bak, return defaults
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

    fn migrate_from_v6_0(legacy: serde_json::Value) -> Self {
        let mut settings = Self::default();

        if let Some(theme) = legacy.get("theme").and_then(|v| v.as_str()) {
            settings.ui.theme = theme.to_string();
        }

        if let Some(vad_t) = legacy.get("vad_threshold").and_then(|v| v.as_f64()) {
            settings.vad.threshold = vad_t as f32;
        }

        if let Some(ptt_g) = legacy.get("ptt_noise_gate").and_then(|v| v.as_f64()) {
            settings.vad.ptt_noise_gate = ptt_g as f32;
        }

        if let Some(ctx) = legacy.get("llm_ctx_size").and_then(|v| v.as_u64()) {
            settings.llm.ctx_size = ctx as u32;
        }

        if let Some(threads) = legacy.get("llm_threads").and_then(|v| v.as_u64()) {
            settings.llm.threads = threads as u32;
        }

        // Model paths: reset to standardized ~/.vox/models/ defaults
        // (legacy paths were relative `assets/` paths which no longer apply)
        let _ = settings.save();
        settings
    }
}
