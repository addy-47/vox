use crate::{
    core::{
        defaults::MIN_LLM_CONTEXT_WINDOW,
        settings::{
            LlmActiveProvider, LlmProviderConfig, LlmRemoteConfig, SttActiveProvider,
            SttCloudConfig, SttProviderConfig, TtsActiveProvider, TtsProviderConfig, VoxSettings,
        },
    },
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

fn apply_audio_mutation(
    settings: &mut VoxSettings,
    key: &str,
    value: &serde_json::Value,
) -> Result<bool, String> {
    match key {
        "output_mode" => {
            settings.audio.output_mode = serde_json::from_value(value.clone())
                .map_err(|e| format!("Invalid output_mode: {}", e))?;
        }
        "input_device" => {
            settings.audio.input_device = serde_json::from_value(value.clone())
                .map_err(|e| format!("Invalid input_device: {}", e))?;
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
            settings.stt.active = serde_json::from_value(value.clone())
                .map_err(|e| format!("Invalid STT active provider: {}", e))?;
        }
        "model" => {
            settings.stt.embedded.model =
                value.as_str().ok_or("model must be a string")?.to_string();
        }
        "partial_throttle_ms" => {
            let throttle = value
                .as_u64()
                .ok_or("partial_throttle_ms must be an integer")?;
            if !(50..=2000).contains(&throttle) {
                return Err("partial_throttle_ms must be between 50 and 2000 ms".to_string());
            }
            settings.stt.embedded.partial_throttle_ms = throttle;
        }
        "threads" => {
            let t = value.as_u64().ok_or("threads must be a positive integer")? as u32;
            if !(1..=64).contains(&t) {
                return Err("stt threads must be between 1 and 64".to_string());
            }
            settings.stt.embedded.threads = t;
        }
        "transliterate_enabled" => {
            settings.stt.transliterate_enabled = value
                .as_bool()
                .ok_or("transliterate_enabled must be a boolean")?;
        }
        "embedded" => {
            settings.stt.embedded = serde_json::from_value(value.clone())
                .map_err(|e| format!("Invalid STT embedded config: {}", e))?;
        }
        "cloud" => {
            settings.stt.cloud = serde_json::from_value(value.clone())
                .map_err(|e| format!("Invalid STT cloud config: {}", e))?;
        }
        "provider" => {
            if let Ok(config) = serde_json::from_value::<SttProviderConfig>(value.clone()) {
                match config {
                    SttProviderConfig::Embedded { model_type } => {
                        settings.stt.active = SttActiveProvider::Embedded;
                        settings.stt.embedded.model = model_type;
                    }
                    SttProviderConfig::Cloud {
                        provider,
                        model,
                        language,
                        region,
                        credentials_path,
                        credentials_json,
                        project_id,
                        endpoint,
                    } => {
                        settings.stt.active = SttActiveProvider::Cloud;
                        settings.stt.cloud = SttCloudConfig {
                            provider,
                            model,
                            language,
                            region,
                            credentials_path,
                            credentials_json,
                            project_id,
                            endpoint,
                        };
                    }
                }
            }
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
            settings.llm.active = serde_json::from_value(value.clone())
                .map_err(|e| format!("Invalid LLM active provider: {}", e))?;
        }
        "model" => {
            let model_str = value.as_str().ok_or("model must be a string")?.to_string();
            match settings.llm.active {
                LlmActiveProvider::Embedded => {
                    settings.llm.embedded.model = model_str;
                }
                LlmActiveProvider::Server => {
                    settings.llm.server.model = model_str;
                }
                LlmActiveProvider::Cloud => {
                    settings.llm.cloud.model = model_str;
                }
            }
        }
        "temperature" => {
            settings.llm.temperature = value.as_f64().ok_or("temperature must be a number")? as f32;
        }
        "compaction_temperature" => {
            settings.llm.compaction_temperature = value
                .as_f64()
                .ok_or("compaction_temperature must be a number")?
                as f32;
        }
        "max_output_tokens" => {
            let val = value
                .as_u64()
                .ok_or("max_output_tokens must be a positive integer")?
                as u32;
            if !(1..=32768).contains(&val) {
                return Err("max_output_tokens must be between 1 and 32768".to_string());
            }
            settings.llm.max_output_tokens = val;
        }
        "reasoning_enabled" => {
            settings.llm.reasoning_enabled = value
                .as_bool()
                .ok_or("reasoning_enabled must be a boolean")?;
        }
        "context_window" => {
            let val = value
                .as_u64()
                .ok_or("context_window must be a positive integer")? as u32;
            if val < MIN_LLM_CONTEXT_WINDOW {
                return Err(format!(
                    "Context window cannot be less than {} tokens",
                    MIN_LLM_CONTEXT_WINDOW
                ));
            }
            settings.llm.context_window = val;
        }
        "threads" => {
            let val = value.as_u64().ok_or("threads must be a positive integer")? as u32;
            if !(1..=64).contains(&val) {
                return Err("threads must be between 1 and 64".to_string());
            }
            settings.llm.threads = val;
        }
        "embedded" => {
            settings.llm.embedded = serde_json::from_value(value.clone())
                .map_err(|e| format!("Invalid LLM embedded config: {}", e))?;
        }
        "server" => {
            settings.llm.server = serde_json::from_value(value.clone())
                .map_err(|e| format!("Invalid LLM server config: {}", e))?;
        }
        "cloud" => {
            settings.llm.cloud = serde_json::from_value(value.clone())
                .map_err(|e| format!("Invalid LLM cloud config: {}", e))?;
        }
        "cloud_keys" => {
            settings.llm.cloud_keys = serde_json::from_value(value.clone())
                .map_err(|e| format!("Invalid LLM cloud_keys config: {}", e))?;
        }
        "provider" => {
            if let Ok(prov) = serde_json::from_value::<LlmProviderConfig>(value.clone()) {
                match prov {
                    LlmProviderConfig::Embedded => {
                        settings.llm.active = LlmActiveProvider::Embedded;
                    }
                    LlmProviderConfig::OpenAiCompat {
                        base_url,
                        model,
                        api_key,
                        provider_name,
                    } => {
                        let is_cloud = provider_name.as_deref().is_some_and(|p| {
                            let pl = p.to_lowercase();
                            pl.contains("nvidia")
                                || pl.contains("groq")
                                || pl.contains("openrouter")
                                || pl.contains("together")
                                || pl.contains("openai")
                                || pl.contains("gemini")
                                || pl.contains("mistral")
                        });
                        if is_cloud {
                            settings.llm.active = LlmActiveProvider::Cloud;
                            settings.llm.cloud = LlmRemoteConfig {
                                base_url,
                                model,
                                api_key,
                                provider_name,
                            };
                        } else {
                            settings.llm.active = LlmActiveProvider::Server;
                            settings.llm.server = LlmRemoteConfig {
                                base_url,
                                model,
                                api_key,
                                provider_name,
                            };
                        }
                    }
                }
            }
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
            settings.tts.active = serde_json::from_value(value.clone())
                .map_err(|e| format!("Invalid TTS active provider: {}", e))?;
        }
        "voice" | "voice_index" => {
            let val = value.as_i64().ok_or("voice must be an integer")? as i32;
            if !(0..=1000).contains(&val) {
                return Err("voice index must be between 0 and 1000".to_string());
            }
            settings.tts.voice_index = val;
        }
        "quality_steps" => {
            let val = value
                .as_u64()
                .ok_or("quality_steps must be a positive integer")? as u32;
            if !(1..=20).contains(&val) {
                return Err("quality_steps must be between 1 and 20".to_string());
            }
            settings.tts.quality_steps = val;
        }
        "speed" => {
            settings.tts.speed = value.as_f64().ok_or("speed must be a number")? as f32;
        }
        "threads" => {
            let t = value.as_u64().ok_or("threads must be a positive integer")? as u32;
            if !(1..=64).contains(&t) {
                return Err("tts threads must be between 1 and 64".to_string());
            }
            settings.tts.threads = t;
        }
        "edge_tts" => {
            settings.tts.edge_tts = serde_json::from_value(value.clone())
                .map_err(|e| format!("Invalid edge_tts config: {}", e))?;
        }
        "supertonic" => {
            settings.tts.supertonic = serde_json::from_value(value.clone())
                .map_err(|e| format!("Invalid supertonic config: {}", e))?;
        }
        "chatterbox" => {
            settings.tts.chatterbox = serde_json::from_value(value.clone())
                .map_err(|e| format!("Invalid chatterbox config: {}", e))?;
        }
        "chatterbox_remote" => {
            settings.tts.chatterbox_remote = serde_json::from_value(value.clone())
                .map_err(|e| format!("Invalid chatterbox_remote config: {}", e))?;
        }
        "provider" => {
            if let Ok(prov) = serde_json::from_value::<TtsProviderConfig>(value.clone()) {
                match prov {
                    TtsProviderConfig::Supertonic => {
                        settings.tts.active = TtsActiveProvider::Supertonic;
                    }
                    TtsProviderConfig::Kokoro => {
                        settings.tts.active = TtsActiveProvider::Kokoro;
                    }
                    TtsProviderConfig::EdgeTts { voice } => {
                        settings.tts.active = TtsActiveProvider::EdgeTts;
                        settings.tts.edge_tts.voice = voice;
                    }
                    TtsProviderConfig::Chatterbox {
                        language,
                        quality_steps,
                        speed,
                        voice_id,
                    } => {
                        settings.tts.active = TtsActiveProvider::Chatterbox;
                        settings.tts.chatterbox.language = language;
                        settings.tts.chatterbox.voice_id = voice_id;
                        settings.tts.quality_steps = quality_steps;
                        settings.tts.speed = speed;
                    }
                    TtsProviderConfig::ChatterboxRemote {
                        endpoint,
                        language,
                        quality_steps,
                        speed,
                        remote_path,
                        voice_id,
                    } => {
                        settings.tts.active = TtsActiveProvider::ChatterboxRemote;
                        settings.tts.chatterbox_remote.endpoint = endpoint;
                        settings.tts.chatterbox_remote.language = language;
                        settings.tts.chatterbox_remote.remote_path = remote_path;
                        settings.tts.chatterbox_remote.voice_id = voice_id;
                        settings.tts.quality_steps = quality_steps;
                        settings.tts.speed = speed;
                    }
                }
            }
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
            settings.interaction.mode = serde_json::from_value(value.clone())
                .map_err(|e| format!("Invalid interaction mode: {}", e))?;
        }
        "auto_sleep_timeout" => {
            settings.interaction.auto_sleep_timeout = value
                .as_u64()
                .ok_or("auto_sleep_timeout must be a positive integer")?
                as u32;
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
        "active" | "provider" => {
            settings.realtime.active = serde_json::from_value(value.clone())
                .map_err(|e| format!("Invalid realtime provider: {}", e))?;
        }
        "gemini" | "gemini_live" => {
            settings.realtime.gemini_live = serde_json::from_value(value.clone())
                .map_err(|e| format!("Invalid gemini config: {}", e))?;
        }
        "openai" | "openai_realtime" => {
            settings.realtime.openai_realtime = serde_json::from_value(value.clone())
                .map_err(|e| format!("Invalid openai config: {}", e))?;
        }
        "deepgram" | "deepgram_voice_agent" => {
            settings.realtime.deepgram_voice_agent = serde_json::from_value(value.clone())
                .map_err(|e| format!("Invalid deepgram config: {}", e))?;
        }
        "elevenlabs" | "elevenlabs_convai" => {
            settings.realtime.elevenlabs_convai = serde_json::from_value(value.clone())
                .map_err(|e| format!("Invalid elevenlabs config: {}", e))?;
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
        "telemetry_enabled" => {
            settings.system.telemetry_enabled = value
                .as_bool()
                .ok_or("telemetry_enabled must be a boolean")?;
        }
        "log_level" => {
            settings.system.log_level = value
                .as_str()
                .ok_or("log_level must be a string")?
                .to_string();
        }
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
        "audio" => apply_audio_mutation(settings, key, value),
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
