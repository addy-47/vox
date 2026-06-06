use tauri::{AppHandle, Emitter, Manager};
use std::sync::atomic::Ordering;

/// Spawns a background task running at 30Hz (~33ms ticks) that pushes unified
/// telemetry (audio energy + VAD probability) to the active Tauri window.
///
/// Decouples UI drawing from real-time audio threads to guarantee no stuttering.
pub fn spawn_telemetry_emitter(app: AppHandle) {
    let state = app.state::<std::sync::Arc<crate::core::state::AppState>>().inner().clone();
    
    tauri::async_runtime::spawn(async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_millis(33)).await;
            
            if state.is_sleeping.load(Ordering::Relaxed) {
                continue;
            }
            
            // Read output energy if assistant is speaking; otherwise read microphone energy
            let is_assistant = state.pipeline.is_assistant_speaking.load(Ordering::Relaxed);
            let energy = if is_assistant {
                f32::from_bits(state.latest_playback_energy.load(Ordering::Relaxed))
            } else {
                f32::from_bits(state.latest_energy.load(Ordering::Relaxed))
            };
            
            let vad_prob = f32::from_bits(state.latest_vad_prob.load(Ordering::Relaxed));
            
            let owner_enum: crate::core::state::InteractionOwner = state.owner.load(Ordering::Relaxed).into();
            let target = match owner_enum {
                crate::core::state::InteractionOwner::MainWindow | crate::core::state::InteractionOwner::Ptt => "main",
                crate::core::state::InteractionOwner::Tray => "tray",
                crate::core::state::InteractionOwner::Wizard => "wizard",
            };
            
            let _ = app.emit_to(target, "telemetry", crate::core::state::TelemetryData {
                energy,
                vad_prob,
            });
        }
    });
}
