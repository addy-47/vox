use super::supertonic::TtsEngine as SupertonicEngine;
use crate::core::events::VoxEvent;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

pub enum TtsCommand {
    Generate {
        turn_id: u32,
        voice_sid: i32,
        text: String,
    },
    /// Hot-update the number of diffusion steps (2-12).
    UpdateQualitySteps(u32),
    /// Hot-update the speed factor (0.7-2.0).
    UpdateSpeed(f32),
    Shutdown,
}

pub fn spawn_tts_worker(
    app: tauri::AppHandle,
    rx: std::sync::mpsc::Receiver<TtsCommand>,
    super_model_path: std::path::PathBuf,
    event_tx: std::sync::mpsc::Sender<VoxEvent>,
    cancel_flag: Arc<AtomicBool>,
    is_loaded: Arc<AtomicBool>,
    quality_steps: u32,
    speed: f32,
) {
    use tauri::Emitter;
    let _ = app.emit(crate::core::constants::EVENT_MODEL_LOADING, "TTS");

    let mut engine: Box<dyn crate::services::traits::TtsEngine + Send> =
        match SupertonicEngine::new(&super_model_path, quality_steps, speed) {
            Ok(e) => {
                is_loaded.store(true, Ordering::Relaxed);
                let _ = app.emit(crate::core::constants::EVENT_MODEL_READY, "TTS");
                Box::new(e)
            }
            Err(e) => {
                log::error!("[TTS] CRITICAL: Failed to load Supertonic engine: {}", e);
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
            TtsCommand::UpdateQualitySteps(steps) => {
                engine.set_quality_steps(steps);
                log::info!("[TTS Worker] Quality steps updated to {}", steps);
            }
            TtsCommand::UpdateSpeed(speed) => {
                engine.set_speed(speed);
                log::info!("[TTS Worker] Speed updated to {:.2}", speed);
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
