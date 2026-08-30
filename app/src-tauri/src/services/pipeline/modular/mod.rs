pub mod passive;
pub mod ptt;

use crate::core::events::VoxEvent;
use crate::core::settings::InteractionMode;
use crate::core::state::AppState;
use crate::services::audio::PlaybackEngine;
use std::sync::Arc;
use tauri::AppHandle;

/// Initializes and warms up the LLM and TTS actor threads if not already loaded.
pub async fn ensure_modular_workers<R: tauri::Runtime + 'static>(
    app: &AppHandle<R>,
    state: &AppState,
) -> Result<(), String> {
    let (llm_path, tts_path, settings) = {
        let s = state.settings.read().unwrap_or_else(|p| p.into_inner()).clone();
        let models_dir = crate::utils::paths::get().models.clone();
        let llm = models_dir
            .join(crate::services::llm::QWEN_MODEL_DIR)
            .join(crate::services::llm::QWEN_MODEL_FILE);
        let tts = models_dir.join(crate::services::tts::SUPERTONIC_MODEL_DIR);
        (llm, tts, s)
    };

    let mut lock = state.engine.lock().await;
    let engine = lock.as_mut().ok_or("Audio engine not ready")?;

    crate::services::llm::actor::warm_up_llm(
        app,
        crate::services::llm::actor::LlmWarmUpHandles {
            llm_tx: &mut engine.llm_tx,
            llm_handle: &mut engine.llm_handle,
            is_loaded: Arc::clone(&state.is_llm_loaded),
            is_sleeping: Arc::clone(&state.is_sleeping),
            llm_provider_cache: Some(Arc::clone(&state.llm_provider)),
        },
        &settings,
        &llm_path,
        engine.pipeline_tx.clone(),
    )?;

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

    crate::services::tts::actor::warm_up_tts(
        app,
        crate::services::tts::actor::TtsWarmUpHandles {
            tts_tx: &mut engine.tts_tx,
            tts_handle: &mut engine.tts_handle,
            cancel_flag: Arc::clone(&state.pipeline.cancel_flag),
            is_loaded: Arc::clone(&state.is_tts_loaded),
            is_sleeping: Arc::clone(&state.is_sleeping),
        },
        &settings,
        &tts_path,
        reference_audio.as_deref(),
        engine.pipeline_tx.clone(),
    )?;

    Ok(())
}

/// Dispatches a VoxEvent to the active modular interaction mode handler.
pub fn handle_event<R: tauri::Runtime>(
    mode: InteractionMode,
    app: &AppHandle<R>,
    state: &AppState,
    playback: &Arc<PlaybackEngine>,
    event: VoxEvent,
) {
    match mode {
        InteractionMode::Passive => passive::handle_event(app, state, playback, event),
        InteractionMode::PTT => ptt::handle_event(app, state, playback, event),
    }
}
