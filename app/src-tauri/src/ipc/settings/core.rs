use std::{
    sync::{atomic::Ordering, Arc},
    time::Duration,
};

use tauri::{AppHandle, Manager, State};

use super::mutation::apply_setting_mutation;
use crate::{
    core::{
        engine::{start_audio_engine, stop_audio_engine},
        error::VoxIpcError,
        events::{emit_ipc, IpcEvent},
        settings::{
            get_setting_reload_policy, AudioOutputMode, DictationInteractionMode,
            DictationOutputMode, InteractionMode, SettingReloadPolicy, VoxSettings,
        },
        state::{AppState, InteractionOwner, InteractionState},
    },
    ipc::pipeline::{launch_engine, stop_engine},
    pipeline::dictation::transition_dictation,
    services::{
        dictation::init_dictation_hotkey_listener,
        memory::{start_consolidation_scheduler, stop_consolidation_scheduler},
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
                    if let Err(e) = engine
                        .vad_tx
                        .send(VadCommand::SetOperationalMode(vad_op_mode))
                    {
                        log::warn!(
                            "[SettingsMutation] Failed to send SetOperationalMode to VAD: {}",
                            e
                        );
                    }
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
    if domain == "working_memory" && key == "private_mode" {
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
    } else if domain == "personal_memory"
        && (key == "consolidation_cadence" || key == "consolidation_time")
    {
        let cadence = state
            .settings
            .read()
            .map(|s| s.personal_memory.consolidation_cadence.clone())
            .unwrap_or_default();
        if cadence == "daily" {
            let state_arc = app.state::<Arc<AppState>>().inner().clone();
            start_consolidation_scheduler(app.clone(), state_arc);
        } else {
            stop_consolidation_scheduler(state);
        }
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
            ("tts", "speed") => {
                if let Some(ref tts_tx) = engine.tts_tx {
                    let speed_opt = value
                        .as_f64()
                        .map(|v| v as f32)
                        .or_else(|| value.as_str().and_then(|s| s.parse::<f32>().ok()));
                    if let Some(speed) = speed_opt {
                        if let Err(e) = tts_tx.send(TtsCommand::SetSpeed(speed)) {
                            log::warn!("[Settings] Failed to send TtsCommand::SetSpeed: {}", e);
                        }
                        log::debug!("[Settings] TtsCommand::SetSpeed({}) dispatched", speed);
                    }
                }
            }
            ("tts", "quality_steps") => {
                if let Some(ref tts_tx) = engine.tts_tx {
                    let steps_opt = value
                        .as_u64()
                        .map(|v| v as u32)
                        .or_else(|| value.as_str().and_then(|s| s.parse::<u32>().ok()));
                    if let Some(steps) = steps_opt {
                        if let Err(e) = tts_tx.send(TtsCommand::SetQualitySteps(steps)) {
                            log::warn!(
                                "[Settings] Failed to send TtsCommand::SetQualitySteps: {}",
                                e
                            );
                        }
                        log::debug!(
                            "[Settings] TtsCommand::SetQualitySteps({}) dispatched",
                            steps
                        );
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
