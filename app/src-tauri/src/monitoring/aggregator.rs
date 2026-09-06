use crate::monitoring::TELEMETRY_AGGREGATOR_CHANNEL_CAPACITY;
use crossbeam_channel::{bounded, Receiver, Sender};
use serde::Serialize;
use std::sync::atomic::AtomicU32;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;
use std::sync::Arc;

/// Structured telemetry events emitted by various engine subsystems.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", content = "data")]
pub enum TelemetryEvent {
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
pub struct TelemetryAggregator {
    rx: Receiver<TelemetryEvent>,
    latest_energy: Arc<AtomicU32>,
    latest_vad_prob: Arc<AtomicU32>,
    latest_low: Arc<AtomicU32>,
    latest_mid: Arc<AtomicU32>,
    latest_high: Arc<AtomicU32>,
    latest_sys_cpu: Arc<AtomicU32>,
    latest_sys_ram: Arc<AtomicU32>,
    latest_vox_cpu: Arc<AtomicU32>,
    latest_vox_ram: Arc<AtomicU32>,
    dropped_events: Arc<AtomicU64>,
}

/// Target atomics updated by the telemetry aggregator loop.
pub struct TelemetryAggregatorHandles {
    pub latest_energy: Arc<AtomicU32>,
    pub latest_vad_prob: Arc<AtomicU32>,
    pub latest_low: Arc<AtomicU32>,
    pub latest_mid: Arc<AtomicU32>,
    pub latest_high: Arc<AtomicU32>,
    pub latest_sys_cpu: Arc<AtomicU32>,
    pub latest_sys_ram: Arc<AtomicU32>,
    pub latest_vox_cpu: Arc<AtomicU32>,
    pub latest_vox_ram: Arc<AtomicU32>,
    pub dropped_events: Arc<AtomicU64>,
}

impl TelemetryAggregator {
    /// Creates a new TelemetryAggregator and bounded Sender channel.
    pub fn new(handles: TelemetryAggregatorHandles) -> (Self, Sender<TelemetryEvent>) {
        let (tx, rx) = bounded(TELEMETRY_AGGREGATOR_CHANNEL_CAPACITY);
        (
            Self {
                rx,
                latest_energy: handles.latest_energy,
                latest_vad_prob: handles.latest_vad_prob,
                latest_low: handles.latest_low,
                latest_mid: handles.latest_mid,
                latest_high: handles.latest_high,
                latest_sys_cpu: handles.latest_sys_cpu,
                latest_sys_ram: handles.latest_sys_ram,
                latest_vox_cpu: handles.latest_vox_cpu,
                latest_vox_ram: handles.latest_vox_ram,
                dropped_events: handles.dropped_events,
            },
            tx,
        )
    }

    /// Spawns the aggregator loop on a dedicated OS thread.
    pub fn start(self) {
        std::thread::Builder::new()
            .name("vox-telemetry".to_string())
            .spawn(move || {
                log::info!("[Monitoring::Aggregator] Aggregator worker started");

                while let Ok(event) = self.rx.recv() {
                    self.handle_event(event);
                }
                log::info!("[Monitoring::Aggregator] Channel disconnected. Aggregator exiting");
            })
            .expect("[Monitoring::Aggregator] Failed to spawn aggregator thread");
    }

    fn handle_event(&self, event: TelemetryEvent) {
        match event {
            TelemetryEvent::SystemHealth {
                system_cpu,
                system_ram_pct,
                vox_cpu,
                vox_ram_mb,
            } => {
                log::debug!(
                    "[Monitoring::Aggregator] System health sys_cpu={}% sys_ram={}% vox_cpu={}% vox_ram={}MB",
                    system_cpu,
                    system_ram_pct,
                    vox_cpu,
                    vox_ram_mb
                );
                self.latest_sys_cpu
                    .store(system_cpu.to_bits(), Ordering::Relaxed);
                self.latest_sys_ram
                    .store(system_ram_pct.to_bits(), Ordering::Relaxed);
                self.latest_vox_cpu
                    .store(vox_cpu.to_bits(), Ordering::Relaxed);
                self.latest_vox_ram.store(vox_ram_mb, Ordering::Relaxed);

                let dropped = self.dropped_events.load(Ordering::Relaxed);
                if dropped > 0 {
                    log::warn!(
                        "[Monitoring::Aggregator] Dropped {} telemetry events due to channel saturation",
                        dropped
                    );
                }
            }
            TelemetryEvent::AudioEnergy {
                energy,
                vad_prob,
                low,
                mid,
                high,
            } => {
                log::debug!(
                    "[Monitoring::Aggregator] Audio energy e={} vad={} low={} mid={} high={}",
                    energy,
                    vad_prob,
                    low,
                    mid,
                    high
                );
                self.latest_energy
                    .store(energy.to_bits(), Ordering::Relaxed);
                self.latest_vad_prob
                    .store(vad_prob.to_bits(), Ordering::Relaxed);
                self.latest_low.store(low.to_bits(), Ordering::Relaxed);
                self.latest_mid.store(mid.to_bits(), Ordering::Relaxed);
                self.latest_high.store(high.to_bits(), Ordering::Relaxed);
            }
        }
    }
}
