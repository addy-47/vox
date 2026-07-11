use anyhow::Result;
use turso::Connection;

/// Runs the CREATE TABLE IF NOT EXISTS migrations against the given connection.
///
/// Idempotent — safe to call on every startup to ensure schema is current.
/// Using INTEGER PRIMARY KEY for sessions.id (epoch ms) gives natural ordering.
pub async fn run_migrations(conn: &Connection) -> Result<()> {
    let statements = [
        "CREATE TABLE IF NOT EXISTS sessions (
            id               INTEGER PRIMARY KEY,   -- epoch milliseconds
            started_at       INTEGER NOT NULL,
            ended_at         INTEGER,
            turn_count       INTEGER NOT NULL DEFAULT 0,
            embedding_status TEXT    NOT NULL DEFAULT 'pending'
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
            id          TEXT    PRIMARY KEY,    -- UUID v4
            name        TEXT    NOT NULL,
            source_kind TEXT    NOT NULL,       -- 'reference_audio' | 'pre_baked'
            wav_path    TEXT,                   -- ~/.vox/voices/{uuid}/source.wav
            voice_dir   TEXT,                   -- Phase B: ~/.vox/voices/{uuid}/baked/
            created_at  INTEGER NOT NULL,       -- Unix epoch seconds
            preview_wav TEXT                    -- ~/.vox/voices/{uuid}/preview.wav
        );",
        "CREATE INDEX IF NOT EXISTS idx_voices_created ON voices(created_at DESC);",
        "CREATE TABLE IF NOT EXISTS memory_entries (
            id         TEXT PRIMARY KEY,            -- UUID v4
            content    TEXT NOT NULL,               -- The text snippet
            embedding  F32_BLOB(384) NOT NULL,      -- MiniLM embedding (384 floats)
            created_at INTEGER NOT NULL             -- Unix epoch seconds
        );",
        "CREATE TABLE IF NOT EXISTS episodes (
            id          INTEGER PRIMARY KEY AUTOINCREMENT,
            session_id  INTEGER NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
            summary     TEXT    NOT NULL,
            embedding   F32_BLOB(1024) NOT NULL,
            created_at  INTEGER NOT NULL,
            token_count INTEGER NOT NULL
        );",
        "CREATE INDEX IF NOT EXISTS idx_episodes_session ON episodes(session_id);",
        "CREATE INDEX IF NOT EXISTS idx_episodes_created ON episodes(created_at DESC);",
        "CREATE TABLE IF NOT EXISTS personal_memory (
            key        TEXT PRIMARY KEY,
            category   TEXT NOT NULL,
            value      TEXT NOT NULL,
            updated_at INTEGER NOT NULL
        );",
        "CREATE TABLE IF NOT EXISTS personal_memory_history (
            id          INTEGER PRIMARY KEY AUTOINCREMENT,
            key         TEXT NOT NULL,
            category    TEXT NOT NULL,
            value       TEXT NOT NULL,
            recorded_at INTEGER NOT NULL
        );",
        "CREATE INDEX IF NOT EXISTS idx_pm_history_key ON personal_memory_history(key, recorded_at DESC);",
    ];

    for stmt in statements {
        conn.execute(stmt, ()).await?;
    }

    // Safely add embedding_status column to existing sessions table if missing (idempotent migration)
    let _ = conn
        .execute(
            "ALTER TABLE sessions ADD COLUMN embedding_status TEXT NOT NULL DEFAULT 'pending'",
            (),
        )
        .await;

    if let Err(e) = seed_packaged_voices(conn).await {
        log::warn!("[Persistence] Failed to seed packaged voices (non-fatal): {}", e);
    }

    Ok(())
}

async fn seed_packaged_voices(conn: &Connection) -> Result<()> {
    let home = match dirs::home_dir() {
        Some(h) => h,
        None => return Ok(()),
    };
    let packaged_voices_dir = home.join(".vox").join("models").join("tts").join("chatterbox").join("voices");
    if !packaged_voices_dir.exists() {
        return Ok(());
    }

    let entries = std::fs::read_dir(&packaged_voices_dir)
        .map_err(|e| anyhow::anyhow!("Failed to read packaged voices: {}", e))?;

    for entry in entries {
        let entry = entry.map_err(|e| anyhow::anyhow!("Entry error: {}", e))?;
        let path = entry.path();
        if path.is_dir() {
            if let Some(name_str) = path.file_name().and_then(|n| n.to_str()) {
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
                    log::info!("[Persistence] Seeded packaged voice '{}' (id={})", name, id);
                }
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_schema_migrations_and_episodes_table() -> Result<()> {
        let db = turso::Builder::new_local(":memory:").build().await?;
        let conn = db.connect()?;
        run_migrations(&conn).await?;
        // Test idempotency (run migrations twice)
        run_migrations(&conn).await?;

        // Verify episodes table exists
        let mut rows = conn
            .query(
                "SELECT count(*) FROM sqlite_master WHERE type='table' AND name='episodes'",
                (),
            )
            .await?;
        let count: i64 = rows.next().await?.unwrap().get(0)?;
        assert_eq!(count, 1);

        // Verify sessions.embedding_status column exists
        let mut col_rows = conn.query("PRAGMA table_info(sessions)", ()).await?;
        let mut found_embedding_status = false;
        while let Some(row) = col_rows.next().await? {
            let col_name: String = row.get(1)?;
            if col_name == "embedding_status" {
                found_embedding_status = true;
                break;
            }
        }
        assert!(
            found_embedding_status,
            "embedding_status column must exist in sessions table"
        );
        
        // Verify personal_memory table exists
        let mut rows = conn
            .query(
                "SELECT count(*) FROM sqlite_master WHERE type='table' AND name='personal_memory'",
                (),
            )
            .await?;
        let pm_count: i64 = rows.next().await?.unwrap().get(0)?;
        assert_eq!(pm_count, 1);

        // Verify personal_memory_history table exists
        let mut rows = conn
            .query(
                "SELECT count(*) FROM sqlite_master WHERE type='table' AND name='personal_memory_history'",
                (),
            )
            .await?;
        let pmh_count: i64 = rows.next().await?.unwrap().get(0)?;
        assert_eq!(pmh_count, 1);

        Ok(())
    }
}
