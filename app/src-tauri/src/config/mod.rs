use serde::Serialize;

pub mod dispatch;
pub mod files;
pub mod mutation;
pub mod persistence;
pub mod settings;

pub use dispatch::{
    dispatch_worker_command, dispatch_worker_command_has_arm, handle_setting_side_effects,
    request_engine_restart, schedule_debounced_save, RESTART_COALESCE_MS,
    SETTINGS_SAVE_DEBOUNCE_MS,
};
pub use files::{
    AgentConfigFile, CognitiveSettings, LlmProvidersConfig, LlmWiringSettings, ProvidersConfigFile,
    SettingsConfigFile, SttProvidersConfig, SttWiringSettings, TtsProvidersConfig,
    TtsWiringSettings,
};
pub use mutation::apply_setting_mutation;
pub use settings::{
    get_preset_colors, AppearanceSettings, AudioOutputMode, AudioSettings,
    DeepgramVoiceAgentConfig, ElevenLabsConvaiConfig, GeminiRealtimeConfig, InteractionSettings,
    OpenAiRealtimeConfig, PersonaSettings, PersonalMemorySettings, RealtimeProviderKind,
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

/// Classification of every `(domain, key)` pair that `apply_setting_mutation`
pub fn get_setting_reload_policy(domain: &str, key: &str) -> SettingReloadPolicy {
    if let Some(policy) = classify_known(domain, key) {
        return policy;
    }

    log::warn!(
        "[Settings] Unknown setting key: {}.{} falling back to Restart",
        domain,
        key
    );
    SettingReloadPolicy::Restart
}

fn classify_known(domain: &str, key: &str) -> Option<SettingReloadPolicy> {
    const WORKER_COMMAND: &[(&str, &[&str])] = &[
        ("audio", &["output_mode"]),
        (
            "vad",
            &[
                "threshold",
                "ptt_noise_gate",
                "silence_duration_ms",
                "speech_onset_ms",
            ],
        ),
        ("tts", &["speed", "voice_index", "voice"]),
    ];

    const ALL_HOT: &[&str] = &[
        "appearance",
        "interaction",
        "dictation",
        "working_memory",
        "persona",
        "personal_memory",
        "system",
        "realtime",
    ];

    const HOT_KEYED: &[(&str, &str)] = &[
        ("llm", "temperature"),
        ("llm", "compaction_temperature"),
        ("llm", "max_output_tokens"),
        ("llm", "reasoning_enabled"),
        ("stt", "transliterate_enabled"),
        ("stt", "partial_throttle_ms"),
    ];

    const KEYED: &[(&str, &str, SettingReloadPolicy)] = &[
        ("audio", "input_device", SettingReloadPolicy::Restart),
        ("stt", "active", SettingReloadPolicy::Restart),
        ("stt", "model", SettingReloadPolicy::Restart),
        ("stt", "provider", SettingReloadPolicy::Restart),
        ("stt", "embedded", SettingReloadPolicy::Restart),
        ("stt", "cloud", SettingReloadPolicy::Restart),
        ("stt", "threads", SettingReloadPolicy::Restart),
        ("llm", "active", SettingReloadPolicy::Restart),
        ("llm", "model", SettingReloadPolicy::Restart),
        ("llm", "provider", SettingReloadPolicy::Restart),
        ("llm", "server", SettingReloadPolicy::Restart),
        ("llm", "cloud", SettingReloadPolicy::Restart),
        ("llm", "cloud_keys", SettingReloadPolicy::Restart),
        ("llm", "embedded", SettingReloadPolicy::Restart),
        ("llm", "context_window", SettingReloadPolicy::Restart),
        ("llm", "threads", SettingReloadPolicy::Restart),
        ("tts", "active", SettingReloadPolicy::Restart),
        ("tts", "provider", SettingReloadPolicy::Restart),
        ("tts", "threads", SettingReloadPolicy::Restart),
        ("tts", "edge_tts", SettingReloadPolicy::Restart),
        ("tts", "supertonic", SettingReloadPolicy::Restart),
        ("tts", "kokoro", SettingReloadPolicy::Restart),
        ("tts", "chatterbox", SettingReloadPolicy::Restart),
        ("tts", "chatterbox_remote", SettingReloadPolicy::Restart),
        ("tts", "zipvoice", SettingReloadPolicy::Restart),
        ("vad", "vad_backend", SettingReloadPolicy::Restart),
        ("vad", "max_speech_duration_s", SettingReloadPolicy::Restart),
    ];

    if WORKER_COMMAND
        .iter()
        .any(|(d, keys)| *d == domain && keys.contains(&key))
    {
        return Some(SettingReloadPolicy::WorkerCommand);
    }

    if let Some((_, _, policy)) = KEYED.iter().find(|(d, k, _)| *d == domain && *k == key) {
        return Some(*policy);
    }

    if HOT_KEYED.iter().any(|(d, k)| *d == domain && *k == key) {
        return Some(SettingReloadPolicy::Hot);
    }

    if ALL_HOT.contains(&domain) {
        return Some(SettingReloadPolicy::Hot);
    }

    None
}
