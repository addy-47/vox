use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use turso::Connection;

/// A user-created cloned voice entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoiceEntry {
    /// UUID — stable identifier used in settings and filesystem paths.
    pub id: String,
    /// User-visible display name.
    pub name: String,
    /// `"reference_audio"` or `"pre_baked"`.
    pub source_kind: String,
    /// Absolute path to `~/.vox/voices/{id}/source.wav`.
    pub wav_path: Option<String>,
    /// Absolute path to `~/.vox/voices/{id}/baked/`.
    pub voice_dir: Option<String>,
    /// Unix epoch seconds at creation.
    pub created_at: i64,
    /// Absolute path to a short synthesized preview WAV.
    pub preview_wav: Option<String>,
}

/// Returns all voice entries ordered by creation date (newest first).
pub async fn list_voices(conn: &Connection) -> Result<Vec<VoiceEntry>> {
    let mut rows = conn
        .query(
            "SELECT id, name, source_kind, wav_path, voice_dir, created_at, preview_wav
             FROM voices
             ORDER BY created_at DESC",
            (),
        )
        .await?;

    let mut entries = Vec::new();
    while let Some(row) = rows.next().await? {
        entries.push(VoiceEntry {
            id: row.get(0)?,
            name: row.get(1)?,
            source_kind: row.get(2)?,
            wav_path: row.get(3)?,
            voice_dir: row.get(4)?,
            created_at: row.get(5)?,
            preview_wav: row.get(6)?,
        });
    }

    Ok(entries)
}

/// Returns a single voice entry by ID, or `None` if not found.
pub async fn get_voice(conn: &Connection, id: &str) -> Result<Option<VoiceEntry>> {
    let mut rows = conn
        .query(
            "SELECT id, name, source_kind, wav_path, voice_dir, created_at, preview_wav
             FROM voices
             WHERE id = ?",
            (id.to_string(),),
        )
        .await?;

    if let Some(row) = rows.next().await? {
        Ok(Some(VoiceEntry {
            id: row.get(0)?,
            name: row.get(1)?,
            source_kind: row.get(2)?,
            wav_path: row.get(3)?,
            voice_dir: row.get(4)?,
            created_at: row.get(5)?,
            preview_wav: row.get(6)?,
        }))
    } else {
        Ok(None)
    }
}

/// Inserts a new voice entry.
pub async fn insert_voice(conn: &Connection, entry: &VoiceEntry) -> Result<()> {
    conn.execute(
        "INSERT INTO voices (id, name, source_kind, wav_path, voice_dir, created_at, preview_wav)
         VALUES (?, ?, ?, ?, ?, ?, ?)",
        (
            entry.id.clone(),
            entry.name.clone(),
            entry.source_kind.clone(),
            entry.wav_path.clone(),
            entry.voice_dir.clone(),
            entry.created_at,
            entry.preview_wav.clone(),
        ),
    )
    .await?;
    Ok(())
}

/// Deletes a voice entry by ID from the database.
pub async fn delete_voice(conn: &Connection, id: &str) -> Result<()> {
    let affected = conn
        .execute("DELETE FROM voices WHERE id = ?", (id.to_string(),))
        .await?;
    if affected == 0 {
        return Err(anyhow!("Voice not found: {}", id));
    }
    Ok(())
}

/// Updates the display name of a voice entry.
pub async fn rename_voice(conn: &Connection, id: &str, name: &str) -> Result<()> {
    let name = name.trim();
    if name.is_empty() {
        return Err(anyhow!("Voice name cannot be empty"));
    }
    let affected = conn
        .execute(
            "UPDATE voices SET name = ? WHERE id = ?",
            (name.to_string(), id.to_string()),
        )
        .await?;
    if affected == 0 {
        return Err(anyhow!("Voice not found: {}", id));
    }
    Ok(())
}

