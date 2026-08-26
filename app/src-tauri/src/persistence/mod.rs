use std::time::Duration;

// ─── Persistence Subsystem Constants ─────────────────────────────────────────

/// Bounded capacity of the persistence worker event channel.
pub const PERSISTENCE_CHANNEL_CAPACITY: usize = 128;

/// Bounded capacity of the memory worker event channel.
pub const MEMORY_WORKER_CHANNEL_CAPACITY: usize = 32;

/// Minimum continuous idle debounce required before executing memory pipeline sweeps.
pub const MIN_IDLE_DEBOUNCE_SECS: u64 = 30;

/// Polling timeout for worker event loop receivers (100ms).
pub const WORKER_EVENT_POLL_TIMEOUT: Duration = Duration::from_millis(100);

/// Memory worker event loop receiver timeout (500ms).
pub const MEMORY_WORKER_POLL_TIMEOUT: Duration = Duration::from_millis(500);

/// Rate calculation interval for database writes (1s).
pub const PERSISTENCE_RATE_INTERVAL: Duration = Duration::from_secs(1);

/// Maximum retry attempts before marking a personal memory queue item as permanently failed.
pub const MAX_QUEUE_RETRY_ATTEMPTS: u32 = 3;

/// SQLite busy timeout in milliseconds (5000ms).
pub const SQLITE_BUSY_TIMEOUT_MS: u32 = 5000;

pub mod db;
pub mod events;
pub mod memory_worker;
pub mod mutations;
pub mod queries;
pub mod schema;
pub mod voices;
pub mod worker;

/// Floating-point vector byte-blob encoding and decoding helpers for Turso F32_BLOB columns.
pub fn encode_f32_blob(floats: &[f32]) -> Vec<u8> {
    floats.iter().flat_map(|f| f.to_le_bytes()).collect()
}

/// Decodes byte-blob into a float vector.
pub fn decode_f32_blob(bytes: &[u8]) -> Vec<f32> {
    bytes
        .chunks_exact(4)
        .map(|chunk| f32::from_le_bytes(chunk.try_into().unwrap_or_default()))
        .collect()
}

