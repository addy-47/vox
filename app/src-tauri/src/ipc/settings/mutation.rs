use std::{
    sync::{atomic::Ordering, Arc},
    time::Duration,
};

use tauri::{AppHandle, Manager, State};

use crate::{
    core::{
        error::VoxIpcError,
        events::{emit_ipc, IpcEvent},
        settings::{
            get_setting_reload_policy, AudioOutputMode, DictationInteractionMode,
            DictationOutputMode, InteractionMode, LlmActiveProvider, LlmProviderConfig,
            LlmRemoteConfig, SettingReloadPolicy, SttActiveProvider, SttCloudConfig,
            SttProviderConfig, TtsActiveProvider, TtsProviderConfig, VoxSettings,
        },
        start_audio_engine,
        state::{AppState, InteractionOwner, InteractionState},
        stop_audio_engine,
    },
    ipc::pipeline::{launch_engine, stop_engine},
    pipeline::dictation::transition_dictation,
    services::{
        dictation::init_dictation_hotkey_listener,
        tts::TtsCommand,
        vad::{VadCommand, VadOperationalMode},
    },
    tray::{destroy_tray_window, ensure_tray_window},
};

/// Disk write is deferred by this duration after the last setting change.
/// Prevents thrashing disk on rapid slider updates (dozens of changes/sec).
const SETTINGS_SAVE_DEBOUNCE_MS: u64 = 1500;

#[derive(Debug, Clone, serde::Serialize)]
pub struct SettingUpdateResult {
    pub applied: bool,
    pub reload_policy: String,
    pub message: String,
}

