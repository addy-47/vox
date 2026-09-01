use crate::core::error::VoxIpcError;
use crate::core::state::AppState;
use crate::persistence::db::VoxDb;
use serde::Serialize;
use std::sync::atomic::Ordering;
use tauri::State;

/// Memory graph relation table entry.
#[derive(Debug, Serialize, Clone)]
pub struct MemoryRelationEntry {
    pub id: i64,
    pub from_id: String,
    pub to_id: String,
    pub relation: String,
    pub source: String,
    pub created_at: i64,
}

/// Item in the personal memory staging ingestion queue.
#[derive(Debug, Serialize, Clone)]
pub struct MemoryQueueItem {
    pub id: i64,
    pub fact: String,
    pub collection: String,
    pub source: String,
    pub session_id: String,
    pub status: String,
    pub attempts: u32,
    pub error_msg: Option<String>,
    pub created_at: i64,
}

/// Summary counts and recent entries in the memory ingestion queue.
#[derive(Debug, Serialize, Clone)]
pub struct MemoryQueueSummary {
    pub staged_pending: u32,
    pub dedup_pass: u32,
    pub nli_evaluated: u32,
    pub paused: u32,
    pub failed: u32,
    pub recent_items: Vec<MemoryQueueItem>,
}

/// Retrieve queue status counts and the most recent 50 queue items.
#[tauri::command]
pub async fn get_memory_queue_status() -> Result<MemoryQueueSummary, VoxIpcError> {
    let db_path = crate::utils::paths::get().db.clone();
    let conn = VoxDb::open_readonly(&db_path)
        .await
        .map_err(|e| VoxIpcError::Database(format!("DB open failed: {}", e)))?;

    let mut rows = conn
        .query(
            "SELECT status, count(*) FROM personal_memory_queue GROUP BY status",
            (),
        )
        .await
        .map_err(|e| VoxIpcError::Database(e.to_string()))?;

    let mut staged_pending = 0;
    let mut dedup_pass = 0;
    let mut nli_evaluated = 0;
    let mut paused = 0;
    let mut failed = 0;

    while let Some(row) = rows
        .next()
        .await
        .map_err(|e| VoxIpcError::Database(e.to_string()))?
    {
        let status: String = row.get(0).unwrap_or_default();
        let count: i64 = row.get(1).unwrap_or(0);
        match status.as_str() {
            "staged_pending" => staged_pending = count as u32,
            "dedup_pass" => dedup_pass = count as u32,
            "nli_evaluated" => nli_evaluated = count as u32,
            "paused" => paused = count as u32,
            "failed" => failed = count as u32,
            _ => {}
        }
    }

    let mut recent_rows = conn
        .query(
            "SELECT id, fact, collection, source, session_id, status, attempts, error_msg, created_at 
             FROM personal_memory_queue 
             ORDER BY created_at DESC LIMIT 50",
            (),
        )
        .await
        .map_err(|e| VoxIpcError::Database(e.to_string()))?;

    let mut recent_items = Vec::new();
    while let Some(row) = recent_rows
        .next()
        .await
        .map_err(|e| VoxIpcError::Database(e.to_string()))?
    {
        let attempts_i64: i64 = row.get(6).unwrap_or(0);
        recent_items.push(MemoryQueueItem {
            id: row
                .get(0)
                .map_err(|e| VoxIpcError::Database(e.to_string()))?,
            fact: row
                .get(1)
                .map_err(|e| VoxIpcError::Database(e.to_string()))?,
            collection: row
                .get(2)
                .map_err(|e| VoxIpcError::Database(e.to_string()))?,
            source: row
                .get(3)
                .map_err(|e| VoxIpcError::Database(e.to_string()))?,
            session_id: row
                .get(4)
                .map_err(|e| VoxIpcError::Database(e.to_string()))?,
            status: row
                .get(5)
                .map_err(|e| VoxIpcError::Database(e.to_string()))?,
            attempts: attempts_i64 as u32,
            error_msg: row.get(7).ok(),
            created_at: row
                .get(8)
                .map_err(|e| VoxIpcError::Database(e.to_string()))?,
        });
    }

    Ok(MemoryQueueSummary {
        staged_pending,
        dedup_pass,
        nli_evaluated,
        paused,
        failed,
        recent_items,
    })
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

    let affected = match item_ids {
        Some(ids) if !ids.is_empty() => {
            if ids.len() > 1000 {
                return Err(VoxIpcError::InvalidArgument(
                    "Too many items in retry batch. Maximum allowed is 1000.".to_string(),
                ));
            }
            let placeholders = ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
            let sql = format!(
                "UPDATE personal_memory_queue 
                 SET status = 'staged_pending', attempts = 0, retry_count = 0, error_msg = NULL 
                 WHERE status = 'failed' AND id IN ({})",
                placeholders
            );
            let params: Vec<turso::Value> = ids.into_iter().map(|id| id.into()).collect();
            conn.execute(&sql, params)
                .await
                .map_err(|e| VoxIpcError::Database(e.to_string()))?
        }
        _ => conn
            .execute(
                "UPDATE personal_memory_queue 
                 SET status = 'staged_pending', attempts = 0, retry_count = 0, error_msg = NULL 
                 WHERE status = 'failed'",
                (),
            )
            .await
            .map_err(|e| VoxIpcError::Database(e.to_string()))?,
    };

    state.memory.graph_version.fetch_add(1, Ordering::SeqCst);
    Ok(affected as u32)
}
