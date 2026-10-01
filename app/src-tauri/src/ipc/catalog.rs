use std::{fs::read_to_string, sync::Arc};

use tauri::{AppHandle, Manager, State};

use crate::{
    config::get_preset_colors,
    core::{error::VoxIpcError, state::AppState},
    persistence::voices::{
        display_name_for_slug, get_voice, list_voices as list_db_voices, VoiceListScope,
    },
    services::{
        health::{self as health_svc, ProviderConfigPayload},
        llm::{
            catalog::{self as llm_catalog, LlmModelInfo, ModelProbeResult},
            LlmProviderConfig,
        },
        tts::{
            factory::caps_for_id,
            voice::{get_supertonic_voice_profiles, get_voice_profiles},
            ProviderCaps, TtsActiveProvider, VoiceProfile,
        },
    },
    setup::{
        manifest::{ModelGroup, VoxManifest},
        remote_server,
    },
    utils::paths,
};

/// Categorized catalog of available local and cloud AI models.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ModelCatalog {
    pub llm: Vec<ModelGroup>,
    pub stt: Vec<ModelGroup>,
    pub tts: Vec<ModelGroup>,
    pub vad: Vec<ModelGroup>,
    pub auxiliary: Vec<ModelGroup>,
    pub model_groups: Vec<ModelGroup>,
    pub voices: Vec<VoiceProfile>,
    pub preset_colors: Vec<String>,
    pub active_voice_name: Option<String>,
}

/// Query the model manifest catalog filtered into distinct model categories.
#[tauri::command]
pub async fn get_model_catalog<R: tauri::Runtime>(
    app: AppHandle<R>,
) -> Result<ModelCatalog, VoxIpcError> {
    let state: State<'_, Arc<AppState>> = app.state();
    let manifest_opt = {
        let guard = state.manifest.read().await;
        guard.clone()
    };

    let manifest = if let Some(m) = manifest_opt {
        m
    } else {
        let manifest_path = paths::get().models.join("models_manifest.json");
        if manifest_path.exists() {
            let p = manifest_path.clone();
            let content = tokio::task::spawn_blocking(move || read_to_string(&p))
                .await
                .map_err(|e| VoxIpcError::Internal(e.to_string()))?
                .map_err(|e| VoxIpcError::Internal(e.to_string()))?;
            serde_json::from_str::<VoxManifest>(&content)
                .map_err(|e| VoxIpcError::Internal(e.to_string()))?
        } else {
            return Err(VoxIpcError::NotFound("Manifest not available".to_string()));
        }
    };

    let groups = manifest.model_groups;

    let llm = groups
        .iter()
        .filter(|g| g.category == "llm")
        .cloned()
        .collect();
    let stt = groups
        .iter()
        .filter(|g| g.category == "stt")
        .cloned()
        .collect();
    let tts = groups
        .iter()
        .filter(|g| g.category == "tts")
        .cloned()
        .collect();
    let vad = groups
        .iter()
        .filter(|g| g.category == "vad")
        .cloned()
        .collect();
    let auxiliary = groups
        .iter()
        .filter(|g| {
            g.subcategory.as_deref() == Some("auxiliary")
                || matches!(
                    g.category.as_str(),
                    "translit" | "embedding" | "nli" | "classifier"
                )
        })
        .cloned()
        .collect();

    let (tts_active, tts_settings) = {
        let guard = state
            .settings
            .read()
            .map_err(|e| VoxIpcError::Internal(e.to_string()))?;
        (guard.tts.active, guard.tts.clone())
    };

    let (voices, active_voice_name) = match tts_active {
        TtsActiveProvider::Supertonic => {
            let p = get_supertonic_voice_profiles();
            let active = p
                .get(tts_settings.voice_index as usize)
                .or(p.first())
                .map(|v| v.name.clone());
            (p, active)
        }
        TtsActiveProvider::Kokoro => {
            let p = get_voice_profiles();
            let active = p
                .get(tts_settings.voice_index as usize)
                .or(p.first())
                .map(|v| v.name.clone());
            (p, active)
        }
        TtsActiveProvider::Zipvoice => {
            let conn = state.db.connect().ok();
            let pack = if let Some(ref conn) = conn {
                list_db_voices(conn, VoiceListScope::ZipvoicePack)
                    .await
                    .unwrap_or_default()
            } else {
                Vec::new()
            };

            let profiles: Vec<VoiceProfile> = pack
                .iter()
                .enumerate()
                .map(|(i, v)| VoiceProfile {
                    id: i as i32,
                    name: v.name.clone(),
                    gender: None,
                    accent: None,
                    language: Some("en".to_string()),
                })
                .collect();

            let active = if let Some(ref slug) = tts_settings.zipvoice.voice_id {
                pack.iter()
                    .find(|v| v.slug.as_deref() == Some(slug) || v.id == *slug)
                    .map(|v| v.name.clone())
                    .or_else(|| Some(display_name_for_slug(slug)))
            } else {
                pack.first()
                    .map(|v| v.name.clone())
                    .or_else(|| Some("Alfred".to_string()))
            };

            (profiles, active)
        }
        TtsActiveProvider::Chatterbox | TtsActiveProvider::ChatterboxRemote => {
            let voice_id = if tts_active == TtsActiveProvider::Chatterbox {
                tts_settings.chatterbox.voice_id.as_deref()
            } else {
                tts_settings.chatterbox_remote.voice_id.as_deref()
            };

            let active = if let Some(id) = voice_id {
                let conn = state.db.connect().ok();
                if let Some(ref conn) = conn {
                    get_voice(conn, id).await.ok().flatten().map(|v| v.name)
                } else {
                    None
                }
            } else {
                Some("Default".to_string())
            };
            (Vec::new(), active)
        }
        TtsActiveProvider::EdgeTts => {
            let active = tts_settings.edge_tts.voice.map(|v| {
                if let Some(stripped) = v.strip_suffix("Neural") {
                    if let Some(name) = stripped.rsplit('-').next() {
                        return name.to_string();
                    }
                }
                v
            });
            (Vec::new(), active)
        }
    };

    Ok(ModelCatalog {
        llm,
        stt,
        tts,
        vad,
        auxiliary,
        model_groups: groups,
        voices,
        preset_colors: get_preset_colors(),
        active_voice_name,
    })
}

