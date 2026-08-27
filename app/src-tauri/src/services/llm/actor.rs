use super::{EmbeddedProvider, LlmProvider, OpenAiCompatProvider, ProviderKind};
use crate::core::constants::{EVENT_MODEL_FAILED, EVENT_MODEL_LOADING, EVENT_MODEL_READY};
use crate::core::events::VoxEvent;
use crate::core::settings::{LlmProviderConfig, VoxSettings};
use crate::services::llm::types::GenerationRequest;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::Emitter;

/// Commands processed by the background LLM worker thread.
pub enum LlmCommand {
    Generate {
        request: GenerationRequest,
        turn_id: u32,
        cancel_flag: Arc<AtomicBool>,
    },
    Shutdown,
}

/// Spawns the dedicated LLM generation worker thread and runs its command loop.
pub fn spawn_llm_worker<R: tauri::Runtime + 'static>(
    app: tauri::AppHandle<R>,
    rx: std::sync::mpsc::Receiver<LlmCommand>,
    provider: Box<dyn LlmProvider>,
    event_tx: std::sync::mpsc::Sender<VoxEvent>,
    is_loaded: Arc<AtomicBool>,
) {
    let is_local = provider.kind() == ProviderKind::Embedded;
    is_loaded.store(is_local, Ordering::Relaxed);
    if let Err(e) = app.emit(EVENT_MODEL_READY, "LLM") {
        log::warn!("[LLM Worker] Failed to emit EVENT_MODEL_READY: {}", e);
    }

    log::info!("[LLM Worker] Persistent loop started.");

    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("Failed to build LLM worker runtime");

    while let Ok(cmd) = rx.recv() {
        match cmd {
            LlmCommand::Generate {
                request,
                turn_id,
                cancel_flag,
            } => {
                let res =
                    runtime.block_on(provider.generate(request, turn_id, &cancel_flag, &event_tx));

                if let Err(e) = res {
                    log::error!("[LLM Worker] Generation error (turn {}): {}", turn_id, e);
                    if let Err(send_err) = event_tx.send(VoxEvent::Error {
                        turn_id,
                        message: e.to_string(),
                    }) {
                        log::warn!("[LLM Worker] Failed to dispatch error event: {}", send_err);
                    }
                }
            }
            LlmCommand::Shutdown => {
                log::info!("[LLM Worker] Shutdown command received. Exiting loop.");
                break;
            }
        }
    }

    is_loaded.store(false, Ordering::Relaxed);
    log::info!("[LLM Worker] Loop exited. Provider will be dropped.");
}

/// Creates a boxed LLM provider based on settings configuration.
pub fn create_llm_provider(
    settings: &VoxSettings,
    llm_path: &Path,
) -> Result<Box<dyn LlmProvider>, String> {
    let provider_config = settings.llm.to_provider_config();
    let ctx_size = settings.llm.context_window;
    let n_threads = settings.llm.threads;

    match provider_config {
        LlmProviderConfig::Embedded => EmbeddedProvider::new(llm_path, ctx_size, n_threads)
            .map(|p| Box::new(p) as Box<dyn LlmProvider>)
            .map_err(|e| e.to_string()),
        LlmProviderConfig::OpenAiCompat {
            base_url,
            model,
            api_key,
            provider_name,
        } => {
            let provider = OpenAiCompatProvider::new(
                &base_url,
                &model,
                api_key.as_deref(),
                provider_name.as_deref(),
            );
            Ok(Box::new(provider) as Box<dyn LlmProvider>)
        }
    }
}

/// Handles and flags passed when warming up the LLM actor.
pub struct LlmWarmUpHandles<'a> {
    pub llm_tx: &'a mut Option<std::sync::mpsc::Sender<LlmCommand>>,
    pub llm_handle: &'a mut Option<std::thread::JoinHandle<()>>,
    pub is_loaded: Arc<AtomicBool>,
    pub is_sleeping: Arc<AtomicBool>,
}

/// Spawns and initializes a persistent LLM worker actor thread.
pub fn warm_up_llm<R: tauri::Runtime + 'static>(
    app: &tauri::AppHandle<R>,
    handles: LlmWarmUpHandles<'_>,
    settings: &VoxSettings,
    llm_path: &Path,
    event_tx: std::sync::mpsc::Sender<VoxEvent>,
) -> Result<(), String> {
    if handles.llm_tx.is_some() {
        return Ok(());
    }

    log::info!("[LLM Actor] Warming up LLM worker");
    if let Err(e) = app.emit(EVENT_MODEL_LOADING, "LLM") {
        log::warn!("[LLM Actor] Failed to emit EVENT_MODEL_LOADING: {}", e);
    }

    let provider = match create_llm_provider(settings, llm_path) {
        Ok(p) => p,
        Err(e) => {
            log::error!("[LLM Actor] Failed to create provider: {}", e);
            if let Err(emit_err) = app.emit(EVENT_MODEL_FAILED, format!("LLM: {}", e)) {
                log::warn!("[LLM Actor] Failed to emit EVENT_MODEL_FAILED: {}", emit_err);
            }
            handles.is_loaded.store(false, Ordering::Relaxed);
            return Err(e);
        }
    };

    let (tx, rx) = std::sync::mpsc::channel();
    *handles.llm_tx = Some(tx);

    let app_clone = app.clone();
    let is_loaded = handles.is_loaded;
    let is_sleeping = handles.is_sleeping;

    let handle = std::thread::Builder::new()
        .name("vox-llm-persistent".to_string())
        .spawn(move || {
            spawn_llm_worker(app_clone, rx, provider, event_tx, is_loaded);
        })
        .map_err(|e| e.to_string())?;

    *handles.llm_handle = Some(handle);
    is_sleeping.store(false, Ordering::Relaxed);
    Ok(())
}

/// Signals the running LLM worker thread to shutdown and drop its model instance.
pub fn cool_down_llm(llm_tx: &mut Option<std::sync::mpsc::Sender<LlmCommand>>) {
    if let Some(tx) = llm_tx.take() {
        if let Err(e) = tx.send(LlmCommand::Shutdown) {
            log::warn!("[LLM Actor] Failed to send Shutdown command: {}", e);
        }
        log::info!("[LLM Actor] Shutdown command sent (offloaded)");
    }
}
