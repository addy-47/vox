use crate::core::state::AppState;
use crate::persistence::db::VoxDb;
use serde::{Deserialize, Serialize};
use tauri::State;

#[derive(Debug, Serialize, Clone)]
pub struct MemoryNodeTopology {
    pub id: String,
    pub collection: String,
    pub is_superseded: bool,
    pub created_at: i64,
    pub fact: Option<String>,
}

#[derive(Debug, Serialize, Clone)]
pub struct MemoryEdgeTopology {
    pub id: i64,
    pub from_id: String,
    pub to_id: String,
    pub relation: String,
    pub created_at: i64,
}

#[derive(Debug, Serialize, Clone)]
pub struct MemoryGraphPayload {
    pub version: u64,
    pub nodes: Vec<MemoryNodeTopology>,
    pub edges: Vec<MemoryEdgeTopology>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct MemoryGraphQueryFilter {
    pub collections: Option<Vec<String>>,
    pub include_inactive: Option<bool>,
}

#[derive(Debug, Serialize, Clone)]
pub struct MemoryFactDetail {
    pub id: String,
    pub collection: String,
    pub fact: String,
    pub source: String,
    pub session_id: String,
    pub created_at: i64,
    pub is_superseded: bool,
    pub incoming_relations: Vec<MemoryEdgeTopology>,
    pub outgoing_relations: Vec<MemoryEdgeTopology>,
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
pub async fn get_graph_version(
    state: State<'_, std::sync::Arc<AppState>>,
) -> Result<u64, String> {
    Ok(state.memory.graph_version.load(std::sync::atomic::Ordering::SeqCst))
}

#[tauri::command]
pub async fn get_memory_graph_topology(
    state: State<'_, std::sync::Arc<AppState>>,
    filter: Option<MemoryGraphQueryFilter>,
) -> Result<MemoryGraphPayload, String> {
    let db_path = crate::utils::paths::get().db.clone();
    let conn = VoxDb::open_readonly(&db_path)
        .await
        .map_err(|e| format!("DB open failed: {}", e))?;

    let include_inactive = filter.as_ref().and_then(|f| f.include_inactive).unwrap_or(false);

    let query_str = if include_inactive {
        "SELECT f.id, f.collection, f.created_at, 
                EXISTS(SELECT 1 FROM memory_relations r WHERE r.to_id = f.id AND r.relation = 'SUPERSEDES') as is_superseded,
                f.fact
         FROM memory_facts f
         WHERE f.fact != ''
         ORDER BY f.collection, f.created_at DESC"
    } else {
        "SELECT f.id, f.collection, f.created_at, 0 as is_superseded, f.fact
         FROM memory_facts f
         WHERE f.fact != '' AND f.id NOT IN (SELECT to_id FROM memory_relations WHERE relation = 'SUPERSEDES')
         ORDER BY f.collection, f.created_at DESC"
    };

    let mut rows = conn.query(query_str, ()).await.map_err(|e| e.to_string())?;

    let mut nodes = Vec::new();
    while let Some(row) = rows.next().await.map_err(|e| e.to_string())? {
        let is_sup_val: i64 = row.get(3).unwrap_or(0);
        let fact_val: Option<String> = row.get(4).ok();
        nodes.push(MemoryNodeTopology {
            id: row.get(0).map_err(|e| e.to_string())?,
            collection: row.get(1).map_err(|e| e.to_string())?,
            is_superseded: is_sup_val != 0,
            created_at: row.get(2).map_err(|e| e.to_string())?,
            fact: fact_val,
        });
    }

    let mut rel_rows = conn
        .query(
            "SELECT id, from_id, to_id, relation, created_at FROM memory_relations ORDER BY id ASC",
            (),
        )
        .await
        .map_err(|e| e.to_string())?;

    let mut edges = Vec::new();
    while let Some(row) = rel_rows.next().await.map_err(|e| e.to_string())? {
        edges.push(MemoryEdgeTopology {
            id: row.get(0).map_err(|e| e.to_string())?,
            from_id: row.get(1).map_err(|e| e.to_string())?,
            to_id: row.get(2).map_err(|e| e.to_string())?,
            relation: row.get(3).map_err(|e| e.to_string())?,
            created_at: row.get(4).map_err(|e| e.to_string())?,
        });
    }

    let version = state.memory.graph_version.load(std::sync::atomic::Ordering::SeqCst);

    Ok(MemoryGraphPayload {
        version,
        nodes,
        edges,
    })
}

#[tauri::command]
pub async fn get_memory_fact_detail(
    _state: State<'_, std::sync::Arc<AppState>>,
    fact_id: String,
) -> Result<MemoryFactDetail, String> {
    let db_path = crate::utils::paths::get().db.clone();
    let conn = VoxDb::open_readonly(&db_path)
        .await
        .map_err(|e| format!("DB open failed: {}", e))?;

    let mut fact_rows = conn
        .query(
            "SELECT id, collection, fact, source, session_id, created_at FROM memory_facts WHERE id = ?",
            (fact_id.clone(),),
        )
        .await
        .map_err(|e| e.to_string())?;

    let row = fact_rows
        .next()
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("Memory fact not found: {}", fact_id))?;

