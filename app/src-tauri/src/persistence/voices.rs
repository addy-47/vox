use std::{
    fs::read_dir,
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

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

pub async fn seed_packaged_voices(conn: &Connection) -> Result<()> {
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

    let entries = read_dir(&packaged_voices_dir)?;

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

async fn seed_single_voice(conn: &Connection, name_str: &str, path: &Path) -> Result<()> {
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
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
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
        )
        .await?;
        log::info!(
            "[Persistence::Schema] Seeded packaged voice '{}' (id={})",
            name,
            id
        );
    }
    Ok(())
}
