use serde::Serialize;

pub mod contract;
pub mod dispatch;
pub mod files;
pub mod mutation;
pub mod persistence;
pub mod settings;
pub mod types;

pub use contract::*;

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

pub fn classify_known(domain: &str, key: &str) -> Option<SettingReloadPolicy> {
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

    // `realtime` used to live in ALL_HOT, which silently classified
    // every `realtime.*` key as Hot and applied nothing — provider/voice
    // changes were persisted to disk and never reached the running actor.
    // Realtime runs on its own actor; provider/voice/key changes require a
    // restart to take effect, so they are classified Restart. If a
    // worker-command channel for realtime is added later, reclassify here.
    const ALL_HOT: &[&str] = &[
        "appearance",
        "interaction",
        "dictation",
        "working_memory",
        "persona",
        "personal_memory",
        "system",
    ];

    const KEYED: &[(&str, &str, SettingReloadPolicy)] = &[
        ("stt", "active", SettingReloadPolicy::Restart),
        ("stt", "last_selected", SettingReloadPolicy::Restart),
        ("stt", "transliterate_enabled", SettingReloadPolicy::Restart),
        ("llm", "active", SettingReloadPolicy::Restart),
        ("llm", "last_selected", SettingReloadPolicy::Restart),
        ("llm", "servers", SettingReloadPolicy::Restart),
        ("llm", "cloud_keys", SettingReloadPolicy::Hot),
        ("tts", "active", SettingReloadPolicy::Restart),
        ("tts", "last_selected", SettingReloadPolicy::Restart),
        ("tts", "servers", SettingReloadPolicy::Restart),
        ("tts", "azure_tts", SettingReloadPolicy::Restart),
        ("tts", "google_tts", SettingReloadPolicy::Restart),
        ("vad", "vad_backend", SettingReloadPolicy::Restart),
        // realtime provider/voice keys require a restart — the running realtime actor does
        // not apply hot. API keys apply hot without cycling the engine.
        ("realtime", "active", SettingReloadPolicy::Restart),
        ("realtime", "active_voice_id", SettingReloadPolicy::Restart),
        ("realtime", "voice", SettingReloadPolicy::Restart),
        ("realtime", "voice_id", SettingReloadPolicy::Restart),
        ("realtime", "api_keys", SettingReloadPolicy::Hot),
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

/// Canonical, strongly-typed list of every setting key that carries secret
/// material. This is the SINGLE SOURCE OF TRUTH for secret classification.
/// `ipc/settings.rs::redact_setting_value` and any contract emission MUST
/// consult this through `is_secret_key`. Adding a secret-bearing key to
/// `mutation::supported_keys_for_domain` without adding it here is a bug.
pub fn secret_keys() -> &'static [(&'static str, &'static str)] {
    &[
        ("llm", "cloud_keys"),
        ("llm", "servers"), // embedded `api_key` on LlmCustomServer
        ("tts", "servers"), // embedded `api_key` on TtsCustomServer
        ("realtime", "api_keys"),
    ]
}

pub fn is_secret_key(domain: &str, key: &str) -> bool {
    secret_keys().iter().any(|(d, k)| *d == domain && *k == key)
}

/// Invariant: every secret-bearing key must be a supported key (so a mutation
/// can write it) and must be classified so that at least one dispatcher exists.
/// This is checked at compile-test time to prevent drift.
pub fn secret_keys_are_supported() -> bool {
    use crate::config::mutation::supported_keys_for_domain;
    secret_keys().iter().all(|(d, k)| {
        supported_keys_for_domain(d).contains(k)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn secret_keys_are_supported_keys() {
        assert!(secret_keys_are_supported());
    }

    #[test]
    fn realtime_keys_are_restart_not_hot() {
        // B-04 regression guard: realtime provider/voice keys MUST NOT be
        // classified Hot, or the running actor won't observe changes.
        // API keys apply hot without cycling the engine.
        for k in ["active", "active_voice_id", "voice", "voice_id"] {
            match classify_known("realtime", k) {
                Some(SettingReloadPolicy::Hot) => {
                    panic!("realtime.{k} still classified Hot (B-04)")
                }
                None => panic!("realtime.{k} unclassified (B-04)"),
                _ => {}
            }
        }
        assert_eq!(
            classify_known("realtime", "api_keys"),
            Some(SettingReloadPolicy::Hot)
        );
    }

    #[test]
    fn policy_table_matches_mutation_contract() {
        assert_eq!(
            get_setting_reload_policy("llm", "servers"),
            SettingReloadPolicy::Restart
        );
        assert_eq!(
            get_setting_reload_policy("llm", "cloud_keys"),
            SettingReloadPolicy::Hot
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
