use super::VoxSettings;
use crate::{
    config::ActiveSelection,
    core::{events::InteractionMode, state::AppState},
    persistence::models::update_model_parameters,
    services::{
        llm::LlmCustomServer,
        memory::scheduler::parse_consolidation_time,
        tts::TtsCustomServer,
    },
};

/// Lists every key `apply_setting_mutation` accepts for a domain, used in rejection messages.
pub fn supported_keys_for_domain(domain: &str) -> &'static [&'static str] {
    match domain {
        "appearance" => &["theme", "accent_seed"],
        "vad" => &[
            "threshold",
            "ptt_noise_gate",
            "silence_duration_ms",
            "speech_onset_ms",
            "vad_backend",
        ],
        "stt" => &["active", "last_selected", "transliterate_enabled"],
        "llm" => &["active", "last_selected", "servers", "cloud_keys"],
        "tts" => &[
            "active",
            "last_selected",
            "active_voice_id",
            "voice",
            "voice_id",
            "servers",
            "azure_tts",
            "google_tts",
        ],
        "interaction" => &["mode", "pipeline_mode"],
        "dictation" => &[
            "enabled",
            "interaction_mode",
            "hotkey",
            "output_mode",
            "silence_auto_stop_ms",
        ],
        "working_memory" => &[
            "private_mode",
            "auto_compaction",
            "max_context_share",
            "web_search_enabled",
        ],
        "persona" => &["modular_prompt", "realtime_prompt"],
        "realtime" => &["active", "active_voice_id", "voice", "voice_id", "api_keys"],
        "personal_memory" => &[
            "context_retrieval_enabled",
            "pipeline_processing_enabled",
            "top_k_facts",
            "semantic_similarity_cutoff",
            "consolidation_cadence",
            "consolidation_time",
            "suggestion_policy",
        ],
        "system" => &["setup_completed"],
        _ => &[],
    }
}

/// Normalizes one custom server URL in place (trim, strip trailing slash).
fn normalize_server_url(url: &str) -> Result<String, String> {
    let trimmed = url.trim().trim_end_matches('/');
    if trimmed.is_empty() {
        return Err("server url must not be empty".to_string());
    }
    if !(trimmed.starts_with("http://") || trimmed.starts_with("https://")) {
        return Err(format!("server url must start with http:// or https://: {}", trimmed));
    }
    if trimmed.contains(char::is_whitespace) {
        return Err(format!("server url must not contain whitespace: {}", trimmed));
    }
    Ok(trimmed.to_string())
}

/// Validates an LLM custom server list: non-empty ids, well-formed normalized URLs, no duplicate ids or URLs.
fn validate_llm_server_list(servers: &mut [LlmCustomServer]) -> Result<(), String> {
    for server in servers.iter_mut() {
        if server.id.trim().is_empty() {
            return Err("server id must not be empty".to_string());
        }
        server.url = normalize_server_url(&server.url)?;
    }
    let mut seen_ids = std::collections::HashSet::new();
    let mut seen_urls = std::collections::HashSet::new();
    for server in servers.iter() {
        if !seen_ids.insert(server.id.clone()) {
            return Err(format!("duplicate server id: {}", server.id));
        }
        let url_key = server.url.to_lowercase();
        if !seen_urls.insert(url_key) {
            return Err(format!("duplicate server url: {}", server.url));
        }
    }
    Ok(())
}

/// Validates a TTS custom server list: non-empty ids, well-formed normalized URLs and paths, no duplicate ids or URLs.
fn validate_tts_server_list(servers: &mut [TtsCustomServer]) -> Result<(), String> {
    for server in servers.iter_mut() {
        if server.id.trim().is_empty() {
            return Err("server id must not be empty".to_string());
        }
        server.url = normalize_server_url(&server.url)?;
        if server.remote_path.trim().is_empty() {
            return Err(format!("server '{}' remote_path must not be empty", server.id));
        }
    }
    let mut seen_ids = std::collections::HashSet::new();
    let mut seen_urls = std::collections::HashSet::new();
    for server in servers.iter() {
        if !seen_ids.insert(server.id.clone()) {
            return Err(format!("duplicate server id: {}", server.id));
        }
        let url_key = server.url.to_lowercase();
        if !seen_urls.insert(url_key) {
            return Err(format!("duplicate server url: {}", server.url));
        }
    }
    Ok(())
}

