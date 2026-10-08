use std::sync::Arc;

use tauri::{AppHandle, State};

use crate::{
    core::{
        error::VoxIpcError,
        events::{emit_ipc, IpcEvent, Severity},
        state::AppState,
    },
    persistence::{
        fetch_all_observations, fetch_ingestion_aggregate_stats,
        fetch_pending_queue_observations, list_personal_memory_versions,
        personal_memory::get_personal_memory as db_get_personal_memory,
        set_active_personal_memory_version as db_set_active_version, IngestionStatsRecord,
        ObservationRecord, PersonalMemoryRecord, RevisionDecision, VoxDb,
    },
    services::{
        llm::{
            actor::create_llm_provider_from_llm_settings, LlmProvider, QWEN_MODEL_DIR,
            QWEN_MODEL_FILE,
        },
        memory::personal::{
            batch_resolve_memory_revisions as service_batch_resolve_memory_revisions,
            consolidate_personal_memory as service_consolidate_personal_memory,
            list_memory_revision_views,
            regenerate_personal_memory as service_regenerate_personal_memory,
            save_personal_memory_from_markdown, ConsolidateOutcome, ConsolidationRequest,
            MemoryRevisionError, MemoryRevisionView,
        },
        notifications::{notify, Action, NotificationCategory, NotificationParams},
    },
    utils::paths,
};

/// Retrieves the active Personal Memory rendered as Markdown.
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

/// Saves direct manual edits to the rendered Markdown document.
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
    let record = save_personal_memory_from_markdown(
        &conn,
        project_id.as_deref(),
        &content,
        expected_version as i64,
    )
    .await
    .map_err(map_memory_save_error)?;

    emit_memory_updated(&app, record.clone());
    Ok(record)
}

/// Integrates active personal observations into the semantic model, or stages comment-directed edits.
#[tauri::command]
pub async fn consolidate_personal_memory(
    app: AppHandle,
    comments: Option<Vec<String>>,
    project_id: Option<String>,
    forced: Option<bool>,
    state: State<'_, Arc<AppState>>,
) -> Result<ConsolidateOutcome, VoxIpcError> {
    let (llm_settings, memory_settings) = {
        let guard = state
            .settings
            .read()
            .map_err(|_| VoxIpcError::Internal("Failed to acquire settings".to_string()))?;
        (guard.llm.clone(), guard.personal_memory.clone())
    };
    let provider = resolve_llm_provider(&state, &llm_settings)?;

    log::info!(
        "[IPC::Memory] consolidate_personal_memory initiated: comments_count={}, project_id={:?}, forced={}",
        comments.as_ref().map(|c| c.len()).unwrap_or(0),
        project_id,
        forced.unwrap_or(false)
    );

    let conn = state.db.connect().map_err(|e| {
        log::error!("[IPC::Memory] Database connection error: {}", e);
        VoxIpcError::Database(e.to_string())
    })?;
    let outcome = match service_consolidate_personal_memory(ConsolidationRequest {
        conn: &conn,
        llm_provider: provider.as_ref(),
        comments,
        project_id: project_id.as_deref(),
        memory_settings: &memory_settings,
        llm_settings: Some(&llm_settings),
        forced: forced.unwrap_or(false),
    })
    .await
    {
        Ok(outcome) => outcome,
        Err(e) => {
            log::error!("[IPC::Memory] consolidate_personal_memory failed: {}", e);
            notify_consolidation_failure(&app, &state.db, &e.to_string()).await;
            return Err(VoxIpcError::Engine(e.to_string()));
        }
    };

    if let ConsolidateOutcome::Completed { record } = &outcome {
        if let Err(e) = emit_ipc(&app, IpcEvent::PersonalMemoryUpdated(record.clone())) {
            log::warn!("[IPC::Memory] Failed to emit PersonalMemoryUpdated: {}", e);
        }
    }
    Ok(outcome)
}

/// Fires a Transient toast for a consolidation failure; toast delivery itself never fails the IPC error.
async fn notify_consolidation_failure(app: &AppHandle, db: &VoxDb, reason: &str) {
    let one_line: String = reason.split_whitespace().collect::<Vec<_>>().join(" ");
    let trimmed: String = one_line.chars().take(180).collect();
    let message = format!("Memory integration failed: {}", trimmed);
    if let Err(e) = notify(
        app,
        db,
        NotificationParams {
            group_key: Some("memory_integration:failure"),
            category: NotificationCategory::MemoryConsolidation,
            severity: Severity::Warning,
            impact: None,
            action: Action::Transient,
            title: "Memory integration failed",
            message: &message,
            session_id: None,
            metadata: None,
            duration_ms: None,
        },
    )
    .await
    {
        log::warn!(
            "[IPC::Memory] Failed to dispatch consolidation failure toast: {}",
            e
        );
    }
}

/// Retrieves all historical versions of Personal Memory, each rendered as Markdown.
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

/// Restores a historical version as the active Personal Memory, deactivating previous versions.
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
    let record = db_set_active_version(&conn, project_id.as_deref(), version)
        .await
        .map_err(|e| VoxIpcError::Database(e.to_string()))?;

    emit_memory_updated(&app, record.clone());
    Ok(record)
}

