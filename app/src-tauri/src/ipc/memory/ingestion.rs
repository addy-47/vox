use crate::core::state::AppState;
use crate::persistence::db::VoxDb;
use serde::Serialize;
use std::sync::atomic::Ordering;
use tauri::State;

#[derive(Debug, Serialize, Clone)]
pub struct MemoryRelationEntry {
    pub id: i64,
    pub from_id: String,
    pub to_id: String,
    pub relation: String,
    pub source: String,
    pub created_at: i64,
}

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

#[derive(Debug, Serialize, Clone)]
pub struct MemoryQueueSummary {
    pub staged_pending: u32,
    pub dedup_pass: u32,
    pub nli_evaluated: u32,
    pub paused: u32,
    pub failed: u32,
    pub recent_items: Vec<MemoryQueueItem>,
}

#[tauri::command]
pub async fn get_memory_relations(
    _state: State<'_, std::sync::Arc<AppState>>,
) -> Result<Vec<MemoryRelationEntry>, String> {
    let db_path = crate::utils::paths::get().db.clone();
    let conn = VoxDb::open_readonly(&db_path)
        .await
        .map_err(|e| format!("DB open failed: {}", e))?;

    let mut rows = conn
        .query(
            "SELECT id, from_id, to_id, relation, source, created_at FROM memory_relations ORDER BY id ASC",
            (),
        )
        .await
        .map_err(|e| e.to_string())?;

    let mut relations = Vec::new();
    while let Some(row) = rows.next().await.map_err(|e| e.to_string())? {
        relations.push(MemoryRelationEntry {
            id: row.get(0).map_err(|e| e.to_string())?,
            from_id: row.get(1).map_err(|e| e.to_string())?,
            to_id: row.get(2).map_err(|e| e.to_string())?,
            relation: row.get(3).map_err(|e| e.to_string())?,
            source: row.get(4).map_err(|e| e.to_string())?,
            created_at: row.get(5).map_err(|e| e.to_string())?,
        });
    }

    Ok(relations)
}

#[tauri::command]
pub async fn get_memory_queue_status(
    _state: State<'_, std::sync::Arc<AppState>>,
) -> Result<MemoryQueueSummary, String> {
    let db_path = crate::utils::paths::get().db.clone();
    let conn = VoxDb::open_readonly(&db_path)
        .await
        .map_err(|e| format!("DB open failed: {}", e))?;

    let mut rows = conn
        .query(
            "SELECT status, count(*) FROM personal_memory_queue GROUP BY status",
            (),
        )
        .await
        .map_err(|e| e.to_string())?;

    let mut staged_pending = 0;
    let mut dedup_pass = 0;
    let mut nli_evaluated = 0;
    let mut paused = 0;
    let mut failed = 0;

    while let Some(row) = rows.next().await.map_err(|e| e.to_string())? {
        let status_str: String = row.get(0).map_err(|e| e.to_string())?;
        let cnt: u32 = row.get::<i64>(1).unwrap_or(0) as u32;
        match status_str.as_str() {
            "staged_pending" => staged_pending = cnt,
            "dedup_pass" => dedup_pass = cnt,
            "nli_evaluated" => nli_evaluated = cnt,
            "paused" => paused = cnt,
            "failed" => failed = cnt,
            _ => {}
        }
    }

    let mut rows = conn
        .query(
            "SELECT id, fact, collection, source, session_id, status, attempts, error_msg, created_at 
             FROM personal_memory_queue ORDER BY id DESC LIMIT 50",
            (),
        )
        .await
        .map_err(|e| e.to_string())?;

    let mut recent_items = Vec::new();
    while let Some(row) = rows.next().await.map_err(|e| e.to_string())? {
        recent_items.push(MemoryQueueItem {
            id: row.get(0).map_err(|e| e.to_string())?,
            fact: row.get(1).map_err(|e| e.to_string())?,
            collection: row.get(2).map_err(|e| e.to_string())?,
            source: row.get(3).map_err(|e| e.to_string())?,
            session_id: row.get(4).map_err(|e| e.to_string())?,
            status: row.get(5).map_err(|e| e.to_string())?,
            attempts: row.get::<i64>(6).unwrap_or(0) as u32,
            error_msg: row.get(7).ok(),
            created_at: row.get(8).map_err(|e| e.to_string())?,
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

#[tauri::command]
pub async fn toggle_pipeline_processing(
    state: State<'_, std::sync::Arc<AppState>>,
) -> Result<bool, String> {
    let current = state.memory.pipeline_paused.load(Ordering::SeqCst);
    let new_state = !current;
    state
        .memory
        .pipeline_paused
        .store(new_state, Ordering::SeqCst);

    if let Ok(mut settings) = state.settings.write() {
        settings.memory.pipeline_processing_enabled = !new_state;
    }

    Ok(!new_state)
}

#[tauri::command]
pub async fn trigger_memory_consolidation(
    state: State<'_, std::sync::Arc<AppState>>,
) -> Result<u32, String> {
    let db_path = crate::utils::paths::get().db.clone();
    let conn = VoxDb::open(&db_path)
        .await
        .map_err(|e| format!("DB open failed: {}", e))?;

    let mut compacted_count = 0;
    let cancel_flag = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));

    loop {
        match crate::services::memory::pipeline::run_pipeline_cycle(&conn, &cancel_flag).await {
            Ok(n) if n > 0 => {
                compacted_count += n;
            }
            Ok(_) => {
                break;
            }
            Err(e) => {
                return Err(format!("Pipeline processing failed: {}", e));
            }
        }
    }

    state.memory.graph_version.fetch_add(1, Ordering::SeqCst);
    Ok(compacted_count as u32)
}

#[tauri::command]
pub async fn retry_failed_queue(state: State<'_, std::sync::Arc<AppState>>) -> Result<u32, String> {
    let db_path = crate::utils::paths::get().db.clone();
    let conn = VoxDb::open(&db_path)
        .await
        .map_err(|e| format!("DB open failed: {}", e))?;

    let affected = conn
        .execute(
            "UPDATE personal_memory_queue 
             SET status = 'staged_pending', attempts = 0, retry_count = 0, error_msg = NULL 
             WHERE status = 'failed'",
            (),
        )
        .await
        .map_err(|e| e.to_string())?;

    state.memory.graph_version.fetch_add(1, Ordering::SeqCst);
    Ok(affected as u32)
}

#[tauri::command]
pub async fn retry_failed_queue_items(
    state: State<'_, std::sync::Arc<AppState>>,
    item_ids: Vec<i64>,
) -> Result<u32, String> {
    if item_ids.is_empty() {
        return Ok(0);
    }
    let db_path = crate::utils::paths::get().db.clone();
    let conn = VoxDb::open(&db_path)
        .await
        .map_err(|e| format!("DB open failed: {}", e))?;

    let placeholders = item_ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
    let sql = format!(
        "UPDATE personal_memory_queue 
         SET status = 'staged_pending', attempts = 0, retry_count = 0, error_msg = NULL 
         WHERE status = 'failed' AND id IN ({})",
        placeholders
    );

    // Convert item_ids to parameters
    let params: Vec<turso::Value> = item_ids.into_iter().map(|id| id.into()).collect();
    let affected = conn
        .execute(&sql, params)
        .await
        .map_err(|e| e.to_string())?;

    state.memory.graph_version.fetch_add(1, Ordering::SeqCst);
    Ok(affected as u32)
}
