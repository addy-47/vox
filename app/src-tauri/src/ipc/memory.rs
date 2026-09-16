use std::sync::Arc;

use tauri::{AppHandle, State};

pub use crate::persistence::personal_memory::PersonalMemoryRecord;
use crate::{
    core::{
        error::VoxIpcError,
        events::{emit_ipc, IpcEvent},
        state::AppState,
    },
    persistence::{
        fetch_all_active_facts,
        personal_memory::{
            get_personal_memory as db_get_personal_memory,
            save_personal_memory as db_save_personal_memory,
        },
        FactRecord,
    },
    services::{
        llm::{
            actor::create_llm_provider_from_llm_settings, LlmProvider, QWEN_MODEL_DIR,
            QWEN_MODEL_FILE,
        },
        memory::personal::consolidate_personal_memory as service_consolidate_personal_memory,
    },
    utils::paths,
};

/// Retrieves the consolidated personal memory markdown document.
#[tauri::command]
pub async fn get_personal_memory(
    project_id: Option<String>,
    state: State<'_, Arc<AppState>>,
) -> Result<PersonalMemoryRecord, VoxIpcError> {
    let conn = state
        .db
        .connect()
        .map_err(|e| VoxIpcError::Database(e.to_string()))?;
    db_get_personal_memory(&conn, project_id.as_deref())
        .await
        .map_err(|e| VoxIpcError::Database(e.to_string()))
}

/// Saves direct manual edits made to the personal memory document with optimistic concurrency control.
#[tauri::command]
pub async fn save_personal_memory(
    app: AppHandle,
    content: String,
    expected_version: u64,
    project_id: Option<String>,
    state: State<'_, Arc<AppState>>,
) -> Result<PersonalMemoryRecord, VoxIpcError> {
    let conn = state
        .db
        .connect()
        .map_err(|e| VoxIpcError::Database(e.to_string()))?;
    let record = db_save_personal_memory(
        &conn,
        project_id.as_deref(),
        &content,
        expected_version as i64,
    )
    .await
    .map_err(|e| {
        let err_msg = e.to_string();
        if err_msg.contains("conflict") {
            VoxIpcError::Conflict(err_msg)
        } else {
            VoxIpcError::Database(err_msg)
        }
    })?;

    if let Err(e) = emit_ipc(&app, IpcEvent::PersonalMemoryUpdated(record.clone())) {
        log::warn!("[IPC::Memory] Failed to emit PersonalMemoryUpdated: {}", e);
    }

    Ok(record)
}

/// Merges active personal facts or applies user directive comments to consolidate the personal memory document.
#[tauri::command]
pub async fn consolidate_personal_memory(
    app: AppHandle,
    comments: Option<Vec<String>>,
    project_id: Option<String>,
    state: State<'_, Arc<AppState>>,
) -> Result<PersonalMemoryRecord, VoxIpcError> {
    let llm_settings = state
        .settings
        .read()
        .map(|s| s.llm.clone())
        .unwrap_or_default();

    let provider_opt = state.llm_provider.read().clone();
    let provider: Arc<dyn LlmProvider> = match provider_opt {
        Some(p) => p,
        None => {
            let models_dir = paths::get().models.clone();
            let llm_path = models_dir.join(QWEN_MODEL_DIR).join(QWEN_MODEL_FILE);
            create_llm_provider_from_llm_settings(&llm_settings, &llm_path)
                .map(Arc::from)
                .map_err(|e| {
                    VoxIpcError::Engine(format!("Failed to initialize LLM provider: {e}"))
                })?
        }
    };

    let conn = state
        .db
        .connect()
        .map_err(|e| VoxIpcError::Database(e.to_string()))?;
    let record = service_consolidate_personal_memory(
        &conn,
        provider.as_ref(),
        comments,
        project_id.as_deref(),
    )
    .await
    .map_err(|e| VoxIpcError::Engine(e.to_string()))?;

    if let Err(e) = emit_ipc(&app, IpcEvent::PersonalMemoryUpdated(record.clone())) {
        log::warn!("[IPC::Memory] Failed to emit PersonalMemoryUpdated: {}", e);
    }

    Ok(record)
}


/// Returns active memory facts for graph visualization, optionally scoped to one project, ordered newest first.
#[tauri::command]
pub async fn get_active_facts(
    project_id: Option<String>,
    state: State<'_, Arc<AppState>>,
) -> Result<Vec<FactRecord>, VoxIpcError> {
    let conn = state
        .db
        .connect()
        .map_err(|e| VoxIpcError::Database(e.to_string()))?;
    fetch_all_active_facts(&conn, project_id.as_deref())
        .await
        .map_err(|e| VoxIpcError::Database(e.to_string()))
}
