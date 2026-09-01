//! IPC commands for voice library management and cloning.
//!
//! DB operations run on `spawn_blocking` threads; voices are a standalone
//! persistence and audio management concern.

use crate::persistence::voices::{self, VoiceEntry};
use crate::services::tts::voice::{
    convert_and_validate_audio, fetch_remote_edge_voices, pre_bake_speaker_tensors,
    start_recording, stop_recording, write_pcm_to_wav, EdgeTtsVoiceEntry,
};
use serde::Serialize;
use std::time::{SystemTime, UNIX_EPOCH};

// ─── DTOs ────────────────────────────────────────────────────────────────────

/// Frontend-safe representation of a voice entry.
#[derive(Debug, Clone, Serialize)]
pub struct VoiceEntryDto {
    pub id: String,
    pub name: String,
    pub source_kind: String,
    pub has_preview: bool,
    pub created_at: i64,
}

impl From<VoiceEntry> for VoiceEntryDto {
    fn from(e: VoiceEntry) -> Self {
        Self {
            has_preview: e.preview_wav.is_some(),
            id: e.id,
            name: e.name,
            source_kind: e.source_kind,
            created_at: e.created_at,
        }
    }
}

pub type EdgeTtsVoiceDto = EdgeTtsVoiceEntry;

// ─── Helpers ─────────────────────────────────────────────────────────────────

async fn open_db() -> Result<turso::Connection, String> {
    let db_path = crate::utils::paths::db_path();
    crate::persistence::db::VoxDb::open(&db_path)
        .await
        .map_err(|e| format!("DB open failed: {}", e))
}

fn now_epoch() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

// ─── Commands ────────────────────────────────────────────────────────────────

/// Return saved voices from SQLite database or remote Edge TTS voices based on provider.
#[tauri::command]
pub async fn list_voices(provider: Option<String>) -> Result<Vec<VoiceEntryDto>, String> {
    if let Some(p) = provider.as_deref() {
        if p.to_lowercase() == "edge" || p.to_lowercase() == "edge_tts" {
            let edge_voices = fetch_remote_edge_voices().await?;
            let mapped = edge_voices
                .into_iter()
                .map(|e| VoiceEntryDto {
                    id: e.short_name.clone(),
                    name: e.friendly_name,
                    source_kind: "edge".to_string(),
                    has_preview: true,
                    created_at: 0,
                })
                .collect();
            return Ok(mapped);
        }
    }

    let conn = open_db().await?;
    voices::list_voices(&conn)
        .await
        .map(|entries| entries.into_iter().map(VoiceEntryDto::from).collect())
        .map_err(|e| format!("Failed to list voices: {}", e))
}

const MAX_VOICE_RECORDING_SAMPLES: usize = 1_600_000; // ~100s at 16kHz (~6.4 MB)

/// Add a new cloned voice from an existing audio file.
#[tauri::command]
pub async fn add_voice_from_file(name: String, file_path: String) -> Result<VoiceEntryDto, String> {
    let name = name.trim().to_string();
    if name.is_empty() {
        return Err("Voice name cannot be empty".to_string());
    }

    let path = std::path::Path::new(&file_path);
    if !path.is_file() {
        return Err("Selected path is not a valid regular file".to_string());
    }
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .unwrap_or_default();
    if !matches!(ext.as_str(), "wav" | "mp3" | "m4a" | "ogg" | "flac" | "aac") {
        return Err(format!(
            "Unsupported audio format '.{}'. Supported formats: wav, mp3, m4a, ogg, flac, aac",
            ext
        ));
    }

    let id = uuid::Uuid::new_v4().to_string();
    let voice_dir = crate::utils::paths::voice_dir(&id);
    std::fs::create_dir_all(&voice_dir)
        .map_err(|e| format!("Failed to create voice directory: {}", e))?;

    let dest = voice_dir.join("source.wav");
    let dest_clone = dest.clone();
    let file_path_clone = file_path.clone();
    tokio::task::spawn_blocking(move || convert_and_validate_audio(&file_path_clone, &dest_clone))
        .await
        .map_err(|e| format!("Task panicked: {}", e))??;

    let baked_dir = voice_dir.join("baked");
    let dest_clone2 = dest.clone();
    let baked_dir_clone = baked_dir.clone();
    tokio::task::spawn_blocking(move || pre_bake_speaker_tensors(&dest_clone2, &baked_dir_clone))
        .await
        .map_err(|e| format!("Task panicked: {}", e))??;

    let entry = VoiceEntry {
        id: id.clone(),
        name: name.clone(),
        source_kind: "pre_baked".to_string(),
        wav_path: Some(dest.to_string_lossy().into_owned()),
        voice_dir: Some(baked_dir.to_string_lossy().into_owned()),
        created_at: now_epoch(),
        preview_wav: None,
    };

    let conn = open_db().await?;
    voices::insert_voice(&conn, &entry)
        .await
        .map_err(|e| format!("Failed to save voice: {}", e))?;

    log::info!(
        "[Voices] Added voice '{}' (id={}) with pre-baked tensors",
        name,
        id
    );
    Ok(VoiceEntryDto::from(entry))
}

