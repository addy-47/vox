//! ============================================================================
//! src/services/pipeline/llm_lifecycle.rs — LLM worker thread initialization and cooldown
//! ============================================================================

use super::PipelineOrchestrator;
use std::sync::atomic::Ordering;
use std::sync::Arc;

impl PipelineOrchestrator {
    /// Initialize the LLM worker if it's not already running.
    pub fn warm_up_llm(&self, app: &tauri::AppHandle) -> Result<(), String> {
        let mut lock = self.llm_tx.lock();
        if lock.is_some() {
            return Ok(());
        }

        log::info!("[Pipeline] Warming up LLM worker...");
        let (tx, rx) = std::sync::mpsc::channel();

        let (provider_config, ctx_size, n_threads) = {
            let s = self.settings.read().map_err(|e| e.to_string())?;
            (s.llm.provider.clone(), s.llm.ctx_size, s.llm.threads)
        };

        let event_tx = self.event_tx.clone();
        let is_loaded = Arc::clone(&self.is_llm_loaded);
        let app_clone = app.clone();
        *lock = Some(tx);

        let llm_path_clone = self.llm_path.clone();
        let handle = std::thread::Builder::new()
            .name("vox-llm-persistent".to_string())
            .spawn(move || {
                use crate::core::settings::LlmProviderConfig;
                use crate::services::llm::{EmbeddedProvider, LlmProvider, OpenAiCompatProvider};
                use tauri::Emitter;

                let _ = app_clone.emit(crate::core::constants::EVENT_MODEL_LOADING, "LLM");

                let provider_res: Result<Box<dyn LlmProvider>, String> = match &provider_config {
                    LlmProviderConfig::Embedded => {
                        EmbeddedProvider::new(&llm_path_clone, ctx_size, n_threads)
                            .map(|p| Box::new(p) as Box<dyn LlmProvider>)
                            .map_err(|e| e.to_string())
                    }
                    LlmProviderConfig::OpenAiCompat {
                        base_url,
                        model,
                        api_key,
                        provider_name,
                    } => {
                        let provider = OpenAiCompatProvider::new(
                            base_url,
                            model,
                            api_key.as_deref(),
                            provider_name.as_deref(),
                        );
                        Ok(Box::new(provider) as Box<dyn LlmProvider>)
                    }
                };

                match provider_res {
                    Ok(provider) => {
                        crate::services::llm::spawn_llm_worker(
                            app_clone, rx, provider, event_tx, is_loaded,
                        );
                    }
                    Err(e) => {
                        log::error!("[LLM] CRITICAL: Failed to load provider: {}", e);
                        let _ = app_clone.emit(
                            crate::core::constants::EVENT_MODEL_FAILED,
                            format!("LLM: {}", e),
                        );
                        is_loaded.store(false, Ordering::Relaxed);
                    }
                }
            })
            .map_err(|e| e.to_string())?;

        let mut handle_lock = self.llm_handle.lock();
        *handle_lock = Some(handle);

        // Reset sleep state when warming up
        self.is_sleeping.store(false, Ordering::Relaxed);

        Ok(())
    }

    pub fn cool_down_llm(&self) {
        let mut lock = self.llm_tx.lock();
        if let Some(tx) = lock.take() {
            let _ = tx.send(crate::services::llm::LlmCommand::Shutdown);
            log::info!("[Pipeline] LLM Shutdown sent (Offloading).");
        }
    }
}
