use std::{
    fs::read_dir,
    path::Path,
    str::FromStr,
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use turso::{params, Connection};

/// Declared voice origin. Serializes to the exact `source_kind` strings stored
/// in the `voices` table, so the mapping is compiler-checked instead of an
/// inline literal at each write site.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum VoiceSourceKind {
    Bundled,
    Custom,
    Remote,
    PreBaked,
    ZipvoicePack,
    Edge,
}

impl VoiceSourceKind {
    /// Returns the stored `source_kind` string for queries and inserts.
    pub fn as_str(&self) -> &'static str {
        match self {
            VoiceSourceKind::Bundled => "bundled",
            VoiceSourceKind::Custom => "custom",
            VoiceSourceKind::Remote => "remote",
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
            "bundled" => Ok(VoiceSourceKind::Bundled),
            "custom" => Ok(VoiceSourceKind::Custom),
            "remote" => Ok(VoiceSourceKind::Remote),
            "pre_baked" => Ok(VoiceSourceKind::PreBaked),
            "zipvoice_pack" => Ok(VoiceSourceKind::ZipvoicePack),
            "edge" => Ok(VoiceSourceKind::Edge),
            other => Err(anyhow!("Unknown voice source_kind: {}", other)),
        }
    }
}

/// Server-side voice list scope.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VoiceListScope {
    All,
    Custom,
    ZipvoicePack,
}

/// A unified voice entry stored in the Turso `voices` table.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoiceEntry {
    /// Unique identifier: e.g. "bella", "en-US-AriaNeural", or UUID.
    pub id: String,
    /// Owning engine ID: e.g. "kokoro", "supertonic", "zipvoice", "chatterbox", "edge_tts".
    pub provider_id: String,
    /// User-visible display name.
    pub name: String,
    /// Declared voice origin.
    pub source_kind: VoiceSourceKind,
    /// Pack subdirectory slug for packaged voices; None for user clones.
    pub slug: Option<String>,
    /// Normalized gender facet: "female", "male", or "neutral".
    pub gender: String,
    /// BCP-47 locale (e.g. "en-US", "en-GB").
    pub locale: String,
    /// Display accent label (e.g. "American", "British").
    pub accent: Option<String>,
    /// Display description blurb for catalog modal.
    pub description: Option<String>,
    /// JSON-encoded tag array.
    pub tags: String,
    /// Default preview test sentence.
    pub sample_text: String,
    /// Absolute path to source WAV audio.
    pub wav_path: Option<String>,
    /// Absolute path to pre-baked tensor directory.
    pub voice_dir: Option<String>,
    /// Absolute path to a short synthesized preview WAV.
    pub preview_wav: Option<String>,
    /// Starred favorite flag.
    pub is_favorite: bool,
    /// Epoch millisecond of last selection/preview.
    pub last_used_at: Option<i64>,
    /// Unix epoch seconds at creation.
    pub created_at: i64,
}

const SELECT_COLUMNS: &str =
    "id, provider_id, name, source_kind, slug, gender, locale, accent, description, tags, \
     sample_text, wav_path, voice_dir, preview_wav, is_favorite, last_used_at, created_at";

/// Decodes one `voices` row into a `VoiceEntry`.
fn decode_row(row: &turso::Row) -> Result<VoiceEntry> {
    let kind_str: String = row.get(3)?;
    Ok(VoiceEntry {
        id: row.get(0)?,
        provider_id: row.get(1)?,
        name: row.get(2)?,
        source_kind: kind_str.parse()?,
        slug: row.get(4)?,
        gender: row.get::<String>(5).unwrap_or_else(|_| "neutral".to_string()),
        locale: row.get::<String>(6).unwrap_or_else(|_| "en-US".to_string()),
        accent: row.get(7).ok().flatten(),
        description: row.get(8).ok().flatten(),
        tags: row.get::<String>(9).unwrap_or_else(|_| "[]".to_string()),
        sample_text: row
            .get::<String>(10)
            .unwrap_or_else(|_| "Hello, this is a preview of my voice.".to_string()),
        wav_path: row.get(11)?,
        voice_dir: row.get(12)?,
        preview_wav: row.get(13)?,
        is_favorite: row.get::<i64>(14).unwrap_or(0) == 1,
        last_used_at: row.get(15)?,
        created_at: row.get(16)?,
    })
}

/// Returns voice entries matching provider and scope, ordered by favorites, recents, and name.
pub async fn list_voices(
    conn: &Connection,
    provider: Option<&str>,
    scope: VoiceListScope,
) -> Result<Vec<VoiceEntry>> {
    let mut conditions = Vec::new();

    match scope {
        VoiceListScope::All => {}
        VoiceListScope::Custom => conditions.push("source_kind = 'custom'"),
        VoiceListScope::ZipvoicePack => conditions.push("source_kind = 'zipvoice_pack'"),
    }

    let provider_clause = if let Some(p) = provider {
        if !p.is_empty() {
            conditions.push("LOWER(provider_id) = LOWER(?)");
            Some(p.to_string())
        } else {
            None
        }
    } else {
        None
    };

    let where_clause = if conditions.is_empty() {
        String::new()
    } else {
        format!("WHERE {}", conditions.join(" AND "))
    };

    let sql = format!(
        "SELECT {SELECT_COLUMNS} FROM voices
         {where_clause}
         ORDER BY is_favorite DESC, COALESCE(last_used_at, 0) DESC, created_at DESC"
    );

    let mut rows = if let Some(p) = provider_clause {
        conn.query(&sql, (p,)).await?
    } else {
        conn.query(&sql, ()).await?
    };

    let mut entries = Vec::new();
    while let Some(row) = rows.next().await? {
        entries.push(decode_row(&row)?);
    }
    Ok(entries)
}

