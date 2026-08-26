use crate::core::error::PersistenceError;
use crate::persistence::SQLITE_BUSY_TIMEOUT_MS;
use turso::{Builder, Connection};

/// Global static cell to hold the main Tokio runtime handle.
pub static TOKIO_HANDLE: once_cell::sync::OnceCell<tokio::runtime::Handle> =
    once_cell::sync::OnceCell::new();

/// Returns the active Tokio runtime handle, falling back to a lightweight local runtime if not initialized.
pub fn get_tokio_handle() -> tokio::runtime::Handle {
    TOKIO_HANDLE.get().cloned().unwrap_or_else(|| {
        tokio::runtime::Handle::try_current().unwrap_or_else(|_| {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("[Persistence::Db] Failed to create fallback tokio runtime");
            let handle = rt.handle().clone();
            Box::leak(Box::new(rt));
            handle
        })
    })
}

/// Async database connection wrapper for the Turso engine.
pub struct VoxDb;

impl VoxDb {
    /// Opens a connection to the local database file.
    pub async fn open(path: &std::path::Path) -> Result<Connection, PersistenceError> {
        let path_str = path.to_string_lossy();
        let db = Builder::new_local(&path_str)
            .experimental_index_method(true)
            .build()
            .await?;
        let conn = db.connect()?;

        if let Err(e) = conn.execute("PRAGMA journal_mode = WAL;", ()).await {
            log::warn!("[Persistence::Db] Failed to set journal_mode WAL: {}", e);
        }
        let timeout_pragma = format!("PRAGMA busy_timeout = {};", SQLITE_BUSY_TIMEOUT_MS);
        if let Err(e) = conn.execute(&timeout_pragma, ()).await {
            log::warn!("[Persistence::Db] Failed to set busy_timeout: {}", e);
        }
        if let Err(e) = conn.execute("PRAGMA foreign_keys = ON;", ()).await {
            log::warn!("[Persistence::Db] Failed to enable foreign_keys: {}", e);
        }

        Ok(conn)
    }

    /// Open a connection for IPC history queries.
    pub async fn open_readonly(path: &std::path::Path) -> Result<Connection, PersistenceError> {
        Self::open(path).await
    }
}

