use serde::{Deserialize, Serialize};
use std::fs;
use anyhow::Result;
use crate::utils::paths;

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
    #[default]
    Earshot,
    /// TenVAD — ONNX-based, requires ten_vad.onnx model file.
    /// Legacy option, kept for user preference.
    TenVad,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Default)]
pub enum InteractionMode {
    #[default]
    Passive,
    PTT,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ModelMetadata {
    pub id: String,
    pub name: String,
    pub description: String,
    pub ram_usage: String,
    pub parameters: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct VoiceProfile {
    pub id: i32,
    pub name: String,
    pub language: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_file: Option<String>,
}

pub fn get_voice_profiles() -> Vec<VoiceProfile> {
    let mut voices = vec![
        VoiceProfile { id: 0, name: "Bella".to_string(), language: "en".into(), model_file: None },
        VoiceProfile { id: 1, name: "Sarah".to_string(), language: "en".into(), model_file: None },
        VoiceProfile { id: 2, name: "Sky".to_string(), language: "en".into(), model_file: None },
        VoiceProfile { id: 3, name: "Adam".to_string(), language: "en".into(), model_file: None },
        VoiceProfile { id: 4, name: "Michael".to_string(), language: "en".into(), model_file: None },
        VoiceProfile { id: 5, name: "Eric".to_string(), language: "en".into(), model_file: None },
        VoiceProfile { id: 6, name: "Emma".to_string(), language: "en".into(), model_file: None },
        VoiceProfile { id: 7, name: "Isabella".to_string(), language: "en".into(), model_file: None },
        VoiceProfile { id: 8, name: "Jessica".to_string(), language: "en".into(), model_file: None },
        VoiceProfile { id: 9, name: "Nicole".to_string(), language: "en".into(), model_file: None },
        VoiceProfile { id: 10, name: "William".to_string(), language: "en".into(), model_file: None },
    ];

    voices.push(VoiceProfile { 
        id: 100, 
        name: "Priyamvada".to_string(), 
        language: "hi".into(), 
        model_file: Some(crate::core::constants::MODEL_FILE_TTS_HI_PRIYAMVADA.into()) 
    });
    voices.push(VoiceProfile { 
        id: 101, 
        name: "Pratham".to_string(), 
        language: "hi".into(), 
        model_file: Some(crate::core::constants::MODEL_FILE_TTS_HI_PRATHAM.into()) 
    });
    voices.push(VoiceProfile { 
        id: 102, 
        name: "Rohan".to_string(), 
        language: "hi".into(), 
        model_file: Some(crate::core::constants::MODEL_FILE_TTS_HI_ROHAN.into()) 
    });

    voices
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
            id: "gemma4".to_string(),
            name: "Gemma 4".to_string(),
            description: "Fast and smart core for general conversation and tasks.".to_string(),
            ram_usage: " ~1.4GB".to_string(),
            parameters: "2.4B (Q4_K_M)".to_string(),
        },
        ModelMetadata {
            id: "llm_llama_3_2_1b_instruct_q6_k".to_string(),
            name: "Llama 3.2 1B".to_string(),
            description: "Highly optimized low-latency instruction-following agent.".to_string(),
            ram_usage: " ~1.0GB".to_string(),
            parameters: "1.2B (Q6_K)".to_string(),
        },
        ModelMetadata {
            id: "llm_gemma_4_e2b_uncensored_aggressive_q2_k_p".to_string(),
            name: "Gemma 4 Uncensored".to_string(),
            description: "Unrestricted high-speed agent with ultra-quantized weights.".to_string(),
            ram_usage: " ~2.9GB".to_string(),
            parameters: "2.4B (Q2_K_P)".to_string(),
        }
    ]
}

pub fn get_asr_metadata() -> Vec<ModelMetadata> {
    vec![
        ModelMetadata {
            id: "qwen3-asr".to_string(),
            name: "Qwen3-ASR".to_string(),
            description: "Multi-lingual speech recognition.".to_string(),
            ram_usage: " ~800MB".to_string(),
            parameters: "Sherpa-ONNX".to_string(),
        }
    ]
}