#[tauri::command]
pub fn get_provider_caps(provider_id: String) -> Result<ProviderCaps, VoxIpcError> {
    caps_for_id(&provider_id).map_err(VoxIpcError::InvalidArgument)
}

/// Unified health-check command across LLM, STT, and TTS engine providers.
#[tauri::command]
pub async fn check_provider_health(
    state: State<'_, Arc<AppState>>,
    kind: String,
    provider: Option<ProviderConfigPayload>,
) -> Result<health_svc::ProviderHealthCheckResult, VoxIpcError> {
    health_svc::check_health(&state, &kind, provider)
        .await
        .map_err(VoxIpcError::Engine)
}

/// List available LLM models for embedded GGUFs or OpenAI-compatible remote servers.
#[tauri::command]
pub async fn list_llm_models(
    state: State<'_, Arc<AppState>>,
    provider: Option<LlmProviderConfig>,
) -> Result<Vec<LlmModelInfo>, VoxIpcError> {
    llm_catalog::list_models(&state, provider)
        .await
        .map_err(VoxIpcError::Engine)
}

/// Probe capabilities for an LLM model and return capabilities, ceiling token cap, and cached map.
#[tauri::command]
pub async fn probe_model_capabilities(
    state: State<'_, Arc<AppState>>,
    provider: Option<LlmProviderConfig>,
    model_id: Option<String>,
    target_cap: Option<u32>,
) -> Result<ModelProbeResult, VoxIpcError> {
    llm_catalog::probe_capabilities(&state, provider, model_id, target_cap)
        .await
        .map_err(VoxIpcError::Engine)
}

/// Execute remote server bootstrap script over SSH and stream progress events.
#[tauri::command]
pub async fn setup_remote_server<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    connection_string: String,
    ssh_port: Option<u16>,
    identity_key_path: Option<String>,
    remote_path: String,
    server_port: u16,
) -> Result<(), VoxIpcError> {
    remote_server::start_remote_setup(
        app,
        connection_string,
        ssh_port,
        identity_key_path,
        remote_path,
        server_port,
    )
    .map_err(VoxIpcError::Network)
}
