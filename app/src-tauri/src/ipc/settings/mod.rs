use std::sync::Arc;

use tauri::State;

pub mod catalog;
pub mod core;
pub mod mutation;

pub use core::*;

pub use catalog::*;
pub use mutation::*;

use crate::{
    core::{
        error::VoxIpcError,
        settings::{LlmModelInfo, LlmProviderConfig},
        state::AppState,
    },
    services::{
        health::{self as health_svc, ProviderConfigPayload},
        llm::catalog::{self as llm_catalog, ModelProbeResult},
    },
    setup::remote_server,
};

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
