use tauri::{State, Manager, AppHandle, Emitter};
use crate::core::state::{AppState, VoxEngine, InteractionOwner, InteractionState};
use crate::core::events::VoxEvent;
use crate::services::stt::{spawn_stt_worker, SttCommand};
use crate::services::audio::AudioStream;
use crate::services::vad::VadEngine;
use crate::services::pipeline::PipelineOrchestrator;
use crate::services::playback::PlaybackEngine;
use crate::tray::position_tray_window;
use crate::telemetry::aggregator::spawn_telemetry_aggregator;
use crate::telemetry::system_monitor::spawn_system_monitor;
use ringbuf::traits::Split;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

#[tauri::command]
pub async fn check_engine_status(state: State<'_, AppState>) -> Result<bool, String> {
    let lock = state.engine.lock().await;
    Ok(lock.is_some())
}

#[tauri::command]
pub async fn engage(state: State<'_, AppState>, app: AppHandle) -> Result<(), String> {
    let current = state.pipeline.is_engaged.load(Ordering::Relaxed);
    let new_state = !current;
    
    if new_state {
        log::info!("[Pipeline] Engaging main application pipeline...");
        state.pipeline.is_engaged.store(true, Ordering::Relaxed);
        let mut owner = state.owner.lock().await;
        *owner = InteractionOwner::MainWindow;

        if let Some(engine) = state.engine.lock().await.as_ref() {
            let _ = engine.pipeline_tx.send(VoxEvent::WarmUp);
        }
    } else {
        log::info!("[Pipeline] Disengaging pipeline (Stopping session)...");
        state.pipeline.is_engaged.store(false, Ordering::Relaxed);
        state.pipeline.cancel_flag.store(true, Ordering::Relaxed);
        
        if let Some(engine) = state.engine.lock().await.as_ref() {
            let session_id = state.pipeline.session_id.load(Ordering::Relaxed);
            let _ = engine.pipeline_tx.send(VoxEvent::Cancelled { session_id });
            let _ = engine.stt_tx.send(SttCommand::ResetStream);
        }

        let mut owner = state.owner.lock().await;
        *owner = InteractionOwner::Tray;

        {
            let mut state_lock = state.pipeline.state.lock().unwrap();
            *state_lock = InteractionState::Idle;
        }
        let _ = app.emit_to("main", "state_changed", InteractionState::Idle);
        let _ = app.emit_to("tray", "state_changed", InteractionState::Idle);
    }
    
    Ok(())
}

