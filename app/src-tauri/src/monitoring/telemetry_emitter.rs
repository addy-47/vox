use std::sync::{atomic::Ordering, Arc};

use tauri::{AppHandle, Manager};

use crate::{
    core::{
        events::{emit_ipc_to, IpcEvent, TelemetryData},
        state::{AppState, InteractionOwner, InteractionState},
    },
    monitoring::TELEMETRY_EMITTER_INTERVAL,
};

/// Spawns periodic background task pushing audio and VAD telemetry to active window.
pub fn spawn_telemetry_emitter(app: AppHandle) {
    let state = app.state::<Arc<AppState>>().inner().clone();

    tauri::async_runtime::spawn(async move {
        loop {
            tokio::time::sleep(TELEMETRY_EMITTER_INTERVAL).await;

            if state.pipeline.state() == InteractionState::Paused {
                continue;
            }

            let (energy, low, mid, high) = get_current_audio_levels(&state);
            let vad_prob = f32::from_bits(state.telemetry.latest_vad_prob.load(Ordering::Relaxed));
            let target = get_target_window(&state);

            if let Err(e) = emit_ipc_to(
                &app,
                target,
                IpcEvent::Telemetry(TelemetryData {
                    energy,
                    vad_prob,
                    low,
                    mid,
                    high,
                }),
            ) {
                log::warn!(
                    "[Monitoring::TelemetryEmitter] Failed to emit telemetry event: {}",
                    e
                );
            }
        }
    });
}

fn get_current_audio_levels(state: &AppState) -> (f32, f32, f32, f32) {
    if state.pipeline.state() == InteractionState::Speaking {
        (
            f32::from_bits(
                state
                    .telemetry
                    .latest_playback_energy
                    .load(Ordering::Relaxed),
            ),
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

fn get_target_window(state: &AppState) -> &'static str {
    let owner_enum: InteractionOwner = state.owner.load(Ordering::Relaxed).into();
    match owner_enum {
        InteractionOwner::Assistant => "main",
        InteractionOwner::Dictation => "tray",
    }
}
