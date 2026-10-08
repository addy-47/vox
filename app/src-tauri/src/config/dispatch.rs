use std::{
    sync::{atomic::Ordering, Arc},
    time::Duration,
};

use tauri::{AppHandle, Manager, State};

use super::AudioOutputMode;
use crate::{
    core::{
        engine::{ensure_memory_embedder, start_audio_engine, stop_audio_engine},
        events::InteractionMode,
        state::{AppState, InteractionOwner, InteractionState},
    },
    ipc::pipeline::{launch_engine, restart_engine_inner, stop_engine},
    pipeline::dictation::{transition_dictation, DictationInteractionMode, DictationOutputMode},
    services::{
        dictation::init_dictation_hotkey_listener,
        memory::{
            spawn_ingestion_sweep, start_consolidation_scheduler, stop_consolidation_scheduler,
            unload_memory_pipeline_onnx_models,
        },
        tts::TtsCommand,
        vad::{VadCommand, VadOperationalMode},
    },
    tray::{destroy_tray_window, ensure_tray_window},
};

/// Disk write is deferred by this duration after the last setting change.
/// Prevents thrashing disk on rapid slider updates (dozens of changes/sec).
pub const SETTINGS_SAVE_DEBOUNCE_MS: u64 = 1500;

pub const RESTART_COALESCE_MS: u64 = 150;

pub fn request_engine_restart<R: tauri::Runtime>(
    app: &AppHandle<R>,
    state: &Arc<AppState>,
    reason: &str,
) -> bool {
    state.restart_requested.store(true, Ordering::Release);

    if state
        .restart_runner
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        log::debug!(
            "[Settings] Restart already in progress; folding '{}' into it",
            reason
        );
        return true;
    }

    let app = app.clone();
    let state = Arc::clone(state);
    let reason = reason.to_string();
    tauri::async_runtime::spawn(async move {
        log::info!("[Settings] Engine restart scheduled ({})", reason);
        loop {
            tokio::time::sleep(Duration::from_millis(RESTART_COALESCE_MS)).await;

            if !state.restart_requested.swap(false, Ordering::AcqRel) {
                state.restart_runner.store(false, Ordering::Release);
                if !state.restart_requested.swap(false, Ordering::AcqRel) {
                    break;
                }
                if state
                    .restart_runner
                    .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
                    .is_err()
                {
                    break;
                }
                continue;
            }

            state.restart_in_flight.store(true, Ordering::Release);
            let outcome = restart_engine_inner(&app, &state).await;
            state.restart_in_flight.store(false, Ordering::Release);

            match outcome {
                Ok(()) => log::info!("[Settings] Engine restart completed"),
                Err(e) => log::error!("[Settings] Engine restart failed: {}", e),
            }
        }
        log::debug!("[Settings] Engine restart runner finished");
    });

    true
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
                .map(|s| s.dictation.interaction_mode)
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
            .map(|s| (s.dictation.enabled, s.dictation.output_mode))
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
                "[Dictation::Trace] Settings mutation: re-registering global dictation hotkey: '{}'",
                new_shortcut
            );
            if let Err(e) = init_dictation_hotkey_listener(app, new_shortcut) {
                log::warn!(
                    "[Dictation::Trace] Failed to re-register dictation hotkey: {:?}",
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
            .map(|s| (s.dictation.enabled, s.interaction.mode))
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

pub async fn handle_setting_side_effects<R: tauri::Runtime>(
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
    } else if domain == "personal_memory" {
        if key == "consolidation_cadence" || key == "consolidation_time" {
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
        } else if key == "context_retrieval_enabled" {
            let enabled = value.as_bool().unwrap_or(false);
            if let Some(ref mut h) = *state.harness.lock() {
                h.set_memory_retrieval_enabled(enabled);
            }
            if enabled {
                let is_active = state.pipeline.state() != InteractionState::Idle;
                if is_active {
                    let settings = state
                        .settings
                        .read()
                        .unwrap_or_else(|p| p.into_inner())
                        .clone();
                    ensure_memory_embedder(&settings);
                }
            } else {
                tauri::async_runtime::spawn_blocking(|| {
                    unload_memory_pipeline_onnx_models();
                    log::info!("[Settings] Memory embedder evicted on retrieval disable");
                });
            }
        } else if key == "pipeline_processing_enabled" {
            let enabled = value.as_bool().unwrap_or(false);
            if enabled {
                let state_arc = app.state::<Arc<AppState>>().inner().clone();
                spawn_ingestion_sweep(state_arc, Some(app.clone()), None);
                log::info!("[Settings] Pipeline processing enabled; spawned ingestion sweep");
            } else if let Some(token) = state.ingestion_cancel.lock().take() {
                token.cancel();
                log::info!("[Settings] Pipeline processing disabled; cancelled running ingestion sweep");
            }
        }
    } else if domain == "working_memory" && key == "web_search_enabled" {
        let enabled = value.as_bool().unwrap_or(false);
        if let Some(ref mut h) = *state.harness.lock() {
            h.set_web_search_enabled(enabled);
        }
        log::info!(
            "[Settings] Working memory web search runtime enabled={}",
            enabled
        );
    }
}

/// Dispatches a hot-update command to the appropriate worker thread.
/// Called only for `WorkerCommand` policy settings.
pub async fn dispatch_worker_command<R: tauri::Runtime>(
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
            _ => {}
        }
    }
}

/// Returns true if `dispatch_worker_command` handles the given `(domain, key)` combination.
pub fn dispatch_worker_command_has_arm(domain: &str, key: &str) -> bool {
    matches!(
        (domain, key),
        ("vad", "threshold")
            | ("vad", "ptt_noise_gate")
            | ("vad", "silence_duration_ms")
            | ("vad", "speech_onset_ms")
            | ("audio", "output_mode")
            | ("tts", "voice_index" | "voice")
            | ("tts", "speed")
    )
}

/// Schedules a debounced settings save: cancels any pending save, spawns a new
/// task that waits `SETTINGS_SAVE_DEBOUNCE_MS` then writes to disk.
pub async fn schedule_debounced_save(state: Arc<AppState>) {
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
