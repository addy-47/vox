use crate::core::error::VoxIpcError;
use crate::core::state::AppState;
use crate::persistence::db::VoxDb;
pub use crate::persistence::graph::{
    MemoryConflictItem as MemoryConflict, MemoryEdgeTopology, MemoryFactDetail,
    MemoryGraphPayload, MemoryGraphQueryFilter, MemoryNodeTopology,
};
pub use crate::persistence::{MemoryQueueItem, MemoryQueueSummary};
use serde::{Deserialize, Serialize};
use std::sync::atomic::Ordering;
use tauri::State;

// ── Graph Commands ─────────────────────────────────────────────────────────────

/// Retrieve the current monotonic memory graph version.
#[tauri::command]
pub async fn get_graph_version(
    state: State<'_, std::sync::Arc<AppState>>,
) -> Result<u64, VoxIpcError> {
    Ok(state
        .memory
        .graph_version
        .load(Ordering::SeqCst))
}

/// Retrieve the full memory graph topology filtered by collection or active status.
#[tauri::command]
pub async fn get_memory_graph_topology(
    state: State<'_, std::sync::Arc<AppState>>,
    filter: Option<MemoryGraphQueryFilter>,
) -> Result<MemoryGraphPayload, VoxIpcError> {
    let db_path = crate::utils::paths::get().db.clone();
    let conn = VoxDb::open_readonly(&db_path)
        .await
        .map_err(|e| VoxIpcError::Database(format!("DB open failed: {}", e)))?;

    let version = state
        .memory
        .graph_version
        .load(Ordering::SeqCst);

    crate::persistence::graph::fetch_memory_graph(&conn, filter.as_ref(), version)
        .await
        .map_err(|e| VoxIpcError::Database(e.to_string()))
}

/// Retrieve detailed information for a single memory fact by ID.
#[tauri::command]
pub async fn get_memory_fact_detail(fact_id: String) -> Result<MemoryFactDetail, VoxIpcError> {
    let db_path = crate::utils::paths::get().db.clone();
    let conn = VoxDb::open_readonly(&db_path)
        .await
        .map_err(|e| VoxIpcError::Database(format!("DB open failed: {}", e)))?;

    crate::persistence::graph::fetch_fact_detail(&conn, &fact_id)
        .await
        .map_err(|e| VoxIpcError::Database(e.to_string()))?
        .ok_or_else(|| VoxIpcError::NotFound(format!("Memory fact not found: {}", fact_id)))
}

// ── Conflicts Commands ─────────────────────────────────────────────────────────

/// Retrieve all unresolved memory fact conflicts from the graph.
#[tauri::command]
pub async fn get_unresolved_conflicts() -> Result<Vec<MemoryConflict>, VoxIpcError> {
    let db_path = crate::utils::paths::get().db.clone();
    let conn = VoxDb::open_readonly(&db_path)
        .await
        .map_err(|e| VoxIpcError::Database(format!("DB open failed: {}", e)))?;

    crate::persistence::graph::fetch_memory_conflicts(&conn)
        .await
        .map_err(|e| VoxIpcError::Database(e.to_string()))
}

/// Resolve a memory conflict by marking the loser as superseded and linking the winner.
#[tauri::command]
pub async fn resolve_memory_conflict(
    state: State<'_, std::sync::Arc<AppState>>,
    winner_id: String,
    loser_id: String,
) -> Result<(), VoxIpcError> {
    let db_path = crate::utils::paths::get().db.clone();
    let conn = VoxDb::open(&db_path)
        .await
        .map_err(|e| VoxIpcError::Database(format!("DB open failed: {}", e)))?;

    crate::persistence::memory_mutations::resolve_fact_conflict(&conn, &winner_id, &loser_id)
        .await
        .map_err(|e| VoxIpcError::Database(e.to_string()))?;

    state.memory.graph_version.fetch_add(1, Ordering::SeqCst);
    Ok(())
}

// ── Fact Mutations Commands ───────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct ManageFactPayload {
    pub action: String,
    pub fact_id: String,
    pub new_content: Option<String>,
    pub new_collection: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum ManageFactResult {
    NewId(String),
    Done,
}

