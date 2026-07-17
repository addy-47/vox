use anyhow::{anyhow, Result};
use std::collections::{HashMap, HashSet};
use turso::Connection;
use crate::core::settings::MemorySettings;
use crate::services::memory::estimate_tokens;
use crate::core::constants::{
    PM_COLLECTIONS, PM_RELATION_USER_SUPERSEDES, PM_SOURCE_USER
};

#[derive(Debug, Clone)]
pub struct MemoryFact {
    pub id: String,
    pub collection: String,
    pub fact: String,
    pub source: String,
    pub created_at: i64,
}

/// Loads always-inject collections (Identity) and perform semantic vector retrieval across other collections.
/// Returns a formatted <user_profile> prompt context block after applying Edge Resolution.
pub async fn retrieve_personal_context(
    conn: &Connection,
    query_embedding: &[f32],
    settings: &MemorySettings,
    context_size: usize,
    tauri_app: Option<&tauri::AppHandle>,
) -> Result<String> {
    if !settings.personal_enabled {
        return Ok(String::new());
    }

    // 1. Fetch always-inject facts (Identity collection)
    let mut identity_facts = fetch_identity_facts(conn).await?;

    // 2. Fetch semantic candidate facts from other collections
    let mut vector_candidates = Vec::new();
    let query_blob = crate::persistence::memory_worker::encode_f32_blob(query_embedding);

    for collection in PM_COLLECTIONS {
        if *collection == "Identity" {
            continue;
        }
        
        // Exclude tool collections (Tasks, Devices, Locations) - ##TODO
        if *collection == "Tasks" || *collection == "Devices" || *collection == "Locations" {
            continue;
        }

        let candidates = fetch_vector_candidates_for_collection(
            conn,
            &query_blob,
            collection,
            settings.personal_top_k_per_collection as i64,
        ).await?;
        
        vector_candidates.extend(candidates);
    }

    // Combine all fetched candidates for Edge Resolution
    let mut candidate_map: HashMap<String, MemoryFact> = HashMap::new();
    let mut direct_hit_ids = HashSet::new();

    for fact in identity_facts.drain(..) {
        candidate_map.insert(fact.id.clone(), fact);
    }
    for fact in vector_candidates {
        direct_hit_ids.insert(fact.id.clone());
        candidate_map.insert(fact.id.clone(), fact);
    }

    // 3. Edge Resolution Phase
    let resolved_candidates = resolve_edges(conn, candidate_map, &direct_hit_ids, settings, tauri_app).await?;

    // 4. Token Budgeting & Formatted Block Construction
    let budget_cap = ((context_size as f32 * settings.personal_max_context_share) as usize).max(120);

    let mut identity_list = Vec::new();
    let mut collection_buckets: HashMap<String, Vec<MemoryFact>> = HashMap::new();

    for fact in resolved_candidates {
        if fact.collection == "Identity" {
            identity_list.push(fact);
        } else {
            collection_buckets.entry(fact.collection.clone()).or_default().push(fact);
        }
    }

    let mut current_tokens = 0;
    let mut identity_block = String::new();

    for fact in identity_list {
        let line = format!("- {}\n", fact.fact);
        identity_block.push_str(&line);
        current_tokens += estimate_tokens(&line);
    }

    let mut selected_vector_facts = Vec::new();
    let mut round = 0;
    let collection_keys: Vec<String> = collection_buckets.keys().cloned().collect();

    loop {
        let mut added_any = false;
        for col in &collection_keys {
            if let Some(bucket) = collection_buckets.get(col) {
                if round < bucket.len() {
                    let fact = &bucket[round];
                    let line = format!("- {}\n", fact.fact);
                    let tokens = estimate_tokens(&line);

                    if current_tokens + tokens <= budget_cap {
                        current_tokens += tokens;
                        selected_vector_facts.push(fact.clone());
                        added_any = true;
                    }
                }
            }
        }
        round += 1;
        if !added_any || current_tokens >= budget_cap {
            break;
        }
    }

    let mut vector_block = String::new();
    for fact in selected_vector_facts {
        vector_block.push_str(&format!("- {}\n", fact.fact));
    }

    if identity_block.is_empty() && vector_block.is_empty() {
        return Ok(String::new());
    }

    let mut out = String::new();
    out.push_str("<user_profile>\n");
    if !identity_block.is_empty() {
        out.push_str("[Identity]\n");
        out.push_str(&identity_block);
    }
    if !vector_block.is_empty() {
        out.push_str("[Context Details]\n");
        out.push_str(&vector_block);
    }
    out.push_str("</user_profile>");

    Ok(out)
}

