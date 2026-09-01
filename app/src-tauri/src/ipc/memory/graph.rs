use crate::core::error::VoxIpcError;
use crate::core::state::AppState;
use crate::persistence::db::VoxDb;
use serde::{Deserialize, Serialize};
use tauri::State;

/// Topology node representing a single fact entity in the memory graph.
#[derive(Debug, Serialize, Clone)]
pub struct MemoryNodeTopology {
    pub id: String,
    pub collection: String,
    pub is_superseded: bool,
    pub created_at: i64,
    pub fact: Option<String>,
}

/// Topology edge representing a relational connection between two memory nodes.
#[derive(Debug, Serialize, Clone)]
pub struct MemoryEdgeTopology {
    pub id: i64,
    pub from_id: String,
    pub to_id: String,
    pub relation: String,
    pub created_at: i64,
}

/// Complete graph topology payload with atomic version counter.
#[derive(Debug, Serialize, Clone)]
pub struct MemoryGraphPayload {
    pub version: u64,
    pub nodes: Vec<MemoryNodeTopology>,
    pub edges: Vec<MemoryEdgeTopology>,
}

/// Query filter for memory graph topology extraction.
#[derive(Debug, Deserialize, Clone)]
pub struct MemoryGraphQueryFilter {
    pub collections: Option<Vec<String>>,
    pub include_inactive: Option<bool>,
}

/// Detailed descriptor for a single memory fact node and its adjacent edges.
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

/// Aggregate memory subsystem statistics for sessions, episodes, and queue sizes.
#[derive(Debug, Serialize, Clone)]
pub struct MemoryStats {
    pub pending_sessions: u32,
    pub embedded_sessions: u32,
    pub total_episodes: u32,
    pub personal_memories: u32,
    pub history_entries: u32,
}

/// Retrieve the current monotonic memory graph version.
#[tauri::command]
pub async fn get_graph_version(
    state: State<'_, std::sync::Arc<AppState>>,
) -> Result<u64, VoxIpcError> {
    Ok(state
        .memory
        .graph_version
        .load(std::sync::atomic::Ordering::SeqCst))
}

fn build_topology_query(filter: Option<&MemoryGraphQueryFilter>) -> (String, Vec<turso::Value>) {
    let include_inactive = filter.and_then(|f| f.include_inactive).unwrap_or(false);
    let collections = filter.and_then(|f| f.collections.as_ref());

    match collections {
        Some(cols) if !cols.is_empty() => {
            let placeholders = cols.iter().map(|_| "?").collect::<Vec<_>>().join(",");
            let base = if include_inactive {
                format!(
                    "SELECT f.id, f.collection, f.created_at, 
                            EXISTS(SELECT 1 FROM memory_relations r WHERE r.to_id = f.id AND r.relation = 'SUPERSEDES') as is_superseded,
                            f.fact
                     FROM memory_facts f
                     WHERE f.fact != '' AND f.collection IN ({})
                     ORDER BY f.collection, f.created_at DESC",
                    placeholders
                )
            } else {
                format!(
                    "SELECT f.id, f.collection, f.created_at, 0 as is_superseded, f.fact
                     FROM memory_facts f
                     WHERE f.fact != '' AND f.id NOT IN (SELECT to_id FROM memory_relations WHERE relation = 'SUPERSEDES')
                       AND f.collection IN ({})
                     ORDER BY f.collection, f.created_at DESC",
                    placeholders
                )
            };
            let vals = cols.iter().map(|c| c.clone().into()).collect();
            (base, vals)
        }
        _ => {
            let base = if include_inactive {
                "SELECT f.id, f.collection, f.created_at, 
                        EXISTS(SELECT 1 FROM memory_relations r WHERE r.to_id = f.id AND r.relation = 'SUPERSEDES') as is_superseded,
                        f.fact
                 FROM memory_facts f
                 WHERE f.fact != ''
                 ORDER BY f.collection, f.created_at DESC"
                    .to_string()
            } else {
                "SELECT f.id, f.collection, f.created_at, 0 as is_superseded, f.fact
                 FROM memory_facts f
                 WHERE f.fact != '' AND f.id NOT IN (SELECT to_id FROM memory_relations WHERE relation = 'SUPERSEDES')
                 ORDER BY f.collection, f.created_at DESC"
                    .to_string()
            };
            (base, Vec::new())
        }
    }
}

