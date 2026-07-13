use crate::core::state::AppState;
use crate::persistence::db::VoxDb;
use serde::Serialize;
use tauri::State;

#[derive(Debug, Serialize, Clone)]
pub struct ProfileEntry {
    pub key: String,
    pub category: String,
    pub value: String,
    pub updated_at: i64,
}

#[derive(Debug, Serialize, Clone)]
pub struct MemoryStats {
    pub pending_sessions: u32,
    pub embedded_sessions: u32,
    pub total_episodes: u32,
    pub personal_memories: u32,
    pub history_entries: u32,
}

#[derive(Debug, Serialize, Clone)]
pub struct HistoryEntry {
    pub id: i64,
    pub key: String,
    pub category: String,
    pub value: String,
    pub recorded_at: i64,
}

#[tauri::command]
pub async fn get_personal_profile(
    _state: State<'_, std::sync::Arc<AppState>>,
) -> Result<Vec<ProfileEntry>, String> {
    let db_path = crate::utils::paths::get().db.clone();
    let conn = VoxDb::open_readonly(&db_path)
        .await
        .map_err(|e| format!("DB open failed: {}", e))?;

    let mut rows = conn
        .query(
            "SELECT key, category, value, updated_at FROM personal_memory ORDER BY category, key",
            (),
        )
        .await
        .map_err(|e| e.to_string())?;

    let mut profile = Vec::new();
    while let Some(row) = rows.next().await.map_err(|e| e.to_string())? {
        profile.push(ProfileEntry {
            key: row.get(0).map_err(|e| e.to_string())?,
            category: row.get(1).map_err(|e| e.to_string())?,
            value: row.get(2).map_err(|e| e.to_string())?,
            updated_at: row.get(3).map_err(|e| e.to_string())?,
        });
    }

    Ok(profile)
}

#[tauri::command]
pub async fn get_memory_stats(
    _state: State<'_, std::sync::Arc<AppState>>,
) -> Result<MemoryStats, String> {
    let db_path = crate::utils::paths::get().db.clone();
    let conn = VoxDb::open_readonly(&db_path)
        .await
        .map_err(|e| format!("DB open failed: {}", e))?;

    // Query pending sessions count
    let mut rows = conn
        .query("SELECT count(*) FROM sessions WHERE embedding_status = 'pending'", ())
        .await
        .map_err(|e| e.to_string())?;
    let pending_sessions = if let Some(row) = rows.next().await.map_err(|e| e.to_string())? {
        row.get::<i64>(0).unwrap_or(0) as u32
    } else {
        0
    };

    // Query embedded sessions count
    let mut rows = conn
        .query("SELECT count(*) FROM sessions WHERE embedding_status = 'embedded'", ())
        .await
        .map_err(|e| e.to_string())?;
    let embedded_sessions = if let Some(row) = rows.next().await.map_err(|e| e.to_string())? {
        row.get::<i64>(0).unwrap_or(0) as u32
    } else {
        0
    };

    // Query total episodes count
    let mut rows = conn
        .query("SELECT count(*) FROM episodes", ())
        .await
        .map_err(|e| e.to_string())?;
    let total_episodes = if let Some(row) = rows.next().await.map_err(|e| e.to_string())? {
        row.get::<i64>(0).unwrap_or(0) as u32
    } else {
        0
    };

    // Query personal memory entries count
    let mut rows = conn
        .query("SELECT count(*) FROM personal_memory", ())
        .await
        .map_err(|e| e.to_string())?;
    let personal_memories = if let Some(row) = rows.next().await.map_err(|e| e.to_string())? {
        row.get::<i64>(0).unwrap_or(0) as u32
    } else {
        0
    };

    // Query history entries count
    let mut rows = conn
        .query("SELECT count(*) FROM personal_memory_history", ())
        .await
        .map_err(|e| e.to_string())?;
    let history_entries = if let Some(row) = rows.next().await.map_err(|e| e.to_string())? {
        row.get::<i64>(0).unwrap_or(0) as u32
    } else {
        0
    };

    Ok(MemoryStats {
        pending_sessions,
        embedded_sessions,
        total_episodes,
        personal_memories,
        history_entries,
    })
}

#[tauri::command]
pub async fn get_memory_history(
    _state: State<'_, std::sync::Arc<AppState>>,
    limit: Option<u32>,
) -> Result<Vec<HistoryEntry>, String> {
    let db_path = crate::utils::paths::get().db.clone();
    let conn = VoxDb::open_readonly(&db_path)
        .await
        .map_err(|e| format!("DB open failed: {}", e))?;

    let query_limit = limit.unwrap_or(30);
    let mut rows = conn
        .query(
            &format!(
                "SELECT id, key, category, value, recorded_at FROM personal_memory_history ORDER BY id DESC LIMIT {}",
                query_limit
            ),
            (),
        )
        .await
        .map_err(|e| e.to_string())?;

    let mut history = Vec::new();
    while let Some(row) = rows.next().await.map_err(|e| e.to_string())? {
        history.push(HistoryEntry {
            id: row.get(0).map_err(|e| e.to_string())?,
            key: row.get(1).map_err(|e| e.to_string())?,
            category: row.get(2).map_err(|e| e.to_string())?,
            value: row.get(3).map_err(|e| e.to_string())?,
            recorded_at: row.get(4).map_err(|e| e.to_string())?,
        });
    }

    Ok(history)
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
    
    loop {
        let active_sess_id = state.conversation_id.load(std::sync::atomic::Ordering::Relaxed);
        
        match crate::persistence::memory_worker::sweep_next_pending_session(&conn, active_sess_id).await {
            Ok(true) => {
                compacted_count += 1;
            }
            Ok(false) => {
                break;
            }
            Err(e) => {
                return Err(format!("Sweep failed: {}", e));
            }
        }
    }
    
    Ok(compacted_count)
}
