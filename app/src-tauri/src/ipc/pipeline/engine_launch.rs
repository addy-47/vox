//! ============================================================================
//! src/ipc/pipeline/engine_launch.rs — 3-Tier Audio Engine launching and worker initialization
//! ============================================================================

use crate::core::events::VoxEvent;
use crate::core::state::{AppState, InteractionOwner, VoxEngine};
use crate::services::audio::{AudioStream, PlaybackEngine};
use crate::services::pipeline::PipelineOrchestrator;
use crate::services::stt::providers::create_stt_provider;
use crate::services::stt::{spawn_stt_worker, SttCommand};
use crate::services::vad::{
    earshot_vad::EarshotVadEngine, ten_onnx::VadEngine as TenVadEngine, VadBackend,
};
use crate::tray::position_tray_window;
use crate::utils::paths;
use ringbuf::traits::Split;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use tauri::{Emitter, Manager, State};

#[tauri::command]
pub async fn launch_engine(app: tauri::AppHandle) -> Result<(), String> {
    let state: State<'_, std::sync::Arc<AppState>> = app.state();
    let mut lock = state.engine.lock().await;

    if lock.is_some() {
        let (should_show_tray, setup_completed) = {
            let s = state.settings.read().unwrap();
            (s.dictation.enabled && s.dictation.output_mode == crate::core::settings::DictationOutputMode::Tray, s.setup.completed)
        };
        if setup_completed && should_show_tray {
            if let Some(window) = app.get_webview_window("tray") {
                let _ = window.show();
                position_tray_window(&window).await;
                let _ = window.set_focus();
            }
        }
        return Ok(());
    }

    log::info!("[PIPELINE] >>> Launching 3-Tier Audio Engine...");

    // We MUST reset them before spawning workers, otherwise they exit immediately.
    state
        .pipeline
        .engine_shutdown
        .store(false, Ordering::Relaxed);
    state.pipeline.cancel_flag.store(false, Ordering::Relaxed);

    // ── Re-spawn Persistence Worker if needed ──────────────────────────────
    // If the app was idle, the persistence worker was shut down to free the DB lock.
    {
        let mut persist_lock = state.persist_tx.lock();
        if persist_lock.is_none() {
            log::info!("[PIPELINE] Re-spawning Persistence Worker...");
            let tx = crate::persistence::worker::spawn_persistence_worker(
                crate::utils::paths::get().db.clone(),
                std::sync::Arc::clone(&state.is_db_healthy),
                std::sync::Arc::clone(&state.latest_persistence_rate),
                std::sync::Arc::clone(&state.is_private_mode),
            );
            *persist_lock = Some(tx);
        }
    }
    // ────────────────────────────────────────────────────────────────────────

    // Ensure manifest is loaded from cache/disk before launching engine
    {
        let mut m = state.manifest.write().await;
        if m.is_none() {
            let manifest_path = crate::utils::paths::get()
                .models
                .join("models_manifest.json");
            if manifest_path.exists() {
                if let Ok(content) = std::fs::read_to_string(&manifest_path) {
                    if let Ok(manifest) =
                        serde_json::from_str::<crate::setup::manifest::VoxManifest>(&content)
                    {
                        *m = Some(manifest);
                    }
                }
            }
        }
    }

    app.emit(crate::core::constants::EVENT_MODEL_LOADING, "VAD")
        .ok();

    let (_stt_model_path, stt_provider, vad_model_path_opt, vad_backend_opt, input_device) = {
        let (vad_backend, asr_provider, input_device) = {
            let settings = state.settings.read().unwrap();
            (
                settings.vad.vad_backend.clone(),
                settings.asr.provider.clone(),
                settings.audio.input_device.clone(),
            )
        };
        let models_dir = paths::get().models.clone();
        let manifest_lock = state.manifest.read().await;

        // Resolve STT model path and create the provider (always pre-loaded)
        app.emit(crate::core::constants::EVENT_MODEL_LOADING, "STT")
            .ok();
        let (stt_path, p) = match asr_provider {
            crate::core::settings::SttProviderConfig::Embedded { ref model_type } => {
                let path = match model_type.as_str() {
                    "nvidia_nemotron" => {
                        models_dir.join(crate::services::stt::MODEL_DIR_STT_NEMOTRON)
                    }
                    _ => models_dir.join(crate::services::stt::MODEL_DIR_STT_QWEN),
                };
                match create_stt_provider(&asr_provider, &path) {
                    Ok(provider) => {
                        app.emit(crate::core::constants::EVENT_MODEL_READY, "STT")
                            .ok();
                        (path, provider)
                    }
                    Err(e) => {
                        app.emit(
                            crate::core::constants::EVENT_MODEL_FAILED,
                            format!("STT: {}", e),
                        )
                        .ok();
                        return Err(format!("[Pipeline] Failed to create STT provider: {}", e));
                    }
                }
            }
            crate::core::settings::SttProviderConfig::Cloud { .. } => {
                // For cloud providers, model_path is not used (auth is configured separately)
                let path = models_dir.join("stt");
                match create_stt_provider(&asr_provider, &path) {
                    Ok(provider) => {
                        app.emit(crate::core::constants::EVENT_MODEL_READY, "STT")
                            .ok();
                        (path, provider)
                    }
                    Err(e) => {
                        app.emit(
                            crate::core::constants::EVENT_MODEL_FAILED,
                            format!("STT: {}", e),
                        )
                        .ok();
                        return Err(format!("[Pipeline] Failed to create STT provider: {}", e));
                    }
                }
            }
        };

        // Only resolve the VAD model path for TenVAD — earshot has no external model file.
        let vad_path = if vad_backend == crate::core::settings::VadBackendOption::TenVad {
            if let Some(ref manifest) = *manifest_lock {
                if let Some(group) = manifest.model_groups.iter().find(|g| g.id == "ten_vad") {
                    if let Some(file) = group.files.first() {
                        Some(models_dir.join(&file.path))
                    } else {
                        Some(
                            models_dir
                                .join(crate::services::vad::MODEL_DIR_VAD)
                                .join(crate::services::vad::MODEL_FILE_VAD),
                        )
                    }
                } else {
                    Some(
                        models_dir
                            .join(crate::services::vad::MODEL_DIR_VAD)
                            .join(crate::services::vad::MODEL_FILE_VAD),
                    )
                }
            } else {
                Some(
                    models_dir
                        .join(crate::services::vad::MODEL_DIR_VAD)
                        .join(crate::services::vad::MODEL_FILE_VAD),
                )
            }
        } else {
            None
        };

        (stt_path, p, vad_path, vad_backend, input_device)
    };

    let (event_tx, mut event_rx) = tokio::sync::mpsc::channel::<serde_json::Value>(100);
    let (stt_tx_internal, stt_rx_internal) = std::sync::mpsc::channel::<SttCommand>();
    let (vad_tx_internal, vad_rx_internal) =
        std::sync::mpsc::channel::<crate::core::state::VadCommand>();
    let (vox_event_tx, vox_event_rx) = std::sync::mpsc::channel::<VoxEvent>();

    let stt_handle = spawn_stt_worker(
        app.clone(),
        stt_rx_internal,
        stt_provider,
        Some(vox_event_tx.clone()),
        state.pipeline.cancel_flag.clone(),
        state.is_stt_loaded.clone(),
        state.pipeline.engine_shutdown.clone(),
    )?;

    let threshold = state.settings.read().unwrap().vad.threshold;
    let vad = match vad_backend_opt {
        crate::core::settings::VadBackendOption::Earshot => {
            log::info!("[PIPELINE] VAD backend: Earshot (pure Rust, no model file required).");
            match EarshotVadEngine::new(threshold) {
                Ok(engine) => {
                    app.emit(crate::core::constants::EVENT_MODEL_READY, "VAD")
                        .ok();
                    VadBackend::Earshot(engine)
                }
                Err(e) => {
                    app.emit(
                        crate::core::constants::EVENT_MODEL_FAILED,
                        format!("VAD: {}", e),
                    )
                    .ok();
                    return Err(e.to_string());
                }
            }
        }
        crate::core::settings::VadBackendOption::TenVad => {
            let vad_model_path = vad_model_path_opt.ok_or_else(|| {
                "[PIPELINE] TenVAD selected but model path could not be resolved.".to_string()
            })?;
            log::info!(
                "[PIPELINE] VAD backend: TenVAD (ONNX, model={:?}).",
                vad_model_path
            );
            match TenVadEngine::new(&vad_model_path, threshold) {
                Ok(engine) => {
                    app.emit(crate::core::constants::EVENT_MODEL_READY, "VAD")
                        .ok();
                    VadBackend::Ten(engine)
                }
                Err(e) => {
                    app.emit(
                        crate::core::constants::EVENT_MODEL_FAILED,
                        format!("VAD: {}", e),
                    )
                    .ok();
                    return Err(e.to_string());
                }
            }
        }
    };
    let (producer, consumer) = ringbuf::HeapRb::<f32>::new(16000 * 4).split();

    let stt_tx_for_vad = stt_tx_internal.clone();
    let vad_rx_for_vad = vad_rx_internal;
    let app_handle_vad = app.clone();
    let telemetry_tx_for_vad = state.telemetry_tx.clone();
    let vox_event_tx_for_vad = vox_event_tx.clone();
    let state_vad = state.inner().clone();
    let vad_handle = std::thread::Builder::new()
        .name("vox-vad-worker".to_string())
        .spawn(move || {
            if let Err(e) = crate::services::vad::spawn_vad_actor(
                vad,
                app_handle_vad,
                consumer,
                event_tx,
                stt_tx_for_vad,
                vad_rx_for_vad,
                telemetry_tx_for_vad,
                Some(vox_event_tx_for_vad),
                state_vad.is_vad_loaded.clone(),
            ) {
                log::error!("[VAD] CRITICAL: Worker thread crashed: {}", e);
            }
        })
        .map_err(|e| e.to_string())?;

    let app_handle_emit = app.clone();
    tauri::async_runtime::spawn(async move {
        let app_state: State<'_, std::sync::Arc<AppState>> = app_handle_emit.state();
        while let Some(event) = event_rx.recv().await {
            if let Some(msg_type) = event.get("type").and_then(|v| v.as_str()) {
                let target = {
                    let owner: InteractionOwner = app_state.owner.load(Ordering::Relaxed).into();
                    match owner {
                        InteractionOwner::MainWindow | InteractionOwner::Ptt => "main",
                        InteractionOwner::Dictation => "tray",
                        InteractionOwner::Wizard => "wizard",
                    }
                };
                let _ = app_handle_emit.emit_to(target, msg_type, &event);
            }
        }
    });

    let audio_stream = AudioStream::new(producer, input_device).map_err(|e| e.to_string())?;
    audio_stream.start().map_err(|e| e.to_string())?;

    let (super_tts_path, llm_path) = {
        let llm_model = {
            let settings = state.settings.read().unwrap();
            settings.llm.model.clone()
        };
        let models_dir = paths::get().models.clone();
        let manifest_lock = state.manifest.read().await;

        let llm = if let Some(ref manifest) = *manifest_lock {
            if let Some(group) = manifest.model_groups.iter().find(|g| g.id == llm_model) {
                if let Some(file) = group.files.first() {
                    models_dir.join(&file.path)
                } else {
                    models_dir
                        .join(crate::services::llm::MODEL_DIR_LLM)
                        .join(crate::services::llm::MODEL_FILE_LLM_GGUF)
                }
            } else {
                models_dir
                    .join(crate::services::llm::MODEL_DIR_LLM)
                    .join(crate::services::llm::MODEL_FILE_LLM_GGUF)
            }
        } else {
            models_dir
                .join(crate::services::llm::MODEL_DIR_LLM)
                .join(crate::services::llm::MODEL_FILE_LLM_GGUF)
        };

        let super_tts = models_dir.join(crate::services::tts::MODEL_DIR_TTS_SUPER);

        (super_tts, llm)
    };

    let playback_energy = state.latest_playback_energy.clone();
    let playback_low = state.latest_playback_low.clone();
    let playback_mid = state.latest_playback_mid.clone();
    let playback_high = state.latest_playback_high.clone();

    let playback_engine = match PlaybackEngine::new(
        std::sync::Arc::clone(&state.pipeline.playback_active),
        std::sync::Arc::clone(&state.pipeline.cancel_flag),
        Arc::clone(&playback_energy),
        Arc::clone(&playback_low),
        Arc::clone(&playback_mid),
        Arc::clone(&playback_high),
        std::sync::Arc::clone(&state.pipeline.playback_underruns),
        std::sync::Arc::clone(&state.pipeline.is_assistant_speaking),
    ) {
        Ok(pe) => std::sync::Arc::new(pe),
        Err(e) => {
            log::error!(
                "[Pipeline] PlaybackEngine init failed: {} — TTS output disabled",
                e
            );
            return Ok(());
        }
    };

    // Pipeline Orchestrator now takes Arc<RwLock<VoxSettings>> and the pre-resolved llm_path + super_tts_path
    let orchestrator = PipelineOrchestrator::new(
        std::sync::Arc::clone(&state.pipeline.cancel_flag),
        std::sync::Arc::clone(&state.pipeline.is_paused),
        std::sync::Arc::clone(&state.pipeline.playback_active),
        std::sync::Arc::clone(&state.pipeline.tts_generating),
        std::sync::Arc::clone(&state.pipeline.turn_id),
        std::sync::Arc::clone(&state.pipeline.state),
        vox_event_tx.clone(),
        Arc::clone(&state.settings),
        llm_path,
        super_tts_path,
        std::sync::Arc::clone(&state.pipeline.is_engaged),
        std::sync::Arc::clone(&state.pipeline.transcript_history),
        std::sync::Arc::clone(&state.conversation_id),
        state.persist_tx.lock().clone(),
        std::sync::Arc::clone(&state.dropped_persistence_events),
        std::sync::Arc::clone(&state.latest_voice_latency_ms),
        std::sync::Arc::clone(&state.latest_tts_rtf),
        std::sync::Arc::clone(&state.latest_playback_start_ms),
        std::sync::Arc::clone(&state.is_llm_loaded),
        std::sync::Arc::clone(&state.is_tts_loaded),
        std::sync::Arc::clone(&state.is_sleeping),
        std::sync::Arc::clone(&state.conversation_manager),
    );

    let playback_for_orch = std::sync::Arc::clone(&playback_engine);
    let app_for_orch = app.clone();
    let orchestrator_handle = std::thread::Builder::new()
        .name("vox-pipeline".to_string())
        .spawn(move || {
            orchestrator.run_event_loop(vox_event_rx, playback_for_orch, app_for_orch);
        })
        .map_err(|e| e.to_string())?;

    log::info!("[Pipeline] Phase 4 pipeline online (LLM + TTS + Playback)");

    *lock = Some(VoxEngine {
        audio_stream,
        stt_tx: stt_tx_internal,
        vad_tx: vad_tx_internal,
        telemetry_tx: state.telemetry_tx.clone(),
        pipeline_tx: vox_event_tx.clone(),
        playback_engine,
        stt_handle: Some(stt_handle),
        vad_handle: Some(vad_handle),
        orchestrator_handle: Some(orchestrator_handle),
    });

    Ok(())
}
