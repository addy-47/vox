use std::time::Duration;

// ─── Monitoring Subsystem Constants ──────────────────────────────────────────

/// Capacity of the lock-free telemetry aggregator channel.
pub const TELEMETRY_AGGREGATOR_CHANNEL_CAPACITY: usize = 4096;

/// Collector tick interval (10Hz).
pub const COLLECTOR_TICK_INTERVAL: Duration = Duration::from_millis(100);

/// System monitor interval for polling global/vox system usage (30s).
pub const SYSTEM_MONITOR_INTERVAL: Duration = Duration::from_millis(30000);

/// Telemetry emitter interval for pushing audio/VAD levels to UI (30Hz / 33ms).
pub const TELEMETRY_EMITTER_INTERVAL: Duration = Duration::from_millis(33);

/// Maximum number of snapshots to retain in memory (~60 seconds at 10Hz).
pub const MAX_SNAPSHOT_HISTORY: usize = 600;

pub mod aggregator;
pub mod collector;
pub mod profiler;
pub mod runtime_state;
pub mod snapshot;
pub mod system_monitor;
pub mod telemetry_emitter;

pub use profiler::{
    collect_profiler_snapshot, persist_memory_profile_event, MemoryProfileLogEvent,
    ProcessMemoryEntry, ProfilerSnapshot,
};