#[tauri::command]
pub async fn launch_engine(app: tauri::AppHandle) -> Result<(), String> {
    let state: State<'_, AppState> = app.state();
    let mut lock = state.engine.lock().await;
    
    if lock.is_some() {
        if let Some(window) = app.get_webview_window("tray") {
            position_tray_window(&window).await;
            let _ = window.set_focus();
        }
        return Ok(());
    }

    log::info!("[PIPELINE] >>> Launching 3-Tier Audio Engine...");

    let (stt_model_path, vad_model_path) = {
        let settings = state.settings.lock().await;
        let resource_dir = app.path().resource_dir().unwrap_or_default();
        let models_dir = state.config_dir.join(crate::core::constants::MODELS_DIRNAME);
        
        let stt = if settings.stt_model_dir.is_absolute() {
            settings.stt_model_dir.clone()
        } else {
            // Priority: ~/.vox/models -> assets/ -> current_dir
            let p_persisted = models_dir.join(&settings.stt_model_dir);
            let p_resource = resource_dir.join(&settings.stt_model_dir);
            if p_persisted.exists() { p_persisted } 
            else if p_resource.exists() { p_resource }
            else { std::env::current_dir().unwrap_or_default().join(&settings.stt_model_dir) }
        };

        let vad = if settings.vad_model_path.is_absolute() {
            settings.vad_model_path.clone()
        } else {
            let p_persisted = models_dir.join(&settings.vad_model_path);
            let p_resource = resource_dir.join(&settings.vad_model_path);
            if p_persisted.exists() { p_persisted }
            else if p_resource.exists() { p_resource }
            else { std::env::current_dir().unwrap_or_default().join(&settings.vad_model_path) }
        };

        (stt, vad)
    };

    let (event_tx, mut event_rx) = tokio::sync::mpsc::channel::<serde_json::Value>(100);
    let (stt_tx_internal, stt_rx_internal) = std::sync::mpsc::channel::<SttCommand>();
    let (vox_event_tx, vox_event_rx) = std::sync::mpsc::channel::<VoxEvent>();

    spawn_stt_worker(app.clone(), stt_rx_internal, stt_model_path, Some(vox_event_tx.clone()), state.pipeline.is_engaged.clone());

    let mut vad = VadEngine::new(&vad_model_path).map_err(|e| e.to_string())?;
    let (producer, consumer) = ringbuf::HeapRb::<f32>::new(16000 * 4).split(); 
    
    let playback_energy = Arc::new(AtomicU32::new(0f32.to_bits()));
    let telemetry_tx = spawn_telemetry_aggregator(app.clone(), Arc::clone(&playback_energy));

    let stt_tx_for_vad = stt_tx_internal.clone();
    let app_handle_vad = app.clone();
    let telemetry_tx_for_vad = telemetry_tx.clone();
    let vox_event_tx_for_vad = vox_event_tx.clone();
    std::thread::spawn(move || {
        if let Err(e) = vad.run_sync_loop(app_handle_vad, consumer, event_tx, stt_tx_for_vad, telemetry_tx_for_vad, Some(vox_event_tx_for_vad)) {
            log::error!("[VAD] CRITICAL: Worker thread crashed: {}", e);
        }
    });

    let app_handle_emit = app.clone();
    tauri::async_runtime::spawn(async move {
        let app_state: State<'_, AppState> = app_handle_emit.state();
        while let Some(event) = event_rx.recv().await {
            if let Some(msg_type) = event.get("type").and_then(|v| v.as_str()) {
                if msg_type == "speech_start" {
                    let hud_visible = {
                        let hud_lock = app_state.hud_visible.lock().await;
                        *hud_lock
                    };

                    if hud_visible {
                        if let Some(window) = app_handle_emit.get_webview_window("tray") {
                            let w = window.clone();
                            tauri::async_runtime::spawn(async move {
                                let _ = w.show();
                                position_tray_window(&w).await;
                            });
                        }
                    }
                }
                
                let target = {
                    let owner = app_state.owner.lock().await;
                    match *owner {
                        InteractionOwner::MainWindow | InteractionOwner::Ptt => "main",
                        InteractionOwner::Tray => "tray",
                    }
                };
                let _ = app_handle_emit.emit_to(target, msg_type, &event);
            }
        }
    });

    let audio_stream = AudioStream::new(producer).map_err(|e| e.to_string())?;
    audio_stream.start().map_err(|e| e.to_string())?;

    let (en_tts_dir, hi_tts_dir) = {
        let settings = state.settings.lock().await;
        let resource_dir = app.path().resource_dir().unwrap_or_default();
        let models_dir = state.config_dir.join(crate::core::constants::MODELS_DIRNAME);
        
        let en_tts = if settings.tts_model_dir.is_absolute() {
            settings.tts_model_dir.clone()
        } else {
            let p_persisted = models_dir.join(&settings.tts_model_dir);
            let p_resource = resource_dir.join(&settings.tts_model_dir);
            if p_persisted.exists() { p_persisted }
            else if p_resource.exists() { p_resource }
            else { std::env::current_dir().unwrap_or_default().join(&settings.tts_model_dir) }
        };

        let hi_tts = if settings.tts_hindi_model_dir.is_absolute() {
            settings.tts_hindi_model_dir.clone()
        } else {
            let p_persisted = models_dir.join(&settings.tts_hindi_model_dir);
            let p_resource = resource_dir.join(&settings.tts_hindi_model_dir);
            if p_persisted.exists() { p_persisted }
            else if p_resource.exists() { p_resource }
            else { std::env::current_dir().unwrap_or_default().join(&settings.tts_hindi_model_dir) }
        };

        (en_tts, hi_tts)
    };

    let playback_engine = match PlaybackEngine::new(
        std::sync::Arc::clone(&state.pipeline.playback_active),
        std::sync::Arc::clone(&state.pipeline.cancel_flag),
        Arc::clone(&playback_energy),
    ) {
        Ok(pe) => std::sync::Arc::new(pe),
        Err(e) => {
            log::error!("[Pipeline] PlaybackEngine init failed: {} — TTS output disabled", e);
            return Ok(());
        }
    };

    let mut vox_settings = state.settings.lock().await.clone();
    
    // Resolve LLM path
    let models_dir = state.config_dir.join(crate::core::constants::MODELS_DIRNAME);
    let resource_dir = app.path().resource_dir().unwrap_or_default();
    if !vox_settings.llm_model_path.is_absolute() {
        let p_persisted = models_dir.join(&vox_settings.llm_model_path);
        let p_resource = resource_dir.join(&vox_settings.llm_model_path);
        if p_persisted.exists() {
            vox_settings.llm_model_path = p_persisted;
        } else if p_resource.exists() {
            vox_settings.llm_model_path = p_resource;
        } else {
            vox_settings.llm_model_path = std::env::current_dir().unwrap_or_default().join(&vox_settings.llm_model_path);
        }
    }

    let orchestrator = PipelineOrchestrator::new(
        std::sync::Arc::clone(&state.pipeline.cancel_flag),
        std::sync::Arc::clone(&state.pipeline.playback_active),
        std::sync::Arc::clone(&state.pipeline.llm_generating),
        std::sync::Arc::clone(&state.pipeline.tts_generating),
        std::sync::Arc::clone(&state.pipeline.session_id),
        std::sync::Arc::clone(&state.pipeline.state),
        vox_event_tx.clone(),
        vox_settings,
        std::sync::Arc::clone(&state.pipeline.is_engaged),
        std::sync::Arc::clone(&state.pipeline.transcript_history),
    );

    let playback_for_orch = std::sync::Arc::clone(&playback_engine);
    let app_for_orch = app.clone();
    std::thread::Builder::new()
        .name("vox-pipeline".to_string())
        .spawn(move || {
            orchestrator.run_event_loop(
                vox_event_rx,
                en_tts_dir,
                hi_tts_dir,
                playback_for_orch,
                app_for_orch,
            );
        })
        .map_err(|e| e.to_string())?;

    log::info!("[Pipeline] Phase 4 pipeline online (LLM + TTS + Playback)");

    *lock = Some(VoxEngine {
        audio_stream,
        stt_tx: stt_tx_internal,
        telemetry_tx,
        pipeline_tx: vox_event_tx.clone(),
    });

    spawn_system_monitor(app.clone());

    Ok(())
}