fn apply_appearance_mutation(
    settings: &mut VoxSettings,
    key: &str,
    value: &serde_json::Value,
) -> Result<bool, String> {
    match key {
        "theme" => {
            settings.appearance.theme = value.as_str().ok_or("theme must be a string")?.to_string();
        }
        "accent_seed" => {
            settings.appearance.accent_seed = value
                .as_str()
                .ok_or("accent_seed must be a string")?
                .to_string();
        }
        _ => return Ok(false),
    }
    Ok(true)
}

fn apply_vad_mutation(
    settings: &mut VoxSettings,
    key: &str,
    value: &serde_json::Value,
) -> Result<bool, String> {
    match key {
        "threshold" => {
            let threshold = value.as_f64().ok_or("threshold must be a number")? as f32;
            if !(0.0..=1.0).contains(&threshold) {
                return Err("threshold must be between 0.0 and 1.0".to_string());
            }
            settings.vad.threshold = threshold;
        }
        "ptt_noise_gate" => {
            settings.vad.ptt_noise_gate =
                value.as_f64().ok_or("ptt_noise_gate must be a number")? as f32;
        }
        "silence_duration_ms" => {
            let duration = value
                .as_u64()
                .ok_or("silence_duration_ms must be an integer")? as u32;
            if !(100..=5000).contains(&duration) {
                return Err("silence_duration_ms must be between 100 and 5000 ms".to_string());
            }
            settings.vad.silence_duration_ms = duration;
        }
        "speech_onset_ms" => {
            let onset = value.as_u64().ok_or("speech_onset_ms must be an integer")? as u32;
            if !(16..=1000).contains(&onset) {
                return Err("speech_onset_ms must be between 16 and 1000 ms".to_string());
            }
            settings.vad.speech_onset_ms = onset;
        }
        "vad_backend" => {
            settings.vad.vad_backend = serde_json::from_value(value.clone())
                .map_err(|e| format!("Invalid vad backend: {}", e))?;
        }
        _ => return Ok(false),
    }
    Ok(true)
}

fn apply_stt_mutation(
    settings: &mut VoxSettings,
    key: &str,
    value: &serde_json::Value,
) -> Result<bool, String> {
    match key {
        "active" => {
            let active: ActiveSelection = serde_json::from_value(value.clone())
                .map_err(|e| format!("Invalid STT active selection: {}", e))?;
            if active.category == "embedded" {
                settings.stt.last_selected.embedded = Some(active.model.clone());
            }
            settings.stt.active = active;
        }
        "last_selected" => {
            settings.stt.last_selected = serde_json::from_value(value.clone())
                .map_err(|e| format!("Invalid STT last_selected: {}", e))?;
        }
        "transliterate_enabled" => {
            settings.stt.transliterate_enabled = value
                .as_bool()
                .ok_or("transliterate_enabled must be a boolean")?;
        }
        _ => return Ok(false),
    }
    Ok(true)
}

fn apply_llm_mutation(
    settings: &mut VoxSettings,
    key: &str,
    value: &serde_json::Value,
) -> Result<bool, String> {
    match key {
        "active" => {
            let active: ActiveSelection = serde_json::from_value(value.clone())
                .map_err(|e| format!("Invalid LLM active selection: {}", e))?;
            match active.category.as_str() {
                "embedded" => settings.llm.last_selected.embedded = Some(active.model.clone()),
                "server" => settings.llm.last_selected.server = Some(active.model.clone()),
                "cloud" => settings.llm.last_selected.cloud = Some(active.model.clone()),
                _ => {}
            }
            settings.llm.active = active;
        }
        "last_selected" => {
            settings.llm.last_selected = serde_json::from_value(value.clone())
                .map_err(|e| format!("Invalid LLM last_selected: {}", e))?;
        }
        "servers" => {
            let mut servers: Vec<LlmCustomServer> = serde_json::from_value(value.clone())
                .map_err(|e| format!("Invalid LLM servers list: {}", e))?;
            validate_llm_server_list(&mut servers)?;
            settings.providers.llm.servers = servers;
        }
        "cloud_keys" => {
            settings.providers.llm.cloud_keys = serde_json::from_value(value.clone())
                .map_err(|e| format!("Invalid LLM cloud_keys config: {}", e))?;
        }
        _ => return Ok(false),
    }
    Ok(true)
}

