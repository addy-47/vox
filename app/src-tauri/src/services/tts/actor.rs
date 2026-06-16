use crate::core::events::VoxEvent;
use crate::services::tts::providers::TtsProvider;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

pub enum TtsCommand {
    Generate {
        turn_id: u32,
        text: String,
    },
    /// Hot-update the number of diffusion steps (2-12).
    UpdateQualitySteps(u32),
    /// Hot-update the speed factor (0.7-2.0).
    UpdateSpeed(f32),
    Shutdown,
}

/// Spawn a persistent TTS worker thread.
///
/// The worker takes ownership of the provider and processes `TtsCommand`s
/// from the pipeline in a blocking loop. The provider must be fully initialized
/// before calling this function.
///
/// # Parameters
/// - `app` — Tauri app handle for emitting model lifecycle events.
/// - `rx` — Receiver for `TtsCommand` from the pipeline.
/// - `provider` — The TTS provider to use (e.g. Supertonic, Pocket, etc.).
/// - `event_tx` — Channel to emit `VoxEvent`s (TtsChunk, TtsFinished) back to the pipeline.
/// - `cancel_flag` — Shared atomic flag for barge-in cancellation.
/// - `is_loaded` — Set to true after successful init, false on shutdown.
pub fn spawn_tts_worker(
    app: tauri::AppHandle,
    rx: std::sync::mpsc::Receiver<TtsCommand>,
    provider: Box<dyn TtsProvider>,
    event_tx: std::sync::mpsc::Sender<VoxEvent>,
    cancel_flag: Arc<AtomicBool>,
    is_loaded: Arc<AtomicBool>,
) {
    use tauri::Emitter;

    // Provider is pre-initialized — signal ready immediately.
    is_loaded.store(true, Ordering::Relaxed);
    let _ = app.emit(crate::core::constants::EVENT_MODEL_READY, "TTS");

    log::info!(
        "[TTS Worker] Persistent loop started with provider: {:?}",
        provider.kind()
    );

    while let Ok(cmd) = rx.recv() {
        match cmd {
            TtsCommand::Generate { turn_id, text } => {
                if let Err(e) = provider.synthesize_chunk(
                    &text,
                    turn_id,
                    cancel_flag.clone(),
                    event_tx.clone(),
                ) {
                    log::error!("[TTS Worker] Synthesis error (turn {}): {}", turn_id, e);
                }
            }
            TtsCommand::UpdateQualitySteps(steps) => {
                provider.set_quality_steps(steps);
                log::info!("[TTS Worker] Quality steps updated to {}", steps);
            }
            TtsCommand::UpdateSpeed(speed) => {
                provider.set_speed(speed);
                log::info!("[TTS Worker] Speed updated to {:.2}", speed);
            }
            TtsCommand::Shutdown => {
                log::info!("[TTS Worker] Shutdown command received. Exiting loop.");
                break;
            }
        }
    }

    is_loaded.store(false, Ordering::Relaxed);
    log::info!("[TTS Worker] Loop exited. Provider will be dropped.");
}