/// Unified command for modifying, superseding, reassigning, and deleting memory facts.
#[tauri::command]
pub async fn manage_memory_fact(
    state: State<'_, std::sync::Arc<AppState>>,
    payload: ManageFactPayload,
) -> Result<ManageFactResult, VoxIpcError> {
    match payload.action.to_lowercase().as_str() {
        "edit_in_place" | "edit" => edit_fact_in_place_internal(&state, payload).await,
        "supersede" | "user_edit" => supersede_fact_internal(&state, payload).await,
        "reassign" => reassign_fact_internal(&state, payload).await,
        "delete" | "soft_delete" => delete_fact_internal(&state, payload).await,
        _ => Err(VoxIpcError::InvalidArgument(format!(
            "Unknown manage_memory_fact action: {}",
            payload.action
        ))),
    }
}

async fn edit_fact_in_place_internal(
    state: &State<'_, std::sync::Arc<AppState>>,
    payload: ManageFactPayload,
) -> Result<ManageFactResult, VoxIpcError> {
    let new_content = payload.new_content.ok_or_else(|| {
        VoxIpcError::InvalidArgument("new_content required for edit action".to_string())
    })?;
    let trimmed = new_content.trim();
    if trimmed.is_empty() {
        return Err(VoxIpcError::InvalidArgument(
            "Fact content cannot be empty".to_string(),
        ));
    }

    let memory_enabled = state
        .settings
        .read()
        .map(|s| s.memory.pipeline_processing_enabled || s.memory.context_retrieval_enabled)
        .unwrap_or(false);
    if !memory_enabled {
        return Err(VoxIpcError::InvalidState(
            "Memory subsystem is disabled".to_string(),
        ));
    }

    let db_path = crate::utils::paths::get().db.clone();
    let conn = VoxDb::open(&db_path)
        .await
        .map_err(|e| VoxIpcError::Database(format!("DB open failed: {}", e)))?;

    let trimmed_clone = trimmed.to_string();
    let embedding = tokio::task::spawn_blocking(move || {
        crate::services::memory::ensure_embedder_loaded(true)
            .map_err(|e| VoxIpcError::Engine(format!("Embedder loading failed: {}", e)))?;
        crate::services::memory::generate_embedding(&trimmed_clone)
            .map_err(|e| VoxIpcError::Engine(format!("Embedding generation failed: {}", e)))?
            .ok_or_else(|| VoxIpcError::Engine("Failed to generate embedding vector".to_string()))
    })
    .await
    .map_err(|e| VoxIpcError::Internal(format!("Task panicked: {}", e)))??;

    crate::persistence::memory_mutations::update_memory_fact(
        &conn,
        &payload.fact_id,
        trimmed,
        &embedding,
    )
    .await
    .map_err(|e| VoxIpcError::Database(e.to_string()))?;

    state.memory.graph_version.fetch_add(1, Ordering::SeqCst);
    Ok(ManageFactResult::Done)
}

async fn supersede_fact_internal(
    state: &State<'_, std::sync::Arc<AppState>>,
    payload: ManageFactPayload,
) -> Result<ManageFactResult, VoxIpcError> {
    let new_content = payload.new_content.ok_or_else(|| {
        VoxIpcError::InvalidArgument("new_content required for supersede action".to_string())
    })?;
    let collection = payload
        .new_collection
        .unwrap_or_else(|| "Identity".to_string());
    let trimmed = new_content.trim();
    if trimmed.is_empty() {
        return Err(VoxIpcError::InvalidArgument(
            "Fact content cannot be empty".to_string(),
        ));
    }

    let memory_enabled = state
        .settings
        .read()
        .map(|s| s.memory.pipeline_processing_enabled || s.memory.context_retrieval_enabled)
        .unwrap_or(false);
    if !memory_enabled {
        return Err(VoxIpcError::InvalidState(
            "Memory subsystem is disabled".to_string(),
        ));
    }

    let db_path = crate::utils::paths::get().db.clone();
    let conn = VoxDb::open(&db_path)
        .await
        .map_err(|e| VoxIpcError::Database(format!("DB open failed: {}", e)))?;

    let new_id = crate::persistence::memory_mutations::supersede_user_fact(
        &conn,
        &payload.fact_id,
        trimmed,
        &collection,
    )
    .await
    .map_err(|e| VoxIpcError::Database(e.to_string()))?;

    state.memory.graph_version.fetch_add(1, Ordering::SeqCst);
    log::info!(
        "[Memory] Successfully superseded fact {} -> new_id={}",
        payload.fact_id,
        new_id
    );
    Ok(ManageFactResult::NewId(new_id))
}

