use anyhow::Result;
use std::collections::{HashMap, HashSet};
use turso::Connection;
use crate::core::settings::MemorySettings;
use crate::services::memory::estimate_tokens;
use crate::core::constants::PM_SEMANTIC_COLLECTIONS;
use crate::persistence::queries;

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

/// Phase 4 Seed-and-Expand Graph Traversal & Context Tree Assembly.
/// Assembles Class A, Class B, and Class C seeds into Global Seed Pool, executes BFS graph expansion,
/// applies dynamic parent quota budgeting, and renders clean prompt tree context into <user_profile>.
pub async fn retrieve_personal_context(
    conn: &Connection,
    query_embedding: &[f32],
    settings: &MemorySettings,
    context_size: usize,
    current_session_id: &str,
    _tauri_app: Option<&tauri::AppHandle>,
) -> Result<String> {
    if !settings.context_retrieval_enabled {
        return Ok(String::new());
    }

    let operational_budget = (context_size as f32 * settings.operational_budget_share) as usize;
    let semantic_budget = (context_size as f32 * settings.semantic_budget_share) as usize;

    // ─── 1. Seed Generation Phase ─────────────────────────────────

    // Class A: Identity & Context (Direct Isolation, Deterministic SQL)
    let foundational_facts = queries::fetch_foundational_facts(conn, current_session_id).await?;
    let context_block = fetch_context_chain(conn, settings.context_chaining_window_hours, operational_budget / 2).await?;

    // Class B: Constraints, Tasks, Goals (Operational State, Top-K per collection)
    let operational_facts = queries::fetch_operational_facts(conn, current_session_id).await?;

    // Class C: Skills, Preferences, Projects, Experiences, Relationships (Semantic Knowledge, ANN Vector Search)
    let semantic_seeds = queries::fetch_semantic_seeds(
        conn,
        query_embedding,
        settings.semantic_similarity_cutoff,
        settings.top_k_facts as i64,
        current_session_id,
    ).await?;

    // ─── 2. Global Seed Pool & Seed Deduplication ────────────────────────────

    let mut visited_fact_ids: HashSet<String> = HashSet::new();
    let mut parent_seeds: Vec<MemoryFact> = Vec::new();

    // Add Identity, Constraints, Tasks, Goals, and Semantic seeds to Global Seed Pool
    for fact in foundational_facts.iter().chain(operational_facts.iter()).chain(semantic_seeds.iter()) {
        if visited_fact_ids.insert(fact.id.clone()) {
            parent_seeds.push(fact.clone());
        }
    }

    if parent_seeds.is_empty() && context_block.is_empty() {
        return Ok(String::new());
    }

    // ─── 3. Bi-directional Seed-and-Expand Traversal (max_hops = 2) ──────────

    let mut frontier: Vec<String> = parent_seeds.iter().map(|f| f.id.clone()).collect();
    let max_hops = settings.max_hops.min(3) as usize;
    let mut child_edges_by_parent: HashMap<String, Vec<(String, MemoryFact)>> = HashMap::new(); // parent_id -> Vec<(relation, child_fact)>
    let mut superseded_target_ids: HashSet<String> = HashSet::new();
    let mut conflict_pairs: Vec<(String, String)> = Vec::new();

    for _hop in 1..=max_hops {
        if frontier.is_empty() {
            break;
        }

        let neighbors = queries::fetch_graph_neighbors(conn, &frontier).await?;
        if neighbors.is_empty() {
            break;
        }

        let mut next_unvisited_ids: Vec<String> = Vec::new();
        let mut edge_mapping: Vec<(String, String, String)> = Vec::new(); // (parent_id, child_id, relation)

        for (from_id, to_id, relation, _source) in neighbors {
            match relation.as_str() {
                "SUPERSEDES" => {
                    // to_id is superseded by from_id -> mark to_id for hard exclusion
                    superseded_target_ids.insert(to_id.clone());
                }
                "CONFLICTS" => {
                    conflict_pairs.push((from_id.clone(), to_id.clone()));
                }
                _ => {
                    let parent_id = if visited_fact_ids.contains(&from_id) {
                        from_id.clone()
                    } else {
                        to_id.clone()
                    };
                    let child_id = if parent_id == from_id { to_id } else { from_id };

                    if !visited_fact_ids.contains(&child_id) {
                        next_unvisited_ids.push(child_id.clone());
                        edge_mapping.push((parent_id, child_id, relation));
                    }
                }
            }
        }

        next_unvisited_ids.sort();
        next_unvisited_ids.dedup();

        if next_unvisited_ids.is_empty() {
            break;
        }

        let fetched_children = queries::fetch_facts_by_ids(conn, &next_unvisited_ids).await?;

        for (parent_id, child_id, relation) in edge_mapping {
            if let Some(child_fact) = fetched_children.get(&child_id) {
                if !superseded_target_ids.contains(&child_id) {
                    child_edges_by_parent.entry(parent_id).or_default().push((relation, child_fact.clone()));
                    visited_fact_ids.insert(child_id);
                }
            }
        }

        frontier = next_unvisited_ids;
    }

    // Filter out superseded seeds
    parent_seeds.retain(|f| !superseded_target_ids.contains(&f.id));

    // ─── 4. Dynamic Fair-Share Parent Budget Allocation with Redistribution (§8.2) ─────────────

    let mut remaining_semantic_budget = semantic_budget;
    let num_parents = parent_seeds.len();

    // ─── 5. Context Manifest Header & Memory Sections ───────────────────────

    let active_counts = queries::fetch_active_collection_counts(conn, current_session_id).await.unwrap_or_default();
    let total_active_facts: usize = active_counts.values().sum();
    let manifest_parts: Vec<String> = PM_SEMANTIC_COLLECTIONS
        .iter()
        .map(|c| format!("{}: {}", c, active_counts.get(*c).copied().unwrap_or(0)))
        .collect();
    let manifest_header = format!(
        "<memory_manifest total_active_facts=\"{}\">\n  {}\n</memory_manifest>\n\n",
        total_active_facts,
        manifest_parts.join(" | ")
    );

    let mut identity_block = String::new();
    let mut constraints_block = String::new();
    let mut tasks_block = String::new();
    let mut goals_block = String::new();
    let mut semantic_block = String::new();
    let mut conflict_block = String::new();

    // Render Conflict Warnings
    let mut printed_conflicts = HashSet::new();
    let all_known_facts = queries::fetch_facts_by_ids(conn, &visited_fact_ids.into_iter().collect::<Vec<_>>()).await?;
    for (a_id, b_id) in conflict_pairs {
        let mut pair = (a_id.clone(), b_id.clone());
        if pair.0 > pair.1 {
            std::mem::swap(&mut pair.0, &mut pair.1);
        }
        if printed_conflicts.insert(pair) {
            if let (Some(fa), Some(fb)) = (all_known_facts.get(&a_id), all_known_facts.get(&b_id)) {
                conflict_block.push_str(&format!(
                    "- [Unresolved Conflict] \"{}\" CONFLICTS WITH \"{}\"\n",
                    fa.fact, fb.fact
                ));
            }
        }
    }

    // Partition Parent Seeds with Dynamic Budget Redistribution (§8.2 point 5)
    for (i, parent) in parent_seeds.iter().enumerate() {
        let remaining_parents = num_parents - i;
        let parent_quota_tokens = (remaining_semantic_budget / remaining_parents.max(1)).max(30);

        let ts_label = format_relative_timestamp(parent.created_at);
        let mut parent_tree = format!("- [{}] {}\n", ts_label, parent.fact);

        let mut used_parent_tokens = estimate_tokens(&parent_tree);

        // Render Child Edges under Dynamic Parent Quota
        if let Some(children) = child_edges_by_parent.get(&parent.id) {
            for (rel, child) in children {
                let child_ts = format_relative_timestamp(child.created_at);
                let child_line = format!("  └─ ({}) -> [{}] {}\n", rel, child_ts, child.fact);
                let child_tokens = estimate_tokens(&child_line);
                if used_parent_tokens + child_tokens <= parent_quota_tokens {
                    parent_tree.push_str(&child_line);
                    used_parent_tokens += child_tokens;
                }
            }
        }

        remaining_semantic_budget = remaining_semantic_budget.saturating_sub(used_parent_tokens);

        match parent.collection.as_str() {
            "Identity" => identity_block.push_str(&format!("- {}\n", parent.fact)),
            "Constraints" => constraints_block.push_str(&parent_tree),
            "Tasks" => tasks_block.push_str(&parent_tree),
            "Goals" => goals_block.push_str(&parent_tree),
            _ => semantic_block.push_str(&format!("[{}]\n{}", parent.collection, parent_tree)),
        }
    }

    // ─── 6. Clean Prompt Tree Assembly (<user_profile>) ─────────────────────

    Ok(format_user_profile_context(
        &manifest_header,
        &conflict_block,
        &identity_block,
        &constraints_block,
        &tasks_block,
        &goals_block,
        &context_block,
        &semantic_block,
    ))
}

