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

const SCHEMA_VERSION: u32 = 3;

const DROP_LEGACY_TABLES: &[&str] = &[
    "DROP TABLE IF EXISTS memory_relations;",
    "DROP TABLE IF EXISTS personal_memory_queue;",
    "DROP TABLE IF EXISTS memory_pipeline_metrics;",
    "DROP TABLE IF EXISTS memory_facts_vectors;",
    "DROP TABLE IF EXISTS memory_facts;",
    "DROP TABLE IF EXISTS memory_ingestion_queue;",
    "DROP TABLE IF EXISTS session_compactions;",
    "DROP TABLE IF EXISTS turns;",
    "DROP TABLE IF EXISTS notifications;",
    "DROP TABLE IF EXISTS personal_memory;",
    "DROP TABLE IF EXISTS sessions;",
    "DROP TABLE IF EXISTS projects;",
];

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
        last_consolidated_at INTEGER NOT NULL,
        updated_at INTEGER NOT NULL
    );",
    "CREATE UNIQUE INDEX IF NOT EXISTS idx_personal_memory_project ON personal_memory(project_id);",
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
        category TEXT NOT NULL,
        title TEXT NOT NULL,
        message TEXT NOT NULL,
        status TEXT NOT NULL DEFAULT 'pending',
        session_id INTEGER REFERENCES sessions(id) ON DELETE CASCADE,
        metadata TEXT NOT NULL DEFAULT '{}',
        is_read INTEGER NOT NULL DEFAULT 0,
        created_at INTEGER NOT NULL
    );",
    "CREATE INDEX IF NOT EXISTS idx_notifications_status_cat ON notifications(status, category);",
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
];

/// Runs schema migrations, dropping obsolete legacy tables and initializing v2 schema.
pub async fn run_migrations(conn: &Connection) -> Result<()> {
    let mut rows = conn.query("PRAGMA user_version;", ()).await?;
    let current_version: u32 = if let Some(row) = rows.next().await? {
        row.get(0).unwrap_or(0)
    } else {
        0
    };

    if current_version < SCHEMA_VERSION {
        log::info!(
            "[Persistence::Schema] Migrating database schema from v{} to v{}",
            current_version,
            SCHEMA_VERSION
        );

        conn.execute("PRAGMA foreign_keys = OFF;", ()).await?;
        for drop_stmt in DROP_LEGACY_TABLES {
            conn.execute(drop_stmt, ()).await?;
        }
        conn.execute("PRAGMA foreign_keys = ON;", ()).await?;

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
            "[Persistence::Schema] Failed to seed packaged voices (non-fatal): {}",
            e
        );
    }

    Ok(())
}

