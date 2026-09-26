use std::{
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

use turso::{Builder, Connection};

use crate::{
    core::error::PersistenceError,
    persistence::{voices::seed_packaged_voices, SQLITE_BUSY_TIMEOUT_MS},
};

pub type Result<T> = std::result::Result<T, PersistenceError>;

pub const SCHEMA_VERSION: u32 = 7;

const V2_TABLE_STATEMENTS: &[&str] = &[
    "CREATE TABLE IF NOT EXISTS projects (
        id TEXT PRIMARY KEY,
        name TEXT NOT NULL,
        created_at INTEGER NOT NULL,
        updated_at INTEGER NOT NULL
    );",
    "CREATE TABLE IF NOT EXISTS sessions (
        id INTEGER PRIMARY KEY,
        project_id TEXT NOT NULL DEFAULT 'default' REFERENCES projects(id) ON DELETE RESTRICT,
        title TEXT,
        is_pinned INTEGER NOT NULL DEFAULT 0,
        deleted_at INTEGER,
        created_at INTEGER NOT NULL,
        updated_at INTEGER NOT NULL
    );",
    "CREATE INDEX IF NOT EXISTS idx_sessions_project_updated ON sessions(project_id, updated_at DESC);",
    "CREATE INDEX IF NOT EXISTS idx_sessions_active ON sessions(deleted_at);",
    "CREATE TABLE IF NOT EXISTS turns (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        session_id INTEGER NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
        turn_id INTEGER NOT NULL,
        user_text TEXT NOT NULL,
        assistant_text TEXT NOT NULL,
        created_at INTEGER NOT NULL
    );",
    "CREATE UNIQUE INDEX IF NOT EXISTS idx_turns_session_turn ON turns(session_id, turn_id);",
    "CREATE TABLE IF NOT EXISTS session_compactions (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        session_id INTEGER NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
        trigger_kind TEXT NOT NULL,
        from_turn_id INTEGER NOT NULL,
        to_turn_id INTEGER NOT NULL,
        compaction_output TEXT NOT NULL,
        status TEXT NOT NULL,
        error_msg TEXT,
        created_at INTEGER NOT NULL,
        finished_at INTEGER
    );",
    "CREATE INDEX IF NOT EXISTS idx_compactions_session_status ON session_compactions(session_id, status);",
    "CREATE UNIQUE INDEX IF NOT EXISTS idx_compactions_one_in_progress ON session_compactions(session_id) WHERE status = 'in_progress';",
    "CREATE TABLE IF NOT EXISTS personal_memory (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        project_id TEXT REFERENCES projects(id) ON DELETE CASCADE,
        content TEXT NOT NULL,
        version INTEGER NOT NULL DEFAULT 1,
        is_active INTEGER NOT NULL DEFAULT 1,
        last_consolidated_at INTEGER NOT NULL,
        updated_at INTEGER NOT NULL
    );",
    "CREATE UNIQUE INDEX IF NOT EXISTS idx_personal_memory_active_project ON personal_memory(project_id) WHERE is_active = 1;",
    "CREATE INDEX IF NOT EXISTS idx_personal_memory_history ON personal_memory(project_id, version DESC);",
    "CREATE TABLE IF NOT EXISTS memory_ingestion_queue (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        session_id INTEGER REFERENCES sessions(id) ON DELETE SET NULL,
        compaction_id INTEGER NOT NULL REFERENCES session_compactions(id) ON DELETE CASCADE,
        type TEXT NOT NULL,
        text TEXT NOT NULL,
        status TEXT NOT NULL DEFAULT 'pending',
        retry_count INTEGER NOT NULL DEFAULT 0,
        error_msg TEXT,
        created_at INTEGER NOT NULL,
        processed_at INTEGER
    );",
    "CREATE INDEX IF NOT EXISTS idx_queue_status_type ON memory_ingestion_queue(status, type);",
    "CREATE TABLE IF NOT EXISTS memory_facts (
        id TEXT PRIMARY KEY,
        session_id INTEGER REFERENCES sessions(id) ON DELETE SET NULL,
        compaction_id INTEGER NOT NULL REFERENCES session_compactions(id) ON DELETE CASCADE,
        type TEXT NOT NULL,
        text TEXT NOT NULL,
        status TEXT NOT NULL DEFAULT 'active',
        created_at INTEGER NOT NULL,
        updated_at INTEGER NOT NULL
    );",
    "CREATE INDEX IF NOT EXISTS idx_facts_status_type ON memory_facts(status, type);",
    "CREATE INDEX IF NOT EXISTS idx_facts_session ON memory_facts(session_id);",
    "CREATE TABLE IF NOT EXISTS memory_facts_vectors (
        fact_id TEXT PRIMARY KEY REFERENCES memory_facts(id) ON DELETE CASCADE,
        type TEXT NOT NULL,
        status TEXT NOT NULL DEFAULT 'active',
        project_id TEXT,
        created_at INTEGER NOT NULL,
        embedding F32_BLOB(384) NOT NULL
    );",
    "CREATE INDEX IF NOT EXISTS idx_vectors_filter ON memory_facts_vectors(status, type, project_id);",
    "CREATE TABLE IF NOT EXISTS notifications (
        id TEXT PRIMARY KEY,
        group_key TEXT NOT NULL,
        category TEXT NOT NULL,
        severity TEXT NOT NULL DEFAULT 'info',
        action_type TEXT NOT NULL,
        action_payload TEXT NOT NULL DEFAULT '{}',
        title TEXT NOT NULL,
        message TEXT NOT NULL,
        status TEXT NOT NULL DEFAULT 'unread',
        session_id INTEGER REFERENCES sessions(id) ON DELETE CASCADE,
        metadata TEXT NOT NULL DEFAULT '{}',
        created_at INTEGER NOT NULL,
        updated_at INTEGER NOT NULL
    );",
    "CREATE INDEX IF NOT EXISTS idx_notifications_status_created ON notifications(status, created_at DESC);",
    "CREATE INDEX IF NOT EXISTS idx_notifications_group_status ON notifications(group_key, status);",
    "CREATE INDEX IF NOT EXISTS idx_notifications_session ON notifications(session_id);",
    "CREATE TABLE IF NOT EXISTS voices (
        id TEXT PRIMARY KEY,
        name TEXT NOT NULL,
        source_kind TEXT NOT NULL,
        wav_path TEXT,
        voice_dir TEXT,
        preview_wav TEXT,
        created_at INTEGER NOT NULL
    );",
    "CREATE INDEX IF NOT EXISTS idx_voices_created ON voices(created_at DESC);",
    "CREATE TABLE IF NOT EXISTS session_tool_calls (
        id TEXT PRIMARY KEY,
        session_id INTEGER NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
        turn_id INTEGER NOT NULL,
        tool_name TEXT NOT NULL,
        tool_kind TEXT NOT NULL,
        arguments TEXT NOT NULL,
        result TEXT,
        is_error INTEGER NOT NULL DEFAULT 0,
        duration_ms INTEGER,
        created_at INTEGER NOT NULL
    );",
    "CREATE INDEX IF NOT EXISTS idx_tool_calls_session_turn ON session_tool_calls(session_id, turn_id);",
    "CREATE INDEX IF NOT EXISTS idx_tool_calls_created ON session_tool_calls(created_at DESC);",
    "CREATE TABLE IF NOT EXISTS personal_memory_suggestions (
        id TEXT PRIMARY KEY,
        base_memory_version INTEGER NOT NULL,
        project_id TEXT REFERENCES projects(id) ON DELETE CASCADE,
        op TEXT NOT NULL,
        section TEXT NOT NULL,
        target_text TEXT,
        proposed_text TEXT NOT NULL,
        source_fact_ids TEXT NOT NULL,
        status TEXT NOT NULL DEFAULT 'pending',
        created_at INTEGER NOT NULL,
        resolved_at INTEGER
    );",
    "CREATE INDEX IF NOT EXISTS idx_suggestions_pending ON personal_memory_suggestions(base_memory_version, status);",
    "CREATE INDEX IF NOT EXISTS idx_suggestions_status_proj ON personal_memory_suggestions(project_id, status);",
    "CREATE INDEX IF NOT EXISTS idx_suggestions_created ON personal_memory_suggestions(created_at DESC);",
];