/// Time-Windowed Context Chaining.
async fn fetch_context_chain(
    conn: &Connection,
    window_hours: u32,
    budget_tokens: usize,
) -> Result<String> {
    let contexts = queries::fetch_context_records(conn, window_hours).await?;
    if contexts.is_empty() {
        return Ok(String::new());
    }

    // Single distant fallback record check
    if contexts.len() == 1 {
        let (ref fact, created_at) = contexts[0];
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64;
        let window_start = now_ms - (window_hours as i64 * 3600 * 1000);
        if created_at < window_start {
            let ts_label = format_relative_timestamp(created_at);
            return Ok(format!(
                "[Recollection (Distant Memory)]\n- {}: {}\n",
                ts_label, fact
            ));
        }
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

pub use crate::services::memory::formatter::{format_relative_timestamp, format_user_profile_context};

// ─── Pure Vector Search & Ranking Helper Functions ─────────────────────────────

/// Calculates cosine similarity between two float vectors.
pub fn calculate_cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    let mut dot = 0.0f32;
    let mut norm_a = 0.0f32;
    let mut norm_b = 0.0f32;
    for (x, y) in a.iter().zip(b.iter()) {
        dot += x * y;
        norm_a += x * x;
        norm_b += y * y;
    }
    if norm_a <= 0.0 || norm_b <= 0.0 {
        return 0.0;
    }
    dot / (norm_a.sqrt() * norm_b.sqrt())
}

/// Calculates cosine vector distance (1.0 - cosine_similarity).
pub fn calculate_vector_distance_cos(a: &[f32], b: &[f32]) -> f32 {
    1.0 - calculate_cosine_similarity(a, b)
}

/// Represents a candidate vector search result with similarity score.
#[derive(Debug, Clone, PartialEq)]
pub struct VectorSearchResult<T> {
    pub item: T,
    pub collection: String,
    pub similarity: f32,
}

/// Filters vector search candidates by similarity cutoff and ranks them in descending order of similarity score (ascending vector distance).
pub fn filter_and_rank_candidates<T: Clone>(
    candidates: Vec<VectorSearchResult<T>>,
    similarity_threshold: f32,
) -> Vec<VectorSearchResult<T>> {
    let mut filtered: Vec<_> = candidates
        .into_iter()
        .filter(|c| c.similarity >= similarity_threshold)
        .collect();

    // Sort descending by similarity (highest similarity score first)
    filtered.sort_by(|a, b| {
        b.similarity
            .partial_cmp(&a.similarity)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    filtered
}

/// Performs top-k truncation math per collection group on ranked vector candidates.
pub fn truncate_top_k_per_collection<T: Clone>(
    candidates: Vec<VectorSearchResult<T>>,
    top_k_per_collection: usize,
) -> Vec<VectorSearchResult<T>> {
    let mut collection_counts: HashMap<String, usize> = HashMap::new();
    let mut truncated = Vec::new();

    for candidate in candidates {
        let count = collection_counts
            .entry(candidate.collection.clone())
            .or_insert(0);
        if *count < top_k_per_collection {
            *count += 1;
            truncated.push(candidate);
        }
    }

    truncated
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vector_distance_cos_and_similarity_math() {
        let v1 = vec![1.0, 0.0, 0.0];
        let v2 = vec![1.0, 0.0, 0.0];
        let v3 = vec![0.0, 1.0, 0.0];
        let v4 = vec![-1.0, 0.0, 0.0];

        // Identical vectors -> similarity = 1.0, distance = 0.0
        assert!((calculate_cosine_similarity(&v1, &v2) - 1.0).abs() < 1e-6);
        assert!((calculate_vector_distance_cos(&v1, &v2) - 0.0).abs() < 1e-6);

        // Orthogonal vectors -> similarity = 0.0, distance = 1.0
        assert!((calculate_cosine_similarity(&v1, &v3) - 0.0).abs() < 1e-6);
        assert!((calculate_vector_distance_cos(&v1, &v3) - 1.0).abs() < 1e-6);

        // Diametrically opposite vectors -> similarity = -1.0, distance = 2.0
        assert!((calculate_cosine_similarity(&v1, &v4) - (-1.0)).abs() < 1e-6);
        assert!((calculate_vector_distance_cos(&v1, &v4) - 2.0).abs() < 1e-6);

        // Length mismatch or empty -> returns 0.0
        assert_eq!(calculate_cosine_similarity(&v1, &vec![1.0, 0.0]), 0.0);
        assert_eq!(calculate_cosine_similarity(&[], &[]), 0.0);
    }

    #[test]
    fn test_threshold_filtering_logic() {
        let candidates = vec![
            VectorSearchResult {
                item: "fact_high",
                collection: "Skills".to_string(),
                similarity: 0.95,
            },
            VectorSearchResult {
                item: "fact_exact_cutoff",
                collection: "Skills".to_string(),
                similarity: 0.75,
            },
            VectorSearchResult {
                item: "fact_below_cutoff",
                collection: "Skills".to_string(),
                similarity: 0.74,
            },
            VectorSearchResult {
                item: "fact_low",
                collection: "Skills".to_string(),
                similarity: 0.40,
            },
        ];

        let threshold = 0.75;
        let filtered = filter_and_rank_candidates(candidates, threshold);

        assert_eq!(filtered.len(), 2);
        assert_eq!(filtered[0].item, "fact_high");
        assert_eq!(filtered[1].item, "fact_exact_cutoff");
    }

    #[test]
    fn test_vector_distance_ranking_order() {
        let candidates = vec![
            VectorSearchResult {
                item: "item_mid",
                collection: "Preferences".to_string(),
                similarity: 0.80,
            },
            VectorSearchResult {
                item: "item_top",
                collection: "Preferences".to_string(),
                similarity: 0.98,
            },
            VectorSearchResult {
                item: "item_low",
                collection: "Preferences".to_string(),
                similarity: 0.60,
            },
        ];

        let ranked = filter_and_rank_candidates(candidates, 0.50);

        assert_eq!(ranked.len(), 3);
        assert_eq!(ranked[0].item, "item_top");
        assert_eq!(ranked[1].item, "item_mid");
        assert_eq!(ranked[2].item, "item_low");

        // Verify distances are strictly monotonically non-decreasing
        let d0 = 1.0 - ranked[0].similarity;
        let d1 = 1.0 - ranked[1].similarity;
        let d2 = 1.0 - ranked[2].similarity;

        assert!(d0 <= d1);
        assert!(d1 <= d2);
    }

    #[test]
    fn test_top_k_truncation_per_collection_math() {
        let candidates = vec![
            VectorSearchResult { item: "skill_1", collection: "Skills".to_string(), similarity: 0.95 },
            VectorSearchResult { item: "skill_2", collection: "Skills".to_string(), similarity: 0.90 },
            VectorSearchResult { item: "skill_3", collection: "Skills".to_string(), similarity: 0.85 },
            VectorSearchResult { item: "skill_4", collection: "Skills".to_string(), similarity: 0.80 },
            VectorSearchResult { item: "project_1", collection: "Projects".to_string(), similarity: 0.92 },
            VectorSearchResult { item: "project_2", collection: "Projects".to_string(), similarity: 0.88 },
            VectorSearchResult { item: "pref_1", collection: "Preferences".to_string(), similarity: 0.78 },
        ];

        let top_k = 2;
        let truncated = truncate_top_k_per_collection(candidates, top_k);

        // Skills should be truncated from 4 to 2 (skill_1, skill_2)
        // Projects has 2 items -> both retained (project_1, project_2)
        // Preferences has 1 item -> retained (pref_1)
        // Total items expected: 2 + 2 + 1 = 5
        assert_eq!(truncated.len(), 5);

        let skills_retained: Vec<_> = truncated
            .iter()
            .filter(|c| c.collection == "Skills")
            .map(|c| c.item)
            .collect();
        assert_eq!(skills_retained, vec!["skill_1", "skill_2"]);

        let projects_retained: Vec<_> = truncated
            .iter()
            .filter(|c| c.collection == "Projects")
            .map(|c| c.item)
            .collect();
        assert_eq!(projects_retained, vec!["project_1", "project_2"]);

        let prefs_retained: Vec<_> = truncated
            .iter()
            .filter(|c| c.collection == "Preferences")
            .map(|c| c.item)
            .collect();
        assert_eq!(prefs_retained, vec!["pref_1"]);
    }

    #[test]
    fn test_combined_retrieval_ranking_pipeline() {
        let raw_candidates = vec![
            VectorSearchResult { item: "s1", collection: "Skills".to_string(), similarity: 0.95 },
            VectorSearchResult { item: "s2", collection: "Skills".to_string(), similarity: 0.70 }, // below threshold
            VectorSearchResult { item: "s3", collection: "Skills".to_string(), similarity: 0.88 },
            VectorSearchResult { item: "s4", collection: "Skills".to_string(), similarity: 0.82 },
            VectorSearchResult { item: "p1", collection: "Projects".to_string(), similarity: 0.91 },
            VectorSearchResult { item: "p2", collection: "Projects".to_string(), similarity: 0.85 },
        ];

        let cutoff = 0.75;
        let top_k = 2;

        let filtered_and_ranked = filter_and_rank_candidates(raw_candidates, cutoff);
        let final_results = truncate_top_k_per_collection(filtered_and_ranked, top_k);

        // Filtered out: s2 (0.70 < 0.75)
        // Skills remaining ranked: s1 (0.95), s3 (0.88), s4 (0.82)
        // Skills top-k=2 truncation: s1, s3
        // Projects remaining ranked: p1 (0.91), p2 (0.85)
        // Projects top-k=2 truncation: p1, p2
        // Total expected: 4 items
        assert_eq!(final_results.len(), 4);

        let final_items: Vec<_> = final_results.iter().map(|c| c.item).collect();
        assert!(final_items.contains(&"s1"));
        assert!(final_items.contains(&"s3"));
        assert!(!final_items.contains(&"s4")); // truncated out
        assert!(!final_items.contains(&"s2")); // filtered out
        assert!(final_items.contains(&"p1"));
        assert!(final_items.contains(&"p2"));
    }
}

