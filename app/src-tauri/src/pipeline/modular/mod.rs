pub mod passive;
pub mod ptt;

use crate::core::events::VoxEvent;
use crate::core::settings::InteractionMode;
use crate::core::state::AppState;
use std::sync::Arc;
use tauri::AppHandle;

/// Initializes and warms up the LLM and TTS actor threads if not already loaded.
pub async fn ensure_modular_workers<R: tauri::Runtime + 'static>(
    app: &AppHandle<R>,
    state: &AppState,
) -> Result<(), String> {
    let (llm_path, tts_path, settings) = {
        let s = state
            .settings
            .read()
            .unwrap_or_else(|p| p.into_inner())
            .clone();
        let models_dir = crate::utils::paths::get().models.clone();
        let llm = models_dir
            .join(crate::services::llm::QWEN_MODEL_DIR)
            .join(crate::services::llm::QWEN_MODEL_FILE);
        let tts = models_dir.join(crate::services::tts::SUPERTONIC_MODEL_DIR);
        (llm, tts, s)
    };

    let voice_id = match settings.tts.active {
        crate::core::settings::TtsActiveProvider::Chatterbox => {
            settings.tts.chatterbox.voice_id.as_deref()
        }
        crate::core::settings::TtsActiveProvider::ChatterboxRemote => {
            settings.tts.chatterbox_remote.voice_id.as_deref()
        }
        _ => None,
    };
    let reference_audio = crate::services::tts::resolve_reference_audio(voice_id).await;

    // Check if workers are already active under a short lock
    let (needs_llm, needs_tts, playback_engine, pipeline_tx) = {
        let lock = state.engine.lock().await;
        let engine = lock.as_ref().ok_or("Audio engine not ready")?;
        (
            engine.llm_tx.is_none(),
            engine.tts_tx.is_none(),
            Arc::clone(&engine.playback_engine),
            engine.pipeline_tx.clone(),
        )
    };

    let mut new_llm_tx = None;
    let mut new_llm_handle = None;
    if needs_llm {
        crate::services::llm::actor::warm_up_llm(
            app,
            crate::services::llm::actor::LlmWarmUpHandles {
                llm_tx: &mut new_llm_tx,
                llm_handle: &mut new_llm_handle,
                llm_provider_cache: Some(Arc::clone(&state.llm_provider)),
            },
            &settings,
            &llm_path,
            pipeline_tx.clone(),
        )?;
    }

    let mut new_tts_tx = None;
    let mut new_tts_handle = None;
    if needs_tts {
        crate::services::tts::actor::warm_up_tts(
            crate::services::tts::actor::TtsWarmUpHandles {
                tts_tx: &mut new_tts_tx,
                tts_handle: &mut new_tts_handle,
                cancel_flag: Arc::clone(&state.pipeline.cancel_flag),
                playback_engine,
                pending_synthesis_jobs: Some(Arc::clone(&state.pipeline.pending_synthesis_jobs)),
                telemetry_rtf: Some(Arc::clone(&state.telemetry.latest_tts_rtf)),
            },
            &settings,
            &tts_path,
            reference_audio.as_deref(),
            pipeline_tx,
        )?;
    }

    if needs_llm || needs_tts {
        let mut lock = state.engine.lock().await;
        if let Some(ref mut engine) = *lock {
            if needs_llm && engine.llm_tx.is_none() {
                engine.llm_tx = new_llm_tx;
                engine.llm_handle = new_llm_handle;
            }
            if needs_tts && engine.tts_tx.is_none() {
                engine.tts_tx = new_tts_tx;
                engine.tts_handle = new_tts_handle;
            }
        }
    }

    Ok(())
}

/// Dispatches a VoxEvent to the active modular interaction mode handler.
pub fn handle_event<R: tauri::Runtime>(
    mode: InteractionMode,
    app: &AppHandle<R>,
    state: &AppState,
    event: VoxEvent,
) {
    match mode {
        InteractionMode::Passive => passive::handle_event(app, state, event),
        InteractionMode::PTT => ptt::handle_event(app, state, event),
    }
}
