use super::VoxSettings;
use crate::{
    config::ActiveSelection,
    core::{events::InteractionMode, state::AppState},
    persistence::models::update_model_parameters,
    services::memory::scheduler::parse_consolidation_time,
};

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
            settings.providers.llm.servers = serde_json::from_value(value.clone())
                .map_err(|e| format!("Invalid LLM servers list: {}", e))?;
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
            settings.providers.tts.servers = serde_json::from_value(value.clone())
                .map_err(|e| format!("Invalid TTS servers list: {}", e))?;
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
