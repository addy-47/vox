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

        CREATE TABLE IF NOT EXISTS voices (
            id          TEXT    PRIMARY KEY,    -- UUID v4
            name        TEXT    NOT NULL,
            source_kind TEXT    NOT NULL,       -- 'reference_audio' | 'pre_baked'
            wav_path    TEXT,                   -- ~/.vox/voices/{uuid}/source.wav
            voice_dir   TEXT,                   -- Phase B: ~/.vox/voices/{uuid}/baked/
            created_at  INTEGER NOT NULL,       -- Unix epoch seconds
            preview_wav TEXT                    -- ~/.vox/voices/{uuid}/preview.wav
        );

        CREATE INDEX IF NOT EXISTS idx_voices_created ON voices(created_at DESC);
        ",
    )?;

    if let Err(e) = seed_packaged_voices(conn) {
        log::warn!("[Persistence] Failed to seed packaged voices (non-fatal): {}", e);
    }

    Ok(())
}

fn seed_packaged_voices(conn: &Connection) -> Result<()> {
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
                
                let mut stmt = conn.prepare("SELECT 1 FROM voices WHERE id = ?1")?;
                let exists = stmt.exists([&id])?;
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
                         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                        rusqlite::params![
                            id,
                            name,
                            "pre_baked",
                            wav_path,
                            voice_dir,
                            now
                        ],
                    )?;
                    log::info!("[Persistence] Seeded packaged voice '{}' (id={})", name, id);
                }
            }
        }
    }

    Ok(())
}

