use std::{
    path::{Path, PathBuf},
    sync::{Arc, LazyLock},
    time::Duration,
};

use turso::{Builder, Connection, Database};

pub const PERSISTENCE_CHANNEL_CAPACITY: usize = 128;
pub const WORKER_EVENT_POLL_TIMEOUT: Duration = Duration::from_millis(100);
pub const PERSISTENCE_RATE_INTERVAL: Duration = Duration::from_secs(1);
pub const MAX_QUEUE_RETRY_ATTEMPTS: u32 = 3;
pub const SQLITE_BUSY_TIMEOUT_MS: u32 = 5000;

pub mod compactions;
pub mod facts;
pub mod notifications;
pub mod personal_memory;
pub mod projects;
pub mod queue;
pub mod schema;
pub mod sessions;
pub mod tool_calls;
pub mod voices;
pub mod worker;

pub use compactions::{
    commit_compaction_output, has_in_progress_compaction, pause_in_progress_compactions,
    record_compaction_start, resolve_uncompacted_range, CompactionRecord,
};
pub use facts::{
    deactivate_fact, deactivate_facts_batch, fetch_active_episodic_memory,
    fetch_active_facts_by_type, fetch_all_active_facts, mark_facts_consolidated,
    mark_facts_rejected, mark_facts_staged, EpisodicFactCandidate, FactRecord,
};
pub use notifications::{NewNotification, NotificationRecord};
pub use personal_memory::{
    fetch_pending_suggestions, get_personal_memory, insert_personal_memory_suggestions,
    list_personal_memory_versions, resolve_suggestions_transaction, save_consolidated_memory,
    save_personal_memory, set_active_personal_memory_version, update_consolidated_memory,
    MemorySuggestionRecord, PersonalMemoryRecord, PersonalMemorySuggestionRecord,
};
pub use projects::ProjectRow;
pub use queue::{
    enqueue_fact, has_unfinished_items, record_queue_item_failure, update_queue_item_status,
    QueueItem,
};
pub use sessions::{
    ensure_session_exists, fetch_session_project_id, set_session_title, SessionRow, TurnRow,
};
pub use tool_calls::persist_tool_call;

/// Asynchronous pipeline events offloaded from the voice hot-path to the persistence worker.
#[derive(Debug, Clone)]
pub enum PersistenceEvent {
    SessionStarted {
        session_id: i64,
        timestamp_ms: u64,
    },
    SessionEnded {
        session_id: i64,
        timestamp_ms: u64,
    },
    TurnCompleted {
        session_id: i64,
        turn_id: u32,
        user_text: String,
        assistant_text: String,
    },
    ToolCallExecuted {
        id: String,
        session_id: i64,
        turn_id: u32,
        tool_name: String,
        tool_flow: crate::services::llm::ToolFlow,
        arguments: serde_json::Value,
        result: String,
        is_error: bool,
        duration_ms: u64,
        created_at: u64,
    },
    UpdateSessionMetadata {
        session_id: i64,
        key: String,
        value: String,
    },
    Shutdown,
}

/// Floating-point vector byte-blob encoding helper for Turso F32_BLOB columns.
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

/// Global static cell to hold the main Tokio runtime handle.
pub static TOKIO_HANDLE: once_cell::sync::OnceCell<tokio::runtime::Handle> =
    once_cell::sync::OnceCell::new();

/// Returns the active Tokio runtime handle, falling back to a lightweight local runtime if not initialized.
pub fn get_tokio_handle() -> tokio::runtime::Handle {
    TOKIO_HANDLE.get().cloned().unwrap_or_else(|| {
        tokio::runtime::Handle::try_current().unwrap_or_else(|_| {
            static FALLBACK_RT: LazyLock<tokio::runtime::Runtime> = LazyLock::new(|| {
                tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .expect("[Persistence::Db] Failed to create fallback tokio runtime")
            });
            FALLBACK_RT.handle().clone()
        })
    })
}

/// Thread-safe database engine wrapper for Turso.
#[derive(Clone)]
pub struct VoxDb {
    db: Arc<Database>,
    path: PathBuf,
}

impl VoxDb {
    /// Opens the local database engine and runs initial pragmas on a bootstrap connection.
    pub async fn open(
        path: impl AsRef<Path>,
    ) -> Result<Self, crate::core::error::PersistenceError> {
        let path_buf = path.as_ref().to_path_buf();
        let path_str = path_buf.to_string_lossy().to_string();
        let db = Builder::new_local(&path_str)
            .experimental_index_method(true)
            .build()
            .await?;

        // Single bootstrap connection to initialize PRAGMAs
        let conn = db.connect()?;
        if let Err(e) = conn.pragma_update("journal_mode", "'mvcc'").await {
            log::warn!("[Persistence::Db] Failed to set journal_mode mvcc: {}", e);
        }
        if let Err(e) = conn.busy_timeout(Duration::from_millis(SQLITE_BUSY_TIMEOUT_MS as u64)) {
            log::warn!("[Persistence::Db] Failed to set busy_timeout: {}", e);
        }
        if let Err(e) = conn.pragma_update("foreign_keys", "ON").await {
            log::warn!("[Persistence::Db] Failed to enable foreign_keys: {}", e);
        }

        Ok(Self {
            db: Arc::new(db),
            path: path_buf,
        })
    }

    /// Vends an independent connection execution context sharing the underlying page cache & WAL.
    pub fn connect(&self) -> Result<Connection, crate::core::error::PersistenceError> {
        let conn = self.db.connect()?;
        if let Err(e) = conn.busy_timeout(Duration::from_millis(SQLITE_BUSY_TIMEOUT_MS as u64)) {
            log::warn!(
                "[Persistence::Db] Failed to set busy_timeout on vended connection: {}",
                e
            );
        }
        Ok(conn)
    }

    /// Returns the database file path.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Checks if a Turso error is a retryable MVCC conflict or busy condition.
    pub fn is_retryable(e: &turso::Error) -> bool {
        matches!(e, turso::Error::Busy(_) | turso::Error::BusySnapshot(_))
            || matches!(e, turso::Error::Error(msg) if msg.contains("conflict"))
    }
}
