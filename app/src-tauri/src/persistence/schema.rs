use anyhow::Result;
use rusqlite::Connection;

/// Runs the CREATE TABLE IF NOT EXISTS migrations against the given connection.
///
/// Idempotent — safe to call on every startup to ensure schema is current.
/// Using INTEGER PRIMARY KEY for sessions.id (epoch ms) gives natural ordering.
pub fn run_migrations(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS sessions (
            id         INTEGER PRIMARY KEY,   -- epoch milliseconds
            started_at INTEGER NOT NULL,
            ended_at   INTEGER,
            turn_count INTEGER NOT NULL DEFAULT 0
        );

        CREATE TABLE IF NOT EXISTS turns (
            id              INTEGER PRIMARY KEY AUTOINCREMENT,
            session_id      INTEGER NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
            turn_id         INTEGER NOT NULL,
            user_text       TEXT    NOT NULL DEFAULT '',
            assistant_text  TEXT    NOT NULL DEFAULT '',
            stt_latency_ms  INTEGER,
            ttft_ms         INTEGER,
            created_at      INTEGER NOT NULL
        );

        CREATE INDEX IF NOT EXISTS idx_turns_session ON turns(session_id);
        "
    )?;
    Ok(())
}
