use serde::Serialize;

pub mod dispatch;
pub mod files;
pub mod mutation;
pub mod persistence;
pub mod settings;
pub mod types;

pub use dispatch::{
    dispatch_worker_command, dispatch_worker_command_has_arm, handle_setting_side_effects,
    request_engine_restart, schedule_debounced_save, RESTART_COALESCE_MS,
    SETTINGS_SAVE_DEBOUNCE_MS,
};
pub use files::{AgentConfigFile, ProvidersConfigFile, SettingsConfigFile};
pub use mutation::{apply_model_parameter_mutation, apply_setting_mutation, supported_keys_for_domain};
pub use settings::{
    get_preset_colors, AppearanceSettings, AudioOutputMode, InteractionSettings, PersonaSettings,
    PersonalMemorySettings, SystemSettings, VoxSettings, WorkingMemorySettings,
};
pub use types::{ActiveSelection, LastSelectedModels};

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
        (
            "vad",
            &[
                "threshold",
                "ptt_noise_gate",
                "silence_duration_ms",
                "speech_onset_ms",
            ],
        ),
        ("tts", &["voice_id", "voice", "active_voice_id"]),
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

    const KEYED: &[(&str, &str, SettingReloadPolicy)] = &[
        ("stt", "active", SettingReloadPolicy::Restart),
        ("stt", "last_selected", SettingReloadPolicy::Restart),
        ("stt", "transliterate_enabled", SettingReloadPolicy::Restart),
        ("llm", "active", SettingReloadPolicy::Restart),
        ("llm", "last_selected", SettingReloadPolicy::Restart),
        ("llm", "servers", SettingReloadPolicy::Restart),
        ("llm", "cloud_keys", SettingReloadPolicy::Restart),
        ("tts", "active", SettingReloadPolicy::Restart),
        ("tts", "last_selected", SettingReloadPolicy::Restart),
        ("tts", "servers", SettingReloadPolicy::Restart),
        ("vad", "vad_backend", SettingReloadPolicy::Restart),
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

    if ALL_HOT.contains(&domain) {
        return Some(SettingReloadPolicy::Hot);
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn policy_table_matches_mutation_contract() {
        assert_eq!(
            get_setting_reload_policy("llm", "servers"),
            SettingReloadPolicy::Restart
        );
        assert_eq!(
            get_setting_reload_policy("llm", "cloud_keys"),
            SettingReloadPolicy::Restart
        );
        for phantom in ["server", "cloud", "embedded", "temperature", "threads"] {
            assert!(
                classify_known("llm", phantom).is_none(),
                "policy names unappliable key llm.{}",
                phantom
            );
        }
        for (domain, key) in [
            ("stt", "active"),
            ("stt", "last_selected"),
            ("llm", "active"),
            ("llm", "last_selected"),
            ("tts", "active"),
            ("tts", "servers"),
            ("vad", "vad_backend"),
        ] {
            assert!(
                supported_keys_for_domain(domain).contains(&key),
                "policy key {}.{} missing from mutation contract",
                domain,
                key
            );
        }
    }
}