fn apply_tts_mutation(
    settings: &mut VoxSettings,
    key: &str,
    value: &serde_json::Value,
) -> Result<bool, String> {
    match key {
        "active" => {
            let active: ActiveSelection = serde_json::from_value(value.clone())
                .map_err(|e| format!("Invalid TTS active selection: {}", e))?;
            match active.category.as_str() {
                "embedded" => settings.tts.last_selected.embedded = Some(active.model.clone()),
                "server" => settings.tts.last_selected.server = Some(active.model.clone()),
                "cloud" => settings.tts.last_selected.cloud = Some(active.model.clone()),
                _ => {}
            }
            settings.tts.active = active;
        }
        "last_selected" => {
            settings.tts.last_selected = serde_json::from_value(value.clone())
                .map_err(|e| format!("Invalid TTS last_selected: {}", e))?;
        }
        "active_voice_id" | "voice" | "voice_id" => {
            let val = value
                .as_str()
                .ok_or("voice must be a string id")?
                .to_string();
            if val.trim().is_empty() || val.len() > 128 {
                return Err("voice id must be 1..128 chars".to_string());
            }
            settings.tts.active_voice_id = val;
        }
        "servers" => {
            let mut servers: Vec<TtsCustomServer> = serde_json::from_value(value.clone())
                .map_err(|e| format!("Invalid TTS servers list: {}", e))?;
            validate_tts_server_list(&mut servers)?;
            settings.providers.tts.servers = servers;
        }
        "azure_tts" => {
            let cfg: crate::services::tts::config::TtsAzureConfig = serde_json::from_value(value.clone())
                .map_err(|e| format!("Invalid Azure TTS config: {}", e))?;
            if cfg.region_or_endpoint.trim().is_empty() {
                return Err("azure region_or_endpoint must not be empty".to_string());
            }
            if cfg.voice.trim().is_empty() {
                return Err("azure voice must not be empty".to_string());
            }
            if !cfg.key.trim().is_empty() && cfg.key.trim().len() < 8 {
                return Err("azure key must be at least 8 chars".to_string());
            }
            settings.providers.tts.azure_tts = cfg;
        }
        "google_tts" => {
            let cfg: crate::services::tts::config::TtsGoogleConfig = serde_json::from_value(value.clone())
                .map_err(|e| format!("Invalid Google TTS config: {}", e))?;
            if cfg.voice.trim().is_empty() {
                return Err("google voice must not be empty".to_string());
            }
            if let Some(ref p) = cfg.project_override {
                if p.trim().is_empty() {
                    return Err("google project_override must not be empty when provided".to_string());
                }
            }
            settings.providers.tts.google_tts = cfg;
        }
        _ => return Ok(false),
    }
    Ok(true)
}

fn apply_interaction_mutation(
    settings: &mut VoxSettings,
    key: &str,
    value: &serde_json::Value,
) -> Result<bool, String> {
    match key {
        "mode" => {
            let mode: InteractionMode = serde_json::from_value(value.clone())
                .map_err(|e| format!("Invalid interaction mode: {}", e))?;
            settings.interaction.mode = mode;
        }
        "pipeline_mode" => {
            settings.interaction.pipeline_mode = serde_json::from_value(value.clone())
                .map_err(|e| format!("Invalid pipeline_mode: {}", e))?;
        }
        _ => return Ok(false),
    }
    Ok(true)
}

fn apply_dictation_mutation(
    settings: &mut VoxSettings,
    key: &str,
    value: &serde_json::Value,
) -> Result<bool, String> {
    match key {
        "enabled" => {
            settings.dictation.enabled = value.as_bool().ok_or("enabled must be a boolean")?;
        }
        "interaction_mode" => {
            settings.dictation.interaction_mode = serde_json::from_value(value.clone())
                .map_err(|e| format!("Invalid interaction_mode: {}", e))?;
        }
        "hotkey" => {
            settings.dictation.hotkey =
                value.as_str().ok_or("hotkey must be a string")?.to_string();
        }
        "output_mode" => {
            settings.dictation.output_mode = serde_json::from_value(value.clone())
                .map_err(|e| format!("Invalid output_mode: {}", e))?;
        }
        "silence_auto_stop_ms" => {
            settings.dictation.silence_auto_stop_ms = value
                .as_u64()
                .ok_or("silence_auto_stop_ms must be an unsigned integer")?;
        }
        _ => return Ok(false),
    }
    Ok(true)
}