async fn handle_dictation_side_effects<R: tauri::Runtime>(
    app: &AppHandle<R>,
    state: &AppState,
    key: &str,
    value: &serde_json::Value,
) {
    if key == "enabled" {
        let enabled = value.as_bool().unwrap_or(true);
        log::info!("[Settings] Dictation Lifecycle Event: enabled={}", enabled);

        let new_dict_state = if enabled {
            InteractionState::Ready
        } else {
            InteractionState::Idle
        };
        transition_dictation(new_dict_state, app, state);

        let owner: InteractionOwner = state.owner.load(Ordering::Relaxed).into();
        if enabled && owner == InteractionOwner::Dictation {
            let dictation_mode = state
                .settings
                .read()
                .map(|s| s.dictation.interaction_mode.clone())
                .unwrap_or(DictationInteractionMode::Ptt);
            let vad_op_mode = match dictation_mode {
                DictationInteractionMode::Passive => VadOperationalMode::ContinuousSegmentation,
                DictationInteractionMode::Ptt => VadOperationalMode::WindowedValidation,
            };
            if let Ok(guard) = state.engine.try_lock() {
                if let Some(ref engine) = *guard {
                    let _ = engine
                        .vad_tx
                        .send(VadCommand::SetOperationalMode(vad_op_mode));
                }
            }
        }

        let is_tray_mode = state
            .settings
            .read()
            .map(|s| s.dictation.output_mode == DictationOutputMode::Tray)
            .unwrap_or(false);
        let is_clickable = enabled && is_tray_mode;
        let menu_item_lock = state.hud_menu_item.lock();
        if let Some(ref live_i) = *menu_item_lock {
            if let Err(e) = live_i.set_enabled(is_clickable) {
                log::warn!(
                    "[Settings::Mutation] Failed to set menu item enabled: {}",
                    e
                );
            }
            let hud_visible = state.hud_visible.load(Ordering::Relaxed);
            if let Err(e) = live_i.set_checked(hud_visible && is_clickable) {
                log::warn!(
                    "[Settings::Mutation] Failed to set menu item checked: {}",
                    e
                );
            }
        }

        if !enabled {
            destroy_tray_window(app);

            if state.pipeline.state() == InteractionState::Idle {
                let state_clone = app.state::<Arc<AppState>>().inner().clone();
                tauri::async_runtime::spawn(async move {
                    if let Err(e) = stop_audio_engine(&state_clone).await {
                        log::warn!("[Settings::Mutation] Failed to stop audio engine: {}", e);
                    }
                });
            }
        } else {
            if is_tray_mode {
                if let Err(e) = ensure_tray_window(app) {
                    log::warn!("[Settings::Mutation] Failed to ensure tray window: {}", e);
                }
            }
            let app_clone = app.clone();
            let state_clone = app.state::<Arc<AppState>>().inner().clone();
            tauri::async_runtime::spawn(async move {
                if let Err(e) = start_audio_engine(&app_clone, &state_clone).await {
                    log::error!("[Settings] Failed to launch engine for dictation: {}", e);
                }
            });
        }
    } else if key == "output_mode" {
        let (enabled, output_mode) = state
            .settings
            .read()
            .map(|s| (s.dictation.enabled, s.dictation.output_mode.clone()))
            .unwrap_or((false, DictationOutputMode::Paste));
        let is_tray_mode = output_mode == DictationOutputMode::Tray;
        let is_clickable = enabled && is_tray_mode;

        let menu_item_lock = state.hud_menu_item.lock();
        if let Some(ref live_i) = *menu_item_lock {
            if let Err(e) = live_i.set_enabled(is_clickable) {
                log::warn!(
                    "[Settings::Mutation] Failed to set menu item enabled: {}",
                    e
                );
            }
            let hud_visible = state.hud_visible.load(Ordering::Relaxed);
            if let Err(e) = live_i.set_checked(hud_visible && is_clickable) {
                log::warn!(
                    "[Settings::Mutation] Failed to set menu item checked: {}",
                    e
                );
            }
        }

        if enabled && is_tray_mode {
            if let Err(e) = ensure_tray_window(app) {
                log::warn!("[Settings::Mutation] Failed to ensure tray window: {}", e);
            }
        } else if !is_tray_mode {
            destroy_tray_window(app);
        }
    } else if key == "interaction_mode" {
        let owner: InteractionOwner = state.owner.load(Ordering::Relaxed).into();
        if owner == InteractionOwner::Dictation {
            if let Ok(mode) = serde_json::from_value::<DictationInteractionMode>(value.clone()) {
                let vad_op_mode = match mode {
                    DictationInteractionMode::Passive => VadOperationalMode::ContinuousSegmentation,
                    DictationInteractionMode::Ptt => VadOperationalMode::WindowedValidation,
                };
                if let Ok(guard) = state.engine.try_lock() {
                    if let Some(ref engine) = *guard {
                        if let Err(e) = engine
                            .vad_tx
                            .send(VadCommand::SetOperationalMode(vad_op_mode))
                        {
                            log::warn!("[Settings::Mutation] Failed to update VAD mode on dictation interaction_mode change: {}", e);
                        }
                    }
                }
            }
        }
    } else if key == "hotkey" {
        if let Some(new_shortcut) = value.as_str() {
            log::info!(
                "[Settings::Mutation] Re-registering global dictation hotkey: {}",
                new_shortcut
            );
            if let Err(e) = init_dictation_hotkey_listener(app, new_shortcut) {
                log::warn!(
                    "[Settings::Mutation] Failed to re-register dictation hotkey: {:?}",
                    e
                );
            }
        }
    }
}

async fn handle_interaction_side_effects<R: tauri::Runtime>(
    app: &AppHandle<R>,
    state: &AppState,
    key: &str,
    value: &serde_json::Value,
) {
    if key == "mode" {
        let (dictation_enabled, interaction_mode) = state
            .settings
            .read()
            .map(|s| (s.dictation.enabled, s.interaction.mode.clone()))
            .unwrap_or((false, InteractionMode::PTT));

        if !dictation_enabled
            && state.pipeline.state() == InteractionState::Idle
            && interaction_mode == InteractionMode::PTT
        {
            let app_clone = app.clone();
            tauri::async_runtime::spawn(async move {
                if let Err(e) = stop_engine(app_clone).await {
                    log::warn!("[Settings::Mutation] Failed to stop engine: {}", e);
                }
            });
        } else if interaction_mode == InteractionMode::Passive {
            let app_clone = app.clone();
            tauri::async_runtime::spawn(async move {
                if let Err(e) = launch_engine(app_clone).await {
                    log::warn!("[Settings::Mutation] Failed to launch engine: {}", e);
                }
            });
        }

        let owner: InteractionOwner = state.owner.load(Ordering::Relaxed).into();
        if owner == InteractionOwner::Assistant {
            if let Some(engine) = state.engine.lock().await.as_ref() {
                if let Ok(mode) = serde_json::from_value::<InteractionMode>(value.clone()) {
                    if let Err(e) = engine.vad_tx.send(VadCommand::UpdateMode(mode)) {
                        log::warn!("[Settings] Failed to send VadCommand::UpdateMode: {}", e);
                    }
                }
            }
        }
    }
}

