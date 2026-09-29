use serde::Serialize;

pub mod dispatch;
pub mod files;
pub mod mutation;
pub mod persistence;
pub mod settings;

pub use dispatch::{
    dispatch_worker_command, dispatch_worker_command_has_arm, handle_setting_side_effects,
    schedule_debounced_save, SETTINGS_SAVE_DEBOUNCE_MS,
};
pub use mutation::apply_setting_mutation;

pub use files::{
    AgentConfigFile, CognitiveSettings, LlmProvidersConfig, LlmWiringSettings, ProvidersConfigFile,
    SettingsConfigFile, SttProvidersConfig, SttWiringSettings, TtsProvidersConfig,
    TtsWiringSettings,
};
pub use settings::{
    get_preset_colors, AppearanceSettings, AudioOutputMode, AudioSettings,
    DeepgramVoiceAgentConfig, ElevenLabsConvaiConfig, GeminiRealtimeConfig, InteractionSettings,
    OpenAiRealtimeConfig, PersonalMemorySettings, PersonaSettings, RealtimeProviderKind,
    RealtimeSettings, SystemSettings, VoxSettings, WorkingMemorySettings,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
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
        "appearance"
        | "working_memory"
        | "personal_memory"
        | "persona"
        | "realtime"
        | "interaction"
        | "dictation" => SettingReloadPolicy::Hot,
        "audio" if key == "output_mode" => SettingReloadPolicy::WorkerCommand,
        "tts" if key == "speed" || key == "voice_index" || key == "voice" => {
            SettingReloadPolicy::WorkerCommand
        }
        "llm"
            if key == "temperature"
                || key == "compaction_temperature"
                || key == "reasoning_enabled"
                || key == "max_output_tokens" =>
        {
            SettingReloadPolicy::Hot
        }
        // Provider construction reads cloud_keys/cloud/server once and the actor caches it for the engine lifetime.
        "llm"
            if key == "cloud_keys"
                || key == "cloud"
                || key == "server"
                || key == "provider"
                || key == "active"
                || key == "model" =>
        {
            SettingReloadPolicy::Restart
        }
        "vad"
            if key == "threshold"
                || key == "ptt_noise_gate"
                || key == "silence_duration_ms"
                || key == "speech_onset_ms" =>
        {
            SettingReloadPolicy::WorkerCommand
        }
        // max_speech_duration_s has no VadCommand variant in dispatch_worker_command
        "vad" if key == "max_speech_duration_s" => SettingReloadPolicy::Restart,
        "stt"
            if key == "transliterate"
                || key == "transliterate_enabled"
                || key == "partial_throttle_ms" =>
        {
            SettingReloadPolicy::Hot
        }
        "stt" if key == "threads" => SettingReloadPolicy::Restart,
        "tts" if key == "threads" => SettingReloadPolicy::Restart,
        "system" if key == "setup_completed" => SettingReloadPolicy::Hot,
        _ => {
            log::warn!(
                "[Settings] Unknown setting key: {}.{} falling back to Restart",
                domain,
                key
            );
            SettingReloadPolicy::Restart
        }
    }
}