/// Fetch active identity collection facts directly from database.
async fn fetch_identity_facts(conn: &Connection) -> Result<Vec<MemoryFact>> {
    let mut rows = conn
        .query(
            "SELECT id, collection, fact, source, created_at FROM memory_facts WHERE collection = 'Identity'",
            (),
        )
        .await?;

    let mut list = Vec::new();
    while let Some(row) = rows.next().await? {
        list.push(MemoryFact {
            id: row.get(0)?,
            collection: row.get(1)?,
            fact: row.get(2)?,
            source: row.get(3)?,
            created_at: row.get(4)?,
        });
    }
    Ok(list)
}

/// Fetch candidate facts for a specific collection using native SQL-level vector scans.
async fn fetch_vector_candidates_for_collection(
    conn: &Connection,
    query_blob: &[u8],
    collection: &str,
    limit: i64,
) -> Result<Vec<MemoryFact>> {
    let mut rows = conn
        .query(
            "SELECT mf.id, mf.collection, mf.fact, mf.source, mf.created_at
             FROM memory_facts mf
             JOIN memory_facts_vectors mfv ON mfv.fact_id = mf.id
             WHERE mfv.collection = ?
             ORDER BY vector_distance_cos(mfv.embedding, ?) ASC
             LIMIT ?",
            (collection.to_string(), query_blob.to_vec(), limit),
        )
        .await?;

    let mut list = Vec::new();
    while let Some(row) = rows.next().await? {
        list.push(MemoryFact {
            id: row.get(0)?,
            collection: row.get(1)?,
            fact: row.get(2)?,
            source: row.get(3)?,
            created_at: row.get(4)?,
        });
    }
    Ok(list)
}

/// Runs three-pass Edge Resolution over retrieved candidate facts in Rust memory.
pub async fn resolve_edges(
    conn: &Connection,
    mut candidate_map: HashMap<String, MemoryFact>,
    direct_hit_ids: &HashSet<String>,
    _settings: &MemorySettings,
    tauri_app: Option<&tauri::AppHandle>,
) -> Result<Vec<MemoryFact>> {
    let now_inst = std::time::Instant::now();

    // 1. Fetch ALL relationships in a single fast query
    let mut rows = conn
        .query(
            "SELECT from_id, to_id, relation FROM memory_relations",
            (),
        )
        .await?;

    let mut supersedes_map: HashMap<String, String> = HashMap::new(); // old -> new
    let mut supports_map: HashMap<String, Vec<String>> = HashMap::new(); // from_id -> Vec<to_id>
    let mut conflicts_set: HashSet<(String, String)> = HashSet::new(); // (id_a, id_b) where id_a < id_b

    while let Some(row) = rows.next().await? {
        let from_id: String = row.get(0)?;
        let to_id: String = row.get(1)?;
        let relation: String = row.get(2)?;

        match relation.as_str() {
            "USER_SUPERSEDES" => {
                supersedes_map.insert(to_id, from_id);
            }
            "SUPPORTS" => {
                supports_map.entry(from_id).or_default().push(to_id);
            }
            "CONFLICTS" => {
                let mut pair = (from_id, to_id);
                if pair.0 > pair.1 {
                    std::mem::swap(&mut pair.0, &mut pair.1);
                }
                conflicts_set.insert(pair);
            }
            _ => {}
        }
    }

    // 2. In-memory Pass 1: Resolve USER_SUPERSEDES Pointer Swaps recursively with Cycle Detection
    let mut superseded_swaps = HashMap::new();
    let mut required_ids = HashSet::new();

    for id in candidate_map.keys() {
        let mut current_id = id.clone();
        let mut visited = HashSet::new();
        visited.insert(current_id.clone());
        let mut depth = 0;

        while let Some(newer_id) = supersedes_map.get(&current_id) {
            if visited.contains(newer_id) || depth >= 10 {
                log::warn!("[Memory] Cycle or excessive depth detected in USER_SUPERSEDES for: {}", id);
                break;
            }
            current_id = newer_id.clone();
            visited.insert(current_id.clone());
            depth += 1;
        }

        if current_id != *id {
            superseded_swaps.insert(id.clone(), current_id.clone());
        }
        required_ids.insert(current_id);
    }

    // 3. In-memory Pass 2: Pull Supporting facts for active surviving direct-hits
    let mut supporting_to_pull = HashSet::new();
    for id in direct_hit_ids {
        let active_id = superseded_swaps.get(id).unwrap_or(id);
        if let Some(supporting_ids) = supports_map.get(active_id) {
            for sup_id in supporting_ids {
                required_ids.insert(sup_id.clone());
                supporting_to_pull.insert(sup_id.clone());
            }
        }
    }

    // 4. Batch fetch missing facts in exactly one step
    let missing_ids: Vec<String> = required_ids
        .iter()
        .filter(|id| !candidate_map.contains_key(*id))
        .cloned()
        .collect();

    if !missing_ids.is_empty() {
        let placeholders = vec!["?"; missing_ids.len()].join(",");
        let query_str = format!(
            "SELECT id, collection, fact, source, created_at FROM memory_facts WHERE id IN ({})",
            placeholders
        );

        let mut fact_rows = conn.query(&query_str, missing_ids).await?;
        while let Some(row) = fact_rows.next().await? {
            let fact = MemoryFact {
                id: row.get(0)?,
                collection: row.get(1)?,
                fact: row.get(2)?,
                source: row.get(3)?,
                created_at: row.get(4)?,
            };
            candidate_map.insert(fact.id.clone(), fact);
        }
    }

    // Apply swaps
    for old_id in superseded_swaps.keys() {
        candidate_map.remove(old_id);
    }

    // 5. In-memory Pass 3: CONFLICTS Detection & Shadow Policy
    let mut to_suppress = HashSet::new();
    let final_candidate_ids: Vec<String> = candidate_map.keys().cloned().collect();

    for i in 0..final_candidate_ids.len() {
        for j in (i + 1)..final_candidate_ids.len() {
            let id_a = &final_candidate_ids[i];
            let id_b = &final_candidate_ids[j];

            let mut pair = (id_a.clone(), id_b.clone());
            if pair.0 > pair.1 {
                std::mem::swap(&mut pair.0, &mut pair.1);
            }

            if conflicts_set.contains(&pair) {
                let fact_a = &candidate_map[id_a];
                let fact_b = &candidate_map[id_b];

                let (_winner_id, loser_id) = if fact_a.created_at >= fact_b.created_at {
                    (id_a, id_b)
                } else {
                    (id_b, id_a)
                };

                to_suppress.insert(loser_id.clone());

                if let Some(app) = tauri_app {
                    use tauri::Emitter;
                    let payload = serde_json::json!({
                        "fact_a_id": id_a,
                        "fact_b_id": id_b,
                    });
                    let _ = app.emit("memory:conflict_detected", payload);
                }
            }
        }
    }

    for id in to_suppress {
        candidate_map.remove(&id);
    }

    let mut resolved: Vec<MemoryFact> = candidate_map.into_values().collect();
    resolved.sort_by(|a, b| b.created_at.cmp(&a.created_at));

    log::info!("[Memory] Optimized edge resolution completed in {}ms", now_inst.elapsed().as_millis());
    Ok(resolved)
}

