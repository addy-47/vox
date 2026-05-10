use crossbeam_channel::{unbounded, Receiver, Sender};
use serde::Serialize;

/// Structured telemetry events emitted by various engine subsystems.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", content = "data")]
pub enum TelemetryEvent {
    /// Performance metrics for a completed interaction turn
    InteractionMetric {
        conversation_id: u64,
        turn_id: u32,
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
    latest_energy: std::sync::Arc<std::sync::atomic::AtomicU32>,
    latest_vad_prob: std::sync::Arc<std::sync::atomic::AtomicU32>,
    latest_cpu: std::sync::Arc<std::sync::atomic::AtomicU32>,
    latest_ram: std::sync::Arc<std::sync::atomic::AtomicU32>,
    latest_stt_ms: std::sync::Arc<std::sync::atomic::AtomicU32>,
    latest_ttft_ms: std::sync::Arc<std::sync::atomic::AtomicU32>,
}

impl TelemetryAggregator {
    pub fn new(
        latest_energy: std::sync::Arc<std::sync::atomic::AtomicU32>,
        latest_vad_prob: std::sync::Arc<std::sync::atomic::AtomicU32>,
        latest_cpu: std::sync::Arc<std::sync::atomic::AtomicU32>,
        latest_ram: std::sync::Arc<std::sync::atomic::AtomicU32>,
        latest_stt_ms: std::sync::Arc<std::sync::atomic::AtomicU32>,
        latest_ttft_ms: std::sync::Arc<std::sync::atomic::AtomicU32>,
    ) -> (Self, Sender<TelemetryEvent>) {
        let (tx, rx) = unbounded();
        (Self { rx, latest_energy, latest_vad_prob, latest_cpu, latest_ram, latest_stt_ms, latest_ttft_ms }, tx)
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
                    use std::sync::atomic::Ordering;
                    match &event {
                        TelemetryEvent::InteractionMetric { conversation_id, turn_id, stt_latency_ms, ttft_ms, .. } => {
                            tracing::info!(target: "telemetry", conversation_id = %conversation_id, turn_id = %turn_id, "Interaction metrics: {:?}", event);
                            self.latest_stt_ms.store(*stt_latency_ms, Ordering::Relaxed);
                            self.latest_ttft_ms.store(*ttft_ms, Ordering::Relaxed);
                        }
                        TelemetryEvent::SystemHealth { cpu_usage, ram_mb } => {
                            tracing::info!(target: "telemetry", "System Health: {:?}", event);
                            self.latest_cpu.store(cpu_usage.to_bits(), Ordering::Relaxed);
                            self.latest_ram.store(*ram_mb, Ordering::Relaxed);
                        }
                        TelemetryEvent::AudioEnergy { energy, vad_prob } => {
                            // High-frequency debug only
                            tracing::debug!(target: "telemetry", "Audio Energy: {:?}", event);
                            // Update shared atomics for monitoring collector
                            use std::sync::atomic::Ordering;
                            self.latest_energy.store(energy.to_bits(), Ordering::Relaxed);
                            self.latest_vad_prob.store(vad_prob.to_bits(), Ordering::Relaxed);
                        }
                    }
                }
                tracing::info!("[Telemetry] Channel disconnected. Aggregator exiting.");
            })
            .expect("[Telemetry] Failed to spawn aggregator thread");
    }
}
