use anyhow::{anyhow, Result};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

// ─── Domain types ─────────────────────────────────────────────────────────────

/// A user-created cloned voice entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoiceEntry {
    /// UUID v4 — stable identifier used in settings and filesystem paths.
    pub id: String,
    /// User-visible display name.
    pub name: String,
    /// `"reference_audio"` — engine runs VoiceEncoder at init time.
    /// `"pre_baked"` — engine loads pre-computed .npy tensors (Phase B).
    pub source_kind: String,
    /// Absolute path to `~/.vox/voices/{id}/source.wav`.
    /// Always preserved even if pre_baked tensors exist.
    pub wav_path: Option<String>,
    /// Absolute path to `~/.vox/voices/{id}/baked/` (Phase B only).
    pub voice_dir: Option<String>,
    /// Unix epoch seconds at creation.
    pub created_at: i64,
    /// Absolute path to a short synthesized preview WAV.
    /// `None` until `preview_voice` IPC command is called.
    pub preview_wav: Option<String>,
}

// ─── CRUD ────────────────────────────────────────────────────────────────────

/// Returns all voice entries ordered by creation date (newest first).
pub fn list_voices(conn: &Connection) -> Result<Vec<VoiceEntry>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, source_kind, wav_path, voice_dir, created_at, preview_wav
         FROM voices
         ORDER BY created_at DESC",
    )?;

    let entries = stmt
        .query_map([], |row| {
            Ok(VoiceEntry {
                id: row.get(0)?,
                name: row.get(1)?,
                source_kind: row.get(2)?,
                wav_path: row.get(3)?,
                voice_dir: row.get(4)?,
                created_at: row.get(5)?,
                preview_wav: row.get(6)?,
            })
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;

    Ok(entries)
}

/// Returns a single voice entry by ID, or `None` if not found.
pub fn get_voice(conn: &Connection, id: &str) -> Result<Option<VoiceEntry>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, source_kind, wav_path, voice_dir, created_at, preview_wav
         FROM voices
         WHERE id = ?1",
    )?;

    let mut rows = stmt.query_map(params![id], |row| {
        Ok(VoiceEntry {
            id: row.get(0)?,
            name: row.get(1)?,
            source_kind: row.get(2)?,
            wav_path: row.get(3)?,
            voice_dir: row.get(4)?,
            created_at: row.get(5)?,
            preview_wav: row.get(6)?,
        })
    })?;

    match rows.next() {
        Some(Ok(entry)) => Ok(Some(entry)),
        Some(Err(e)) => Err(anyhow!("DB row error: {}", e)),
        None => Ok(None),
    }
}

/// Inserts a new voice entry. Caller is responsible for creating the voice
/// directory on disk before calling this.
pub fn insert_voice(conn: &Connection, entry: &VoiceEntry) -> Result<()> {
    conn.execute(
        "INSERT INTO voices (id, name, source_kind, wav_path, voice_dir, created_at, preview_wav)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            entry.id,
            entry.name,
            entry.source_kind,
            entry.wav_path,
            entry.voice_dir,
            entry.created_at,
            entry.preview_wav,
        ],
    )?;
    Ok(())
}

/// Deletes a voice entry by ID from the database.
/// Caller is responsible for removing the voice directory from disk.
pub fn delete_voice(conn: &Connection, id: &str) -> Result<()> {
    let affected = conn.execute("DELETE FROM voices WHERE id = ?1", params![id])?;
    if affected == 0 {
        return Err(anyhow!("Voice not found: {}", id));
    }
    Ok(())
}

/// Updates the display name of a voice entry.
pub fn rename_voice(conn: &Connection, id: &str, name: &str) -> Result<()> {
    let name = name.trim();
    if name.is_empty() {
        return Err(anyhow!("Voice name cannot be empty"));
    }
    let affected = conn.execute(
        "UPDATE voices SET name = ?1 WHERE id = ?2",
        params![name, id],
    )?;
    if affected == 0 {
        return Err(anyhow!("Voice not found: {}", id));
    }
    Ok(())
}

/// Records the path to a synthesized preview WAV after `preview_voice` IPC runs.
pub fn update_preview_wav(conn: &Connection, id: &str, path: &str) -> Result<()> {
    conn.execute(
        "UPDATE voices SET preview_wav = ?1 WHERE id = ?2",
        params![path, id],
    )?;
    Ok(())
}
