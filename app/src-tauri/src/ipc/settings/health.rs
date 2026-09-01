use crate::core::settings::{LlmModelInfo, LlmProviderConfig};
use crate::core::state::AppState;
use crate::services::health::{self, ProviderConfigPayload};
use crate::services::llm::probe::{self, ModelProbeResult};
use crate::setup::remote_server;
use tauri::State;

/// Unified health-check command across LLM, STT, and TTS engine providers.
#[tauri::command]
pub async fn check_provider_health(
    state: State<'_, std::sync::Arc<AppState>>,
    kind: String,
    provider: Option<ProviderConfigPayload>,
) -> Result<bool, String> {
    health::check_health(&state, &kind, provider).await
}

/// List available LLM models for embedded GGUFs or OpenAI-compatible remote servers.
#[tauri::command]
pub async fn list_llm_models(
    state: State<'_, std::sync::Arc<AppState>>,
    provider: Option<LlmProviderConfig>,
) -> Result<Vec<LlmModelInfo>, String> {
    probe::list_models(&state, provider).await
}

/// Probe capabilities for an LLM model and return capabilities, ceiling token cap, and cached map.
#[tauri::command]
pub async fn probe_model_capabilities(
    state: State<'_, std::sync::Arc<AppState>>,
    provider: Option<LlmProviderConfig>,
    model_id: Option<String>,
    target_cap: Option<u32>,
) -> Result<ModelProbeResult, String> {
    probe::probe_capabilities(&state, provider, model_id, target_cap).await
}

/// Execute remote server bootstrap script over SSH and stream progress events.
#[tauri::command]
pub async fn setup_remote_server(
    app: tauri::AppHandle,
    connection_string: String,
    ssh_port: Option<u16>,
    identity_key_path: Option<String>,
    remote_path: String,
    server_port: u16,
) -> Result<(), String> {
    remote_server::start_remote_setup(
        app,
        connection_string,
        ssh_port,
        identity_key_path,
        remote_path,
        server_port,
    )
}
