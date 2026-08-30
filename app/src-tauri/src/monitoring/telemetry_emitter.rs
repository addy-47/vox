use crate::monitoring::TELEMETRY_EMITTER_INTERVAL;
use std::sync::atomic::Ordering;
use tauri::{AppHandle, Emitter, Manager};

/// Spawns periodic background task pushing audio and VAD telemetry to active window.
pub fn spawn_telemetry_emitter(app: AppHandle) {
    let state = app
        .state::<std::sync::Arc<crate::core::state::AppState>>()
        .inner()
        .clone();

    tauri::async_runtime::spawn(async move {
        loop {
            tokio::time::sleep(TELEMETRY_EMITTER_INTERVAL).await;

            if state.pipeline.state() == crate::core::state::InteractionState::Paused {
                continue;
            }

            let (energy, low, mid, high) = get_current_audio_levels(&state);
            let vad_prob = f32::from_bits(state.telemetry.latest_vad_prob.load(Ordering::Relaxed));
            let target = get_target_window(&state);

            if let Err(e) = app.emit_to(
                target,
                "telemetry",
                crate::core::state::TelemetryData {
                    energy,
                    vad_prob,
                    low,
                    mid,
                    high,
                },
            ) {
                log::warn!("[Monitoring::TelemetryEmitter] Failed to emit telemetry event: {}", e);
            }
        }
    });
}

fn get_current_audio_levels(state: &crate::core::state::AppState) -> (f32, f32, f32, f32) {
    if state.pipeline.state() == crate::core::state::InteractionState::Speaking {
        (
            f32::from_bits(state.telemetry.latest_playback_energy.load(Ordering::Relaxed)),
            f32::from_bits(state.telemetry.latest_playback_low.load(Ordering::Relaxed)),
            f32::from_bits(state.telemetry.latest_playback_mid.load(Ordering::Relaxed)),
            f32::from_bits(state.telemetry.latest_playback_high.load(Ordering::Relaxed)),
        )
    } else {
        (
            f32::from_bits(state.telemetry.latest_energy.load(Ordering::Relaxed)),
            f32::from_bits(state.telemetry.latest_low.load(Ordering::Relaxed)),
            f32::from_bits(state.telemetry.latest_mid.load(Ordering::Relaxed)),
            f32::from_bits(state.telemetry.latest_high.load(Ordering::Relaxed)),
        )
    }
}

fn get_target_window(state: &crate::core::state::AppState) -> &'static str {
    let owner_enum: crate::core::state::InteractionOwner =
        state.owner.load(Ordering::Relaxed).into();
    match owner_enum {
        crate::core::state::InteractionOwner::Assistant => "main",
        crate::core::state::InteractionOwner::Dictation => "tray",
    }
}

