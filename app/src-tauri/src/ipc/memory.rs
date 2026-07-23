use crate::core::state::AppState;
use crate::persistence::db::VoxDb;
use serde::Serialize;
use tauri::State;
use crate::core::constants::{PM_RELATION_USER_SUPERSEDES, PM_RELATION_CONFLICTS, PM_RELATION_SUPPORTS, PM_SOURCE_USER};

#[derive(Debug, Serialize, Clone)]
pub struct MemoryFactEntry {
    pub id: String,
    pub collection: String,
    pub fact: String,
    pub source: String,
    pub created_at: i64,
    pub is_superseded: bool,
    pub conflict_count: u32,
    pub supports_count: u32,
}

#[derive(Debug, Serialize, Clone)]
pub struct MemoryConflict {
    pub fact_a: MemoryFactEntry,
    pub fact_b: MemoryFactEntry,
}

#[derive(Debug, Serialize, Clone)]
pub struct MemoryStats {
    pub pending_sessions: u32,
    pub embedded_sessions: u32,
    pub total_episodes: u32,
    pub personal_memories: u32,
    pub history_entries: u32,
}

#[tauri::command]
pub async fn get_personal_profile(
    _state: State<'_, std::sync::Arc<AppState>>,
) -> Result<Vec<MemoryFactEntry>, String> {
    let db_path = crate::utils::paths::get().db.clone();
    let conn = VoxDb::open_readonly(&db_path)
        .await
        .map_err(|e| format!("DB open failed: {}", e))?;

    // Load active (non-superseded) memory facts
    let mut rows = conn
        .query(
            "SELECT id, collection, fact, source, created_at FROM memory_facts 
             WHERE id NOT IN (SELECT to_id FROM memory_relations WHERE relation = ?) AND fact != ''
             ORDER BY collection, created_at DESC",
            (PM_RELATION_USER_SUPERSEDES.to_string(),),
        )
        .await
        .map_err(|e| e.to_string())?;

    let mut profile = Vec::new();
    while let Some(row) = rows.next().await.map_err(|e| e.to_string())? {
        profile.push(MemoryFactEntry {
            id: row.get(0).map_err(|e| e.to_string())?,
            collection: row.get(1).map_err(|e| e.to_string())?,
            fact: row.get(2).map_err(|e| e.to_string())?,
            source: row.get(3).map_err(|e| e.to_string())?,
            created_at: row.get(4).map_err(|e| e.to_string())?,
            is_superseded: false,
            conflict_count: 0,
            supports_count: 0,
        });
    }

    Ok(profile)
}

#[tauri::command]
pub async fn get_collection_facts(
    _state: State<'_, std::sync::Arc<AppState>>,
    collection: String,
) -> Result<Vec<MemoryFactEntry>, String> {
    let db_path = crate::utils::paths::get().db.clone();
    let conn = VoxDb::open_readonly(&db_path)
        .await
        .map_err(|e| format!("DB open failed: {}", e))?;

    let mut rows = conn
        .query(
            "SELECT id, collection, fact, source, created_at FROM memory_facts 
             WHERE collection = ? AND id NOT IN (SELECT to_id FROM memory_relations WHERE relation = ?) AND fact != ''
             ORDER BY created_at DESC",
            (collection, PM_RELATION_USER_SUPERSEDES.to_string()),
        )
        .await
        .map_err(|e| e.to_string())?;

    let mut facts = Vec::new();
    while let Some(row) = rows.next().await.map_err(|e| e.to_string())? {
        facts.push(MemoryFactEntry {
            id: row.get(0).map_err(|e| e.to_string())?,
            collection: row.get(1).map_err(|e| e.to_string())?,
            fact: row.get(2).map_err(|e| e.to_string())?,
            source: row.get(3).map_err(|e| e.to_string())?,
            created_at: row.get(4).map_err(|e| e.to_string())?,
            is_superseded: false,
            conflict_count: 0,
            supports_count: 0,
        });
    }

    Ok(facts)
}

