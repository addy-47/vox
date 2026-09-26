use std::sync::Arc;

use tauri::{AppHandle, State};

pub use crate::persistence::personal_memory::{
    PersonalMemoryRecord, PersonalMemorySuggestionRecord,
};
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
    crate::persistence::list_personal_memory_versions(&conn, project_id.as_deref())
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
    let record = crate::persistence::set_active_personal_memory_version(
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
    crate::persistence::fetch_pending_suggestions(
        &conn,
        project_id.as_deref(),
    )
    .await
    .map_err(|e| VoxIpcError::Database(e.to_string()))
}

/// Resolves a single pending suggestion or all pending suggestions for personal memory.
#[tauri::command]
pub async fn resolve_memory_suggestion(
    app: AppHandle,
    id: Option<String>,
    action: String,
    project_id: Option<String>,
    state: State<'_, Arc<AppState>>,
) -> Result<PersonalMemoryRecord, VoxIpcError> {
    if action != "accept" && action != "reject" {
        return Err(VoxIpcError::InvalidArgument(format!(
            "Invalid suggestion resolution action: {action}. Expected 'accept' or 'reject'"
        )));
    }

    let conn = state
        .db
        .connect()
        .map_err(|e| VoxIpcError::Database(e.to_string()))?;

    let active_memory = db_get_personal_memory(&conn, project_id.as_deref())
        .await
        .map_err(|e| VoxIpcError::Database(e.to_string()))?;

    let pending = crate::persistence::fetch_pending_suggestions(
        &conn,
        project_id.as_deref(),
    )
    .await
    .map_err(|e| VoxIpcError::Database(e.to_string()))?;

    let to_resolve: Vec<PersonalMemorySuggestionRecord> = if let Some(ref target_id) = id {
        let matching: Vec<_> = pending.into_iter().filter(|s| &s.id == target_id).collect();
        if matching.is_empty() {
            return Err(VoxIpcError::NotFound(format!(
                "Suggestion '{target_id}' not found among pending suggestions"
            )));
        }
        matching
    } else {
        pending
    };

    if to_resolve.is_empty() {
        return Ok(active_memory);
    }

    let updated_record = if action == "accept" {
        let patch_ops: Vec<crate::services::memory::MemoryPatchOperation> = to_resolve
            .iter()
            .map(|s| crate::services::memory::MemoryPatchOperation {
                op: s.op.clone(),
                section: s.section.clone(),
                target_text: s.target_text.clone(),
                proposed_text: s.proposed_text.clone(),
                source_fact_ids: s.source_fact_ids.clone(),
            })
            .collect();

        let patched_content =
            crate::services::memory::apply_patch_operations(&active_memory.content, &patch_ops)
                .map_err(|e| {
                    VoxIpcError::Engine(format!("Failed to apply patch operations: {e}"))
                })?;

        crate::persistence::resolve_suggestions_transaction(
            &conn,
            project_id.as_deref(),
            id.as_deref(),
            "accept",
            Some(&patched_content),
        )
        .await
        .map_err(|e| VoxIpcError::Database(e.to_string()))?
    } else {
        crate::persistence::resolve_suggestions_transaction(
            &conn,
            project_id.as_deref(),
            id.as_deref(),
            "reject",
            None,
        )
        .await
        .map_err(|e| VoxIpcError::Database(e.to_string()))?
    };

    if let Err(e) = emit_ipc(
        &app,
        IpcEvent::PersonalMemoryUpdated(updated_record.clone()),
    ) {
        log::warn!("[IPC::Memory] Failed to emit PersonalMemoryUpdated: {}", e);
    }

    Ok(updated_record)
}