/// Returns a single voice entry by ID, or `None` if not found.
pub async fn get_voice(conn: &Connection, id: &str) -> Result<Option<VoiceEntry>> {
    let mut rows = conn
        .query(
            &format!("SELECT {SELECT_COLUMNS} FROM voices WHERE id = ?"),
            (id.to_string(),),
        )
        .await?;

    if let Some(row) = rows.next().await? {
        Ok(Some(decode_row(&row)?))
    } else {
        Ok(None)
    }
}

/// Inserts a new cloned or custom voice entry.
pub async fn insert_voice(conn: &Connection, entry: &VoiceEntry) -> Result<()> {
    conn.execute(
        "INSERT INTO voices (
            id, provider_id, name, source_kind, slug, gender, locale, accent,
            description, tags, sample_text, wav_path, voice_dir, preview_wav,
            is_favorite, last_used_at, created_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        params![
            entry.id.clone(),
            entry.provider_id.clone(),
            entry.name.clone(),
            entry.source_kind.as_str().to_string(),
            entry.slug.clone(),
            entry.gender.clone(),
            entry.locale.clone(),
            entry.accent.clone(),
            entry.description.clone(),
            entry.tags.clone(),
            entry.sample_text.clone(),
            entry.wav_path.clone(),
            entry.voice_dir.clone(),
            entry.preview_wav.clone(),
            if entry.is_favorite { 1 } else { 0 },
            entry.last_used_at,
            entry.created_at,
        ],
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

/// Atomically toggles the favorite state for a voice. Returns the new state.
pub async fn toggle_favorite(conn: &Connection, voice_id: &str) -> Result<bool> {
    let voice_id = voice_id.trim();
    let mut rows = conn
        .query(
            "SELECT id, is_favorite FROM voices WHERE id = ? OR LOWER(id) = LOWER(?) LIMIT 1",
            (voice_id.to_string(), voice_id.to_string()),
        )
        .await?;
    if let Some(row) = rows.next().await? {
        let matched_id: String = row.get(0).unwrap_or_else(|_| voice_id.to_string());
        let current: i64 = row
            .get::<i64>(1)
            .unwrap_or_else(|_| if row.get::<bool>(1).unwrap_or(false) { 1 } else { 0 });
        let new_state = if current == 1 { 0 } else { 1 };
        conn.execute(
            "UPDATE voices SET is_favorite = ? WHERE id = ?",
            (new_state, matched_id.clone()),
        )
        .await?;
        log::info!(
            "[Voices] toggle_favorite voice_id='{}' (matched='{}') current={} -> new_state={}",
            voice_id,
            matched_id,
            current,
            new_state
        );
        Ok(new_state == 1)
    } else {
        log::warn!(
            "[Voices] toggle_favorite voice_id='{}' not found in DB, inserting stub with is_favorite=1",
            voice_id
        );
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        conn.execute(
            "INSERT INTO voices (id, provider_id, name, source_kind, is_favorite, created_at)
             VALUES (?, 'kokoro', ?, 'bundled', 1, ?)
             ON CONFLICT(id) DO UPDATE SET is_favorite = 1",
            (voice_id.to_string(), voice_id.to_string(), now),
        )
        .await?;
        Ok(true)
    }
}

/// Records that a voice was selected or used.
pub async fn record_recent(conn: &Connection, voice_id: &str) -> Result<()> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;
    conn.execute(
        "UPDATE voices SET last_used_at = ? WHERE id = ?",
        (now, voice_id.to_string()),
    )
    .await?;
    Ok(())
}

/// Executes a bulk upsert of voices in a single Turso transaction.
pub async fn batch_upsert_voices(conn: &Connection, entries: &[VoiceEntry]) -> Result<()> {
    if entries.is_empty() {
        return Ok(());
    }
    conn.execute("BEGIN TRANSACTION;", ()).await?;
    for e in entries {
        conn.execute(
            "INSERT INTO voices (
                id, provider_id, name, source_kind, slug, gender, locale, accent,
                description, tags, sample_text, wav_path, voice_dir, preview_wav,
                is_favorite, last_used_at, created_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(id) DO UPDATE SET
                provider_id = excluded.provider_id,
                name = excluded.name,
                source_kind = excluded.source_kind,
                slug = excluded.slug,
                gender = excluded.gender,
                locale = excluded.locale,
                accent = excluded.accent,
                description = excluded.description,
                tags = excluded.tags,
                sample_text = excluded.sample_text,
                wav_path = excluded.wav_path,
                voice_dir = excluded.voice_dir,
                preview_wav = excluded.preview_wav",
            params![
                e.id.clone(),
                e.provider_id.clone(),
                e.name.clone(),
                e.source_kind.as_str().to_string(),
                e.slug.clone(),
                e.gender.clone(),
                e.locale.clone(),
                e.accent.clone(),
                e.description.clone(),
                e.tags.clone(),
                e.sample_text.clone(),
                e.wav_path.clone(),
                e.voice_dir.clone(),
                e.preview_wav.clone(),
                if e.is_favorite { 1 } else { 0 },
                e.last_used_at,
                e.created_at,
            ],
        )
        .await?;
    }
    conn.execute("COMMIT;", ()).await?;
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
            "INSERT INTO voices (id, provider_id, name, source_kind, slug, wav_path, voice_dir, created_at)
             VALUES (?, 'chatterbox', ?, ?, ?, ?, ?, ?)",
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
            "INSERT INTO voices (id, provider_id, name, source_kind, slug, wav_path, voice_dir, created_at)
             VALUES (?, 'zipvoice', ?, ?, ?, ?, ?, ?)",
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

pub fn display_name_for_slug(slug: &str) -> String {
    let mut chars = slug.chars();
    match chars.next() {
        None => String::new(),
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
    }
}
