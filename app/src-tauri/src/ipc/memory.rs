use std::sync::Arc;

use tauri::{AppHandle, State};

pub use crate::persistence::personal_memory::{
    PersonalMemoryRecord, PersonalMemorySuggestionRecord, SuggestionDecision,
};
use crate::{
    core::{
        error::VoxIpcError,
        events::{emit_ipc, IpcEvent},
        state::AppState,
    },
    persistence::{
        fetch_all_active_facts,
        list_personal_memory_versions,
        set_active_personal_memory_version as db_set_active_personal_memory_version,
        fetch_pending_suggestions,
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
        memory::personal::{
            batch_resolve_memory_suggestions as service_batch_resolve_memory_suggestions,
            consolidate_personal_memory as service_consolidate_personal_memory,
            regenerate_personal_memory as service_regenerate_personal_memory,
            resolve_memory_suggestions as service_resolve_memory_suggestions,
            MemorySuggestionError,
        },
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
    conflict_policy: Option<String>,
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

    log::info!(
        "[IPC::Memory] consolidate_personal_memory initiated: comments_count={}, project_id={:?}, conflict_policy={:?}",
        comments.as_ref().map(|c| c.len()).unwrap_or(0),
        project_id,
        conflict_policy
    );

    let parsed_policy = conflict_policy.and_then(|s| s.parse().ok());

    let conn = state.db.connect().map_err(|e| {
        log::error!("[IPC::Memory] Database connection error: {}", e);
        VoxIpcError::Database(e.to_string())
    })?;
    let record = service_consolidate_personal_memory(
        &conn,
        provider.as_ref(),
        comments,
        project_id.as_deref(),
        Some(&llm_settings),
        parsed_policy,
    )
    .await
    .map_err(|e| {
        log::error!("[IPC::Memory] consolidate_personal_memory failed: {}", e);
        VoxIpcError::Engine(e.to_string())
    })?;

    log::info!(
        "[IPC::Memory] consolidate_personal_memory succeeded: v{} ({} chars)",
        record.version,
        record.content.len()
    );

    if let Err(e) = emit_ipc(&app, IpcEvent::PersonalMemoryUpdated(record.clone())) {
        log::warn!("[IPC::Memory] Failed to emit PersonalMemoryUpdated: {}", e);
    }

    Ok(record)
}

/// Retrieves all historical versions of personal memory for a project or global default.
#[tauri::command]
pub async fn get_personal_memory_versions(
    project_id: Option<String>,
    state: State<'_, Arc<AppState>>,
) -> Result<Vec<PersonalMemoryRecord>, VoxIpcError> {
    let conn = state
        .db
        .connect()
        .map_err(|e| VoxIpcError::Database(e.to_string()))?;
    list_personal_memory_versions(&conn, project_id.as_deref())
        .await
        .map_err(|e| VoxIpcError::Database(e.to_string()))
}

/// Sets a specific historical version of personal memory to active, deactivating previous versions.
#[tauri::command]
pub async fn set_active_personal_memory_version(
    app: AppHandle,
    version: i64,
    project_id: Option<String>,
    state: State<'_, Arc<AppState>>,
) -> Result<PersonalMemoryRecord, VoxIpcError> {
    let conn = state
        .db
        .connect()
        .map_err(|e| VoxIpcError::Database(e.to_string()))?;
    let record = db_set_active_personal_memory_version(
        &conn,
        project_id.as_deref(),
        version,
    )
    .await
    .map_err(|e| VoxIpcError::Database(e.to_string()))?;

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

/// Lists all uncommitted delta suggestions pending review for the active personal memory document.
#[tauri::command]
pub async fn get_memory_suggestions(
    project_id: Option<String>,
    state: State<'_, Arc<AppState>>,
) -> Result<Vec<PersonalMemorySuggestionRecord>, VoxIpcError> {
    let conn = state
        .db
        .connect()
        .map_err(|e| VoxIpcError::Database(e.to_string()))?;
    fetch_pending_suggestions(&conn, project_id.as_deref())
        .await
        .map_err(|e| VoxIpcError::Database(e.to_string()))
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolveSuggestionsRequest {
    #[serde(alias = "project_id")]
    pub project_id: Option<String>,
    pub decisions: Vec<SuggestionDecision>,
}

/// Resolves a batch of pending personal memory suggestions in a single atomic transaction.
#[tauri::command]
pub async fn resolve_memory_suggestions(
    app: AppHandle,
    request: ResolveSuggestionsRequest,
    state: State<'_, Arc<AppState>>,
) -> Result<PersonalMemoryRecord, VoxIpcError> {
    let conn = state
        .db
        .connect()
        .map_err(|e| VoxIpcError::Database(e.to_string()))?;

    let updated_record = service_batch_resolve_memory_suggestions(
        &conn,
        request.project_id.as_deref(),
        &request.decisions,
    )
    .await
    .map_err(map_suggestion_error)?;

    if let Err(e) = emit_ipc(
        &app,
        IpcEvent::PersonalMemoryUpdated(updated_record.clone()),
    ) {
        log::warn!("[IPC::Memory] Failed to emit PersonalMemoryUpdated: {}", e);
    }

    Ok(updated_record)
}

/// Resolves a single pending suggestion or all pending suggestions for personal memory.
/// Maintained for backward compatibility.
#[tauri::command]
pub async fn resolve_memory_suggestion(
    app: AppHandle,
    id: Option<String>,
    action: String,
    project_id: Option<String>,
    state: State<'_, Arc<AppState>>,
) -> Result<PersonalMemoryRecord, VoxIpcError> {
    let conn = state
        .db
        .connect()
        .map_err(|e| VoxIpcError::Database(e.to_string()))?;

    let updated_record =
        service_resolve_memory_suggestions(&conn, project_id.as_deref(), id.as_deref(), &action)
            .await
            .map_err(map_suggestion_error)?;

    if let Err(e) = emit_ipc(
        &app,
        IpcEvent::PersonalMemoryUpdated(updated_record.clone()),
    ) {
        log::warn!("[IPC::Memory] Failed to emit PersonalMemoryUpdated: {}", e);
    }

    Ok(updated_record)
}

/// User-triggered reformatting and reorganization of the existing personal memory document.
/// Operates strictly on the existing document text, NOT raw facts.
#[tauri::command]
pub async fn regenerate_personal_memory(
    app: AppHandle,
    project_id: Option<String>,
    state: State<'_, Arc<AppState>>,
) -> Result<PersonalMemoryRecord, VoxIpcError> {
    let conn = state
        .db
        .connect()
        .map_err(|e| VoxIpcError::Database(e.to_string()))?;

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

    let updated_record = service_regenerate_personal_memory(
        &conn,
        provider.as_ref(),
        project_id.as_deref(),
        Some(&llm_settings),
    )
    .await
    .map_err(|e| VoxIpcError::Engine(e.to_string()))?;

    if let Err(e) = emit_ipc(
        &app,
        IpcEvent::PersonalMemoryUpdated(updated_record.clone()),
    ) {
        log::warn!("[IPC::Memory] Failed to emit PersonalMemoryUpdated: {}", e);
    }

    Ok(updated_record)
}

/// Maps the suggestion-resolution domain error onto the IPC error taxonomy,
/// preserving both the variant and the human-readable message.
fn map_suggestion_error(error: MemorySuggestionError) -> VoxIpcError {
    match error {
        MemorySuggestionError::InvalidAction(message) => VoxIpcError::InvalidArgument(message),
        MemorySuggestionError::NotPending(message) => VoxIpcError::NotFound(message),
        // The engine refused to commit the patched document. Same taxonomy as `PatchEngine`:
        // the request was well-formed, but the engine would not produce a valid document.
        MemorySuggestionError::PatchEngine(message)
        | MemorySuggestionError::StructureGate(message) => VoxIpcError::Engine(message),
        MemorySuggestionError::Database(inner) => VoxIpcError::Database(inner.to_string()),
    }
}