/// Returns observations from `memory_facts` across all or filtered statuses (`active`, `integrated`, etc.).
#[tauri::command]
pub async fn get_observations(
    project_id: Option<String>,
    status: Option<String>,
    limit: Option<u32>,
    offset: Option<u32>,
    observation_type: Option<String>,
    state: State<'_, Arc<AppState>>,
) -> Result<Vec<ObservationRecord>, VoxIpcError> {
    let conn = state
        .db
        .connect()
        .map_err(|e| VoxIpcError::Database(e.to_string()))?;

    // "staged" is the UX name for memory_facts where status = 'active' (extracted, ready to integrate).
    // "pending" refers to memory_ingestion_queue items not yet processed by the LLM extraction pipeline.
    if status.as_deref() == Some("pending") {
        return fetch_pending_queue_observations(&conn, limit, offset, observation_type.as_deref())
            .await
            .map_err(|e| VoxIpcError::Database(e.to_string()));
    }

    // Map frontend "staged" → backend "active" for memory_facts.
    let mapped_status = status
        .as_deref()
        .map(|s| if s == "staged" { "active" } else { s });
    fetch_all_observations(
        &conn,
        project_id.as_deref(),
        mapped_status,
        limit,
        offset,
        observation_type.as_deref(),
    )
    .await
    .map_err(|e| VoxIpcError::Database(e.to_string()))
}

/// Returns aggregate counts for memory ingestion and active facts.
#[tauri::command]
pub async fn get_ingestion_stats(
    state: State<'_, Arc<AppState>>,
) -> Result<IngestionStatsRecord, VoxIpcError> {
    let conn = state
        .db
        .connect()
        .map_err(|e| VoxIpcError::Database(e.to_string()))?;
    fetch_ingestion_aggregate_stats(&conn)
        .await
        .map_err(|e| VoxIpcError::Database(e.to_string()))
}

/// Lists all pending semantic memory revisions awaiting review.
#[tauri::command]
pub async fn get_memory_revisions(
    project_id: Option<String>,
    state: State<'_, Arc<AppState>>,
) -> Result<Vec<MemoryRevisionView>, VoxIpcError> {
    let conn = state
        .db
        .connect()
        .map_err(|e| VoxIpcError::Database(e.to_string()))?;
    list_memory_revision_views(&conn, project_id.as_deref())
        .await
        .map_err(|e| VoxIpcError::Database(e.to_string()))
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolveRevisionsRequest {
    #[serde(alias = "project_id")]
    pub project_id: Option<String>,
    pub decisions: Vec<RevisionDecision>,
}

/// Resolves a batch of pending personal memory revisions in a single atomic transaction.
#[tauri::command]
pub async fn resolve_memory_revisions(
    app: AppHandle,
    request: ResolveRevisionsRequest,
    state: State<'_, Arc<AppState>>,
) -> Result<PersonalMemoryRecord, VoxIpcError> {
    let conn = state
        .db
        .connect()
        .map_err(|e| VoxIpcError::Database(e.to_string()))?;

    let record = service_batch_resolve_memory_revisions(
        &conn,
        request.project_id.as_deref(),
        &request.decisions,
    )
    .await
    .map_err(map_revision_error)?;

    emit_memory_updated(&app, record.clone());
    Ok(record)
}

/// Regenerates the semantic Personal Memory into a fresh, fully reorganized structure.
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

    let (llm_settings, memory_settings) = {
        let guard = state
            .settings
            .read()
            .map_err(|_| VoxIpcError::Internal("Failed to acquire settings".to_string()))?;
        (guard.llm.clone(), guard.personal_memory.clone())
    };
    let provider = resolve_llm_provider(&state, &llm_settings)?;

    let record = service_regenerate_personal_memory(
        &conn,
        provider.as_ref(),
        project_id.as_deref(),
        &memory_settings,
        Some(&llm_settings),
    )
    .await
    .map_err(|e| VoxIpcError::Engine(e.to_string()))?;

    emit_memory_updated(&app, record.clone());
    Ok(record)
}

/// Emits the `personal_memory_updated` event.
fn emit_memory_updated(app: &AppHandle, record: PersonalMemoryRecord) {
    if let Err(e) = emit_ipc(app, IpcEvent::PersonalMemoryUpdated(record)) {
        log::warn!("[IPC::Memory] Failed to emit PersonalMemoryUpdated: {}", e);
    }
}

/// Returns the cached LLM provider, constructing one from settings when none is mounted.
fn resolve_llm_provider(
    state: &State<'_, Arc<AppState>>,
    llm_settings: &crate::services::llm::LlmSettings,
) -> Result<Arc<dyn LlmProvider>, VoxIpcError> {
    if let Some(provider) = state.llm_provider.read().clone() {
        return Ok(provider);
    }
    let models_dir = paths::get().models.clone();
    let llm_path = models_dir.join(QWEN_MODEL_DIR).join(QWEN_MODEL_FILE);
    create_llm_provider_from_llm_settings(llm_settings, &llm_path)
        .map(Arc::from)
        .map_err(|e| VoxIpcError::Engine(format!("Failed to initialize LLM provider: {e}")))
}

/// Maps a manual-save failure onto the IPC error taxonomy, preserving the conflict distinction.
fn map_memory_save_error(error: anyhow::Error) -> VoxIpcError {
    let message = error.to_string();
    if message.contains("conflict") {
        VoxIpcError::Conflict(message)
    } else {
        VoxIpcError::Engine(message)
    }
}

/// Maps the revision-resolution domain error onto the IPC error taxonomy, preserving both the
/// variant and the human-readable message.
fn map_revision_error(error: MemoryRevisionError) -> VoxIpcError {
    match error {
        MemoryRevisionError::InvalidAction(message) => VoxIpcError::InvalidArgument(message),
        MemoryRevisionError::NotPending(message) => VoxIpcError::NotFound(message),
        MemoryRevisionError::OperationEngine(message)
        | MemoryRevisionError::StructureGate(message) => VoxIpcError::Engine(message),
        MemoryRevisionError::Database(inner) => VoxIpcError::Database(inner.to_string()),
    }
}
