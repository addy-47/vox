use anyhow::Result;
use std::collections::{HashMap, HashSet};
use turso::Connection;
use crate::core::settings::MemorySettings;
use crate::services::memory::estimate_tokens;
use crate::core::constants::PM_SEMANTIC_COLLECTIONS;
use crate::persistence::repository;

#[derive(Debug, Clone)]
pub struct MemoryFact {
    pub id: String,
    pub fact_type: String,   // 'foundational', 'operational', 'semantic'
    pub collection: String,
    pub fact: String,
    pub source: String,
    pub status: String,      // 'active', 'superseded', 'deleted'
    pub created_at: i64,
}

/// Phase 4 Budgeted RAG Retrieval & Context Assembly (v5 §5.1 / §5.3 Phase 4).
/// Assembles Tier 1 Foundational/Operational and Tier 2 Semantic Profiles into <user_profile>.
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

    let mut tier1_budget = (context_size as f32 * settings.foundational_budget_share) as usize;
    let mut tier2_budget = (context_size as f32 * settings.semantic_budget_share) as usize;

    if context_size >= 1066 {
        tier1_budget = tier1_budget.max(80);
        tier2_budget = tier2_budget.max(80);
    } else {
        let overall_budget = (context_size as f32 * (settings.foundational_budget_share + settings.semantic_budget_share)) as usize;
        tier1_budget = (overall_budget as f32 * (7.0 / 15.0)) as usize;
        tier2_budget = overall_budget.saturating_sub(tier1_budget);
    }

    // ─── Tier 1: Foundational + Operational (7% hard cap) ───────────

    let foundational_facts = repository::fetch_foundational_facts(conn).await?;
    let operational_facts = repository::fetch_operational_facts(conn).await?;

    let mut tier1_used_tokens = 0;
    let mut identity_block = String::new();
    let mut constraints_block = String::new();

    for fact in &foundational_facts {
        let line = format!("- {}\n", fact.fact);
        let tokens = estimate_tokens(&line);
        if tier1_used_tokens + tokens <= tier1_budget {
            match fact.collection.as_str() {
                "Identity" => {
                    identity_block.push_str(&line);
                    tier1_used_tokens += tokens;
                }
                "Constraints" => {
                    constraints_block.push_str(&line);
                    tier1_used_tokens += tokens;
                }
                _ => {}
            }
        }
    }

    let mut budgeted_tasks = Vec::new();
    let mut budgeted_goals = Vec::new();

    for fact in &operational_facts {
        let line = format!("- {}\n", fact.fact);
        let tokens = estimate_tokens(&line);
        if tier1_used_tokens + tokens <= tier1_budget {
            match fact.collection.as_str() {
                "Tasks" => {
                    budgeted_tasks.push(fact.clone());
                    tier1_used_tokens += tokens;
                }
                "Goals" => {
                    budgeted_goals.push(fact.clone());
                    tier1_used_tokens += tokens;
                }
                _ => {}
            }
        }
    }

    budgeted_tasks.sort_by(|a, b| a.created_at.cmp(&b.created_at));
    budgeted_goals.sort_by(|a, b| a.created_at.cmp(&b.created_at));

    let mut tasks_block = String::new();
    for fact in budgeted_tasks {
        tasks_block.push_str(&format!("- {}\n", fact.fact));
    }

    let mut goals_block = String::new();
    for fact in budgeted_goals {
        goals_block.push_str(&format!("- {}\n", fact.fact));
    }

    let context_remaining = tier1_budget.saturating_sub(tier1_used_tokens);
    let context_block = fetch_context_chain(conn, settings.context_chaining_window_hours, context_remaining).await?;

    // ─── Tier 2: Semantic Profiles (8% hard cap) ────────────────────

    let query_blob = repository::encode_f32_blob(query_embedding);
    let limit = settings.personal_top_k_per_semantic_collection as i64;
    let mut collection_buckets: HashMap<String, Vec<MemoryFact>> = HashMap::new();

    let mut rows = conn
        .query(
            "WITH Ranked AS (
                 SELECT mf.id, mf.type, mf.collection, mf.fact, mf.source, mf.status, mf.created_at,
                        ROW_NUMBER() OVER (
                            PARTITION BY mfv.collection
                            ORDER BY vector_distance_cos(mfv.embedding, ?) ASC
                        ) as rank
                 FROM memory_facts mf
                 JOIN memory_facts_vectors mfv ON mfv.fact_id = mf.id
                 WHERE mfv.collection IN ('Preferences', 'Relationships', 'Skills', 'Projects', 'Experiences')
                   AND mf.status = 'active'
             )
             SELECT id, type, collection, fact, source, status, created_at
             FROM Ranked
             WHERE rank <= ?",
            (query_blob, limit),
        )
        .await?;

    while let Some(row) = rows.next().await? {
        let fact = MemoryFact {
            id: row.get(0)?,
            fact_type: row.get(1)?,
            collection: row.get(2)?,
            fact: row.get(3)?,
            source: row.get(4)?,
            status: row.get(5)?,
            created_at: row.get(6)?,
        };
        collection_buckets.entry(fact.collection.clone()).or_default().push(fact);
    }

    let mut candidate_map: HashMap<String, MemoryFact> = HashMap::new();
    let mut direct_hit_ids = HashSet::new();

    for (_, bucket) in &collection_buckets {
        for fact in bucket {
            direct_hit_ids.insert(fact.id.clone());
            candidate_map.insert(fact.id.clone(), fact.clone());
        }
    }

    if !candidate_map.is_empty() {
        let resolved = resolve_edges(conn, candidate_map, &direct_hit_ids, settings, tauri_app).await?;
        collection_buckets.clear();
        for fact in resolved {
            collection_buckets.entry(fact.collection.clone()).or_default().push(fact);
        }
    }

    let collection_keys: Vec<String> = PM_SEMANTIC_COLLECTIONS
        .iter()
        .filter(|c| collection_buckets.contains_key(**c))
        .map(|c| c.to_string())
        .collect();

    let mut selected_semantic_facts: Vec<MemoryFact> = Vec::new();
    let mut tier2_used_tokens = 0;
    let mut round = 0;

    loop {
        let mut added_any = false;
        for col in &collection_keys {
            if let Some(bucket) = collection_buckets.get(col) {
                if round < bucket.len() {
                    let fact = &bucket[round];
                    let line = format!("- {}\n", fact.fact);
                    let tokens = estimate_tokens(&line);
                    if tier2_used_tokens + tokens <= tier2_budget {
                        tier2_used_tokens += tokens;
                        selected_semantic_facts.push(fact.clone());
                        added_any = true;
                    }
                }
            }
        }
        round += 1;
        if !added_any || tier2_used_tokens >= tier2_budget {
            break;
        }
    }

    selected_semantic_facts.sort_by(|a, b| a.created_at.cmp(&b.created_at));

    // ─── Prompt Assembly ────────────────────────────────────────────

    let has_any = !identity_block.is_empty()
        || !constraints_block.is_empty()
        || !tasks_block.is_empty()
        || !goals_block.is_empty()
        || !context_block.is_empty()
        || !selected_semantic_facts.is_empty();

    if !has_any {
        return Ok(String::new());
    }

    let mut conflict_block = String::new();
    let mut similarity_block = String::new();

    if !selected_semantic_facts.is_empty() {
        let selected_ids: HashSet<String> = selected_semantic_facts.iter().map(|f| f.id.clone()).collect();
        let mut rel_rows = conn.query("SELECT from_id, to_id, relation FROM memory_relations", ()).await?;
        let mut printed_pairs = HashSet::new();

        while let Some(row) = rel_rows.next().await? {
            let from_id: String = row.get(0)?;
            let to_id: String = row.get(1)?;
            let relation: String = row.get(2)?;

            if selected_ids.contains(&from_id) && selected_ids.contains(&to_id) {
                let mut pair = (from_id.clone(), to_id.clone());
                if pair.0 > pair.1 {
                    std::mem::swap(&mut pair.0, &mut pair.1);
                }
                if printed_pairs.contains(&pair) {
                    continue;
                }
                printed_pairs.insert(pair);

                let fact_a = selected_semantic_facts.iter().find(|f| f.id == from_id).map(|f| f.fact.as_str()).unwrap_or("");
                let fact_b = selected_semantic_facts.iter().find(|f| f.id == to_id).map(|f| f.fact.as_str()).unwrap_or("");

                if !fact_a.is_empty() && !fact_b.is_empty() {
                    match relation.as_str() {
                        "CONFLICTS" => {
                            conflict_block.push_str(&format!("- [Unresolved Conflict] \"{}\" CONFLICTS WITH \"{}\"\n", fact_a, fact_b));
                        }
                        "SIMILAR" => {
                            similarity_block.push_str(&format!("- [Unresolved Similarity] \"{}\" is SIMILAR TO \"{}\"\n", fact_a, fact_b));
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    let mut out = String::new();
    out.push_str("<user_profile>\n");

    if !conflict_block.is_empty() {
        out.push_str("[Unresolved Contradictions]\n");
        out.push_str(&conflict_block);
    }
    if !similarity_block.is_empty() {
        out.push_str("[Unresolved Near-Duplicates]\n");
        out.push_str(&similarity_block);
    }

    if !identity_block.is_empty() {
        out.push_str("[Identity]\n");
        out.push_str(&identity_block);
    }
    if !constraints_block.is_empty() {
        out.push_str("[Constraints]\n");
        out.push_str(&constraints_block);
    }
    if !tasks_block.is_empty() {
        out.push_str("[Active Tasks]\n");
        out.push_str(&tasks_block);
    }
    if !goals_block.is_empty() {
        out.push_str("[Active Goals]\n");
        out.push_str(&goals_block);
    }
    if !context_block.is_empty() {
        out.push_str(&context_block);
    }

    let mut semantic_by_collection: HashMap<String, Vec<&MemoryFact>> = HashMap::new();
    for fact in &selected_semantic_facts {
        semantic_by_collection.entry(fact.collection.clone()).or_default().push(fact);
    }

    for collection in PM_SEMANTIC_COLLECTIONS {
        if let Some(facts) = semantic_by_collection.get(*collection) {
            out.push_str(&format!("[{}]\n", collection));
            for fact in facts {
                let ts_label = format_relative_timestamp(fact.created_at);
                out.push_str(&format!("- [{}] {}\n", ts_label, fact.fact));
            }
        }
    }

    out.push_str("</user_profile>");
    Ok(out)
}

/// Time-Windowed Context Chaining (v5 §5.1 / §5.3).
async fn fetch_context_chain(
    conn: &Connection,
    window_hours: u32,
    budget_tokens: usize,
) -> Result<String> {
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;
    let window_start = now_ms - (window_hours as i64 * 3600 * 1000);

    let mut rows = conn
        .query(
            "SELECT fact, created_at FROM memory_facts
             WHERE type = 'operational' AND collection = 'Context' AND status = 'active'
               AND created_at >= ?
             ORDER BY created_at DESC",
            (window_start,),
        )
        .await?;

    let mut contexts: Vec<(String, i64)> = Vec::new();
    while let Some(row) = rows.next().await? {
        let fact: String = row.get(0)?;
        let created_at: i64 = row.get(1)?;
        contexts.push((fact, created_at));
    }

    if contexts.is_empty() {
        let mut fallback_rows = conn
            .query(
                "SELECT fact, created_at FROM memory_facts
                 WHERE type = 'operational' AND collection = 'Context' AND status = 'active'
                   AND created_at < ?
                 ORDER BY created_at DESC LIMIT 1",
                (window_start,),
            )
            .await?;

        if let Some(row) = fallback_rows.next().await? {
            let fact: String = row.get(0)?;
            let created_at: i64 = row.get(1)?;
            let ts_label = format_relative_timestamp(created_at);
            return Ok(format!(
                "[Recollection (Distant Memory)]\n- {}: {}\n",
                ts_label, fact
            ));
        }

        return Ok(String::new());
    }

    let header = format!("[Past Contexts within the Last {} Hours]\n", window_hours);
    let mut used_tokens = estimate_tokens(&header);
    let mut selected_contexts = Vec::new();

    for (fact, created_at) in &contexts {
        let ts_label = format_relative_timestamp(*created_at);
        let line = format!("- {}:\n  {}\n", ts_label, fact);
        let tokens = estimate_tokens(&line);
        if used_tokens + tokens > budget_tokens && used_tokens > 0 {
            break;
        }
        selected_contexts.push((ts_label, fact.clone()));
        used_tokens += tokens;
    }

    let mut block = header;
    for (ts_label, fact) in selected_contexts.into_iter().rev() {
        block.push_str(&format!("- {}:\n  {}\n", ts_label, fact));
    }

    Ok(block)
}

/// Runs Edge Resolution over retrieved candidate facts in Rust memory (v5 §5.3 Phase 4).
pub async fn resolve_edges(
    conn: &Connection,
    mut candidate_map: HashMap<String, MemoryFact>,
    direct_hit_ids: &HashSet<String>,
    _settings: &MemorySettings,
    tauri_app: Option<&tauri::AppHandle>,
) -> Result<Vec<MemoryFact>> {
    let now_inst = std::time::Instant::now();

    let mut rows = conn
        .query(
            "SELECT from_id, to_id, relation FROM memory_relations",
            (),
        )
        .await?;

    let mut supersedes_map: HashMap<String, String> = HashMap::new();
    let mut supports_map: HashMap<String, Vec<String>> = HashMap::new();
    let mut conflicts_set: HashSet<(String, String)> = HashSet::new();

    while let Some(row) = rows.next().await? {
        let from_id: String = row.get(0)?;
        let to_id: String = row.get(1)?;
        let relation: String = row.get(2)?;

        match relation.as_str() {
            "USER_SUPERSEDES" => {
                supersedes_map.insert(to_id, from_id);
            }
            "SUPPORTS" => {
                supports_map.entry(to_id).or_default().push(from_id);
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

    let mut superseded_swaps = HashMap::new();
    let mut required_ids = HashSet::new();

    for id in candidate_map.keys() {
        let mut current_id = id.clone();
        let mut visited = HashSet::new();
        visited.insert(current_id.clone());
        let mut depth = 0;

        while let Some(newer_id) = supersedes_map.get(&current_id) {
            if visited.contains(newer_id) || depth >= 10 {
                log::warn!("[MemoryRetrieval] Cycle or excessive depth detected in USER_SUPERSEDES for: {}", id);
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

    let missing_ids: Vec<String> = required_ids
        .iter()
        .filter(|id| !candidate_map.contains_key(*id))
        .cloned()
        .collect();

    if !missing_ids.is_empty() {
        let placeholders = vec!["?"; missing_ids.len()].join(",");
        let query_str = format!(
            "SELECT id, type, collection, fact, source, status, created_at FROM memory_facts WHERE id IN ({}) AND status = 'active'",
            placeholders
        );

        let mut fact_rows = conn.query(&query_str, missing_ids).await?;
        while let Some(row) = fact_rows.next().await? {
            let fact = MemoryFact {
                id: row.get(0)?,
                fact_type: row.get(1)?,
                collection: row.get(2)?,
                fact: row.get(3)?,
                source: row.get(4)?,
                status: row.get(5)?,
                created_at: row.get(6)?,
            };
            candidate_map.insert(fact.id.clone(), fact);
        }
    }

    for old_id in superseded_swaps.keys() {
        candidate_map.remove(old_id);
    }

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

    let mut resolved: Vec<MemoryFact> = candidate_map.into_values().collect();
    resolved.sort_by(|a, b| b.created_at.cmp(&a.created_at));

    log::info!("[MemoryRetrieval] Edge resolution completed in {}ms", now_inst.elapsed().as_millis());
    Ok(resolved)
}

/// Formats a millisecond epoch timestamp as a human-readable relative time label.
pub fn format_relative_timestamp(created_at_ms: i64) -> String {
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;
    let diff_ms = now_ms - created_at_ms;

    if diff_ms < 0 {
        return "Just now".to_string();
    }

    let minutes = diff_ms / 60_000;
    let hours = diff_ms / 3_600_000;
    let days = diff_ms / 86_400_000;
    let weeks = days / 7;

    if minutes < 1 {
        "Just now".to_string()
    } else if minutes < 60 {
        format!("{} minute{} ago", minutes, if minutes == 1 { "" } else { "s" })
    } else if hours < 24 {
        format!("{} hour{} ago", hours, if hours == 1 { "" } else { "s" })
    } else if days == 1 {
        "Yesterday".to_string()
    } else if days < 7 {
        format!("{} days ago", days)
    } else if weeks < 4 {
        format!("{} week{} ago", weeks, if weeks == 1 { "" } else { "s" })
    } else {
        format!("{} days ago", days)
    }
}
