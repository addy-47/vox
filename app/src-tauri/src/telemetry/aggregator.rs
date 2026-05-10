use crossbeam_channel::{unbounded, Receiver, Sender};
use serde::Serialize;

/// Structured telemetry events emitted by various engine subsystems.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", content = "data")]
pub enum TelemetryEvent {
    /// Performance metrics for a completed interaction turn
    InteractionMetric {
        session_id: String,
        stt_latency_ms: u32,
        ttft_ms: u32,
        tts_rtf: f32,
    },
    /// Periodic system resource utilization
    SystemHealth {
        cpu_usage: f32,
        ram_mb: u32,
    },
    /// Real-time audio signal characteristics (VAD hot-path)
    AudioEnergy {
        energy: f32,
        vad_prob: f32,
    },
}

/// A dedicated background worker that aggregates telemetry events.
///
/// Uses crossbeam_channel for lock-free, zero-async-overhead operation.
/// Critical: the sender is cloned into VAD and audio hot-path threads.
///
/// In Phase 6.2: Events are logged to the tracing file.
/// In Phase 6.3: Events will be persisted to SQLite.
pub struct TelemetryAggregator {
    rx: Receiver<TelemetryEvent>,
}

impl TelemetryAggregator {
    pub fn new() -> (Self, Sender<TelemetryEvent>) {
        let (tx, rx) = unbounded();
        (Self { rx }, tx)
    }

    /// Spawns the aggregator loop on a dedicated OS thread.
    ///
    /// Using a standard OS thread with a blocking recv() is more efficient than
    /// an async task for this workload, as it consumes 0% CPU while idle and
    /// eliminates the overhead of a sleep/poll loop.
    pub fn start(self) {
        std::thread::Builder::new()
            .name("vox-telemetry".to_string())
            .spawn(move || {
                tracing::info!("[Telemetry] Aggregator worker started.");

                while let Ok(event) = self.rx.recv() {
                    match &event {
                        TelemetryEvent::InteractionMetric { session_id, .. } => {
                            tracing::info!(target: "telemetry", session_id = %session_id, "Interaction metrics: {:?}", event);
                        }
                        TelemetryEvent::SystemHealth { .. } => {
                            tracing::info!(target: "telemetry", "System Health: {:?}", event);
                        }
                        TelemetryEvent::AudioEnergy { .. } => {
                            // High-frequency debug only
                            tracing::debug!(target: "telemetry", "Audio Energy: {:?}", event);
                        }
                    }
                }
                tracing::info!("[Telemetry] Channel disconnected. Aggregator exiting.");
            })
            .expect("[Telemetry] Failed to spawn aggregator thread");
    }
}
