use std::time::Duration;

pub const PERSISTENCE_CHANNEL_CAPACITY: usize = 128;
pub const MEMORY_WORKER_CHANNEL_CAPACITY: usize = 32;
pub const MIN_IDLE_DEBOUNCE_SECS: u64 = 30;
pub const WORKER_EVENT_POLL_TIMEOUT: Duration = Duration::from_millis(100);
pub const MEMORY_WORKER_POLL_TIMEOUT: Duration = Duration::from_millis(500);
pub const PERSISTENCE_RATE_INTERVAL: Duration = Duration::from_secs(1);
pub const MAX_QUEUE_RETRY_ATTEMPTS: u32 = 3;
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

/// Decodes byte-blob into a float vector. Returns empty vector and logs warning if misaligned.
pub fn decode_f32_blob(bytes: &[u8]) -> Vec<f32> {
    if !bytes.len().is_multiple_of(4) {
        log::warn!(
            "[Persistence] Misaligned f32 blob length {} (not a multiple of 4)",
            bytes.len()
        );
        return Vec::new();
    }
    bytes
        .chunks_exact(4)
        .map(|chunk| f32::from_le_bytes(chunk.try_into().unwrap_or_default()))
        .collect()
}
