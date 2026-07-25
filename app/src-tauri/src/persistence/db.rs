use crate::core::error::PersistenceError;
use turso::{Builder, Connection};

/// Global static cell to hold the main Tokio runtime handle.
pub static TOKIO_HANDLE: once_cell::sync::OnceCell<tokio::runtime::Handle> = once_cell::sync::OnceCell::new();

/// Returns the active Tokio runtime handle, falling back to a lightweight local runtime if not initialized.
pub fn get_tokio_handle() -> tokio::runtime::Handle {
    TOKIO_HANDLE.get().cloned().unwrap_or_else(|| {
        tokio::runtime::Handle::try_current().unwrap_or_else(|_| {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("Failed to create fallback tokio runtime");
            let handle = rt.handle().clone();
            // Leaking the runtime keeps it alive for the duration of the process
            Box::leak(Box::new(rt));
            handle
        })
    })
}

/// Async database connection wrapper for the Turso (Limbo) engine.
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
        // Optimize SQLite concurrency characteristics
        let _ = conn.execute("PRAGMA journal_mode = WAL;", ()).await;
        let _ = conn.execute("PRAGMA busy_timeout = 5000;", ()).await;
        let _ = conn.execute("PRAGMA foreign_keys = ON;", ()).await;
        Ok(conn)
    }

    /// Open a connection for IPC history queries.
    /// In Turso, since connections are cheap and safe, this behaves exactly like `open`.
    pub async fn open_readonly(path: &std::path::Path) -> Result<Connection, PersistenceError> {
        Self::open(path).await
    }
}
