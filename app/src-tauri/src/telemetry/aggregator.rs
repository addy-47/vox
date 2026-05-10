use tokio::sync::mpsc;
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
/// In Phase 6.2: Events are logged to the tracing file.
/// In Phase 6.3: Events will be persisted to SQLite.
pub struct TelemetryAggregator {
    rx: mpsc::UnboundedReceiver<TelemetryEvent>,
}

impl TelemetryAggregator {
    pub fn new() -> (Self, mpsc::UnboundedSender<TelemetryEvent>) {
        let (tx, rx) = mpsc::unbounded_channel();
        (Self { rx }, tx)
    }

    /// Spawns the aggregator loop on a dedicated Tokio task.
    pub fn start(mut self) {
        tauri::async_runtime::spawn(async move {
            tracing::info!("[Telemetry] Aggregator worker started.");
            
            while let Some(event) = self.rx.recv().await {
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
            
            tracing::info!("[Telemetry] Aggregator worker shutting down.");
        });
    }
}
