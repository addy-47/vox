use std::{
    fs::read_dir,
    path::Path,
    str::FromStr,
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use turso::Connection;

/// Declared voice origin. Serializes to the exact `source_kind` strings stored
/// in the `voices` table, so the mapping is compiler-checked instead of an
/// inline literal at each write site.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum VoiceSourceKind {
    PreBaked,
    ZipvoicePack,
    Edge,
}

impl VoiceSourceKind {
    /// Returns the stored `source_kind` string for queries and inserts.
    pub fn as_str(&self) -> &'static str {
        match self {
            VoiceSourceKind::PreBaked => "pre_baked",
            VoiceSourceKind::ZipvoicePack => "zipvoice_pack",
            VoiceSourceKind::Edge => "edge",
        }
    }
}

impl FromStr for VoiceSourceKind {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        match s {
            "pre_baked" => Ok(VoiceSourceKind::PreBaked),
            "zipvoice_pack" => Ok(VoiceSourceKind::ZipvoicePack),
            "edge" => Ok(VoiceSourceKind::Edge),
            other => Err(anyhow!("Unknown voice source_kind: {}", other)),
        }
    }
}

/// Server-side voice list scope. The frontend never filters by model name;
/// the backend resolves which rows belong to the requesting provider.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VoiceListScope {
    All,
    Custom,
    ZipvoicePack,
}

/// A user-created cloned voice entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoiceEntry {
    /// UUID — stable identifier used in settings and filesystem paths.
    pub id: String,
    /// User-visible display name.
    pub name: String,
    /// Declared voice origin.
    pub source_kind: VoiceSourceKind,
    /// Pack subdirectory slug (`atlas`, `pain`) for packaged voices; `None`
    /// for user-cloned voices. Groundwork for slug-addressed voice ids.
    pub slug: Option<String>,
    /// Absolute path to `~/.vox/voices/{id}/source.wav`.
    pub wav_path: Option<String>,
    /// Absolute path to `~/.vox/voices/{id}/baked/`.
    pub voice_dir: Option<String>,
    /// Unix epoch seconds at creation.
    pub created_at: i64,
    /// Absolute path to a short synthesized preview WAV.
    pub preview_wav: Option<String>,
}

/// Returns voice entries in the given scope ordered by creation date (newest first).
pub async fn list_voices(conn: &Connection, scope: VoiceListScope) -> Result<Vec<VoiceEntry>> {
    let predicate = match scope {
        VoiceListScope::All => "",
        VoiceListScope::Custom => "WHERE source_kind != 'zipvoice_pack'",
        VoiceListScope::ZipvoicePack => "WHERE source_kind = 'zipvoice_pack'",
    };
    let mut rows = conn
        .query(
            &format!(
                "SELECT id, name, source_kind, slug, wav_path, voice_dir, created_at, preview_wav
                 FROM voices
                 {predicate}
                 ORDER BY created_at DESC"
            ),
            (),
        )
        .await?;

    let mut entries = Vec::new();
    while let Some(row) = rows.next().await? {
        entries.push(decode_row(&row)?);
    }

    Ok(entries)
}

/// Decodes one `voices` row, rejecting unrecognised `source_kind` values loudly.
fn decode_row(row: &turso::Row) -> Result<VoiceEntry> {
    let kind: String = row.get(2)?;
    Ok(VoiceEntry {
        id: row.get(0)?,
        name: row.get(1)?,
        source_kind: kind.parse()?,
        slug: row.get(3)?,
        wav_path: row.get(4)?,
        voice_dir: row.get(5)?,
        created_at: row.get(6)?,
        preview_wav: row.get(7)?,
    })
}

/// Returns a single voice entry by ID, or `None` if not found.
pub async fn get_voice(conn: &Connection, id: &str) -> Result<Option<VoiceEntry>> {
    let mut rows = conn
        .query(
            "SELECT id, name, source_kind, slug, wav_path, voice_dir, created_at, preview_wav
             FROM voices
             WHERE id = ?",
            (id.to_string(),),
        )
        .await?;

    if let Some(row) = rows.next().await? {
        Ok(Some(decode_row(&row)?))
    } else {
        Ok(None)
    }
}

/// Inserts a new voice entry.
pub async fn insert_voice(conn: &Connection, entry: &VoiceEntry) -> Result<()> {
    conn.execute(
        "INSERT INTO voices (id, name, source_kind, slug, wav_path, voice_dir, created_at, preview_wav)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        (
            entry.id.clone(),
            entry.name.clone(),
            entry.source_kind.as_str().to_string(),
            entry.slug.clone(),
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
    let models_dir = match crate::utils::paths::try_get() {
        Some(p) => p.models,
        None => return Ok(()),
    };
    let packaged_voices_dir = models_dir.join("tts").join("chatterbox").join("voices");
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
            "INSERT INTO voices (id, name, source_kind, slug, wav_path, voice_dir, created_at)
             VALUES (?, ?, ?, ?, ?, ?, ?)",
            (
                id.clone(),
                name.clone(),
                VoiceSourceKind::PreBaked.as_str().to_string(),
                Some(name_str.to_string()),
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

pub async fn seed_zipvoice_voices(conn: &Connection) -> Result<()> {
    let models_dir = match crate::utils::paths::try_get() {
        Some(p) => p.models,
        None => return Ok(()),
    };
    let packaged_voices_dir = models_dir.join("tts").join("zipvoice").join("voices");
    if !packaged_voices_dir.exists() {
        return Ok(());
    }

    let entries = read_dir(&packaged_voices_dir)?;

    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            if let Some(slug) = path.file_name().and_then(|n| n.to_str()) {
                if path.join("clip.wav").exists() && path.join("reference.txt").exists() {
                    seed_single_zipvoice(conn, slug, &path).await?;
                }
            }
        }
    }

    Ok(())
}

async fn seed_single_zipvoice(conn: &Connection, slug: &str, path: &Path) -> Result<()> {
    let id = slug.to_string();
    let mut rows = conn
        .query("SELECT 1 FROM voices WHERE id = ?", (id.clone(),))
        .await?;

    let exists = rows.next().await?.is_some();
    if !exists {
        let name = display_name_for_slug(slug);

        let wav_path = path.join("clip.wav").to_string_lossy().into_owned();
        let voice_dir = path.to_string_lossy().into_owned();
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        conn.execute(
            "INSERT INTO voices (id, name, source_kind, slug, wav_path, voice_dir, created_at)
             VALUES (?, ?, ?, ?, ?, ?, ?)",
            (
                id.clone(),
                name.clone(),
                VoiceSourceKind::ZipvoicePack.as_str().to_string(),
                Some(slug.to_string()),
                Some(wav_path),
                Some(voice_dir),
                now,
            ),
        )
        .await?;
        log::info!(
            "[Persistence::Schema] Seeded ZipVoice packaged voice '{}' (id={})",
            name,
            id
        );
    }
    Ok(())
}

/// Derives the seeded display name from a pack directory slug. The seeded row
/// is the single source of display names; the frontend holds no fallback list.
fn display_name_for_slug(slug: &str) -> String {
    let mut chars = slug.chars();
    match chars.next() {
        None => String::new(),
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
    }
}