async fn fetch_memory_relations(
    conn: &turso::Connection,
    sql: &str,
    params: impl turso::IntoParams,
) -> Result<Vec<MemoryEdgeTopology>, VoxIpcError> {
    let mut rel_rows = conn
        .query(sql, params)
        .await
        .map_err(|e| VoxIpcError::Database(e.to_string()))?;
    let mut edges = Vec::new();
    while let Some(row) = rel_rows
        .next()
        .await
        .map_err(|e| VoxIpcError::Database(e.to_string()))?
    {
        edges.push(MemoryEdgeTopology {
            id: row
                .get(0)
                .map_err(|e| VoxIpcError::Database(e.to_string()))?,
            from_id: row
                .get(1)
                .map_err(|e| VoxIpcError::Database(e.to_string()))?,
            to_id: row
                .get(2)
                .map_err(|e| VoxIpcError::Database(e.to_string()))?,
            relation: row
                .get(3)
                .map_err(|e| VoxIpcError::Database(e.to_string()))?,
            created_at: row
                .get(4)
                .map_err(|e| VoxIpcError::Database(e.to_string()))?,
        });
    }
    Ok(edges)
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

    let (query_str, params) = build_topology_query(filter.as_ref());
    let mut rows = conn
        .query(&query_str, params)
        .await
        .map_err(|e| VoxIpcError::Database(e.to_string()))?;

    let mut nodes = Vec::new();
    while let Some(row) = rows
        .next()
        .await
        .map_err(|e| VoxIpcError::Database(e.to_string()))?
    {
        let is_sup_val: i64 = row.get(3).unwrap_or(0);
        let fact_val: Option<String> = row.get(4).ok();
        nodes.push(MemoryNodeTopology {
            id: row
                .get(0)
                .map_err(|e| VoxIpcError::Database(e.to_string()))?,
            collection: row
                .get(1)
                .map_err(|e| VoxIpcError::Database(e.to_string()))?,
            is_superseded: is_sup_val != 0,
            created_at: row
                .get(2)
                .map_err(|e| VoxIpcError::Database(e.to_string()))?,
            fact: fact_val,
        });
    }

    let all_edges = fetch_memory_relations(
        &conn,
        "SELECT id, from_id, to_id, relation, created_at FROM memory_relations ORDER BY id ASC",
        (),
    )
    .await?;

    let edges = if filter.is_some() {
        let node_ids: std::collections::HashSet<String> =
            nodes.iter().map(|n| n.id.clone()).collect();
        all_edges
            .into_iter()
            .filter(|e| node_ids.contains(&e.from_id) && node_ids.contains(&e.to_id))
            .collect()
    } else {
        all_edges
    };

    let version = state
        .memory
        .graph_version
        .load(std::sync::atomic::Ordering::SeqCst);

    Ok(MemoryGraphPayload {
        version,
        nodes,
        edges,
    })
}

/// Retrieve detailed information for a single memory fact by ID.
#[tauri::command]
pub async fn get_memory_fact_detail(fact_id: String) -> Result<MemoryFactDetail, VoxIpcError> {
    let db_path = crate::utils::paths::get().db.clone();
    let conn = VoxDb::open_readonly(&db_path)
        .await
        .map_err(|e| VoxIpcError::Database(format!("DB open failed: {}", e)))?;

    let mut fact_rows = conn
        .query(
            "SELECT id, collection, fact, source, session_id, created_at FROM memory_facts WHERE id = ?",
            (fact_id.clone(),),
        )
        .await
        .map_err(|e| VoxIpcError::Database(e.to_string()))?;

    let row = fact_rows
        .next()
        .await
        .map_err(|e| VoxIpcError::Database(e.to_string()))?
        .ok_or_else(|| VoxIpcError::NotFound(format!("Memory fact not found: {}", fact_id)))?;

    let id: String = row
        .get(0)
        .map_err(|e| VoxIpcError::Database(e.to_string()))?;
    let collection: String = row
        .get(1)
        .map_err(|e| VoxIpcError::Database(e.to_string()))?;
    let fact: String = row
        .get(2)
        .map_err(|e| VoxIpcError::Database(e.to_string()))?;
    let source: String = row
        .get(3)
        .map_err(|e| VoxIpcError::Database(e.to_string()))?;
    let session_id: String = row
        .get(4)
        .map_err(|e| VoxIpcError::Database(e.to_string()))?;
    let created_at: i64 = row
        .get(5)
        .map_err(|e| VoxIpcError::Database(e.to_string()))?;

    let is_superseded = {
        let mut s_rows = conn
            .query(
                "SELECT 1 FROM memory_relations WHERE to_id = ? AND relation = 'SUPERSEDES'",
                (id.clone(),),
            )
            .await
            .map_err(|e| VoxIpcError::Database(e.to_string()))?;
        s_rows
            .next()
            .await
            .map_err(|e| VoxIpcError::Database(e.to_string()))?
            .is_some()
    };

    let incoming_relations = fetch_memory_relations(
        &conn,
        "SELECT id, from_id, to_id, relation, created_at FROM memory_relations WHERE to_id = ? ORDER BY id ASC",
        (id.clone(),),
    )
    .await?;

    let outgoing_relations = fetch_memory_relations(
        &conn,
        "SELECT id, from_id, to_id, relation, created_at FROM memory_relations WHERE from_id = ? ORDER BY id ASC",
        (id.clone(),),
    )
    .await?;

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
