use std::time::Duration;
use std::collections::HashMap;

pub const PERSISTENCE_CHANNEL_CAPACITY: usize = 128;
pub const MEMORY_WORKER_CHANNEL_CAPACITY: usize = 32;
pub const MIN_IDLE_DEBOUNCE_SECS: u64 = 30;
pub const WORKER_EVENT_POLL_TIMEOUT: Duration = Duration::from_millis(100);
pub const MEMORY_WORKER_POLL_TIMEOUT: Duration = Duration::from_millis(500);
pub const PERSISTENCE_RATE_INTERVAL: Duration = Duration::from_secs(1);
pub const MAX_QUEUE_RETRY_ATTEMPTS: u32 = 3;
pub const SQLITE_BUSY_TIMEOUT_MS: u32 = 5000;

pub mod compactions;
pub mod db;
pub mod graph;
pub mod memory_mutations;
pub mod memory_queries;
pub mod memory_worker;
pub mod notifications;
pub mod schema;
pub mod sessions;
pub mod voices;
pub mod worker;

pub use graph::{
    MemoryConflictItem, MemoryEdgeTopology, MemoryFactDetail, MemoryGraphPayload,
    MemoryGraphQueryFilter, MemoryNodeTopology,
};
pub use memory_mutations as mutations;
pub use memory_queries as queries;
pub use memory_queries::{MemoryQueueItem, MemoryQueueSummary};
pub use notifications::{NewNotification, NotificationRecord};
pub use sessions::{SessionRow, TurnRow};

#[derive(Debug, Clone)]
pub enum PersistenceEvent {
    SessionStarted {
        id: u64,
        timestamp_ms: u64,
    },
    SessionEnded {
        id: u64,
        timestamp_ms: u64,
    },
    TurnCompleted {
        conversation_id: u64,
        turn_id: u32,
        user_text: String,
        assistant_text: String,
        stt_latency_ms: u32,
        ttft_ms: u32,
    },
    TurnCancelled {
        conversation_id: u64,
        turn_id: u32,
    },
    Shutdown,
}

#[derive(Debug, Clone)]
pub enum MemoryWorkerEvent {
    SessionEnd {
        session_id: String,
        summary: String,
    },
    PersonalFactsReady {
        facts: HashMap<String, Vec<String>>,
        session_id: String,
    },
    ActiveSessionChanged {
        session_id: u64,
    },
    Shutdown,
}

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