/// Add a new cloned voice from raw PCM audio captured in-app.
#[tauri::command]
pub async fn add_voice_from_recording(
    name: String,
    pcm_f32: Vec<f32>,
    sample_rate: u32,
) -> Result<VoiceEntryDto, String> {
    let name = name.trim().to_string();
    if name.is_empty() {
        return Err("Voice name cannot be empty".to_string());
    }
    if sample_rate == 0 {
        return Err("Invalid sample rate (0)".to_string());
    }
    if pcm_f32.len() > MAX_VOICE_RECORDING_SAMPLES {
        return Err(format!(
            "Voice recording payload exceeds maximum allowed size ({} samples).",
            MAX_VOICE_RECORDING_SAMPLES
        ));
    }

    let duration = pcm_f32.len() as f32 / sample_rate as f32;
    if duration < 1.0 {
        return Err(format!(
            "Recording too short ({:.1}s). Minimum is 1.0s for voice cloning.",
            duration
        ));
    }

    let id = uuid::Uuid::new_v4().to_string();
    let voice_dir = crate::utils::paths::voice_dir(&id);
    std::fs::create_dir_all(&voice_dir)
        .map_err(|e| format!("Failed to create voice directory: {}", e))?;

    let dest = voice_dir.join("source.wav");
    let dest_clone = dest.clone();
    tokio::task::spawn_blocking(move || write_pcm_to_wav(&pcm_f32, sample_rate, &dest_clone))
        .await
        .map_err(|e| format!("Task panicked: {}", e))??;

    let baked_dir = voice_dir.join("baked");
    let dest_clone2 = dest.clone();
    let baked_dir_clone = baked_dir.clone();
    tokio::task::spawn_blocking(move || pre_bake_speaker_tensors(&dest_clone2, &baked_dir_clone))
        .await
        .map_err(|e| format!("Task panicked: {}", e))??;

    let entry = VoiceEntry {
        id: id.clone(),
        name: name.clone(),
        source_kind: "pre_baked".to_string(),
        wav_path: Some(dest.to_string_lossy().into_owned()),
        voice_dir: Some(baked_dir.to_string_lossy().into_owned()),
        created_at: now_epoch(),
        preview_wav: None,
    };

    let conn = open_db().await?;
    voices::insert_voice(&conn, &entry)
        .await
        .map_err(|e| format!("Failed to save voice: {}", e))?;

    log::info!(
        "[Voices] Added voice from recording '{}' (id={}) with pre-baked tensors",
        name,
        id
    );
    Ok(VoiceEntryDto::from(entry))
}

/// Delete a voice entry from the database and remove all associated files from disk.
#[tauri::command]
pub async fn delete_voice(id: String) -> Result<(), String> {
    let conn = open_db().await?;
    let entry = voices::get_voice(&conn, &id)
        .await
        .map_err(|e| format!("DB error: {}", e))?
        .ok_or_else(|| format!("Voice not found: {}", id))?;

    voices::delete_voice(&conn, &id)
        .await
        .map_err(|e| format!("Failed to delete voice from DB: {}", e))?;

    let voice_dir = crate::utils::paths::voice_dir(&entry.id);
    if voice_dir.exists() {
        tokio::task::spawn_blocking(move || {
            std::fs::remove_dir_all(&voice_dir)
                .map_err(|e| format!("Failed to remove voice files: {}", e))
        })
        .await
        .map_err(|e| format!("Task panicked: {}", e))??;
    }

    log::info!("[Voices] Deleted voice '{}' (id={})", entry.name, id);
    Ok(())
}

/// Rename a voice entry in the database.
#[tauri::command]
pub async fn rename_voice(id: String, name: String) -> Result<(), String> {
    let name = name.trim().to_string();
    let conn = open_db().await?;
    voices::rename_voice(&conn, &id, &name)
        .await
        .map_err(|e| format!("Failed to rename voice: {}", e))?;
    log::info!("[Voices] Renamed voice {} to '{}'", id, name);
    Ok(())
}

/// Start backend microphone recording for voice cloning.
#[tauri::command]
pub async fn start_backend_recording() -> Result<(), String> {
    start_recording()
}

/// Stop backend microphone recording and return captured audio samples.
#[tauri::command]
pub async fn stop_backend_recording() -> Result<(Vec<f32>, u32), String> {
    stop_recording()
}