async fn reassign_fact_internal(
    state: &State<'_, std::sync::Arc<AppState>>,
    payload: ManageFactPayload,
) -> Result<ManageFactResult, VoxIpcError> {
    let new_collection = payload.new_collection.ok_or_else(|| {
        VoxIpcError::InvalidArgument("new_collection required for reassign action".to_string())
    })?;
    let db_path = crate::utils::paths::get().db.clone();
    let conn = VoxDb::open(&db_path)
        .await
        .map_err(|e| VoxIpcError::Database(format!("DB open failed: {}", e)))?;

    crate::persistence::memory_mutations::reassign_memory_fact(&conn, &payload.fact_id, &new_collection)
        .await
        .map_err(|e| VoxIpcError::Database(e.to_string()))?;

    state.memory.graph_version.fetch_add(1, Ordering::SeqCst);
    Ok(ManageFactResult::Done)
}

async fn delete_fact_internal(
    state: &State<'_, std::sync::Arc<AppState>>,
    payload: ManageFactPayload,
) -> Result<ManageFactResult, VoxIpcError> {
    let db_path = crate::utils::paths::get().db.clone();
    let conn = VoxDb::open(&db_path)
        .await
        .map_err(|e| VoxIpcError::Database(format!("DB open failed: {}", e)))?;

    crate::persistence::memory_mutations::delete_memory_fact(&conn, &payload.fact_id)
        .await
        .map_err(|e| VoxIpcError::Database(e.to_string()))?;

    state.memory.graph_version.fetch_add(1, Ordering::SeqCst);
    Ok(ManageFactResult::Done)
}

// ── Ingestion & Queue Commands ────────────────────────────────────────────────

/// Retrieve queue status counts and the most recent 50 queue items.
#[tauri::command]
pub async fn get_memory_queue_status() -> Result<MemoryQueueSummary, VoxIpcError> {
    let db_path = crate::utils::paths::get().db.clone();
    let conn = VoxDb::open_readonly(&db_path)
        .await
        .map_err(|e| VoxIpcError::Database(format!("DB open failed: {}", e)))?;

    crate::persistence::memory_queries::fetch_memory_queue_status(&conn)
        .await
        .map_err(|e| VoxIpcError::Database(e.to_string()))
}

/// Pause or resume background processing for personal memory queue.
#[tauri::command]
pub async fn toggle_pipeline_processing(
    state: State<'_, std::sync::Arc<AppState>>,
    enabled: Option<bool>,
) -> Result<bool, VoxIpcError> {
    let new_paused = match enabled {
        Some(e) => !e,
        None => !state.memory.user_paused_ingestion.load(Ordering::SeqCst),
    };

    state
        .memory
        .user_paused_ingestion
        .store(new_paused, Ordering::SeqCst);

    if let Ok(mut settings) = state.settings.write() {
        settings.memory.pipeline_processing_enabled = !new_paused;
        if let Err(e) = settings.save() {
            log::warn!("[Memory::Ingestion] Failed to save settings: {}", e);
        }
    }

    log::info!(
        "[Memory] Pipeline processing state updated: enabled={}",
        !new_paused
    );
    Ok(!new_paused)
}

/// Reset failed memory queue items to staged_pending for retry (all items if item_ids is None/empty).
#[tauri::command]
pub async fn retry_failed_queue_items(
    state: State<'_, std::sync::Arc<AppState>>,
    item_ids: Option<Vec<i64>>,
) -> Result<u32, VoxIpcError> {
    if state.memory.user_paused_ingestion.load(Ordering::SeqCst) {
        return Err(VoxIpcError::InvalidState(
            "Memory pipeline processing is currently paused. Please enable processing before retrying.".to_string(),
        ));
    }

    let db_path = crate::utils::paths::get().db.clone();
    let conn = VoxDb::open(&db_path)
        .await
        .map_err(|e| VoxIpcError::Database(format!("DB open failed: {}", e)))?;

    let affected = crate::persistence::memory_mutations::retry_failed_queue_items(&conn, item_ids)
        .await
        .map_err(|e| VoxIpcError::Database(e.to_string()))?;

    state.memory.graph_version.fetch_add(1, Ordering::SeqCst);
    Ok(affected as u32)
}
