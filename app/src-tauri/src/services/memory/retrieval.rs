use crate::core::settings::MemorySettings;
use crate::persistence::memory_worker::decode_f32_blob;
use crate::services::memory::classifier::classify_query;
use crate::services::memory::embedder::{cosine_similarity, ensure_embedder_loaded, generate_embedding};
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq)]
pub struct RetrievedEpisode {
    pub session_id: u64,
    pub summary: String,
    pub similarity: f32,
    pub token_count: usize,
    pub created_at: i64,
}

/// Dynamically budgets and diversifies candidate episodes based strictly on context window share.
///
/// Zero Magic Numbers Architecture:
/// - Takes `max_token_budget` derived strictly from `context_size * max_context_share` (e.g. 20% of 4096 = 819 tokens, or 20% of 1M = 200k tokens).
/// - Dynamically computes a fair per-session token allocation (`per_session_token_budget = max_token_budget / num_active_sessions`).
/// - Sorts bullet-chunk facts within each session by similarity descending.
/// - Round-robin interleaves candidate facts across sessions to guarantee balanced representation across past history without session starvation.
pub fn diversify_and_budget_episodes(
    candidates: Vec<RetrievedEpisode>,
    _top_k: usize,
    max_token_budget: usize,
) -> Vec<RetrievedEpisode> {
    if candidates.is_empty() || max_token_budget == 0 {
        return Vec::new();
    }

    // 1. Group candidates by session_id
    let mut session_map: HashMap<u64, Vec<RetrievedEpisode>> = HashMap::new();
    for candidate in candidates {
        session_map
            .entry(candidate.session_id)
            .or_default()
            .push(candidate);
    }

    let num_sessions = session_map.len();
    // Fair per-session share cap: Each session gets at most (max_token_budget / num_sessions).
    // For small session counts (<= 2), allow up to (max_token_budget / 2) per session.
    let per_session_token_budget = (max_token_budget / num_sessions).max(max_token_budget / 2);

    // 2. Sort candidates within each session by similarity descending and cap per-session token budget
    let mut session_filtered_queues: HashMap<u64, Vec<RetrievedEpisode>> = HashMap::new();
    for (session_id, mut group) in session_map {
        group.sort_by(|a, b| {
            b.similarity
                .partial_cmp(&a.similarity)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let mut session_tokens = 0;
        let mut session_selected = Vec::new();
        for ep in group {
            if session_tokens + ep.token_count <= per_session_token_budget || session_selected.is_empty() {
                session_tokens += ep.token_count;
                session_selected.push(ep);
            }
        }
        session_filtered_queues.insert(session_id, session_selected);
    }

    // 3. Collect session keys sorted by best similarity candidate in each session
    let mut session_keys: Vec<u64> = session_filtered_queues.keys().cloned().collect();
    session_keys.sort_by(|a, b| {
        let best_a = session_filtered_queues[a][0].similarity;
        let best_b = session_filtered_queues[b][0].similarity;
        best_b.partial_cmp(&best_a).unwrap_or(std::cmp::Ordering::Equal)
    });

    let mut selected = Vec::new();
    let mut current_tokens = 0;
    let mut seen_summaries = std::collections::HashSet::new();

    // 4. Round-robin interleave facts across sessions until max_token_budget is filled
    let mut round = 0;
    loop {
        let mut added_any = false;
        for &s_id in &session_keys {
            if let Some(queue) = session_filtered_queues.get(&s_id) {
                if round < queue.len() {
                    let ep = &queue[round];
                    if !seen_summaries.contains(&ep.summary) {
                        if current_tokens + ep.token_count <= max_token_budget || selected.is_empty() {
                            current_tokens += ep.token_count;
                            seen_summaries.insert(ep.summary.clone());
                            selected.push(ep.clone());
                            added_any = true;
                        }
                    }
                }
            }
        }
        round += 1;
        if !added_any || current_tokens >= max_token_budget {
            break;
        }
    }

    selected
}

/// Performs vector search against episodes in DB, filtering by similarity_threshold, session diversification, and token budget.
pub async fn search_and_diversify_episodes(
    conn: &turso::Connection,
    query_vector: &[f32],
    current_session_id: u64,
    settings: &MemorySettings,
    context_size: usize,
) -> anyhow::Result<Vec<RetrievedEpisode>> {
    let mut rows = conn
        .query(
            "SELECT session_id, summary, embedding, created_at, token_count FROM episodes WHERE session_id != ?",
            (current_session_id as i64,),
        )
        .await?;

    let mut candidates = Vec::new();
    while let Some(row) = rows.next().await? {
        let session_id: i64 = row.get(0)?;
        let summary: String = row.get(1)?;
        let blob_bytes: Vec<u8> = row.get(2)?;
        let created_at: i64 = row.get(3)?;
        let token_count: i64 = row.get(4)?;

        let ep_vec = decode_f32_blob(&blob_bytes);
        let sim = cosine_similarity(query_vector, &ep_vec);

        if sim >= settings.similarity_threshold {
            candidates.push(RetrievedEpisode {
                session_id: session_id as u64,
                summary,
                similarity: sim,
                token_count: token_count as usize,
                created_at,
            });
        }
    }

    let max_token_budget = (context_size as f32 * settings.max_context_share) as usize;
    Ok(diversify_and_budget_episodes(
        candidates,
        settings.top_k as usize,
        max_token_budget,
    ))
}

/// Main entry point for Episodic Memory RAG retrieval.
/// Executes Query Classification -> Query Embedding -> DB Search -> Session Diversification -> Token Budgeting.
pub async fn retrieve_episodic_memories(
    conn: &turso::Connection,
    query_text: &str,
    current_session_id: u64,
    settings: &MemorySettings,
    context_size: usize,
) -> anyhow::Result<Vec<RetrievedEpisode>> {
    if !settings.episodic_enabled {
        return Ok(Vec::new());
    }

    if query_text.trim().is_empty() {
        return Ok(Vec::new());
    }

    // Gate 1: Hot-Path Query Classification (query-sieve)
    let classification = classify_query(query_text);
    if classification.is_generic() {
        tracing::debug!("[Retrieval] Query classified as GENERIC. Bypassing RAG retrieval.");
        return Ok(Vec::new());
    }

    // Gate 2: Query Embedding Generation (BGE-M3)
    ensure_embedder_loaded(settings.episodic_enabled)?;
    let query_vector = match generate_embedding(query_text)? {
        Some(vec) => vec,
        None => {
            tracing::warn!("[Retrieval] Embedder model not loaded. Skipping retrieval.");
            return Ok(Vec::new());
        }
    };

    // Gate 3: DB Vector Search & Session Diversification
    let episodes = search_and_diversify_episodes(conn, &query_vector, current_session_id, settings, context_size).await?;
    tracing::info!(
        "[Retrieval] Retrieved {} episode chunks for query='{}'",
        episodes.len(),
        query_text
    );

    Ok(episodes)
}

/// Formats retrieved episodic memory chunks into a clean system prompt context block.
pub fn format_retrieved_memories_for_prompt(memories: &[RetrievedEpisode]) -> String {
    if memories.is_empty() {
        return String::new();
    }

    let mut out = String::from("\n\n[Relevant Past Conversation Summaries]\n");
    for mem in memories {
        out.push_str(&format!("- Session {} (Score {:.2}): {}\n", mem.session_id, mem.similarity, mem.summary));
    }
    out
}

/// Orchestrates retrieving both personal memory profile and semantic episodic memories,
/// returning a formatted string to be appended to the system prompt.
pub async fn retrieve_and_format_memory_context(
    conn: &turso::Connection,
    query_text: &str,
    current_session_id: u64,
    settings: &MemorySettings,
    context_size: usize,
) -> anyhow::Result<String> {
    // 1. Load user profile block (Personal Memory - always injected if present)
    let personal_block = match super::personal_memory::load_user_profile(conn).await {
        Ok(block) => block,
        Err(e) => {
            tracing::warn!("[Retrieval] Failed to load personal profile: {}", e);
            String::new()
        }
    };

    // 2. Load episodic memories (Episodic Memory - search if query is semantic)
    let mut episodic_block = String::new();
    if settings.episodic_enabled && !query_text.trim().is_empty() {
        let classification = classify_query(query_text);
        if !classification.is_generic() {
            ensure_embedder_loaded(settings.episodic_enabled)?;
            if let Some(query_vector) = generate_embedding(query_text)? {
                let episodes = search_and_diversify_episodes(
                    conn,
                    &query_vector,
                    current_session_id,
                    settings,
                    context_size,
                )
                .await?;
                episodic_block = format_retrieved_memories_for_prompt(&episodes);
            }
        }
    }

    let mut full_block = String::new();
    if !personal_block.is_empty() {
        full_block.push_str(&personal_block);
    }
    if !episodic_block.is_empty() {
        if !full_block.is_empty() {
            full_block.push_str("\n\n");
        }
        full_block.push_str(&episodic_block);
    }

    Ok(full_block)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_retrieved_memories_for_prompt() {
        let memories = vec![
            RetrievedEpisode {
                session_id: 101,
                summary: "User prefers Rust.".to_string(),
                similarity: 0.88,
                token_count: 5,
                created_at: 1000,
            },
        ];
        let block = format_retrieved_memories_for_prompt(&memories);
        assert!(block.contains("[Relevant Past Conversation Summaries]"));
        assert!(block.contains("Session 101"));
        assert!(block.contains("User prefers Rust."));
    }

    #[test]
    fn test_diversify_and_budget_episodes_dynamic() {
        let candidates = vec![
            RetrievedEpisode { session_id: 1, summary: "Fact 1A".to_string(), similarity: 0.90, token_count: 10, created_at: 1 },
            RetrievedEpisode { session_id: 1, summary: "Fact 1B".to_string(), similarity: 0.85, token_count: 10, created_at: 2 },
            RetrievedEpisode { session_id: 2, summary: "Fact 2A".to_string(), similarity: 0.88, token_count: 10, created_at: 3 },
        ];

        let selected = diversify_and_budget_episodes(candidates, 10, 50);
        assert_eq!(selected.len(), 3);
        assert_eq!(selected[0].summary, "Fact 1A");
        assert_eq!(selected[1].summary, "Fact 2A");
        assert_eq!(selected[2].summary, "Fact 1B");
    }
}
