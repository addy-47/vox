//! IPC commands for voice library management, cloning, and on-demand preview playback.

use std::{
    fs::{create_dir_all, remove_dir_all},
    path::Path,
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

use tauri::State;

use crate::{
    core::{error::VoxIpcError, state::AppState},
    persistence::voices::{self, VoiceEntry, VoiceSourceKind},
    services::tts::{
        preview::{PreviewSynthesizer, VoicePreviewResult},
        voice::{
            convert_and_validate_audio, pre_bake_speaker_tensors, resolve_voice_scope,
            start_recording, stop_recording, write_pcm_to_wav, UnifiedVoiceDto, VoiceScopeDto,
        },
    },
    utils::paths::voice_dir,
};

pub type VoiceEntryDto = UnifiedVoiceDto;

impl From<VoiceEntry> for UnifiedVoiceDto {
    fn from(e: VoiceEntry) -> Self {
        let tags: Vec<String> = serde_json::from_str(&e.tags).unwrap_or_default();
        Self {
            id: e.id,
            provider_id: e.provider_id,
            name: e.name,
            description: e.description,
            gender: e.gender,
            locale: e.locale,
            accent: e.accent,
            tags,
            sample_text: e.sample_text,
            source_kind: e.source_kind.as_str().to_string(),
            is_favorite: e.is_favorite,
            last_used_at: e.last_used_at,
            has_preview: true,
        }
    }
}

fn now_epoch() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

/// Canonical voice catalog listing command. Returns unified voices from local Turso DB.
#[tauri::command]
pub async fn list_voices(
    provider: Option<String>,
    state: State<'_, Arc<AppState>>,
) -> Result<Vec<UnifiedVoiceDto>, VoxIpcError> {
    let conn = state
        .db
        .connect()
        .map_err(|e| VoxIpcError::Database(e.to_string()))?;

    if let Some(ref p) = provider {
        let p_lower = p.to_lowercase();
        if p_lower == "gemini_live" || p_lower == "deepgram_voice_agent" {
            let has_key = {
                let guard = state
                    .settings
                    .read()
                    .map_err(|e| VoxIpcError::Internal(e.to_string()))?;
                let k = guard
                    .providers
                    .realtime
                    .api_keys
                    .get(&p_lower)
                    .cloned()
                    .or_else(|| {
                        if p_lower == "gemini_live" {
                            guard.providers.llm.cloud_keys.get("gemini").cloned()
                        } else {
                            None
                        }
                    });
                k.map(|s| !s.trim().is_empty()).unwrap_or(false)
            };

            if !has_key {
                return Ok(Vec::new());
            }

            let _ = crate::services::tts::voice::sync_realtime_voices_to_db(&conn, &p_lower).await;
        }
    }

    crate::services::tts::voice::list_unified_voices(&conn, provider.as_deref())
        .await
        .map_err(|e| VoxIpcError::Database(format!("Failed to list voices: {}", e)))
}

/// Resolves an active TTS engine id to its voice catalog scope (provider
/// filter, clone support, cloud-sync hint) without the frontend branching on
/// provider id literals.
#[tauri::command]
pub async fn get_voice_scope(engine: Option<String>) -> Result<VoiceScopeDto, VoxIpcError> {
    Ok(resolve_voice_scope(engine.as_deref().unwrap_or_default()))
}

/// Background non-blocking sync for remote voices (Edge, Azure, Google cloud).
#[tauri::command]
pub async fn sync_remote_voices(state: State<'_, Arc<AppState>>) -> Result<usize, VoxIpcError> {
    let conn = state
        .db
        .connect()
        .map_err(|e| VoxIpcError::Database(e.to_string()))?;
    let (active_category, active_model, azure_base, azure_key) = {
        let guard = state
            .settings
            .read()
            .map_err(|e| VoxIpcError::Internal(e.to_string()))?;
        (
            guard.tts.active.category.clone(),
            guard.tts.active.model.clone(),
            guard.providers.tts.azure_tts.region_or_endpoint.clone(),
            guard.providers.tts.azure_tts.key.clone(),
        )
    };
    let is_cloud = active_category == "cloud";
    let sync_azure = is_cloud
        && (active_model == "azure_tts" || active_model == "azure")
        && !azure_key.trim().is_empty();
    let sync_google = is_cloud && (active_model == "google_tts" || active_model == "google");
    tokio::task::spawn(async move {
        if let Err(e) = crate::services::tts::voice::sync_edge_tts_voices_to_db(&conn).await {
            log::warn!("[Voices] Background Edge TTS sync failed: {}", e);
        }
        if sync_azure {
            let base = crate::services::tts::auth::normalize_azure_endpoint(&azure_base);
            if let Err(e) =
                crate::services::tts::voice::sync_azure_tts_voices_to_db(&conn, &base, &azure_key)
                    .await
            {
                log::warn!("[Voices] Background Azure TTS sync failed: {}", e);
            }
        }
        if sync_google {
            if let Err(e) = crate::services::tts::voice::sync_google_tts_voices_to_db(&conn).await {
                log::warn!("[Voices] Background Google TTS sync failed: {}", e);
            }
        }
    });
    Ok(0)
}

/// Generates or resolves cached audio preview on demand.
#[tauri::command]
pub async fn synthesize_voice_preview(
    voice_id: String,
    prompt: Option<String>,
    engine: Option<String>,
    state: State<'_, Arc<AppState>>,
) -> Result<VoicePreviewResult, VoxIpcError> {
    let id = voice_id.trim().to_string();
    if id.is_empty() {
        return Err(VoxIpcError::InvalidArgument(
            "voice_id must not be empty".to_string(),
        ));
    }
    log::info!(
        "[Voices::IPC] synthesize_voice_preview requested voice_id='{}' engine={:?}",
        id,
        engine
    );
    let res = PreviewSynthesizer::global()
        .request_preview(&id, prompt.as_deref(), engine.as_deref(), &state)
        .await;
    match &res {
        Ok(preview) => {
            log::info!(
                "[Voices::IPC] synthesize_voice_preview succeeded for voice_id='{}': path='{}' duration_ms={} cached={}",
                id,
                preview.path,
                preview.duration_ms,
                preview.cached
            );
        }
        Err(e) => {
            log::error!(
                "[Voices::IPC] synthesize_voice_preview failed for voice_id='{}': {:?}",
                id,
                e
            );
        }
    }
    res
}

/// Toggles a voice favorite. Returns true when now favorited.
#[tauri::command]
pub async fn toggle_voice_favorite(
    voice_id: String,
    state: State<'_, Arc<AppState>>,
) -> Result<bool, VoxIpcError> {
    let id = voice_id.trim().to_string();
    if id.is_empty() || id.len() > 256 {
        return Err(VoxIpcError::InvalidArgument(
            "voice_id must be 1..256 chars".to_string(),
        ));
    }
    log::info!(
        "[Voices::IPC] toggle_voice_favorite requested for voice_id='{}'",
        id
    );
    let conn = state.db.connect().map_err(|e| {
        log::error!("[Voices::IPC] Failed to connect to DB: {}", e);
        VoxIpcError::Database(e.to_string())
    })?;
    let result = voices::toggle_favorite(&conn, &id).await.map_err(|e| {
        log::error!(
            "[Voices::IPC] toggle_favorite failed for id='{}': {}",
            id,
            e
        );
        VoxIpcError::Database(format!("Failed to toggle favorite: {}", e))
    })?;
    log::info!(
        "[Voices::IPC] toggle_voice_favorite for id='{}' succeeded -> is_favorite={}",
        id,
        result
    );
    Ok(result)
}

/// Records a voice use in recents.
#[tauri::command]
pub async fn record_voice_used(
    voice_id: String,
    state: State<'_, Arc<AppState>>,
) -> Result<(), VoxIpcError> {
    let id = voice_id.trim().to_string();
    if id.is_empty() || id.len() > 256 {
        return Err(VoxIpcError::InvalidArgument(
            "voice_id must be 1..256 chars".to_string(),
        ));
    }
    let conn = state
        .db
        .connect()
        .map_err(|e| VoxIpcError::Database(e.to_string()))?;
    voices::record_recent(&conn, &id)
        .await
        .map_err(|e| VoxIpcError::Database(format!("Failed to record voice use: {}", e)))
}

/// Sets the active canonical voice id in settings.
#[tauri::command]
pub async fn set_active_voice(
    voice_id: String,
    state: State<'_, Arc<AppState>>,
) -> Result<(), VoxIpcError> {
    let id = voice_id.trim().to_string();
    if id.is_empty() || id.len() > 128 {
        return Err(VoxIpcError::InvalidArgument(
            "voice_id must be 1..128 chars".to_string(),
        ));
    }
    {
        let mut settings = state
            .settings
            .write()
            .map_err(|e| VoxIpcError::Internal(format!("Settings lock poisoned: {}", e)))?;
        settings.tts.active_voice_id = id.clone();
        settings
            .save()
            .map_err(|e| VoxIpcError::Internal(format!("Failed to persist active voice: {}", e)))?;
    }
    let conn = state
        .db
        .connect()
        .map_err(|e| VoxIpcError::Database(e.to_string()))?;
    if let Err(e) = voices::record_recent(&conn, &id).await {
        log::warn!("[Voices] Failed to record recent for active voice: {}", e);
    }
    log::info!("[Voices] Active voice set to '{}'", id);
    Ok(())
}

const MAX_VOICE_RECORDING_SAMPLES: usize = 1_600_000; // ~100s at 16kHz (~6.4 MB)

/// Add a new cloned voice from an existing audio file.
#[tauri::command]
pub async fn add_voice_from_file(
    name: String,
    file_path: String,
    state: State<'_, Arc<AppState>>,
) -> Result<UnifiedVoiceDto, VoxIpcError> {
    let name = name.trim().to_string();
    if name.is_empty() {
        return Err(VoxIpcError::InvalidArgument(
            "Voice name cannot be empty".to_string(),
        ));
    }
    let path = Path::new(&file_path);
    if !path.exists() {
        return Err(VoxIpcError::NotFound(format!(
            "File does not exist: {}",
            file_path
        )));
    }

    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .unwrap_or_default();
    if !matches!(ext.as_str(), "wav" | "mp3" | "m4a" | "ogg" | "flac" | "aac") {
        return Err(VoxIpcError::InvalidArgument(format!(
            "Unsupported audio format '.{}'. Supported formats: wav, mp3, m4a, ogg, flac, aac",
            ext
        )));
    }

    let id = uuid::Uuid::new_v4().to_string();
    let voice_dir = voice_dir(&id);
    create_dir_all(&voice_dir)
        .map_err(|e| VoxIpcError::Internal(format!("Failed to create voice directory: {}", e)))?;

    let dest = voice_dir.join("source.wav");
    let dest_clone = dest.clone();
    let file_path_clone = file_path.clone();
    tokio::task::spawn_blocking(move || convert_and_validate_audio(&file_path_clone, &dest_clone))
        .await
        .map_err(|e| VoxIpcError::Internal(format!("Task panicked: {}", e)))?
        .map_err(VoxIpcError::Engine)?;

    let baked_dir = voice_dir.join("baked");
    let dest_clone2 = dest.clone();
    let baked_dir_clone = baked_dir.clone();
    tokio::task::spawn_blocking(move || pre_bake_speaker_tensors(&dest_clone2, &baked_dir_clone))
        .await
        .map_err(|e| VoxIpcError::Internal(format!("Task panicked: {}", e)))?
        .map_err(VoxIpcError::Engine)?;

    let now_sec = now_epoch();
    let entry = VoiceEntry {
        id: id.clone(),
        provider_id: "chatterbox".to_string(),
        name: name.clone(),
        source_kind: VoiceSourceKind::Custom,
        slug: None,
        gender: "neutral".to_string(),
        locale: "en-US".to_string(),
        accent: None,
        description: Some("Custom cloned voice.".to_string()),
        tags: "[\"Custom\"]".to_string(),
        sample_text: "Hello, this is my custom cloned voice.".to_string(),
        wav_path: Some(dest.to_string_lossy().into_owned()),
        voice_dir: Some(baked_dir.to_string_lossy().into_owned()),
        preview_wav: None,
        is_favorite: false,
        last_used_at: Some(now_sec * 1000),
        created_at: now_sec,
    };

    let conn = state
        .db
        .connect()
        .map_err(|e| VoxIpcError::Database(e.to_string()))?;
    voices::insert_voice(&conn, &entry)
        .await
        .map_err(|e| VoxIpcError::Database(format!("Failed to save voice: {}", e)))?;

    log::info!(
        "[Voices] Added voice '{}' (id={}) with pre-baked tensors",
        name,
        id
    );

    Ok(UnifiedVoiceDto::from(entry))
}

/// Add a new cloned voice from in-memory recorded audio.
#[tauri::command]
pub async fn add_voice_from_recording(
    name: String,
    pcm_f32: Vec<f32>,
    sample_rate: u32,
    state: State<'_, Arc<AppState>>,
) -> Result<UnifiedVoiceDto, VoxIpcError> {
    let name = name.trim().to_string();
    if name.is_empty() {
        return Err(VoxIpcError::InvalidArgument(
            "Voice name cannot be empty".to_string(),
        ));
    }
    if pcm_f32.is_empty() {
        return Err(VoxIpcError::InvalidArgument(
            "Audio data cannot be empty".to_string(),
        ));
    }
    if pcm_f32.len() > MAX_VOICE_RECORDING_SAMPLES {
        return Err(VoxIpcError::InvalidArgument(
            "Audio recording is too long (max ~100s)".to_string(),
        ));
    }
    if sample_rate == 0 || sample_rate > 192_000 {
        return Err(VoxIpcError::InvalidArgument(format!(
            "Invalid sample rate: {}",
            sample_rate
        )));
    }

    let id = uuid::Uuid::new_v4().to_string();
    let voice_dir = voice_dir(&id);
    create_dir_all(&voice_dir)
        .map_err(|e| VoxIpcError::Internal(format!("Failed to create voice directory: {}", e)))?;

    let dest = voice_dir.join("source.wav");
    let dest_clone = dest.clone();
    let pcm = pcm_f32.clone();
    tokio::task::spawn_blocking(move || write_pcm_to_wav(&pcm, sample_rate, &dest_clone))
        .await
        .map_err(|e| VoxIpcError::Internal(format!("Task panicked: {}", e)))?
        .map_err(VoxIpcError::Engine)?;

    let baked_dir = voice_dir.join("baked");
    let dest_clone2 = dest.clone();
    let baked_dir_clone = baked_dir.clone();
    tokio::task::spawn_blocking(move || pre_bake_speaker_tensors(&dest_clone2, &baked_dir_clone))
        .await
        .map_err(|e| VoxIpcError::Internal(format!("Task panicked: {}", e)))?
        .map_err(VoxIpcError::Engine)?;

    let now_sec = now_epoch();
    let entry = VoiceEntry {
        id: id.clone(),
        provider_id: "chatterbox".to_string(),
        name: name.clone(),
        source_kind: VoiceSourceKind::Custom,
        slug: None,
        gender: "neutral".to_string(),
        locale: "en-US".to_string(),
        accent: None,
        description: Some("Custom cloned voice.".to_string()),
        tags: "[\"Custom\"]".to_string(),
        sample_text: "Hello, this is my custom cloned voice.".to_string(),
        wav_path: Some(dest.to_string_lossy().into_owned()),
        voice_dir: Some(baked_dir.to_string_lossy().into_owned()),
        preview_wav: None,
        is_favorite: false,
        last_used_at: Some(now_sec * 1000),
        created_at: now_sec,
    };

    let conn = state
        .db
        .connect()
        .map_err(|e| VoxIpcError::Database(e.to_string()))?;
    voices::insert_voice(&conn, &entry)
        .await
        .map_err(|e| VoxIpcError::Database(format!("Failed to save voice: {}", e)))?;

    log::info!(
        "[Voices] Added voice '{}' (id={}) with pre-baked tensors",
        name,
        id
    );

    Ok(UnifiedVoiceDto::from(entry))
}

/// Delete a cloned voice by ID.
#[tauri::command]
pub async fn delete_voice(id: String, state: State<'_, Arc<AppState>>) -> Result<(), VoxIpcError> {
    let conn = state
        .db
        .connect()
        .map_err(|e| VoxIpcError::Database(e.to_string()))?;

    let entry = match voices::get_voice(&conn, &id).await {
        Ok(Some(e)) => e,
        Ok(None) => return Err(VoxIpcError::NotFound(format!("Voice not found: {}", id))),
        Err(e) => return Err(VoxIpcError::Database(e.to_string())),
    };

    if entry.source_kind == VoiceSourceKind::ZipvoicePack
        || entry.source_kind == VoiceSourceKind::Bundled
        || entry.source_kind == VoiceSourceKind::PreBaked
    {
        return Err(VoxIpcError::InvalidArgument(
            "Packaged and bundled voices cannot be deleted".to_string(),
        ));
    }

    voices::delete_voice(&conn, &id)
        .await
        .map_err(|e| VoxIpcError::Database(e.to_string()))?;

    let dir = voice_dir(&id);
    if dir.exists() {
        let _ = remove_dir_all(&dir);
    }

    log::info!("[Voices] Deleted voice '{}' (id={})", entry.name, id);
    Ok(())
}

/// Rename an existing voice entry.
#[tauri::command]
pub async fn rename_voice(
    id: String,
    name: String,
    state: State<'_, Arc<AppState>>,
) -> Result<(), VoxIpcError> {
    let conn = state
        .db
        .connect()
        .map_err(|e| VoxIpcError::Database(e.to_string()))?;

    let entry = match voices::get_voice(&conn, &id).await {
        Ok(Some(e)) => e,
        Ok(None) => return Err(VoxIpcError::NotFound(format!("Voice not found: {}", id))),
        Err(e) => return Err(VoxIpcError::Database(e.to_string())),
    };

    if entry.source_kind == VoiceSourceKind::ZipvoicePack
        || entry.source_kind == VoiceSourceKind::Bundled
        || entry.source_kind == VoiceSourceKind::PreBaked
    {
        return Err(VoxIpcError::InvalidArgument(
            "Packaged and bundled voices cannot be renamed".to_string(),
        ));
    }

    voices::rename_voice(&conn, &id, &name)
        .await
        .map_err(|e| VoxIpcError::Database(e.to_string()))?;

    log::info!("[Voices] Renamed voice id={} to '{}'", id, name);
    Ok(())
}

/// Start backend microphone recording for voice cloning.
#[tauri::command]
pub async fn start_backend_recording() -> Result<(), VoxIpcError> {
    start_recording().map_err(VoxIpcError::Engine)
}

/// Stop backend microphone recording and return captured audio samples.
#[tauri::command]
pub async fn stop_backend_recording() -> Result<(Vec<f32>, u32), VoxIpcError> {
    stop_recording().map_err(VoxIpcError::Engine)
}
