use crate::core::state::AppState;
use crate::ipc::memory::graph::MemoryNodeTopology;
use crate::persistence::db::VoxDb;
use crate::services::memory::Relation;
use serde::Serialize;
use std::sync::atomic::Ordering;
use tauri::State;

/// Representation of a conflicting fact pair.
#[derive(Debug, Serialize, Clone)]
pub struct MemoryConflict {
    pub fact_a: MemoryNodeTopology,
    pub fact_b: MemoryNodeTopology,
}

/// Retrieve all unresolved memory fact conflicts from the graph.
#[tauri::command]
pub async fn get_unresolved_conflicts() -> Result<Vec<MemoryConflict>, String> {
    let db_path = crate::utils::paths::get().db.clone();
    let conn = VoxDb::open_readonly(&db_path)
        .await
        .map_err(|e| format!("DB open failed: {}", e))?;

    let mut rows = conn
        .query(
            "SELECT r.from_id, f1.collection, f1.created_at,
                    r.to_id, f2.collection, f2.created_at
             FROM memory_relations r
             JOIN memory_facts f1 ON f1.id = r.from_id
             JOIN memory_facts f2 ON f2.id = r.to_id
             WHERE r.relation = ? 
               AND r.from_id NOT IN (SELECT to_id FROM memory_relations WHERE relation = ?)
               AND r.to_id NOT IN (SELECT to_id FROM memory_relations WHERE relation = ?)",
            (
                Relation::Conflicts.as_str().to_string(),
                Relation::Supersedes.as_str().to_string(),
                Relation::Supersedes.as_str().to_string(),
            ),
        )
        .await
        .map_err(|e| e.to_string())?;

    let mut conflicts = Vec::new();
    while let Some(row) = rows.next().await.map_err(|e| e.to_string())? {
        let from_id: String = row.get(0).map_err(|e| e.to_string())?;
        let from_col: String = row.get(1).map_err(|e| e.to_string())?;
        let from_created: i64 = row.get(2).map_err(|e| e.to_string())?;

        let to_id: String = row.get(3).map_err(|e| e.to_string())?;
        let to_col: String = row.get(4).map_err(|e| e.to_string())?;
        let to_created: i64 = row.get(5).map_err(|e| e.to_string())?;

        conflicts.push(MemoryConflict {
            fact_a: MemoryNodeTopology {
                id: from_id,
                collection: from_col,
                is_superseded: false,
                created_at: from_created,
                fact: None,
            },
            fact_b: MemoryNodeTopology {
                id: to_id,
                collection: to_col,
                is_superseded: false,
                created_at: to_created,
                fact: None,
            },
        });
    }

    Ok(conflicts)
}

/// Resolve a memory conflict by marking the loser as superseded and linking the winner.
#[tauri::command]
pub async fn resolve_memory_conflict(
    state: State<'_, std::sync::Arc<AppState>>,
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

    VoxDb::with_transaction(&conn, async {
        conn.execute(
            "UPDATE memory_facts SET status = 'superseded' WHERE id = ?",
            (loser_id.clone(),),
        )
        .await
        .map_err(|e| e.to_string())?;

        conn.execute(
            "INSERT INTO memory_relations (from_id, to_id, relation, source, created_at) VALUES (?, ?, ?, 'USER', ?)",
            (winner_id, loser_id, Relation::Supersedes.as_str().to_string(), now),
        )
        .await
        .map_err(|e| e.to_string())?;

        Ok(())
    })
    .await?;

    state.memory.graph_version.fetch_add(1, Ordering::SeqCst);
    Ok(())
}
