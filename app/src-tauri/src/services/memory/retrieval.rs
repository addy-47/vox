use crate::core::constants::MemoryCollection;
use crate::core::settings::MemorySettings;
use crate::persistence::queries;
use crate::services::memory::estimate_tokens;
use crate::services::memory::scope_router::ScopeRouting;
use anyhow::Result;
use query_sieve::MemoryScope;
use std::collections::HashSet;
use turso::Connection;

/// A structured memory fact retrieved from persistence.
#[derive(Debug, Clone)]
pub struct MemoryFact {
    pub id: String,
    pub fact_type: String,
    pub collection: String,
    pub fact: String,
    pub source: String,
    pub status: String,
    pub created_at: i64,
}

/// Collects Narrative and Directives facts from SQL storage within token budget.
async fn collect_sql_sections(
    conn: &Connection,
    routing: &ScopeRouting,
    remaining_budget: &mut usize,
) -> Vec<String> {
    let mut out_sections = Vec::new();

    if routing.sql_collections.contains(&MemoryCollection::Narrative) {
        let narrative_facts = queries::fetch_narrative_history(conn, 3)
            .await
            .unwrap_or_default();
        if !narrative_facts.is_empty() {
            let mut narrative_lines = Vec::new();
            for fact in &narrative_facts {
                let line = format!("- {}", fact.fact);
                let tokens = estimate_tokens(&line);
                if *remaining_budget >= tokens {
                    *remaining_budget -= tokens;
                    narrative_lines.push(line);
                }
            }
            if !narrative_lines.is_empty() {
                out_sections.push(format!(
                    "<narrative>\n{}\n</narrative>",
                    narrative_lines.join("\n")
                ));
            }
        }
    }

    if routing.sql_collections.contains(&MemoryCollection::Directives) {
        let directives_facts = queries::fetch_latest_directives(conn, 5)
            .await
            .unwrap_or_default();
        if !directives_facts.is_empty() {
            let mut directive_lines = Vec::new();
            for fact in &directives_facts {
                let line = format!("- {}", fact.fact);
                let tokens = estimate_tokens(&line);
                if *remaining_budget >= tokens {
                    *remaining_budget -= tokens;
                    directive_lines.push(line);
                }
            }
            if !directive_lines.is_empty() {
                out_sections.push(format!(
                    "<directives>\n{}\n</directives>",
                    directive_lines.join("\n")
                ));
            }
        }
    }

    out_sections
}

/// Executes vector candidate search and BFS graph expansion within token budget.
async fn collect_vector_graph_sections(
    conn: &Connection,
    routing: &ScopeRouting,
    query_embedding: &[f32],
    settings: &MemorySettings,
    remaining_budget: &mut usize,
) -> Vec<String> {
    let mut out_sections = Vec::new();
    if routing.vector_collections.is_empty() {
        return out_sections;
    }

    let target_collections: Vec<&str> = routing
        .vector_collections
        .iter()
        .map(|c| c.as_str())
        .collect();

    let fetched_seeds = queries::fetch_inter_collection_candidates(
        conn,
        &target_collections,
        query_embedding,
        settings.semantic_similarity_cutoff,
        None,
    )
    .await
    .unwrap_or_default();

    if fetched_seeds.is_empty() {
        return out_sections;
    }

    let seed_ids: Vec<String> = fetched_seeds
        .iter()
        .map(|(id, _, _, _)| id.clone())
        .collect();
    let parent_quota = (*remaining_budget / seed_ids.len().max(1)).max(30);

    let mut graph_lines = Vec::new();
    let mut visited_ids: HashSet<String> = seed_ids.iter().cloned().collect();

    for (_id, fact_text, collection, _sim) in &fetched_seeds {
        let line = format!("- [{}] {}", collection, fact_text);
        let tokens = estimate_tokens(&line);
        if *remaining_budget >= tokens && tokens <= parent_quota * 2 {
            *remaining_budget -= tokens;
            graph_lines.push(line);
        }
    }

    let max_hops = settings.max_hops.min(2) as usize;
    let mut frontier = seed_ids;

    for _hop in 1..=max_hops {
        if frontier.is_empty() || *remaining_budget < 20 {
            break;
        }

        let neighbors = queries::fetch_graph_neighbors(conn, &frontier)
            .await
            .unwrap_or_default();
        if neighbors.is_empty() {
            break;
        }

        let mut next_frontier = Vec::new();
        let mut child_ids_to_fetch = Vec::new();

        for (from_id, to_id, relation, _src) in neighbors {
            let child_id = if visited_ids.contains(&from_id) {
                to_id
            } else {
                from_id
            };
            if visited_ids.insert(child_id.clone()) {
                child_ids_to_fetch.push((child_id.clone(), relation));
                next_frontier.push(child_id);
            }
        }

        if !child_ids_to_fetch.is_empty() {
            let raw_ids: Vec<String> = child_ids_to_fetch
                .iter()
                .map(|(id, _)| id.clone())
                .collect();
            let fetched_children = queries::fetch_facts_by_ids(conn, &raw_ids)
                .await
                .unwrap_or_default();

            for (child_id, relation) in child_ids_to_fetch {
                if let Some(child_fact) = fetched_children.get(&child_id) {
                    let line = format!(
                        "  ↳ --[{}]--> [{}] {}",
                        relation, child_fact.collection, child_fact.fact
                    );
                    let tokens = estimate_tokens(&line);
                    if *remaining_budget >= tokens {
                        *remaining_budget -= tokens;
                        graph_lines.push(line);
                    }
                }
            }
        }

        frontier = next_frontier;
    }

    if !graph_lines.is_empty() {
        out_sections.push(format!(
            "<semantic_graph>\n{}\n</semantic_graph>",
            graph_lines.join("\n")
        ));
    }

    out_sections
}

/// Executes Scope-Pruned Waterfall Retrieval with dynamic budget allocation and graph expansion.
pub async fn retrieve_personal_context_v7(
    conn: &Connection,
    query_embedding: &[f32],
    scope: MemoryScope,
    settings: &MemorySettings,
    context_size: usize,
) -> Result<String> {
    if !settings.context_retrieval_enabled || scope == MemoryScope::ChitChat {
        return Ok(String::new());
    }

    let routing = crate::services::memory::scope_router::route_scope(scope);
    if routing.sql_collections.is_empty() && routing.vector_collections.is_empty() {
        return Ok(String::new());
    }

    let total_budget = (context_size as f32 * settings.max_context_share) as usize;
    let mut remaining_budget = total_budget;

    let mut out_sections = Vec::new();
    out_sections.extend(collect_sql_sections(conn, &routing, &mut remaining_budget).await);
    out_sections.extend(
        collect_vector_graph_sections(
            conn,
            &routing,
            query_embedding,
            settings,
            &mut remaining_budget,
        )
        .await,
    );

    if out_sections.is_empty() {
        return Ok(String::new());
    }

    Ok(format!(
        "<user_profile>\n{}\n</user_profile>",
        out_sections.join("\n\n")
    ))
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_vector_distance_ranking_order() {
        let u = vec![1.0f32, 0.0f32];
        let v = vec![1.0f32, 0.0f32];
        let sim = crate::services::memory::embedder::cosine_similarity(&u, &v);
        assert!((sim - 1.0).abs() < 1e-5);
    }
}