async fn handle_setting_side_effects<R: tauri::Runtime>(
    app: &AppHandle<R>,
    state: &AppState,
    domain: &str,
    key: &str,
    value: &serde_json::Value,
) {
    if domain == "history" && key == "private_mode" {
        let is_private = value.as_bool().unwrap_or(false);
        state
            .telemetry
            .is_private_mode
            .store(is_private, Ordering::Relaxed);
        log::info!("[Settings] Privacy Mode updated: enabled={}", is_private);
    } else if domain == "dictation" {
        handle_dictation_side_effects(app, state, key, value).await;
    } else if domain == "interaction" {
        handle_interaction_side_effects(app, state, key, value).await;
    } else if domain == "vad" && key == "vad_backend" {
        log::info!("[Settings] VAD backend changed. Hot-swapping 3-Tier Engine...");
        let app_clone = app.clone();
        tauri::async_runtime::spawn(async move {
            if let Err(e) = stop_engine(app_clone.clone()).await {
                log::warn!(
                    "[Settings::Mutation] Failed to stop engine on VAD swap: {}",
                    e
                );
            }
            if let Err(e) = launch_engine(app_clone).await {
                log::warn!(
                    "[Settings::Mutation] Failed to launch engine on VAD swap: {}",
                    e
                );
            }
        });
    }
}

/// Generic settings update command.
#[tauri::command]
pub async fn update_setting<R: tauri::Runtime>(
    domain: String,
    key: String,
    value: serde_json::Value,
    app: AppHandle<R>,
) -> Result<SettingUpdateResult, VoxIpcError> {
    let state: State<'_, Arc<AppState>> = app.state();
    let policy = get_setting_reload_policy(&domain, &key);

    let applied = {
        let mut settings = state
            .settings
            .write()
            .map_err(|e| VoxIpcError::Internal(e.to_string()))?;
        apply_setting_mutation(&mut settings, &domain, &key, &value)
            .map_err(VoxIpcError::InvalidArgument)?
    };

    if applied {
        handle_setting_side_effects(&app, &state, &domain, &key, &value).await;
    } else {
        return Ok(SettingUpdateResult {
            applied: false,
            reload_policy: policy.as_str().to_string(),
            message: format!("Unknown setting: {}.{}", domain, key),
        });
    }

    if policy == SettingReloadPolicy::WorkerCommand {
        dispatch_worker_command(&app, &domain, &key, &value).await;
    }

    schedule_debounced_save(state.clone()).await;

    let action_label = match policy {
        SettingReloadPolicy::Hot => "hot-applied",
        SettingReloadPolicy::WorkerCommand => "dispatched to worker",
        SettingReloadPolicy::Restart => "restart required",
    };

    let message = format!("{}.{} = {} — {}", domain, key, value, action_label);
    log::info!("[Settings] Updated: {}", message);

    if let Err(e) = emit_ipc(&app, IpcEvent::SettingsUpdated) {
        log::warn!(
            "[Settings::Mutation] Failed to emit settings-updated: {}",
            e
        );
    }

    Ok(SettingUpdateResult {
        applied: true,
        reload_policy: policy.as_str().to_string(),
        message,
    })
}

