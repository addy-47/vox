use super::scope::ScopeRouting;
use crate::core::settings::MemorySettings;
use crate::persistence::queries;
use crate::services::memory::ml::estimate_tokens;
use crate::services::memory::MemoryCollection;
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

/// A vector similarity scored fact retrieved from vector search.
#[derive(Debug, Clone)]
pub struct ScoredFact {
    pub id: String,
    pub fact: String,
    pub collection: String,
    pub similarity: f32,
    pub created_at: i64,
}

/// A knowledge graph edge expansion connected to a seed fact.
#[derive(Debug, Clone)]
pub struct GraphEdge {
    pub relation: String,
    pub target_collection: String,
    pub target_fact: String,
    pub created_at: i64,
}

/// Structured memory profile retrieved across SQL, vector seeds, and graph neighbors.
#[derive(Debug, Clone, Default)]
pub struct RetrievedProfile {
    pub sql_sections: Vec<MemoryFact>,
    pub vector_seeds: Vec<ScoredFact>,
    pub graph_children: Vec<GraphEdge>,
}

impl RetrievedProfile {
    /// Returns true if no facts or graph edges were retrieved.
    pub fn is_empty(&self) -> bool {
        self.sql_sections.is_empty()
            && self.vector_seeds.is_empty()
            && self.graph_children.is_empty()
    }
}

/// Collects Narrative and Directives facts from SQL storage within token budget.
async fn collect_sql_sections(
    conn: &Connection,
    routing: &ScopeRouting,
    remaining_budget: &mut usize,
) -> Vec<MemoryFact> {
    let mut out = Vec::new();

    if routing
        .sql_collections
        .contains(&MemoryCollection::Narrative)
    {
        let narrative_facts = queries::fetch_narrative_history(conn, 3)
            .await
            .unwrap_or_default();
        for fact in narrative_facts {
            let tokens = estimate_tokens(&fact.fact);
            if *remaining_budget >= tokens {
                *remaining_budget -= tokens;
                out.push(fact);
            }
        }
    }

    if routing
        .sql_collections
        .contains(&MemoryCollection::Directives)
    {
        let directives_facts = queries::fetch_latest_directives(conn, 5)
            .await
            .unwrap_or_default();
        for fact in directives_facts {
            let tokens = estimate_tokens(&fact.fact);
            if *remaining_budget >= tokens {
                *remaining_budget -= tokens;
                out.push(fact);
            }
        }
    }

    out
}

/// Executes vector candidate search and BFS graph expansion within token budget.
async fn collect_vector_graph_sections(
    conn: &Connection,
    routing: &ScopeRouting,
    query_embedding: &[f32],
    settings: &MemorySettings,
    remaining_budget: &mut usize,
) -> (Vec<ScoredFact>, Vec<GraphEdge>) {
    let mut vector_seeds = Vec::new();
    let mut graph_children = Vec::new();

    if routing.vector_collections.is_empty() {
        return (vector_seeds, graph_children);
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
        return (vector_seeds, graph_children);
    }

    let seed_ids: Vec<String> = fetched_seeds
        .iter()
        .map(|(id, _, _, _)| id.clone())
        .collect();
    let parent_quota = (*remaining_budget / seed_ids.len().max(1)).max(30);

    let seed_facts_map = queries::fetch_facts_by_ids(conn, &seed_ids)
        .await
        .unwrap_or_default();

    let mut visited_ids: HashSet<String> = seed_ids.iter().cloned().collect();

    for (id, fact_text, collection, sim) in fetched_seeds {
        let created_at = seed_facts_map
            .get(&id)
            .map(|f| f.created_at)
            .unwrap_or_default();
        let tokens = estimate_tokens(&fact_text);
        if *remaining_budget >= tokens && tokens <= parent_quota * 2 {
            *remaining_budget -= tokens;
            vector_seeds.push(ScoredFact {
                id,
                fact: fact_text,
                collection,
                similarity: sim,
                created_at,
            });
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
                    let tokens = estimate_tokens(&child_fact.fact);
                    if *remaining_budget >= tokens {
                        *remaining_budget -= tokens;
                        graph_children.push(GraphEdge {
                            relation,
                            target_collection: child_fact.collection.clone(),
                            target_fact: child_fact.fact.clone(),
                            created_at: child_fact.created_at,
                        });
                    }
                }
            }
        }

        frontier = next_frontier;
    }

    (vector_seeds, graph_children)
}

/// Executes Scope-Pruned Waterfall Retrieval with dynamic budget allocation and graph expansion.
pub async fn retrieve_turn_profile(
    conn: &Connection,
    query_embedding: &[f32],
    scope: MemoryScope,
    settings: &MemorySettings,
    context_size: usize,
) -> Result<RetrievedProfile> {
    if !settings.context_retrieval_enabled || scope == MemoryScope::ChitChat {
        return Ok(RetrievedProfile::default());
    }

    let routing = super::scope::route_scope(scope);
    if routing.sql_collections.is_empty() && routing.vector_collections.is_empty() {
        return Ok(RetrievedProfile::default());
    }

    let total_budget = (context_size as f32 * settings.max_context_share) as usize;
    let mut remaining_budget = total_budget;

    let sql_sections = collect_sql_sections(conn, &routing, &mut remaining_budget).await;
    let (vector_seeds, graph_children) = collect_vector_graph_sections(
        conn,
        &routing,
        query_embedding,
        settings,
        &mut remaining_budget,
    )
    .await;

    Ok(RetrievedProfile {
        sql_sections,
        vector_seeds,
        graph_children,
    })
}
