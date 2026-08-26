use crate::core::error::PersistenceError;
use turso::Connection;

pub type Result<T> = std::result::Result<T, PersistenceError>;

/// Runs the CREATE TABLE IF NOT EXISTS migrations against the given connection.
pub async fn run_migrations(conn: &Connection) -> Result<()> {
    let statements = [
        "CREATE TABLE IF NOT EXISTS sessions (
            id               INTEGER PRIMARY KEY,
            started_at       INTEGER NOT NULL,
            ended_at         INTEGER,
            turn_count       INTEGER NOT NULL DEFAULT 0
        );",
        "CREATE TABLE IF NOT EXISTS turns (
            id              INTEGER PRIMARY KEY AUTOINCREMENT,
            session_id      INTEGER NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
            turn_id         INTEGER NOT NULL,
            user_text       TEXT    NOT NULL DEFAULT '',
            assistant_text  TEXT    NOT NULL DEFAULT '',
            stt_latency_ms  INTEGER,
            ttft_ms         INTEGER,
            created_at      INTEGER NOT NULL
        );",
        "CREATE INDEX IF NOT EXISTS idx_turns_session ON turns(session_id);",
        "CREATE TABLE IF NOT EXISTS voices (
            id          TEXT    PRIMARY KEY,
            name        TEXT    NOT NULL,
            source_kind TEXT    NOT NULL,
            wav_path    TEXT,
            voice_dir   TEXT,
            created_at  INTEGER NOT NULL,
            preview_wav TEXT
        );",
        "CREATE INDEX IF NOT EXISTS idx_voices_created ON voices(created_at DESC);",
        "CREATE TABLE IF NOT EXISTS memory_facts (
            id           TEXT PRIMARY KEY,
            type         TEXT NOT NULL,
            collection   TEXT NOT NULL,
            fact         TEXT NOT NULL,
            source       TEXT NOT NULL DEFAULT 'LLM',
            status       TEXT NOT NULL DEFAULT 'active',
            session_id   TEXT NOT NULL DEFAULT '',
            turn_id      TEXT NOT NULL DEFAULT '',
            created_at   INTEGER NOT NULL
        );",
        "CREATE TABLE IF NOT EXISTS memory_facts_vectors (
            id          INTEGER PRIMARY KEY AUTOINCREMENT,
            fact_id     TEXT NOT NULL REFERENCES memory_facts(id) ON DELETE CASCADE,
            collection  TEXT NOT NULL,
            embedding   F32_BLOB(384) NOT NULL
        );",
        "CREATE TABLE IF NOT EXISTS memory_relations (
            id          INTEGER PRIMARY KEY AUTOINCREMENT,
            from_id     TEXT NOT NULL REFERENCES memory_facts(id) ON DELETE CASCADE,
            to_id       TEXT NOT NULL REFERENCES memory_facts(id) ON DELETE CASCADE,
            relation    TEXT NOT NULL,
            source      TEXT NOT NULL DEFAULT 'NLI',
            created_at  INTEGER NOT NULL,
            UNIQUE(from_id, to_id, relation)
        );",
        "CREATE TABLE IF NOT EXISTS personal_memory_queue (
            id               INTEGER PRIMARY KEY AUTOINCREMENT,
            fact             TEXT NOT NULL,
            collection       TEXT NOT NULL,
            source           TEXT NOT NULL DEFAULT 'LLM',
            session_id       TEXT NOT NULL DEFAULT '',
            status           TEXT NOT NULL DEFAULT 'staged_pending',
            attempts         INTEGER NOT NULL DEFAULT 0,
            retry_count      INTEGER NOT NULL DEFAULT 0,
            error_msg        TEXT,
            created_at       INTEGER NOT NULL,
            processed_at     INTEGER,
            claimed_at       INTEGER,
            vector           F32_BLOB(384),
            relations_json   TEXT,
            dedup_match_json TEXT,
            audit_json       TEXT
        );",
        "CREATE TABLE IF NOT EXISTS memory_pipeline_metrics (
            id            INTEGER PRIMARY KEY AUTOINCREMENT,
            run_id        TEXT    NOT NULL,
            stage_name    TEXT    NOT NULL,
            session_id    TEXT    NOT NULL DEFAULT '',
            batch_seq     INTEGER NOT NULL DEFAULT 0,
            items_claimed INTEGER NOT NULL DEFAULT 0,
            error_count   INTEGER NOT NULL DEFAULT 0,
            duration_ms   INTEGER NOT NULL,
            created_at    INTEGER NOT NULL
        );",
        "CREATE INDEX IF NOT EXISTS idx_mf_type_status ON memory_facts(type, status);",
        "CREATE INDEX IF NOT EXISTS idx_mf_collection_status ON memory_facts(collection, status);",
        "CREATE INDEX IF NOT EXISTS idx_mf_created ON memory_facts(created_at DESC);",
        "CREATE INDEX IF NOT EXISTS idx_mfv_collection ON memory_facts_vectors(collection);",
        "CREATE UNIQUE INDEX IF NOT EXISTS idx_mfv_fact_id ON memory_facts_vectors(fact_id);",
        "CREATE INDEX IF NOT EXISTS idx_mr_from ON memory_relations(from_id, relation);",
        "CREATE INDEX IF NOT EXISTS idx_mr_to ON memory_relations(to_id, relation);",
        "CREATE INDEX IF NOT EXISTS idx_pmq_status ON personal_memory_queue(status, created_at ASC);",
        "CREATE INDEX IF NOT EXISTS idx_pmq_session ON personal_memory_queue(session_id);",
        "CREATE INDEX IF NOT EXISTS idx_mpm_run_stage ON memory_pipeline_metrics(run_id, stage_name);",
        "CREATE INDEX IF NOT EXISTS idx_mpm_batch_seq ON memory_pipeline_metrics(run_id, stage_name, batch_seq);",
    ];

    for stmt in statements {
        conn.execute(stmt, ()).await?;
    }

    if let Err(e) = seed_packaged_voices(conn).await {
        log::warn!(
            "[Persistence::Schema] Failed to seed packaged voices (non-fatal): {}",
            e
        );
    }

    Ok(())
}