/// Runs schema migrations, dropping obsolete legacy tables and initializing v2 schema.
pub async fn run_migrations(conn: &Connection) -> Result<()> {
    let current_version: u32 = {
        let mut rows = conn.query("PRAGMA user_version;", ()).await?;
        if let Some(row) = rows.next().await? {
            row.get(0).unwrap_or(0)
        } else {
            0
        }
    };

    if current_version < SCHEMA_VERSION {
        log::info!(
            "[Persistence::Schema] Migrating database schema from v{} to v{}",
            current_version,
            SCHEMA_VERSION
        );
        conn.execute("PRAGMA foreign_keys = OFF;", ()).await?;
        conn.execute("PRAGMA foreign_keys = ON;", ()).await?;

        if current_version > 0 && current_version < 6 {
            let _ = conn
                .execute(
                    "ALTER TABLE personal_memory ADD COLUMN is_active INTEGER NOT NULL DEFAULT 1;",
                    (),
                )
                .await;
            let _ = conn
                .execute("DROP INDEX IF EXISTS idx_personal_memory_project;", ())
                .await;
        }

        if current_version < 7 {
            conn.execute(
                "CREATE TABLE IF NOT EXISTS personal_memory_suggestions (
                    id TEXT PRIMARY KEY,
                    base_memory_version INTEGER NOT NULL,
                    project_id TEXT REFERENCES projects(id) ON DELETE CASCADE,
                    op TEXT NOT NULL,
                    section TEXT NOT NULL,
                    target_text TEXT,
                    proposed_text TEXT NOT NULL,
                    source_fact_ids TEXT NOT NULL,
                    status TEXT NOT NULL DEFAULT 'pending',
                    created_at INTEGER NOT NULL,
                    resolved_at INTEGER
                );",
                (),
            )
            .await?;
            conn.execute(
                "CREATE INDEX IF NOT EXISTS idx_suggestions_pending ON personal_memory_suggestions(base_memory_version, status);",
                (),
            )
            .await?;
            conn.execute(
                "CREATE INDEX IF NOT EXISTS idx_suggestions_status_proj ON personal_memory_suggestions(project_id, status);",
                (),
            )
            .await?;
            conn.execute(
                "CREATE INDEX IF NOT EXISTS idx_suggestions_created ON personal_memory_suggestions(created_at DESC);",
                (),
            )
            .await?;
        }

        for stmt in V2_TABLE_STATEMENTS {
            conn.execute(stmt, ()).await?;
        }

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64;

        conn.execute(
            "INSERT OR IGNORE INTO projects (id, name, created_at, updated_at) VALUES ('default', 'Default', ?, ?);",
            (now, now),
        )
        .await?;

        conn.execute(
            "INSERT OR IGNORE INTO personal_memory (project_id, content, version, last_consolidated_at, updated_at) VALUES (NULL, '', 1, ?, ?);",
            (now, now),
        )
        .await?;

        let pragma_version = format!("PRAGMA user_version = {};", SCHEMA_VERSION);
        conn.execute(&pragma_version, ()).await?;

        log::info!(
            "[Persistence::Schema] Database schema initialized to v{}",
            SCHEMA_VERSION
        );
    }

    if let Err(e) = seed_packaged_voices(conn).await {
        log::warn!(
            "[Persistence::Schema] Failed to seed packaged voices: {}",
            e
        );
    }

    Ok(())
}

