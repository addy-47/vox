use turso::Connection;

use crate::{
    core::error::PersistenceError,
    persistence::voices::{seed_packaged_voices, seed_zipvoice_voices},
    services::tts::voice::sync_manifest_voices_to_db,
};

pub type Result<T> = std::result::Result<T, PersistenceError>;

pub const SCHEMA_VERSION: u32 = 1;

const SCHEMA_TABLE_STATEMENTS: &[&str] = &[
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
    "CREATE TABLE IF NOT EXISTS session_steps (
        id TEXT PRIMARY KEY,
        session_id INTEGER NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
        turn_id INTEGER NOT NULL,
        step_index INTEGER NOT NULL,
        content TEXT,
        tool_count INTEGER NOT NULL DEFAULT 0,
        duration_ms INTEGER,
        prompt_tokens INTEGER,
        completion_tokens INTEGER,
        guardrail_strike INTEGER NOT NULL DEFAULT 0,
        created_at INTEGER NOT NULL
    );",
    "CREATE INDEX IF NOT EXISTS idx_session_steps_lookup ON session_steps(session_id, turn_id, step_index);",
    "CREATE INDEX IF NOT EXISTS idx_session_steps_created ON session_steps(created_at DESC);",
    "CREATE TABLE IF NOT EXISTS session_tool_calls (
        id TEXT PRIMARY KEY,
        session_id INTEGER NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
        turn_id INTEGER NOT NULL,
        step_id TEXT REFERENCES session_steps(id) ON DELETE CASCADE,
        step_index INTEGER NOT NULL DEFAULT 0,
        tool_name TEXT NOT NULL,
        arguments TEXT NOT NULL,
        result TEXT,
        is_error INTEGER NOT NULL DEFAULT 0,
        duration_ms INTEGER,
        truncated INTEGER NOT NULL DEFAULT 0,
        spillover_path TEXT,
        created_at INTEGER NOT NULL
    );",
    "CREATE INDEX IF NOT EXISTS idx_tool_calls_session_turn ON session_tool_calls(session_id, turn_id);",
    "CREATE INDEX IF NOT EXISTS idx_tool_calls_step_lookup ON session_tool_calls(step_id, step_index);",
    "CREATE INDEX IF NOT EXISTS idx_tool_calls_created ON session_tool_calls(created_at DESC);",
    "CREATE TABLE IF NOT EXISTS session_compactions (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        session_id INTEGER NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
        turn_id INTEGER,
        step_id TEXT REFERENCES session_steps(id) ON DELETE SET NULL,
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
    "CREATE INDEX IF NOT EXISTS idx_compactions_step ON session_compactions(step_id);",
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
        provider_id TEXT NOT NULL DEFAULT 'kokoro',
        name TEXT NOT NULL,
        source_kind TEXT NOT NULL,
        slug TEXT,
        gender TEXT NOT NULL DEFAULT 'neutral',
        locale TEXT NOT NULL DEFAULT 'en-US',
        accent TEXT,
        description TEXT,
        tags TEXT NOT NULL DEFAULT '[]',
        sample_text TEXT NOT NULL DEFAULT 'Hello, this is a preview of my voice.',
        wav_path TEXT,
        voice_dir TEXT,
        preview_wav TEXT,
        is_favorite INTEGER NOT NULL DEFAULT 0,
        last_used_at INTEGER,
        created_at INTEGER NOT NULL
    );",
    "CREATE INDEX IF NOT EXISTS idx_voices_created ON voices(created_at DESC);",
    "CREATE INDEX IF NOT EXISTS idx_voices_provider ON voices(provider_id);",
    "CREATE INDEX IF NOT EXISTS idx_voices_source ON voices(source_kind);",
    "CREATE INDEX IF NOT EXISTS idx_voices_favorite ON voices(is_favorite);",
    "CREATE INDEX IF NOT EXISTS idx_voices_last_used ON voices(last_used_at DESC);",
    "CREATE TABLE IF NOT EXISTS personal_memory_revisions (
        id TEXT PRIMARY KEY,
        base_memory_version INTEGER NOT NULL,
        project_id TEXT REFERENCES projects(id) ON DELETE CASCADE,
        op TEXT NOT NULL,
        target_id TEXT NOT NULL,
        content TEXT NOT NULL,
        status TEXT NOT NULL DEFAULT 'pending',
        created_at INTEGER NOT NULL,
        resolved_at INTEGER
    );",
    "CREATE INDEX IF NOT EXISTS idx_revisions_pending ON personal_memory_revisions(base_memory_version, status);",
    "CREATE INDEX IF NOT EXISTS idx_revisions_status_proj ON personal_memory_revisions(project_id, status);",
    "CREATE INDEX IF NOT EXISTS idx_revisions_created ON personal_memory_revisions(created_at DESC);",
    "CREATE TABLE IF NOT EXISTS models (
        id TEXT PRIMARY KEY,
        model_type TEXT NOT NULL,
        category TEXT NOT NULL,
        provider TEXT,
        parameters TEXT NOT NULL DEFAULT '{}',
        is_favorite INTEGER NOT NULL DEFAULT 0,
        last_active_at INTEGER,
        created_at INTEGER NOT NULL,
        updated_at INTEGER NOT NULL
    );",
    "CREATE INDEX IF NOT EXISTS idx_models_type_cat ON models(model_type, category);",
    "CREATE INDEX IF NOT EXISTS idx_models_fav ON models(is_favorite);",
    "CREATE INDEX IF NOT EXISTS idx_models_last_active ON models(last_active_at DESC);",
    "CREATE TABLE IF NOT EXISTS paired_devices (
        device_id    TEXT    PRIMARY KEY,
        public_key   BLOB    NOT NULL CHECK (length(public_key) = 32),
        display_name TEXT    NOT NULL,
        role         TEXT    NOT NULL CHECK (role IN ('controller', 'host')),
        trust_level  TEXT    NOT NULL DEFAULT 'full' CHECK (trust_level IN ('full')),
        hints        TEXT    NOT NULL DEFAULT '[]',
        paired_at    INTEGER NOT NULL,
        last_seen_at INTEGER
    );",
];

/// Initializes the database schema to v1 and seeds initial static assets.
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
            "[Persistence::Schema] Initializing database schema to v{}",
            SCHEMA_VERSION
        );

        conn.execute("PRAGMA foreign_keys = ON;", ()).await?;

        for statement in SCHEMA_TABLE_STATEMENTS {
            conn.execute(statement, ()).await?;
        }

        conn.execute(
            &format!("PRAGMA user_version = {};", SCHEMA_VERSION),
            (),
        )
        .await?;

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64;

        conn.execute(
            "INSERT OR IGNORE INTO projects (id, name, created_at, updated_at) VALUES ('default', 'Default', ?, ?);",
            (now, now),
        )
        .await?;

        conn.execute(
            "INSERT INTO personal_memory (project_id, content, version, last_consolidated_at, updated_at)
             SELECT NULL, '{\"sections\":[]}', 1, ?, ? WHERE NOT EXISTS (SELECT 1 FROM personal_memory WHERE project_id IS NULL);",
            (now, now),
        )
        .await?;

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

    if let Err(e) = seed_zipvoice_voices(conn).await {
        log::warn!(
            "[Persistence::Schema] Failed to seed zipvoice voices: {}",
            e
        );
    }

    if let Err(e) = sync_manifest_voices_to_db(conn).await {
        log::warn!(
            "[Persistence::Schema] Failed to sync manifest voices: {}",
            e
        );
    }

    Ok(())
}
