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
        personal_memory::{
            get_personal_memory as db_get_personal_memory,
            save_personal_memory as db_save_personal_memory,
        },
        fetch_all_active_facts, FactRecord,
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
    db_get_personal_memory(&state.db, project_id.as_deref())
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
    let record = db_save_personal_memory(
        &state.db,
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

    // Update active working memory prompt budget if applicable
    let (context_window, max_context_share) = {
        let s = state.settings.read().unwrap_or_else(|p| p.into_inner());
        (s.llm.context_window as usize, s.memory.max_context_share)
    };
    state
        .conversation_manager
        .lock()
        .set_personal_memory(Some(record.content.clone()), context_window, max_context_share);

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
                .map_err(|e| VoxIpcError::Engine(format!("Failed to initialize LLM provider: {e}")))?
        }
    };

    let record = service_consolidate_personal_memory(
        &state.db,
        provider.as_ref(),
        comments,
        project_id.as_deref(),
    )
    .await
    .map_err(|e| VoxIpcError::Engine(e.to_string()))?;

    // Update active working memory prompt budget
    let (context_window, max_context_share) = {
        let s = state.settings.read().unwrap_or_else(|p| p.into_inner());
        (s.llm.context_window as usize, s.memory.max_context_share)
    };
    state
        .conversation_manager
        .lock()
        .set_personal_memory(Some(record.content.clone()), context_window, max_context_share);

    if let Err(e) = emit_ipc(&app, IpcEvent::PersonalMemoryUpdated(record.clone())) {
        log::warn!("[IPC::Memory] Failed to emit PersonalMemoryUpdated: {}", e);
    }

    Ok(record)
}

/// Exports the personal memory document to a local markdown file.
#[tauri::command]
pub async fn export_personal_memory(
    target_path: String,
    project_id: Option<String>,
    state: State<'_, Arc<AppState>>,
) -> Result<(), VoxIpcError> {
    let record = db_get_personal_memory(&state.db, project_id.as_deref())
        .await
        .map_err(|e| VoxIpcError::Database(e.to_string()))?;

    tokio::fs::write(&target_path, record.content)
        .await
        .map_err(|e| VoxIpcError::Internal(format!("Failed to export personal memory: {e}")))?;

    log::info!("[IPC::Memory] Exported personal memory to {}", target_path);
    Ok(())
}

/// Imports and overwrites the personal memory document from an external markdown file.
#[tauri::command]
pub async fn import_personal_memory(
    app: AppHandle,
    source_path: String,
    project_id: Option<String>,
    state: State<'_, Arc<AppState>>,
) -> Result<PersonalMemoryRecord, VoxIpcError> {
    let content = tokio::fs::read_to_string(&source_path)
        .await
        .map_err(|e| VoxIpcError::Internal(format!("Failed to read memory file {}: {}", source_path, e)))?;

    let current = db_get_personal_memory(&state.db, project_id.as_deref())
        .await
        .map_err(|e| VoxIpcError::Database(e.to_string()))?;

    let record = db_save_personal_memory(
        &state.db,
        project_id.as_deref(),
        &content,
        current.version,
    )
    .await
    .map_err(|e| VoxIpcError::Database(e.to_string()))?;

    let (context_window, max_context_share) = {
        let s = state.settings.read().unwrap_or_else(|p| p.into_inner());
        (s.llm.context_window as usize, s.memory.max_context_share)
    };
    state
        .conversation_manager
        .lock()
        .set_personal_memory(Some(record.content.clone()), context_window, max_context_share);

    if let Err(e) = emit_ipc(&app, IpcEvent::PersonalMemoryUpdated(record.clone())) {
        log::warn!("[IPC::Memory] Failed to emit PersonalMemoryUpdated: {}", e);
    }

    log::info!("[IPC::Memory] Imported personal memory from {}", source_path);
    Ok(record)
}
/// Returns all active memory facts for graph visualization, ordered newest first.
#[tauri::command]
pub async fn get_active_facts(
    project_id: Option<String>,
    state: State<'_, Arc<AppState>>,
) -> Result<Vec<FactRecord>, VoxIpcError> {
    let conn = Arc::clone(&state.db);
    let _ = project_id; // reserved for future project-scoped filtering
    fetch_all_active_facts(&conn)
        .await
        .map_err(|e| VoxIpcError::Database(e.to_string()))
}
