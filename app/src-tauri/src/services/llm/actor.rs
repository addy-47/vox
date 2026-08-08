use super::LlmProvider;
use crate::core::events::VoxEvent;
use crate::services::llm::types::GenerationRequest;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

pub enum LlmCommand {
    Generate {
        request: GenerationRequest,
        turn_id: u32,
        cancel_flag: Arc<AtomicBool>,
    },
    Shutdown,
}

pub fn spawn_llm_worker(
    app: tauri::AppHandle,
    rx: std::sync::mpsc::Receiver<LlmCommand>,
    provider: Box<dyn LlmProvider>,
    event_tx: std::sync::mpsc::Sender<VoxEvent>,
    is_loaded: Arc<AtomicBool>,
) {
    use tauri::Emitter;

    let is_local = provider.kind() == super::ProviderKind::Embedded;
    is_loaded.store(is_local, Ordering::Relaxed);
    let _ = app.emit(crate::core::constants::EVENT_MODEL_READY, "LLM");

    log::info!("[LLM Worker] Persistent loop started.");

    while let Ok(cmd) = rx.recv() {
        match cmd {
            LlmCommand::Generate {
                request,
                turn_id,
                cancel_flag,
            } => {
                let handle = tokio::runtime::Handle::try_current();
                let res = match handle {
                    Ok(h) => {
                        h.block_on(provider.generate(request, turn_id, &cancel_flag, &event_tx))
                    }
                    Err(_) => tokio::runtime::Builder::new_current_thread()
                        .enable_all()
                        .build()
                        .expect("Failed to build temporary tokio runtime")
                        .block_on(provider.generate(request, turn_id, &cancel_flag, &event_tx)),
                };

                if let Err(e) = res {
                    log::error!("[LLM Worker] Generation error (turn {}): {}", turn_id, e);
                    let _ = event_tx.send(VoxEvent::Error {
                        turn_id,
                        message: e.to_string(),
                    });
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
