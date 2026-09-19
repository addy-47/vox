use std::{
    path::{Path, PathBuf},
    sync::{Arc, LazyLock},
    time::Duration,
};

use turso::{Builder, Connection, Database};

use crate::{core::error::PersistenceError, persistence::SQLITE_BUSY_TIMEOUT_MS};

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
///
/// Holds the underlying `Database` engine handle (`Clone + Send + Sync`) which coordinates
/// the shared page cache, WAL, and background checkpointing. Vends independent `Connection`
/// execution contexts via `connect()`.
#[derive(Clone)]
pub struct VoxDb {
    db: Arc<Database>,
    path: PathBuf,
}

impl VoxDb {
    /// Opens the local database engine and runs initial pragmas on a bootstrap connection.
    pub async fn open(path: impl AsRef<Path>) -> Result<Self, PersistenceError> {
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
    pub fn connect(&self) -> Result<Connection, PersistenceError> {
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
