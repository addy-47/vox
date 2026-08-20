use std::sync::atomic::Ordering;
use tauri::{AppHandle, Emitter, Manager};

/// Spawns a background task running at 30Hz (~33ms ticks) that pushes unified
/// telemetry (audio energy + VAD probability) to the active Tauri window.
///
/// Decouples UI drawing from real-time audio threads to guarantee no stuttering.
pub fn spawn_telemetry_emitter(app: AppHandle) {
    let state = app
        .state::<std::sync::Arc<crate::core::state::AppState>>()
        .inner()
        .clone();

    tauri::async_runtime::spawn(async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_millis(33)).await;

            if state.is_sleeping.load(Ordering::Relaxed) {
                continue;
            }

            // Read output energy if assistant is speaking; otherwise read microphone energy
            let local_pipeline_mode = {
                let s = state.settings.read().unwrap();
                s.interaction.pipeline_mode.clone()
            };
            let is_assistant =
                if local_pipeline_mode == crate::core::settings::PipelineMode::Realtime {
                    state.pipeline.playback_active.load(Ordering::Relaxed)
                } else {
                    state.pipeline.is_assistant_speaking.load(Ordering::Relaxed)
                };
            let (energy, low, mid, high) = if is_assistant {
                (
                    f32::from_bits(state.latest_playback_energy.load(Ordering::Relaxed)),
                    f32::from_bits(state.latest_playback_low.load(Ordering::Relaxed)),
                    f32::from_bits(state.latest_playback_mid.load(Ordering::Relaxed)),
                    f32::from_bits(state.latest_playback_high.load(Ordering::Relaxed)),
                )
            } else {
                (
                    f32::from_bits(state.latest_energy.load(Ordering::Relaxed)),
                    f32::from_bits(state.latest_low.load(Ordering::Relaxed)),
                    f32::from_bits(state.latest_mid.load(Ordering::Relaxed)),
                    f32::from_bits(state.latest_high.load(Ordering::Relaxed)),
                )
            };

            let vad_prob = f32::from_bits(state.latest_vad_prob.load(Ordering::Relaxed));

            let owner_enum: crate::core::state::InteractionOwner =
                state.owner.load(Ordering::Relaxed).into();
            let target = match owner_enum {
                crate::core::state::InteractionOwner::MainWindow
                | crate::core::state::InteractionOwner::Ptt => "main",
                crate::core::state::InteractionOwner::Dictation => "tray",
                crate::core::state::InteractionOwner::Wizard => "wizard",
            };

            let _ = app.emit_to(
                target,
                "telemetry",
                crate::core::state::TelemetryData {
                    energy,
                    vad_prob,
                    low,
                    mid,
                    high,
                },
            );
        }
    });
}