#[tauri::command]
pub async fn get_memory_graph(
    _state: State<'_, std::sync::Arc<AppState>>,
) -> Result<Vec<MemoryFactEntry>, String> {
    let db_path = crate::utils::paths::get().db.clone();
    let conn = VoxDb::open_readonly(&db_path)
        .await
        .map_err(|e| format!("DB open failed: {}", e))?;

    let mut rows = conn
        .query(
            "SELECT id, collection, fact, source, created_at FROM memory_facts ORDER BY collection, created_at DESC",
            (),
        )
        .await
        .map_err(|e| e.to_string())?;

    let mut graph_entries = Vec::new();
    while let Some(row) = rows.next().await.map_err(|e| e.to_string())? {
        let id: String = row.get(0).map_err(|e| e.to_string())?;
        
        let is_superseded = {
            let mut s_rows = conn.query("SELECT 1 FROM memory_relations WHERE to_id = ? AND relation = ?", (id.clone(), PM_RELATION_USER_SUPERSEDES.to_string())).await.map_err(|e| e.to_string())?;
            s_rows.next().await.map_err(|e| e.to_string())?.is_some()
        };

        let conflict_count = {
            let mut c_rows = conn.query("SELECT count(*) FROM memory_relations WHERE (from_id = ? OR to_id = ?) AND relation = ?", (id.clone(), id.clone(), PM_RELATION_CONFLICTS.to_string())).await.map_err(|e| e.to_string())?;
            c_rows.next().await.map_err(|e| e.to_string())?.map(|r| r.get::<i64>(0).unwrap_or(0) as u32).unwrap_or(0)
        };

        let supports_count = {
            let mut s_rows = conn.query("SELECT count(*) FROM memory_relations WHERE (from_id = ? OR to_id = ?) AND relation = ?", (id.clone(), id.clone(), PM_RELATION_SUPPORTS.to_string())).await.map_err(|e| e.to_string())?;
            s_rows.next().await.map_err(|e| e.to_string())?.map(|r| r.get::<i64>(0).unwrap_or(0) as u32).unwrap_or(0)
        };

        graph_entries.push(MemoryFactEntry {
            id,
            collection: row.get(1).map_err(|e| e.to_string())?,
            fact: row.get(2).map_err(|e| e.to_string())?,
            source: row.get(3).map_err(|e| e.to_string())?,
            created_at: row.get(4).map_err(|e| e.to_string())?,
            is_superseded,
            conflict_count,
            supports_count,
        });
    }

    Ok(graph_entries)
}

#[tauri::command]
pub async fn get_memory_conflicts(
    _state: State<'_, std::sync::Arc<AppState>>,
) -> Result<Vec<MemoryConflict>, String> {
    let db_path = crate::utils::paths::get().db.clone();
    let conn = VoxDb::open_readonly(&db_path)
        .await
        .map_err(|e| format!("DB open failed: {}", e))?;

    // Unresolved conflicts: CONFLICTS edge exists, but neither side is superseded
    let mut rows = conn
        .query(
            "SELECT from_id, to_id FROM memory_relations 
             WHERE relation = ? 
             AND from_id NOT IN (SELECT to_id FROM memory_relations WHERE relation = ?)
             AND to_id NOT IN (SELECT to_id FROM memory_relations WHERE relation = ?)",
            (PM_RELATION_CONFLICTS.to_string(), PM_RELATION_USER_SUPERSEDES.to_string(), PM_RELATION_USER_SUPERSEDES.to_string()),
        )
        .await
        .map_err(|e| e.to_string())?;

    let mut conflicts = Vec::new();
    while let Some(row) = rows.next().await.map_err(|e| e.to_string())? {
        let from_id: String = row.get(0).map_err(|e| e.to_string())?;
        let to_id: String = row.get(1).map_err(|e| e.to_string())?;

        if let (Some(fact_a), Some(fact_b)) = (fetch_fact_entry(&conn, &from_id).await, fetch_fact_entry(&conn, &to_id).await) {
            conflicts.push(MemoryConflict { fact_a, fact_b });
        }
    }

    Ok(conflicts)
}

async fn fetch_fact_entry(conn: &turso::Connection, id: &str) -> Option<MemoryFactEntry> {
    let mut r = conn.query("SELECT id, collection, fact, source, created_at FROM memory_facts WHERE id = ?", (id.to_string(),)).await.ok()?;
    let row = r.next().await.ok()??;
    Some(MemoryFactEntry {
        id: row.get(0).ok()?,
        collection: row.get(1).ok()?,
        fact: row.get(2).ok()?,
        source: row.get(3).ok()?,
        created_at: row.get(4).ok()?,
        is_superseded: false,
        conflict_count: 1,
        supports_count: 0,
    })
}