/// Drops all tables and forces a full recreation of the v2 schema.
pub async fn recreate_schema(conn: &Connection) -> Result<()> {
    conn.execute("PRAGMA foreign_keys = OFF;", ()).await?;
    for drop_stmt in DROP_LEGACY_TABLES {
        conn.execute(drop_stmt, ()).await?;
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_v2_schema_initialization() {
        let temp_dir = tempfile::tempdir().expect("Failed to create tempdir");
        let db_path = temp_dir.path().join("test_schema.db");

        let db = Builder::new_local(db_path.to_string_lossy().as_ref())
            .experimental_index_method(true)
            .build()
            .await
            .expect("Failed to build turso db");
        let conn = db.connect().expect("Failed to connect");

        conn.execute("PRAGMA foreign_keys = ON;", ())
            .await
            .expect("Enable FK");

        run_migrations(&conn)
            .await
            .expect("Migrations should succeed");

        // Verify user_version == 2
        let mut ver_rows = conn
            .query("PRAGMA user_version;", ())
            .await
            .expect("Query user_version");
        let version: u32 = ver_rows
            .next()
            .await
            .expect("Next row")
            .expect("Row exists")
            .get(0)
            .expect("Version column");
        assert_eq!(version, 3, "Schema version must be 3");

        // Verify all 10 tables exist
        let expected_tables = [
            "projects",
            "sessions",
            "turns",
            "session_compactions",
            "personal_memory",
            "memory_ingestion_queue",
            "memory_facts",
            "memory_facts_vectors",
            "notifications",
            "voices",
        ];

        for table in &expected_tables {
            let mut rows = conn
                .query(
                    "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?",
                    (*table,),
                )
                .await
                .expect("Query sqlite_master");
            assert!(
                rows.next().await.expect("Row next").is_some(),
                "Table '{}' must exist in v2 schema",
                table
            );
        }

        // Verify default project seed exists
        let mut proj_rows = conn
            .query("SELECT name FROM projects WHERE id = 'default'", ())
            .await
            .expect("Query default project");
        let proj_name: String = proj_rows
            .next()
            .await
            .expect("Row next")
            .expect("Default project row exists")
            .get(0)
            .expect("Project name");
        assert_eq!(proj_name, "Default");

        // Verify default personal_memory seed exists
        let mut mem_rows = conn
            .query(
                "SELECT content, version FROM personal_memory WHERE project_id IS NULL",
                (),
            )
            .await
            .expect("Query default personal memory");
        let (content, ver): (String, i64) = {
            let row = mem_rows
                .next()
                .await
                .expect("Row next")
                .expect("Default personal memory exists");
            (row.get(0).expect("Content"), row.get(1).expect("Version"))
        };
        assert_eq!(content, "");
        assert_eq!(ver, 1);

        // Verify foreign key cascades and restrictions
        let now = 1000i64;
        conn.execute(
            "INSERT INTO sessions (id, project_id, title, is_pinned, created_at, updated_at) VALUES (1, 'default', 'Session 1', 0, ?, ?)",
            (now, now),
        )
        .await
        .expect("Insert session");

        conn.execute(
            "INSERT INTO turns (session_id, turn_id, user_text, assistant_text, created_at) VALUES (1, 1, 'Hello', 'Hi', ?)",
            (now,),
        )
        .await
        .expect("Insert turn");

        // Deleting project 'default' must fail due to ON DELETE RESTRICT from sessions
        let proj_del_res = conn
            .execute("DELETE FROM projects WHERE id = 'default'", ())
            .await;
        assert!(
            proj_del_res.is_err(),
            "Deleting project with child sessions must violate foreign key restriction"
        );

        // Deleting session must cascade to turns
        conn.execute("DELETE FROM sessions WHERE id = 1", ())
            .await
            .expect("Delete session");
        let mut turn_rows = conn
            .query("SELECT COUNT(*) FROM turns WHERE session_id = 1", ())
            .await
            .expect("Query turns count");
        let turn_count: i64 = turn_rows
            .next()
            .await
            .expect("Row next")
            .expect("Count row exists")
            .get(0)
            .expect("Count col");
        assert_eq!(turn_count, 0, "Turns must cascade delete with session");

        // Verify mutual exclusion: only one in_progress run per session
        conn.execute(
            "INSERT INTO sessions (id, project_id, title, is_pinned, created_at, updated_at) VALUES (2, 'default', 'Session 2', 0, ?, ?)",
            (now, now),
        )
        .await
        .expect("Insert session 2");
        conn.execute(
            "INSERT INTO session_compactions (session_id, trigger_kind, from_turn_id, to_turn_id, compaction_output, status, created_at) VALUES (2, 'manual', 1, 1, '', 'in_progress', ?)",
            (now,),
        )
        .await
        .expect("Insert first in_progress run");
        let dup = conn
            .execute(
                "INSERT INTO session_compactions (session_id, trigger_kind, from_turn_id, to_turn_id, compaction_output, status, created_at) VALUES (2, 'manual', 1, 1, '', 'in_progress', ?)",
                (now,),
            )
            .await;
        assert!(
            dup.is_err(),
            "Second in_progress run for the same session must be rejected"
        );
    }

    #[tokio::test]
    async fn test_rebuild_fixtures() {
        let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let test_db_path = root.join("tests/assets/test_vox.db");
        let bench_db_path = root.join("benches/assets/bench_vox.db");

        rebuild_fixture_db(&test_db_path)
            .await
            .expect("Failed to rebuild test fixture DB");
        rebuild_fixture_db(&bench_db_path)
            .await
            .expect("Failed to rebuild bench fixture DB");
    }
}
