use crossbeam_channel::{bounded, Receiver, Sender};
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
        system_cpu: f32,
        system_ram_pct: f32,
        vox_cpu: f32,
        vox_ram_mb: u32,
    },
    /// Real-time audio signal characteristics (VAD hot-path)
    AudioEnergy {
        energy: f32,
        vad_prob: f32,
        low: f32,
        mid: f32,
        high: f32,
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
    latest_low: std::sync::Arc<std::sync::atomic::AtomicU32>,
    latest_mid: std::sync::Arc<std::sync::atomic::AtomicU32>,
    latest_high: std::sync::Arc<std::sync::atomic::AtomicU32>,
    latest_sys_cpu: std::sync::Arc<std::sync::atomic::AtomicU32>,
    latest_sys_ram: std::sync::Arc<std::sync::atomic::AtomicU32>,
    latest_vox_cpu: std::sync::Arc<std::sync::atomic::AtomicU32>,
    latest_vox_ram: std::sync::Arc<std::sync::atomic::AtomicU32>,
    latest_stt_ms: std::sync::Arc<std::sync::atomic::AtomicU32>,
    latest_ttft_ms: std::sync::Arc<std::sync::atomic::AtomicU32>,
    dropped_events: std::sync::Arc<std::sync::atomic::AtomicU64>,
}

impl TelemetryAggregator {
    pub fn new(
        latest_energy: std::sync::Arc<std::sync::atomic::AtomicU32>,
        latest_vad_prob: std::sync::Arc<std::sync::atomic::AtomicU32>,
        latest_low: std::sync::Arc<std::sync::atomic::AtomicU32>,
        latest_mid: std::sync::Arc<std::sync::atomic::AtomicU32>,
        latest_high: std::sync::Arc<std::sync::atomic::AtomicU32>,
        latest_sys_cpu: std::sync::Arc<std::sync::atomic::AtomicU32>,
        latest_sys_ram: std::sync::Arc<std::sync::atomic::AtomicU32>,
        latest_vox_cpu: std::sync::Arc<std::sync::atomic::AtomicU32>,
        latest_vox_ram: std::sync::Arc<std::sync::atomic::AtomicU32>,
        latest_stt_ms: std::sync::Arc<std::sync::atomic::AtomicU32>,
        latest_ttft_ms: std::sync::Arc<std::sync::atomic::AtomicU32>,
        dropped_events: std::sync::Arc<std::sync::atomic::AtomicU64>,
    ) -> (Self, Sender<TelemetryEvent>) {
        let (tx, rx) = bounded(4096);
        (
            Self {
                rx,
                latest_energy,
                latest_vad_prob,
                latest_low,
                latest_mid,
                latest_high,
                latest_sys_cpu,
                latest_sys_ram,
                latest_vox_cpu,
                latest_vox_ram,
                latest_stt_ms,
                latest_ttft_ms,
                dropped_events,
            },
            tx,
        )
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
                        TelemetryEvent::SystemHealth { system_cpu, system_ram_pct, vox_cpu, vox_ram_mb } => {
                            tracing::debug!(target: "telemetry", "System Health: {:?}", event);
                            self.latest_sys_cpu.store(system_cpu.to_bits(), Ordering::Relaxed);
                            self.latest_sys_ram.store(system_ram_pct.to_bits(), Ordering::Relaxed);
                            self.latest_vox_cpu.store(vox_cpu.to_bits(), Ordering::Relaxed);
                            self.latest_vox_ram.store(*vox_ram_mb, Ordering::Relaxed);

                            // Periodically log dropped events (Architect correction: avoid hot-path logging)
                            let dropped = self.dropped_events.load(Ordering::Relaxed);
                            if dropped > 0 {
                                tracing::warn!(target: "telemetry", "Dropped {} telemetry events due to channel saturation.", dropped);
                            }
                        }
                        TelemetryEvent::AudioEnergy { energy, vad_prob, low, mid, high } => {
                            // High-frequency debug only
                            tracing::debug!(target: "telemetry", "Audio Energy: {:?}", event);
                            // Update shared atomics for monitoring collector
                            use std::sync::atomic::Ordering;
                            self.latest_energy.store(energy.to_bits(), Ordering::Relaxed);
                            self.latest_vad_prob.store(vad_prob.to_bits(), Ordering::Relaxed);
                            self.latest_low.store(low.to_bits(), Ordering::Relaxed);
                            self.latest_mid.store(mid.to_bits(), Ordering::Relaxed);
                            self.latest_high.store(high.to_bits(), Ordering::Relaxed);
                        }
                    }
                }
                tracing::info!("[Telemetry] Channel disconnected. Aggregator exiting.");
            })
            .expect("[Telemetry] Failed to spawn aggregator thread");
    }
}