/// Resets all settings to system defaults.
#[tauri::command]
pub async fn reset_settings<R: tauri::Runtime>(
    app: AppHandle<R>,
) -> Result<VoxSettings, VoxIpcError> {
    let state: State<'_, Arc<AppState>> = app.state();
    let defaults = VoxSettings::default();
    {
        let mut settings = state
            .settings
            .write()
            .map_err(|e| VoxIpcError::Internal(e.to_string()))?;
        *settings = defaults.clone();
    }

    if let Err(e) = emit_ipc(&app, IpcEvent::SettingsUpdated) {
        log::warn!(
            "[Settings::Mutation] Failed to emit settings-updated: {}",
            e
        );
    }

    schedule_debounced_save(state.clone()).await;

    Ok(defaults)
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
        "context_window" => {
            let val = value
                .as_u64()
                .ok_or("context_window must be a positive integer")? as u32;
            if matches!(
                settings.llm.active,
                LlmActiveProvider::Server | LlmActiveProvider::Cloud
            ) && val < 8192
            {
                return Err(
                    "Cloud/Server LLM providers require a minimum context size of 8192 tokens"
                        .to_string(),
                );
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

fn apply_history_mutation(
    settings: &mut VoxSettings,
    key: &str,
    value: &serde_json::Value,
) -> Result<bool, String> {
    match key {
        "private_mode" => {
            settings.history.private_mode =
                value.as_bool().ok_or("private_mode must be a boolean")?;
        }
        "tray_history_limit" => {
            settings.history.tray_history_limit = value
                .as_u64()
                .ok_or("tray_history_limit must be a positive integer")?
                as u32;
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

fn apply_memory_mutation(
    settings: &mut VoxSettings,
    key: &str,
    value: &serde_json::Value,
) -> Result<bool, String> {
    match key {
        "context_retrieval_enabled" => {
            settings.memory.context_retrieval_enabled = value
                .as_bool()
                .ok_or("context_retrieval_enabled must be a boolean")?;
        }
        "pipeline_processing_enabled" => {
            settings.memory.pipeline_processing_enabled = value
                .as_bool()
                .ok_or("pipeline_processing_enabled must be a boolean")?;
        }
        "max_context_share" => {
            let val = value.as_f64().ok_or("max_context_share must be a number")? as f32;
            if !(0.0..=1.0).contains(&val) {
                return Err("max_context_share must be between 0.0 and 1.0".to_string());
            }
            settings.memory.max_context_share = val;
        }
        "context_chaining_window_hours" => {
            settings.memory.context_chaining_window_hours = value
                .as_u64()
                .ok_or("context_chaining_window_hours must be a positive integer")?
                as u32;
        }
        "top_k_facts" => {
            let top_k = value
                .as_u64()
                .ok_or("top_k_facts must be a positive integer")? as u32;
            if top_k == 0 || top_k > 100 {
                return Err("top_k_facts must be between 1 and 100".to_string());
            }
            settings.memory.top_k_facts = top_k;
        }
        "max_hops" => {
            let max_hops = value
                .as_u64()
                .ok_or("max_hops must be a positive integer")? as u32;
            if max_hops == 0 || max_hops > 10 {
                return Err("max_hops must be between 1 and 10".to_string());
            }
            settings.memory.max_hops = max_hops;
        }
        "semantic_similarity_cutoff" => {
            let val = value
                .as_f64()
                .ok_or("semantic_similarity_cutoff must be a number")? as f32;
            if !(0.0..=1.0).contains(&val) {
                return Err("semantic_similarity_cutoff must be between 0.0 and 1.0".to_string());
            }
            settings.memory.semantic_similarity_cutoff = val;
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
        "history" => apply_history_mutation(settings, key, value),
        "persona" => apply_persona_mutation(settings, key, value),
        "realtime" => apply_realtime_mutation(settings, key, value),
        "memory" => apply_memory_mutation(settings, key, value),
        "system" => apply_system_mutation(settings, key, value),
        _ => Ok(false),
    }
}

/// Dispatches a hot-update command to the appropriate worker thread.
/// Called only for `WorkerCommand` policy settings.
async fn dispatch_worker_command<R: tauri::Runtime>(
    app: &AppHandle<R>,
    domain: &str,
    key: &str,
    value: &serde_json::Value,
) {
    let state: State<'_, Arc<AppState>> = app.state();
    let engine_lock = state.engine.lock().await;

    if let Some(engine) = engine_lock.as_ref() {
        match (domain, key) {
            ("vad", "threshold") => {
                if let Some(v) = value.as_f64() {
                    if let Err(e) = engine.vad_tx.send(VadCommand::UpdateThreshold(v as f32)) {
                        log::warn!(
                            "[Settings] Failed to send VadCommand::UpdateThreshold: {}",
                            e
                        );
                    }
                    log::debug!("[Settings] VadCommand::UpdateThreshold({}) dispatched", v);
                }
            }
            ("vad", "ptt_noise_gate") => {
                if let Some(v) = value.as_f64() {
                    if let Err(e) = engine.vad_tx.send(VadCommand::UpdateNoiseGate(v as f32)) {
                        log::warn!(
                            "[Settings] Failed to send VadCommand::UpdateNoiseGate: {}",
                            e
                        );
                    }
                    log::debug!("[Settings] VadCommand::UpdateNoiseGate({}) dispatched", v);
                }
            }
            ("vad", "silence_duration_ms") => {
                if let Some(v) = value.as_u64() {
                    if let Err(e) = engine
                        .vad_tx
                        .send(VadCommand::UpdateSilenceDuration(v as u32))
                    {
                        log::warn!(
                            "[Settings] Failed to send VadCommand::UpdateSilenceDuration: {}",
                            e
                        );
                    }
                    log::debug!(
                        "[Settings] VadCommand::UpdateSilenceDuration({}) dispatched",
                        v
                    );
                }
            }
            ("vad", "speech_onset_ms") => {
                if let Some(v) = value.as_u64() {
                    if let Err(e) = engine.vad_tx.send(VadCommand::UpdateSpeechOnset(v as u32)) {
                        log::warn!(
                            "[Settings] Failed to send VadCommand::UpdateSpeechOnset: {}",
                            e
                        );
                    }
                    log::debug!("[Settings] VadCommand::UpdateSpeechOnset({}) dispatched", v);
                }
            }
            ("audio", "output_mode") => {
                if let Ok(mode) = serde_json::from_value::<AudioOutputMode>(value.clone()) {
                    if let Err(e) = engine.vad_tx.send(VadCommand::UpdateAudioMode(mode)) {
                        log::warn!(
                            "[Settings] Failed to send VadCommand::UpdateAudioMode: {}",
                            e
                        );
                    }
                    log::debug!("[Settings] VadCommand::UpdateAudioMode dispatched");
                }
            }
            ("tts", "voice_index" | "voice") => {
                if let Some(ref tts_tx) = engine.tts_tx {
                    let voice_opt = value
                        .as_i64()
                        .map(|v| v as i32)
                        .or_else(|| value.as_str().and_then(|s| s.parse::<i32>().ok()));
                    if let Some(voice) = voice_opt {
                        if let Err(e) = tts_tx.send(TtsCommand::SetVoice(voice)) {
                            log::warn!("[Settings] Failed to send TtsCommand::SetVoice: {}", e);
                        }
                        log::debug!("[Settings] TtsCommand::SetVoice({}) dispatched", voice);
                    }
                }
            }
            _ => {}
        }
    }
}

/// Schedules a debounced settings save: cancels any pending save, spawns a new
/// task that waits `SETTINGS_SAVE_DEBOUNCE_MS` then writes to disk.
async fn schedule_debounced_save(state: State<'_, Arc<AppState>>) {
    let mut debounce = state.save_debounce.lock().await;

    // Cancel the previous pending write
    if let Some(handle) = debounce.take() {
        handle.abort();
    }

    let snapshot = state
        .settings
        .read()
        .unwrap_or_else(|p| p.into_inner())
        .clone();

    let handle = tauri::async_runtime::spawn(async move {
        tokio::time::sleep(Duration::from_millis(SETTINGS_SAVE_DEBOUNCE_MS)).await;

        // Use spawn_blocking to avoid stalling the async executor with synchronous I/O
        let result = tokio::task::spawn_blocking(move || snapshot.save()).await;

        match result {
            Ok(Ok(_)) => log::debug!("[Settings] Debounced save completed."),
            Ok(Err(e)) => log::error!("[Settings] Debounced save failed: {}", e),
            Err(e) => log::error!("[Settings] Debounced save task panicked: {}", e),
        }
    });

    *debounce = Some(handle);
}