fn apply_working_memory_mutation(
    settings: &mut VoxSettings,
    key: &str,
    value: &serde_json::Value,
) -> Result<bool, String> {
    match key {
        "private_mode" => {
            settings.working_memory.private_mode =
                value.as_bool().ok_or("private_mode must be a boolean")?;
        }
        "auto_compaction" => {
            settings.working_memory.auto_compaction =
                value.as_bool().ok_or("auto_compaction must be a boolean")?;
        }
        "max_context_share" => {
            let val = value.as_f64().ok_or("max_context_share must be a number")? as f32;
            if !(0.0..=1.0).contains(&val) {
                return Err("max_context_share must be between 0.0 and 1.0".to_string());
            }
            settings.working_memory.max_context_share = val;
        }
        "web_search_enabled" => {
            settings.working_memory.web_search_enabled = value
                .as_bool()
                .ok_or("web_search_enabled must be a boolean")?;
        }
        _ => return Ok(false),
    }
    Ok(true)
}

fn apply_persona_mutation(
    settings: &mut VoxSettings,
    key: &str,
    value: &serde_json::Value,
) -> Result<bool, String> {
    match key {
        "modular_prompt" => {
            settings.persona.modular_prompt = value
                .as_str()
                .ok_or("modular_prompt must be a string")?
                .to_string();
        }
        "realtime_prompt" => {
            settings.persona.realtime_prompt = value
                .as_str()
                .ok_or("realtime_prompt must be a string")?
                .to_string();
        }
        _ => return Ok(false),
    }
    Ok(true)
}

fn apply_realtime_mutation(
    settings: &mut VoxSettings,
    key: &str,
    value: &serde_json::Value,
) -> Result<bool, String> {
    match key {
        "active" => {
            settings.realtime.active = serde_json::from_value(value.clone())
                .map_err(|e| format!("Invalid realtime active selection: {}", e))?;
        }
        "active_voice_id" | "voice" | "voice_id" => {
            settings.realtime.active_voice_id = value
                .as_str()
                .ok_or("active_voice_id must be a string")?
                .to_string();
        }
        "api_keys" => {
            settings.providers.realtime.api_keys = serde_json::from_value(value.clone())
                .map_err(|e| format!("Invalid realtime api_keys: {}", e))?;
        }
        _ => return Ok(false),
    }
    Ok(true)
}

fn apply_personal_memory_mutation(
    settings: &mut VoxSettings,
    key: &str,
    value: &serde_json::Value,
) -> Result<bool, String> {
    match key {
        "context_retrieval_enabled" => {
            settings.personal_memory.context_retrieval_enabled = value
                .as_bool()
                .ok_or("context_retrieval_enabled must be a boolean")?;
        }
        "pipeline_processing_enabled" => {
            settings.personal_memory.pipeline_processing_enabled = value
                .as_bool()
                .ok_or("pipeline_processing_enabled must be a boolean")?;
        }
        "top_k_facts" => {
            let top_k = value
                .as_u64()
                .ok_or("top_k_facts must be a positive integer")? as u32;
            if top_k == 0 || top_k > 100 {
                return Err("top_k_facts must be between 1 and 100".to_string());
            }
            settings.personal_memory.top_k_facts = top_k;
        }
        "semantic_similarity_cutoff" => {
            let val = value
                .as_f64()
                .ok_or("semantic_similarity_cutoff must be a number")? as f32;
            if !(0.0..=1.0).contains(&val) {
                return Err("semantic_similarity_cutoff must be between 0.0 and 1.0".to_string());
            }
            settings.personal_memory.semantic_similarity_cutoff = val;
        }
        "consolidation_cadence" => {
            let val = value
                .as_str()
                .ok_or("consolidation_cadence must be a string")?;
            if !matches!(val, "manual" | "daily") {
                return Err("consolidation_cadence must be one of: manual, daily".to_string());
            }
            settings.personal_memory.consolidation_cadence = val.to_string();
        }
        "consolidation_time" => {
            let val = value
                .as_str()
                .ok_or("consolidation_time must be a string")?;
            parse_consolidation_time(val)
                .ok_or("consolidation_time must be in HH:MM 24-hour format".to_string())?;
            settings.personal_memory.consolidation_time = val.to_string();
        }
        "suggestion_policy" => {
            let val = value.as_str().ok_or("suggestion_policy must be a string")?;
            if !matches!(val, "manual_review" | "auto_apply") {
                return Err(
                    "suggestion_policy must be one of: manual_review, auto_apply".to_string(),
                );
            }
            settings.personal_memory.suggestion_policy = val.to_string();
        }
        _ => return Ok(false),
    }
    Ok(true)
}