/// Inserts a manually edited user fact and writes a USER_SUPERSEDES edge old → new.
pub async fn supersede_user_fact(
    conn: &Connection,
    old_id: &str,
    new_fact_text: &str,
    collection: &str,
) -> Result<String> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;

    let new_id = format!("mem_{}_{}", now, uuid::Uuid::new_v4().simple());

    crate::services::memory::ensure_embedder_loaded(true)?;
    let embedding = match crate::services::memory::generate_embedding(new_fact_text)? {
        Some(v) => v,
        None => return Err(anyhow!("Failed to generate embedding for edited fact.")),
    };

    conn.execute(
        "INSERT INTO memory_facts (id, collection, fact, source, created_at) VALUES (?, ?, ?, ?, ?)",
        (
            new_id.clone(),
            collection.to_string(),
            new_fact_text.to_string(),
            PM_SOURCE_USER.to_string(),
            now,
        ),
    ).await?;

    let blob_bytes = crate::persistence::memory_worker::encode_f32_blob(&embedding);
    let vector_rowid = conn.execute(
        "INSERT INTO memory_facts_vectors (fact_id, collection, embedding) VALUES (?, ?, ?)",
        (new_id.clone(), collection.to_string(), blob_bytes),
    ).await?;

    conn.execute(
        "UPDATE memory_facts SET embedding_id = ? WHERE id = ?",
        (vector_rowid as i64, new_id.clone()),
    ).await?;

    conn.execute(
        "INSERT INTO memory_relations (from_id, to_id, relation, created_at) VALUES (?, ?, ?, ?)",
        (
            new_id.clone(),
            old_id.to_string(),
            PM_RELATION_USER_SUPERSEDES.to_string(),
            now,
        ),
    ).await?;

    Ok(new_id)
}