pub fn get_tts_metadata() -> Vec<ModelMetadata> {
    vec![
        ModelMetadata {
            id: "kokoro".to_string(),
            name: "Kokoro".to_string(),
            description: "High-quality voice output with multiple profiles.".to_string(),
            ram_usage: " ~150MB".to_string(),
            parameters: "82M".to_string(),
        },
        ModelMetadata {
            id: "piper_hi".to_string(),
            name: "Piper Hindi".to_string(),
            description: "Natural Hindi speech optimized for low power devices.".to_string(),
            ram_usage: " ~100MB".to_string(),
            parameters: "VITS (Medium)".to_string(),
        }
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
            Self::Hot           => "hot",
            Self::WorkerCommand => "worker_command",
            Self::Restart       => "restart",
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
        ("ui", _)                        => SettingReloadPolicy::Hot,

        // VAD — threshold and noise gate update via VadCommand channel
        ("vad", "threshold")             => SettingReloadPolicy::WorkerCommand,
        ("vad", "ptt_noise_gate")        => SettingReloadPolicy::WorkerCommand,
        // VAD backend switch requires full engine restart (different constructor path)
        ("vad", "vad_backend")           => SettingReloadPolicy::Restart,

        // Audio output mode — update VAD mic ducking snapshot
        ("audio", "output_mode")         => SettingReloadPolicy::WorkerCommand,
        ("audio", "input_device")        => SettingReloadPolicy::Restart,

        // ASR — model change requires full pipeline restart
        ("asr", "model")                 => SettingReloadPolicy::Restart,
        ("asr", "transliterate_enabled") => SettingReloadPolicy::Hot,

        // LLM — most require restart (model is loaded once)
        ("llm", "model")                 => SettingReloadPolicy::Restart,
        ("llm", "ctx_size")              => SettingReloadPolicy::Restart,
        ("llm", "threads")               => SettingReloadPolicy::Restart,

        // TTS — model and voice changes require restart
        ("tts", "en_model")              => SettingReloadPolicy::Restart,
        ("tts", "en_voice")              => SettingReloadPolicy::Restart,
        ("tts", "hi_model")              => SettingReloadPolicy::Restart,
        ("tts", "hi_voice")              => SettingReloadPolicy::Restart,

        // Interaction — sent as mode-changed event immediately
        ("interaction", "auto_sleep_timeout") => SettingReloadPolicy::Hot,
        ("interaction", _)               => SettingReloadPolicy::Hot,

        // Telemetry toggle — hot
        ("telemetry", "enabled")         => SettingReloadPolicy::Hot,
        // Log level — requires subscriber restart
        ("telemetry", "log_level")       => SettingReloadPolicy::Restart,

        // Persistence — enabled flag requires restart; limits are hot
        ("persistence", "enabled")       => SettingReloadPolicy::Restart,
        ("persistence", "private_mode")  => SettingReloadPolicy::Hot,
        ("persistence", "max_sessions")  => SettingReloadPolicy::Hot,
        ("persistence", "retention_days") => SettingReloadPolicy::Hot,

        // Assistant — system prompt is sent to LLM worker via channel
        ("assistant", "system_prompt")   => SettingReloadPolicy::WorkerCommand,

        // Unknown — conservative default
        _                                => SettingReloadPolicy::Restart,
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
    pub tray_hide_delay: f32,
    pub tray_fade_transition: String,
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
            tray_hide_delay: 5.0,
            tray_fade_transition: "Smooth".into(),
            tray_history_limit: 10,
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
            threshold: 0.65,        // Earshot recommends 0.65 to prevent rapid sessions
            ptt_noise_gate: 0.015,
            vad_backend: VadBackendOption::Earshot,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AsrSettings {
    pub model: String, // e.g., "qwen3-asr"
    pub transliterate_enabled: bool,
}

impl Default for AsrSettings {
    fn default() -> Self {
        Self {
            model: "qwen3-asr".to_string(),
            transliterate_enabled: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmSettings {
    pub model: String, // e.g., "gemma4"
    pub ctx_size: u32,
    pub threads: u32,
}

impl Default for LlmSettings {
    fn default() -> Self {
        Self {
            model: "llama-3.2".to_string(),
            ctx_size: 2048,
            threads: 4,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TtsSettings {
    pub en_model: String, // e.g., "kokoro"
    pub en_voice: i32,    // Kokoro voice index (0-10)
    pub hi_model: String, // e.g., "piper_hi"
    pub hi_voice: String, // Specific Piper .onnx filename
}

impl Default for TtsSettings {
    fn default() -> Self {
        Self {
            en_model: "kokoro".to_string(),
            en_voice: 0, // Bella
            hi_model: "piper_hi".to_string(),
            hi_voice: crate::core::constants::MODEL_FILE_TTS_HI_PRIYAMVADA.to_string(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct InteractionSettings {
    pub main_app_mode: InteractionMode,
    pub tray_mode: InteractionMode,
    pub auto_sleep_timeout: u32,
}

impl Default for InteractionSettings {
    fn default() -> Self {
        Self {
            main_app_mode: InteractionMode::Passive,
            tray_mode: InteractionMode::Passive,
            auto_sleep_timeout: 300,
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
    /// Enable/disable SQLite session persistence. Requires restart to take effect.
    pub enabled: bool,
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
            enabled: true,
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
        Self {
            completed: false,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(default)]
pub struct AssistantSettings {
    /// Generic fallback prompt.
    pub system_prompt: String,
    /// Prompt used when Devanagari (Hindi) input is detected.
    pub hindi_prompt: String,
    /// Prompt used when English/other input is detected.
    pub english_prompt: String,
}

impl Default for AssistantSettings {
    fn default() -> Self {
        Self {
            system_prompt: crate::core::constants::SYSTEM_PROMPT_EN.into(),
            hindi_prompt: crate::core::constants::SYSTEM_PROMPT_HI.into(),
            english_prompt: crate::core::constants::SYSTEM_PROMPT_EN.into(),
        }
    }
}

// ─── Main Settings Struct ─────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(default)]
pub struct VoxSettings {
    pub ui:          UiSettings,
    pub audio:       AudioSettings,
    pub vad:         VadSettings,
    pub asr:         AsrSettings,
    pub llm:         LlmSettings,
    pub tts:         TtsSettings,
    pub interaction: InteractionSettings,
    pub telemetry:   TelemetrySettings,
    pub persistence: PersistenceSettings,
    pub assistant:   AssistantSettings,
    pub setup:       SetupSettings,
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
                if legacy.is_object() && !legacy.as_object().map(|o| o.contains_key("ui")).unwrap_or(false) {
                    log::warn!("[Settings] Phase 6.0 legacy config detected. Migrating...");
                    return Self::migrate_from_v6_0(legacy);
                }
            }

            // 3. Corruption recovery: rename to .bak, return defaults
            let bak = path.with_extension("json.bak");
            log::error!("[Settings] Corrupt settings.json — backing up to {:?} and restoring defaults", bak);
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
