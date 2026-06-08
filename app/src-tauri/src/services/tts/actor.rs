use super::kokoro_piper::TtsEngine;
use crate::core::events::VoxEvent;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

pub enum TtsCommand {
    Generate {
        turn_id: u32,
        voice_sid: i32,
        text: String,
    },
    Shutdown,
}

pub fn spawn_tts_worker(
    app: tauri::AppHandle,
    rx: std::sync::mpsc::Receiver<TtsCommand>,
    en_model_dir: std::path::PathBuf,
    hi_model_path: std::path::PathBuf,
    event_tx: std::sync::mpsc::Sender<VoxEvent>,
    cancel_flag: Arc<AtomicBool>,
    is_loaded: Arc<AtomicBool>,
) {
    use tauri::Emitter;
    let _ = app.emit(crate::core::constants::EVENT_MODEL_LOADING, "TTS");

    let mut engine: Box<dyn crate::services::traits::TtsEngine + Send> =
        match TtsEngine::new(&en_model_dir, &hi_model_path) {
            Ok(e) => {
                is_loaded.store(true, Ordering::Relaxed);
                let _ = app.emit(crate::core::constants::EVENT_MODEL_READY, "TTS");
                Box::new(e)
            }
            Err(e) => {
                log::error!("[TTS] CRITICAL: Failed to load multi-model engine: {}", e);
                let _ = app.emit(
                    crate::core::constants::EVENT_MODEL_FAILED,
                    format!("TTS: {}", e),
                );
                return;
            }
        };

    log::info!("[TTS Worker] Persistent loop started.");
    while let Ok(cmd) = rx.recv() {
        match cmd {
            TtsCommand::Generate {
                turn_id,
                voice_sid,
                text,
            } => {
                if let Err(e) = engine.synthesize_chunk(
                    &text,
                    voice_sid,
                    turn_id,
                    cancel_flag.clone(),
                    event_tx.clone(),
                ) {
                    log::error!("[TTS Worker] Synthesis error (turn {}): {}", turn_id, e);
                }
            }
            TtsCommand::Shutdown => {
                log::info!("[TTS Worker] Shutdown command received. Exiting loop.");
                break;
            }
        }
    }

    is_loaded.store(false, Ordering::Relaxed);
    log::info!("[TTS Worker] Loop exited. Engine will be dropped.");
}