#[tauri::command]
pub async fn user_edit_memory(
    _state: State<'_, std::sync::Arc<AppState>>,
    old_fact_id: String,
    new_fact: String,
    collection: String,
) -> Result<String, String> {
    let db_path = crate::utils::paths::get().db.clone();
    let conn = VoxDb::open(&db_path)
        .await
        .map_err(|e| format!("DB open failed: {}", e))?;

    crate::persistence::repository::supersede_user_fact(&conn, &old_fact_id, &new_fact, &collection)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn user_delete_memory(
    _state: State<'_, std::sync::Arc<AppState>>,
    fact_id: String,
) -> Result<(), String> {
    let db_path = crate::utils::paths::get().db.clone();
    let conn = VoxDb::open(&db_path)
        .await
        .map_err(|e| format!("DB open failed: {}", e))?;

    // Soft delete creates an empty fact record as a tombstone and supersedes the old fact with it
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;
    let tombstone_id = format!("mem_{}_{}", now, uuid::Uuid::new_v4().simple());

    conn.execute(
        "INSERT INTO memory_facts (id, collection, fact, source, created_at) VALUES (?, 'Identity', '', ?, ?)",
        (tombstone_id.clone(), PM_SOURCE_USER.to_string(), now),
    ).await.map_err(|e| e.to_string())?;

    conn.execute(
        "INSERT INTO memory_relations (from_id, to_id, relation, created_at) VALUES (?, ?, ?, ?)",
        (tombstone_id, fact_id, PM_RELATION_USER_SUPERSEDES.to_string(), now),
    ).await.map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
pub async fn resolve_memory_conflict(
    _state: State<'_, std::sync::Arc<AppState>>,
    winner_id: String,
    loser_id: String,
) -> Result<(), String> {
    let db_path = crate::utils::paths::get().db.clone();
    let conn = VoxDb::open(&db_path)
        .await
        .map_err(|e| format!("DB open failed: {}", e))?;

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;

    // Resolve conflict by having winner supersede the loser
    conn.execute(
        "INSERT INTO memory_relations (from_id, to_id, relation, created_at) VALUES (?, ?, ?, ?)",
        (winner_id, loser_id, PM_RELATION_USER_SUPERSEDES.to_string(), now),
    ).await.map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
pub async fn get_memory_stats(
    _state: State<'_, std::sync::Arc<AppState>>,
) -> Result<MemoryStats, String> {
    let db_path = crate::utils::paths::get().db.clone();
    let conn = VoxDb::open_readonly(&db_path)
        .await
        .map_err(|e| format!("DB open failed: {}", e))?;

    let mut rows = conn
        .query("SELECT count(*) FROM sessions WHERE embedding_status = 'pending'", ())
        .await
        .map_err(|e| e.to_string())?;
    let pending_sessions = if let Some(row) = rows.next().await.map_err(|e| e.to_string())? {
        row.get::<i64>(0).unwrap_or(0) as u32
    } else {
        0
    };

    let mut rows = conn
        .query("SELECT count(*) FROM sessions WHERE embedding_status = 'embedded'", ())
        .await
        .map_err(|e| e.to_string())?;
    let embedded_sessions = if let Some(row) = rows.next().await.map_err(|e| e.to_string())? {
        row.get::<i64>(0).unwrap_or(0) as u32
    } else {
        0
    };

    let mut rows = conn
        .query("SELECT count(*) FROM memory_facts WHERE collection = 'Context'", ())
        .await
        .map_err(|e| e.to_string())?;
    let total_episodes = if let Some(row) = rows.next().await.map_err(|e| e.to_string())? {
        row.get::<i64>(0).unwrap_or(0) as u32
    } else {
        0
    };

    let mut rows = conn
        .query("SELECT count(*) FROM memory_facts WHERE fact != ''", ())
        .await
        .map_err(|e| e.to_string())?;
    let personal_memories = if let Some(row) = rows.next().await.map_err(|e| e.to_string())? {
        row.get::<i64>(0).unwrap_or(0) as u32
    } else {
        0
    };

    let mut rows = conn
        .query("SELECT count(*) FROM personal_memory_queue", ())
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
pub async fn trigger_memory_consolidation(
    state: State<'_, std::sync::Arc<AppState>>,
) -> Result<u32, String> {
    let db_path = crate::utils::paths::get().db.clone();
    let conn = VoxDb::open(&db_path)
        .await
        .map_err(|e| format!("DB open failed: {}", e))?;

    let memory_settings = {
        let s = state.settings.read().unwrap();
        s.memory.clone()
    };

    let mut compacted_count = 0;
    
    loop {
        match crate::services::memory::orchestrator::process_one_queue_item(&conn, &memory_settings).await {
            Ok(crate::services::memory::orchestrator::PipelineOutcome::Merged { .. })
          | Ok(crate::services::memory::orchestrator::PipelineOutcome::Ingested { .. }) => {
                compacted_count += 1;
            }
            Ok(crate::services::memory::orchestrator::PipelineOutcome::NoWork) => {
                break;
            }
            Err(e) => {
                return Err(format!("Queue processing failed: {}", e));
            }
        }
    }
    
    Ok(compacted_count)
}