fn apply_system_mutation(
    settings: &mut VoxSettings,
    key: &str,
    value: &serde_json::Value,
) -> Result<bool, String> {
    match key {
        "setup_completed" => {
            settings.system.setup_completed =
                value.as_bool().ok_or("setup_completed must be a boolean")?;
        }
        _ => return Ok(false),
    }
    Ok(true)
}

/// Applies a mutation to the settings struct by domain+key routing.
/// Returns `true` if the key was recognized and applied.
pub fn apply_setting_mutation(
    settings: &mut VoxSettings,
    domain: &str,
    key: &str,
    value: &serde_json::Value,
) -> Result<bool, String> {
    match domain {
        "appearance" => apply_appearance_mutation(settings, key, value),
        "vad" => apply_vad_mutation(settings, key, value),
        "stt" => apply_stt_mutation(settings, key, value),
        "llm" => apply_llm_mutation(settings, key, value),
        "tts" => apply_tts_mutation(settings, key, value),
        "interaction" => apply_interaction_mutation(settings, key, value),
        "dictation" => apply_dictation_mutation(settings, key, value),
        "working_memory" => apply_working_memory_mutation(settings, key, value),
        "persona" => apply_persona_mutation(settings, key, value),
        "realtime" => apply_realtime_mutation(settings, key, value),
        "personal_memory" => apply_personal_memory_mutation(settings, key, value),
        "system" => apply_system_mutation(settings, key, value),
        _ => Ok(false),
    }
}

/// Applies an atomic mutation to per-model parameters stored in the database models table.
pub async fn apply_model_parameter_mutation(
    state: &AppState,
    model_id: &str,
    patch: &serde_json::Value,
) -> Result<(), String> {
    let conn = state
        .db
        .connect()
        .map_err(|e| format!("Database connection error: {}", e))?;
    update_model_parameters(&conn, model_id, patch)
        .await
        .map_err(|e| format!("Failed to update model parameters: {}", e))?;
    log::info!(
        "[Config::Mutation] Model parameters updated in database for model={}",
        model_id
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn llm_contract_accepts_inventory_keys_only() {
        let keys = supported_keys_for_domain("llm");
        for expected in ["active", "last_selected", "servers", "cloud_keys"] {
            assert!(keys.contains(&expected), "missing {}", expected);
        }
        for phantom in ["server", "cloud", "embedded", "model", "provider", "temperature", "threads", "context_window"] {
            assert!(!keys.contains(&phantom), "phantom key accepted: {}", phantom);
        }
    }

    #[test]
    fn server_list_validation_rejects_duplicates_and_bad_urls() {
        let mut servers = vec![
            LlmCustomServer {
                id: "a".to_string(),
                name: "A".to_string(),
                url: "http://10.0.0.1:11434/".to_string(),
                model: String::new(),
                protocol: "openai".to_string(),
                api_key: None,
            },
            LlmCustomServer {
                id: "b".to_string(),
                name: "B".to_string(),
                url: "http://10.0.0.1:11434".to_string(),
                model: String::new(),
                protocol: "openai".to_string(),
                api_key: None,
            },
        ];
        assert!(validate_llm_server_list(&mut servers).is_err());

        let mut bad_scheme = vec![LlmCustomServer {
            id: "a".to_string(),
            name: "A".to_string(),
            url: "10.0.0.1:11434".to_string(),
            model: String::new(),
            protocol: "openai".to_string(),
            api_key: None,
        }];
        assert!(validate_llm_server_list(&mut bad_scheme).is_err());

        let mut ok = vec![LlmCustomServer {
            id: "a".to_string(),
            name: "A".to_string(),
            url: "http://10.0.0.1:11434/".to_string(),
            model: String::new(),
            protocol: "openai".to_string(),
            api_key: None,
        }];
        assert!(validate_llm_server_list(&mut ok).is_ok());
        assert_eq!(ok[0].url, "http://10.0.0.1:11434");
    }
}
