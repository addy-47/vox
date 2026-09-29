use std::{
    fs,
    io::Write,
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::Result;
use serde::Serialize;

use super::{
    files::{AgentConfigFile, ProvidersConfigFile, SettingsConfigFile},
    settings::VoxSettings,
};
use crate::utils::{jsonc, paths};

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
        let mut opts = fs::OpenOptions::new();
        opts.write(true).create(true).truncate(true);
        #[cfg(unix)]
        if let Some(m) = mode {
            use std::os::unix::fs::OpenOptionsExt;
            opts.mode(m);
        }
        let mut file = match opts.open(&tmp_path) {
            Ok(f) => f,
            Err(e) => {
                log::warn!("[Settings] Failed to create tmp file {:?}: {}", tmp_path, e);
                return Err(e.into());
            }
        };
        #[cfg(unix)]
        if let Some(m) = mode {
            use std::os::unix::fs::PermissionsExt;
            file.set_permissions(fs::Permissions::from_mode(m))?;
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
    // Rename durability on ext4 is best-effort without an explicit parent-dir fsync;
    // this is an accepted trade-off for a local-first desktop application with low write frequency.
    #[cfg(unix)]
    if let Some(m) = mode {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(m))?;
    }
    Ok(())
}

fn backup_corrupt(path: &Path, prefix: &str) {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let bak = path.with_file_name(format!("{}.corrupt.{}.jsonc", prefix, nanos));
    log::error!(
        "[Settings] Corrupt {} — backing up to {:?} and restoring in-memory defaults",
        path.display(),
        bak
    );
    if let Err(e) = fs::rename(path, &bak) {
        log::warn!("[Settings] Failed to backup corrupt file: {}", e);
    }
}

impl VoxSettings {
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
            if let Some(v) = obj.get("stt").and_then(|v| serde_json::from_value::<super::files::SttWiringSettings>(v.clone()).ok()) {
                self.stt.active = v.active;
                self.stt.transliterate_enabled = v.transliterate_enabled;
                self.stt.embedded = v.embedded;
            }
            if let Some(v) = obj.get("llm").and_then(|v| serde_json::from_value::<super::files::LlmWiringSettings>(v.clone()).ok()) {
                self.llm.active = v.active;
                self.llm.threads = v.threads;
                self.llm.embedded = v.embedded;
            }
            if let Some(v) = obj.get("tts").and_then(|v| serde_json::from_value::<super::files::TtsWiringSettings>(v.clone()).ok()) {
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
            if let Some(v) = obj.get("llm").and_then(|v| serde_json::from_value::<super::files::LlmProvidersConfig>(v.clone()).ok()) {
                self.llm.server = v.server;
                self.llm.cloud = v.cloud;
                self.llm.cloud_keys = v.cloud_keys;
            }
            if let Some(v) = obj.get("stt").and_then(|v| serde_json::from_value::<super::files::SttProvidersConfig>(v.clone()).ok()) {
                self.stt.cloud = v.cloud;
            }
            if let Some(v) = obj.get("tts").and_then(|v| serde_json::from_value::<super::files::TtsProvidersConfig>(v.clone()).ok()) {
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
            if let Some(v) = obj.get("cognitive").and_then(|v| serde_json::from_value::<super::files::CognitiveSettings>(v.clone()).ok()) {
                self.llm.temperature = v.temperature;
                self.llm.compaction_temperature = v.compaction_temperature;
                self.llm.max_output_tokens = v.max_output_tokens;
                self.llm.context_window = v.context_window;
                self.llm.reasoning_enabled = v.reasoning_enabled;
            }
        }
    }

    pub fn load() -> Self {
        let settings_path = paths::settings_path();
        let providers_path = paths::providers_path();
        let agent_path = paths::agent_path();

        if settings_path.exists() || providers_path.exists() || agent_path.exists() {
            let mut settings = Self::default();

            if let Ok(content) = fs::read_to_string(&settings_path) {
                if let Ok(cfg) = jsonc::from_jsonc_str::<SettingsConfigFile>(&content) {
                    settings.merge_settings_config(cfg);
                } else if let Ok(val) = jsonc::from_jsonc_str::<serde_json::Value>(&content) {
                    log::warn!(
                        "[Settings] Type error in {} — recovering parseable sections, missing sections fall back to defaults.",
                        settings_path.display()
                    );
                    settings.recover_settings_sections(&val);
                } else {
                    backup_corrupt(&settings_path, "settings");
                }
            }

            if let Ok(content) = fs::read_to_string(&providers_path) {
                if let Ok(cfg) = jsonc::from_jsonc_str::<ProvidersConfigFile>(&content) {
                    settings.merge_providers_config(cfg);
                } else if let Ok(val) = jsonc::from_jsonc_str::<serde_json::Value>(&content) {
                    log::warn!(
                        "[Settings] Type error in {} — recovering parseable sections, missing sections fall back to defaults.",
                        providers_path.display()
                    );
                    settings.recover_providers_sections(&val);
                } else {
                    backup_corrupt(&providers_path, "providers");
                }
            }

            if let Ok(content) = fs::read_to_string(&agent_path) {
                if let Ok(cfg) = jsonc::from_jsonc_str::<AgentConfigFile>(&content) {
                    settings.merge_agent_config(cfg);
                } else if let Ok(val) = jsonc::from_jsonc_str::<serde_json::Value>(&content) {
                    log::warn!(
                        "[Settings] Type error in {} — recovering parseable sections, missing sections fall back to defaults.",
                        agent_path.display()
                    );
                    settings.recover_agent_sections(&val);
                } else {
                    backup_corrupt(&agent_path, "agent");
                }
            }

            settings.normalize_retired_voice_ids();
            log::info!("[Settings] Loaded 3-way decomposed configuration from config/ (settings.jsonc, providers.jsonc, agent.jsonc)");
            return settings;
        }

        log::info!("[Settings] No valid settings found. Using in-memory system defaults.");
        Self::default()
    }

    pub fn save(&self) -> Result<()> {
        let mut first_err = None;

        let settings_path = paths::settings_path();
        let providers_path = paths::providers_path();
        let agent_path = paths::agent_path();

        let settings_cfg = self.to_settings_config();
        let providers_cfg = self.to_providers_config();
        let agent_cfg = self.to_agent_config();

        // agent + settings first; providers (the 0600 secret file) commits last so a
        // failure can never leave it newer than the settings that reference it.
        if let Err(e) = atomic_write_json(&agent_path, &agent_cfg, None) {
            log::error!("[Settings] Failed to persist {:?}: {}", agent_path, e);
            first_err.get_or_insert(e);
        }

        if let Err(e) = atomic_write_json(&settings_path, &settings_cfg, None) {
            log::error!("[Settings] Failed to persist {:?}: {}", settings_path, e);
            first_err.get_or_insert(e);
        }

        #[cfg(unix)]
        let p = atomic_write_json(&providers_path, &providers_cfg, Some(0o600));
        #[cfg(not(unix))]
        let p = atomic_write_json(&providers_path, &providers_cfg, None);
        if let Err(e) = p {
            log::error!("[Settings] Failed to persist {:?}: {}", providers_path, e);
            first_err.get_or_insert(e);
        }

        first_err.map_or(Ok(()), Err)
    }
}
