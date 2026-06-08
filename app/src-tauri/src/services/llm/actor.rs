use super::llama_cpp::LlmWorker;
use crate::core::events::VoxEvent;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

pub enum LlmCommand {
    Generate {
        text: String,
        system_prompt: String,
        turn_id: u32,
        cancel_flag: Arc<AtomicBool>,
    },
    Shutdown,
}

pub fn spawn_llm_worker(
    app: tauri::AppHandle,
    rx: std::sync::mpsc::Receiver<LlmCommand>,
    model_path: std::path::PathBuf,
    event_tx: std::sync::mpsc::Sender<VoxEvent>,
    is_loaded: Arc<AtomicBool>,
    ctx_size: u32,
    n_threads: u32,
) {
    use tauri::Emitter;
    let _ = app.emit(crate::core::constants::EVENT_MODEL_LOADING, "LLM");

    let worker = match LlmWorker::new(&model_path, ctx_size, n_threads) {
        Ok(w) => {
            is_loaded.store(true, Ordering::Relaxed);
            let _ = app.emit(crate::core::constants::EVENT_MODEL_READY, "LLM");
            w
        }
        Err(e) => {
            log::error!("[LLM] CRITICAL: Failed to load model: {}", e);
            let _ = app.emit(
                crate::core::constants::EVENT_MODEL_FAILED,
                format!("LLM: {}", e),
            );
            return;
        }
    };

    worker.run_loop(rx, event_tx);
    is_loaded.store(false, Ordering::Relaxed);
}
