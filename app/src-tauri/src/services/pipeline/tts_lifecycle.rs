//! ============================================================================
//! src/services/pipeline/tts_lifecycle.rs — TTS worker thread initialization and cooldown
//! ============================================================================

use super::types::resolve_reference_audio;
use super::PipelineOrchestrator;
use std::sync::atomic::Ordering;
use std::sync::Arc;

impl PipelineOrchestrator {
    /// Initialize the TTS worker if it's not already running.
    ///
    /// Reads `TtsProviderConfig` from settings to determine which provider to
    /// construct (mirrors `warm_up_llm()`).
    pub fn warm_up_tts(&self, app: &tauri::AppHandle) -> Result<(), String> {
        let mut lock = self.tts_tx.lock();
        if lock.is_some() {
            return Ok(());
        }

        let (provider_config, voice, quality_steps, speed) = {
            let s = self.settings.read().map_err(|e| e.to_string())?;
            (
                s.tts.provider.clone(),
                s.tts.voice,
                s.tts.quality_steps,
                s.tts.speed,
            )
        };

        use crate::core::settings::TtsProviderConfig;
        use crate::services::tts::{
            ChatterboxEngine, ChatterboxRemoteProvider, EdgeTtsProvider,
            TtsEngine as SupertonicEngine, TtsProvider,
        };

        let provider: Box<dyn TtsProvider> = match &provider_config {
            TtsProviderConfig::Supertonic => {
                log::info!("[Pipeline] Warming up TTS worker (Supertonic)...");
                Box::new(
                    SupertonicEngine::new(&self.super_tts_path, voice, quality_steps, speed)
                        .map_err(|e| format!("Failed to create Supertonic engine: {}", e))?,
                )
            }
            TtsProviderConfig::Chatterbox {
                language,
                quality_steps: cb_quality,
                speed: cb_speed,
                voice_id,
            } => {
                log::info!("[Pipeline] Warming up TTS worker (Chatterbox)...");
                let chatterbox_path = crate::utils::paths::model_dir("tts").join("chatterbox");
                let ref_audio = resolve_reference_audio(voice_id.as_deref());
                Box::new(
                    ChatterboxEngine::new(
                        &chatterbox_path,
                        language,
                        *cb_quality,
                        *cb_speed,
                        ref_audio.as_deref(),
                    )
                    .map_err(|e| format!("Failed to create Chatterbox engine: {}", e))?,
                )
            }
            TtsProviderConfig::ChatterboxRemote {
                endpoint,
                language,
                quality_steps: remote_quality,
                speed: remote_speed,
                remote_path,
                voice_id: _, // Phase D: remote voice forwarding not yet implemented
            } => {
                log::info!("[Pipeline] Warming up TTS worker (ChatterboxRemote)...");
                Box::new(
                    ChatterboxRemoteProvider::new(
                        endpoint,
                        language,
                        *remote_quality,
                        *remote_speed,
                        remote_path,
                    )
                    .map_err(|e| format!("Failed to create ChatterboxRemote provider: {}", e))?,
                )
            }
            TtsProviderConfig::EdgeTts { voice: edge_voice } => {
                log::info!("[Pipeline] Warming up TTS worker (EdgeTTS)...");
                Box::new(EdgeTtsProvider::new(edge_voice.as_deref()))
            }
        };

        log::info!("[Pipeline] TTS provider: {:?}", provider.kind());

        let (tx, rx) = std::sync::mpsc::channel::<crate::services::tts::TtsCommand>();

        let cancel_tts = Arc::clone(&self.cancel_flag);
        let event_tx = self.event_tx.clone();
        let is_loaded = Arc::clone(&self.is_tts_loaded);
        *lock = Some(tx);

        let app_clone = app.clone();
        let handle = std::thread::Builder::new()
            .name("vox-tts-persistent".to_string())
            .spawn(move || {
                crate::services::tts::spawn_tts_worker(
                    app_clone, rx, provider, event_tx, cancel_tts, is_loaded,
                );
            })
            .map_err(|e| e.to_string())?;

        let mut handle_lock = self.tts_handle.lock();
        *handle_lock = Some(handle);

        // Reset sleep state when warming up
        self.is_sleeping.store(false, Ordering::Relaxed);

        Ok(())
    }

    pub fn cool_down_tts(&self) {
        let mut lock = self.tts_tx.lock();
        *lock = None; // Dropping sender closes worker
        log::info!("[Pipeline] TTS Shutdown (Offloading).");
    }
}