async fn seed_packaged_voices(conn: &Connection) -> Result<()> {
    let home = match dirs::home_dir() {
        Some(h) => h,
        None => return Ok(()),
    };
    let packaged_voices_dir = home
        .join(".vox")
        .join("models")
        .join("tts")
        .join("chatterbox")
        .join("voices");
    if !packaged_voices_dir.exists() {
        return Ok(());
    }

    let entries = std::fs::read_dir(&packaged_voices_dir)?;

    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            if let Some(name_str) = path.file_name().and_then(|n| n.to_str()) {
                seed_single_voice(conn, name_str, &path).await?;
            }
        }
    }

    Ok(())
}

async fn seed_single_voice(conn: &Connection, name_str: &str, path: &std::path::Path) -> Result<()> {
    let id = format!("chatterbox_voice_{}", name_str);
    let mut rows = conn
        .query("SELECT 1 FROM voices WHERE id = ?", (id.clone(),))
        .await?;

    let exists = rows.next().await?.is_some();
    if !exists {
        let name = match name_str {
            "pain" => "Pain (Naruto)".to_string(),
            "madara" => "Madara Uchiha".to_string(),
            "shreya" => "Shreya Ghoshal".to_string(),
            "hayami" => "Hayami Saori".to_string(),
            "ellen" => "Ellen (Serious)".to_string(),
            "juniper" => "Juniper (Professional)".to_string(),
            "mark" => "Mark (Conversational)".to_string(),
            "spuds" => "Spuds Oxley (Wise)".to_string(),
            other => other.to_string(),
        };

        let wav_path = path.join("source.wav").to_string_lossy().into_owned();
        let voice_dir = path.join("baked").to_string_lossy().into_owned();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        conn.execute(
            "INSERT INTO voices (id, name, source_kind, wav_path, voice_dir, created_at)
             VALUES (?, ?, ?, ?, ?, ?)",
            (
                id.clone(),
                name.clone(),
                "pre_baked".to_string(),
                Some(wav_path),
                Some(voice_dir),
                now,
            ),
        ).await?;
        log::info!("[Persistence::Schema] Seeded packaged voice '{}' (id={})", name, id);
    }
    Ok(())
}

