use anyhow::Result;
use rusqlite::Connection;

/// Thin wrapper around a rusqlite Connection with mandatory WAL configuration.
///
/// WAL mode allows concurrent reads without blocking writes, which is critical
/// because IPC history queries use a separate read-only connection while the
/// persistence worker is writing session/turn data.
pub struct VoxDb(pub Connection);

impl VoxDb {
    pub fn open(path: &std::path::Path) -> Result<Self> {
        let conn = Connection::open(path)?;
        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA synchronous = NORMAL;
             PRAGMA temp_store = MEMORY;
             PRAGMA foreign_keys = ON;",
        )?;
        Ok(Self(conn))
    }

    /// Open a read-only connection for IPC history queries.
    /// WAL mode allows concurrent readers alongside the writer thread.
    pub fn open_readonly(path: &std::path::Path) -> Result<Connection> {
        let conn = Connection::open_with_flags(
            path,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_URI,
        )?;
        Ok(conn)
    }
}