/// Drops all tables and forces a full recreation of the v2 schema.
pub async fn recreate_schema(conn: &Connection) -> Result<()> {
    conn.execute("PRAGMA foreign_keys = OFF;", ()).await?;
    conn.execute("DROP TABLE IF EXISTS voices;", ()).await?;
    conn.execute("PRAGMA foreign_keys = ON;", ()).await?;
    conn.execute("PRAGMA user_version = 0;", ()).await?;
    run_migrations(conn).await?;
    Ok(())
}

/// Rebuilds a binary database fixture from scratch with the v2 schema and default seeds.
pub async fn rebuild_fixture_db(path: &Path) -> Result<()> {
    if path.exists() {
        std::fs::remove_file(path)?;
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let path_str = path.to_string_lossy();
    let db = Builder::new_local(&path_str)
        .experimental_index_method(true)
        .build()
        .await?;
    let conn = db.connect()?;

    if let Err(e) = conn.query("PRAGMA journal_mode = WAL;", ()).await {
        log::warn!(
            "[Persistence::Schema] Failed to set journal_mode WAL on fixture: {}",
            e
        );
    }
    let timeout_pragma = format!("PRAGMA busy_timeout = {};", SQLITE_BUSY_TIMEOUT_MS);
    if let Err(e) = conn.execute(&timeout_pragma, ()).await {
        log::warn!(
            "[Persistence::Schema] Failed to set busy_timeout on fixture: {}",
            e
        );
    }
    if let Err(e) = conn.execute("PRAGMA foreign_keys = ON;", ()).await {
        log::warn!(
            "[Persistence::Schema] Failed to enable foreign_keys on fixture: {}",
            e
        );
    }

    run_migrations(&conn).await?;
    if let Err(e) = conn.execute("PRAGMA wal_checkpoint(TRUNCATE);", ()).await {
        log::warn!(
            "[Persistence::Schema] Failed to checkpoint WAL on fixture: {}",
            e
        );
    }
    drop(conn);
    drop(db);

    let wal_path = path.with_extension("db-wal");
    if wal_path.exists() {
        if let Err(e) = std::fs::remove_file(wal_path) {
            log::warn!("[Persistence::Schema] Failed to remove WAL file: {}", e);
        }
    }
    let shm_path = path.with_extension("db-shm");
    if shm_path.exists() {
        if let Err(e) = std::fs::remove_file(shm_path) {
            log::warn!("[Persistence::Schema] Failed to remove SHM file: {}", e);
        }
    }

    log::info!(
        "[Persistence::Schema] Successfully rebuilt fixture database at {:?}",
        path
    );
    Ok(())
}