    let id: String = row.get(0).map_err(|e| e.to_string())?;
    let collection: String = row.get(1).map_err(|e| e.to_string())?;
    let fact: String = row.get(2).map_err(|e| e.to_string())?;
    let source: String = row.get(3).map_err(|e| e.to_string())?;
    let session_id: String = row.get(4).map_err(|e| e.to_string())?;
    let created_at: i64 = row.get(5).map_err(|e| e.to_string())?;

    let is_superseded = {
        let mut s_rows = conn
            .query(
                "SELECT 1 FROM memory_relations WHERE to_id = ? AND relation = 'SUPERSEDES'",
                (id.clone(),),
            )
            .await
            .map_err(|e| e.to_string())?;
        s_rows.next().await.map_err(|e| e.to_string())?.is_some()
    };

    let mut inc_rows = conn
        .query(
            "SELECT id, from_id, to_id, relation, created_at FROM memory_relations WHERE to_id = ? ORDER BY id ASC",
            (id.clone(),),
        )
        .await
        .map_err(|e| e.to_string())?;
    let mut incoming_relations = Vec::new();
    while let Some(r) = inc_rows.next().await.map_err(|e| e.to_string())? {
        incoming_relations.push(MemoryEdgeTopology {
            id: r.get(0).map_err(|e| e.to_string())?,
            from_id: r.get(1).map_err(|e| e.to_string())?,
            to_id: r.get(2).map_err(|e| e.to_string())?,
            relation: r.get(3).map_err(|e| e.to_string())?,
            created_at: r.get(4).map_err(|e| e.to_string())?,
        });
    }

    let mut out_rows = conn
        .query(
            "SELECT id, from_id, to_id, relation, created_at FROM memory_relations WHERE from_id = ? ORDER BY id ASC",
            (id.clone(),),
        )
        .await
        .map_err(|e| e.to_string())?;
    let mut outgoing_relations = Vec::new();
    while let Some(r) = out_rows.next().await.map_err(|e| e.to_string())? {
        outgoing_relations.push(MemoryEdgeTopology {
            id: r.get(0).map_err(|e| e.to_string())?,
            from_id: r.get(1).map_err(|e| e.to_string())?,
            to_id: r.get(2).map_err(|e| e.to_string())?,
            relation: r.get(3).map_err(|e| e.to_string())?,
            created_at: r.get(4).map_err(|e| e.to_string())?,
        });
    }

    Ok(MemoryFactDetail {
        id,
        collection,
        fact,
        source,
        session_id,
        created_at,
        is_superseded,
        incoming_relations,
        outgoing_relations,
    })
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
        .query(
            "SELECT 
                (SELECT count(*) FROM sessions WHERE ended_at IS NULL) as pending_sessions,
                (SELECT count(*) FROM sessions WHERE ended_at IS NOT NULL) as embedded_sessions,
                (SELECT count(*) FROM memory_facts WHERE collection = 'Context') as total_episodes,
                (SELECT count(*) FROM memory_facts WHERE fact != '') as personal_memories,
                (SELECT count(*) FROM personal_memory_queue) as history_entries",
            (),
        )
        .await
        .map_err(|e| e.to_string())?;

    if let Some(row) = rows.next().await.map_err(|e| e.to_string())? {
        Ok(MemoryStats {
            pending_sessions: row.get::<i64>(0).unwrap_or(0) as u32,
            embedded_sessions: row.get::<i64>(1).unwrap_or(0) as u32,
            total_episodes: row.get::<i64>(2).unwrap_or(0) as u32,
            personal_memories: row.get::<i64>(3).unwrap_or(0) as u32,
            history_entries: row.get::<i64>(4).unwrap_or(0) as u32,
        })
    } else {
        Ok(MemoryStats {
            pending_sessions: 0,
            embedded_sessions: 0,
            total_episodes: 0,
            personal_memories: 0,
            history_entries: 0,
        })
    }
}
