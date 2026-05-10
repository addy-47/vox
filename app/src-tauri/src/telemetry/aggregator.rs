use tauri::{AppHandle, Manager, Emitter};
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use crate::core::state::{TelemetryData, AppState, InteractionOwner};
use crate::core::constants::TELEMETRY_INTERVAL;

pub struct TelemetryAggregator {
    app: AppHandle,
    rx: std::sync::mpsc::Receiver<TelemetryData>,
    playback_energy: Arc<AtomicU32>,
}

impl TelemetryAggregator {
    pub fn new(app: AppHandle, rx: std::sync::mpsc::Receiver<TelemetryData>, playback_energy: Arc<AtomicU32>) -> Self {
        Self { app, rx, playback_energy }
    }

    pub fn run(self) {
        let app = self.app;
        let rx = self.rx;
        let playback_energy = self.playback_energy;

        std::thread::spawn(move || {
            log::info!("[Telemetry] Aggregator thread started.");
            
            loop {
                let mut latest = None;
                // 1. Drain the VAD channel to get the latest mic energy
                while let Ok(data) = rx.try_recv() {
                    latest = Some(data);
                }

                // 2. Poll the playback atomic for AI energy
                let p_bits = playback_energy.load(Ordering::Relaxed);
                let p_energy = f32::from_bits(p_bits);
                
                if p_energy > 0.001 {
                    latest = Some(TelemetryData {
                        energy: p_energy,
                        vad_prob: 0.0,
                    });
                }
                
                if let Some(data) = latest {
                    let state: tauri::State<'_, AppState> = app.state();
                    let is_engaged = state.pipeline.is_engaged.load(Ordering::Relaxed);
                    let owner = state.owner.blocking_lock();
                    
                    // Telemetry routing:
                    if is_engaged {
                        let _ = app.emit_to("main", "telemetry", data.clone());
                    }
                    
                    let target_tray = match *owner {
                        InteractionOwner::Tray | InteractionOwner::Ptt => true,
                        _ => false,
                    };
                    
                    if target_tray {
                        let _ = app.emit_to("tray", "telemetry", data);
                    }
                }
                std::thread::sleep(TELEMETRY_INTERVAL);
            }
        });
    }
}

pub fn spawn_telemetry_aggregator(app: AppHandle, playback_energy: Arc<AtomicU32>) -> std::sync::mpsc::Sender<TelemetryData> {
    let (tx, rx) = std::sync::mpsc::channel();
    let aggregator = TelemetryAggregator::new(app, rx, playback_energy);
    aggregator.run();
    tx
}
