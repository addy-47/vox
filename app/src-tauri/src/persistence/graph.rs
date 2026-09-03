use crate::core::error::PersistenceError;
use serde::{Deserialize, Serialize};

/// Topology node representing a single fact entity in the memory graph.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MemoryNodeTopology {
    pub id: String,
    pub collection: String,
    pub is_superseded: bool,
    pub created_at: i64,
    pub fact: Option<String>,
}

/// Topology edge representing a relational connection between two memory nodes.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MemoryEdgeTopology {
    pub id: i64,
    pub from_id: String,
    pub to_id: String,
    pub relation: String,
    pub created_at: i64,
}

/// Complete graph topology payload with atomic version counter.
#[derive(Debug, Serialize, Deserialize, Clone)]
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
#[derive(Debug, Serialize, Deserialize, Clone)]
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

/// A conflict pair between two active or competing memory facts.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MemoryConflictItem {
    pub fact_a: MemoryNodeTopology,
    pub fact_b: MemoryNodeTopology,
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
) -> Result<Vec<MemoryEdgeTopology>, PersistenceError> {
    let mut rel_rows = conn.query(sql, params).await?;
    let mut edges = Vec::new();
    while let Some(row) = rel_rows.next().await? {
        edges.push(MemoryEdgeTopology {
            id: row.get(0)?,
            from_id: row.get(1)?,
            to_id: row.get(2)?,
            relation: row.get(3)?,
            created_at: row.get(4)?,
        });
    }
    Ok(edges)
}

/// Fetch the complete memory graph topology with optional filters.
pub async fn fetch_memory_graph(
    conn: &turso::Connection,
    filter: Option<&MemoryGraphQueryFilter>,
    graph_version: u64,
) -> Result<MemoryGraphPayload, PersistenceError> {
    let (query_str, params) = build_topology_query(filter);
    let mut rows = conn.query(&query_str, params).await?;

    let mut nodes = Vec::new();
    while let Some(row) = rows.next().await? {
        let is_sup_val: i64 = row.get(3).unwrap_or(0);
        let fact_val: Option<String> = row.get(4).ok();
        nodes.push(MemoryNodeTopology {
            id: row.get(0)?,
            collection: row.get(1)?,
            is_superseded: is_sup_val != 0,
            created_at: row.get(2)?,
            fact: fact_val,
        });
    }

    let all_edges = fetch_memory_relations(
        conn,
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

    Ok(MemoryGraphPayload {
        version: graph_version,
        nodes,
        edges,
    })
}

/// Fetch detail for a single fact entity by ID.
pub async fn fetch_fact_detail(
    conn: &turso::Connection,
    fact_id: &str,
) -> Result<Option<MemoryFactDetail>, PersistenceError> {
    let mut fact_rows = conn
        .query(
            "SELECT id, collection, fact, source, session_id, created_at FROM memory_facts WHERE id = ?",
            (fact_id.to_string(),),
        )
        .await?;

    let row = match fact_rows.next().await? {
        Some(r) => r,
        None => return Ok(None),
    };

    let id: String = row.get(0)?;
    let collection: String = row.get(1)?;
    let fact: String = row.get(2)?;
    let source: String = row.get(3)?;
    let session_id: String = row.get(4)?;
    let created_at: i64 = row.get(5)?;

    let is_superseded = {
        let mut s_rows = conn
            .query(
                "SELECT 1 FROM memory_relations WHERE to_id = ? AND relation = 'SUPERSEDES'",
                (id.clone(),),
            )
            .await?;
        s_rows.next().await?.is_some()
    };

    let incoming_relations = fetch_memory_relations(
        conn,
        "SELECT id, from_id, to_id, relation, created_at FROM memory_relations WHERE to_id = ? ORDER BY id ASC",
        (id.clone(),),
    )
    .await?;

    let outgoing_relations = fetch_memory_relations(
        conn,
        "SELECT id, from_id, to_id, relation, created_at FROM memory_relations WHERE from_id = ? ORDER BY id ASC",
        (id.clone(),),
    )
    .await?;

    Ok(Some(MemoryFactDetail {
        id,
        collection,
        fact,
        source,
        session_id,
        created_at,
        is_superseded,
        incoming_relations,
        outgoing_relations,
    }))
}

/// Fetch unresolved conflicts across memory nodes.
pub async fn fetch_memory_conflicts(
    conn: &turso::Connection,
) -> Result<Vec<MemoryConflictItem>, PersistenceError> {
    let sql = "SELECT 
        r.from_id, fa.collection, fa.created_at,
        r.to_id, fb.collection, fb.created_at
     FROM memory_relations r
     JOIN memory_facts fa ON fa.id = r.from_id
     JOIN memory_facts fb ON fb.id = r.to_id
     WHERE r.relation = 'CONFLICTS_WITH'
       AND fa.status = 'active'
       AND fb.status = 'active'
     ORDER BY r.created_at DESC";

    let mut rows = conn.query(sql, ()).await?;

    let mut conflicts = Vec::new();
    while let Some(row) = rows.next().await? {
        let from_id: String = row.get(0)?;
        let from_col: String = row.get(1)?;
        let from_created: i64 = row.get(2)?;

        let to_id: String = row.get(3)?;
        let to_col: String = row.get(4)?;
        let to_created: i64 = row.get(5)?;

        conflicts.push(MemoryConflictItem {
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
