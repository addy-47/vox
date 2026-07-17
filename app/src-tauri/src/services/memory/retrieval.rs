use crate::core::settings::MemorySettings;
use crate::persistence::memory_worker::encode_f32_blob;
use crate::services::memory::classifier::classify_query;
use crate::services::memory::embedder::{ensure_embedder_loaded, generate_embedding};
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
                    if !seen_summaries.contains(&ep.summary)
                        && (current_tokens + ep.token_count <= max_token_budget || selected.is_empty())
                    {
                        current_tokens += ep.token_count;
                        seen_summaries.insert(ep.summary.clone());
                        selected.push(ep.clone());
                        added_any = true;
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

pub fn reciprocal_rank_fusion(
    fts_results: Vec<RetrievedEpisode>,
    vector_results: Vec<RetrievedEpisode>,
    k: f32, // RRF smoothing constant, default = 60.0
) -> Vec<RetrievedEpisode> {
    let mut score_map: HashMap<String, (RetrievedEpisode, f32)> = HashMap::new();

    for (rank, ep) in fts_results.into_iter().enumerate() {
        let score = 1.0 / (k + (rank + 1) as f32);
        score_map.insert(ep.summary.clone(), (ep, score));
    }

    for (rank, ep) in vector_results.into_iter().enumerate() {
        let score = 1.0 / (k + (rank + 1) as f32);
        score_map
            .entry(ep.summary.clone())
            .and_modify(|(_, s)| *s += score)
            .or_insert((ep, score));
    }

    let mut merged: Vec<(RetrievedEpisode, f32)> = score_map.into_values().collect();
    merged.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    merged
        .into_iter()
        .map(|(mut ep, score)| {
            ep.similarity = score; // Overload similarity field with the combined RRF score
            ep
        })
        .collect()
}

pub async fn search_fts_episodes(
    conn: &turso::Connection,
    query_text: &str,
    current_session_id: u64,
) -> anyhow::Result<Vec<RetrievedEpisode>> {
    let sanitized = query_text
        .chars()
        .filter(|c| c.is_alphanumeric() || c.is_whitespace())
        .collect::<String>();
    let clean_query = sanitized.trim();
    if clean_query.is_empty() {
        return Ok(Vec::new());
    }

    let mut rows = conn
        .query(
            "SELECT session_id, summary, created_at, token_count
             FROM episodes
             WHERE fts_match('idx_episodes_search', ?) AND session_id != ?
             LIMIT 50",
            (clean_query.to_string(), current_session_id as i64),
        )
        .await?;

    let mut list = Vec::new();
    while let Some(row) = rows.next().await? {
        let session_id: i64 = row.get(0)?;
        let summary: String = row.get(1)?;
        let created_at: i64 = row.get(2)?;
        let token_count: i64 = row.get(3)?;
        list.push(RetrievedEpisode {
            session_id: session_id as u64,
            summary,
            similarity: 0.0,
            token_count: token_count as usize,
            created_at,
        });
    }
    Ok(list)
}

pub async fn search_vector_episodes(
    conn: &turso::Connection,
    query_vector: &[f32],
    current_session_id: u64,
) -> anyhow::Result<Vec<RetrievedEpisode>> {
    let blob_bytes = encode_f32_blob(query_vector);
    let mut rows = conn
        .query(
            "SELECT session_id, summary, created_at, token_count, vector_distance_cos(embedding, ?) as distance
             FROM episodes
             WHERE session_id != ?
             ORDER BY distance ASC
             LIMIT 50",
            (blob_bytes, current_session_id as i64),
        )
        .await?;

    let mut list = Vec::new();
    while let Some(row) = rows.next().await? {
        let session_id: i64 = row.get(0)?;
        let summary: String = row.get(1)?;
        let created_at: i64 = row.get(2)?;
        let token_count: i64 = row.get(3)?;
        let distance = row.get::<f64>(4)? as f32;
        
        let similarity = 1.0 - distance;
        list.push(RetrievedEpisode {
            session_id: session_id as u64,
            summary,
            similarity,
            token_count: token_count as usize,
            created_at,
        });
    }
    Ok(list)
}

pub async fn search_and_diversify_episodes(
    conn: &turso::Connection,
    query_vector: &[f32],
    query_text: &str,
    current_session_id: u64,
    settings: &MemorySettings,
    context_size: usize,
) -> anyhow::Result<Vec<RetrievedEpisode>> {
    let vector_candidates = search_vector_episodes(conn, query_vector, current_session_id).await?;
    let fts_candidates = if !query_text.trim().is_empty() {
        search_fts_episodes(conn, query_text, current_session_id).await.unwrap_or_default()
    } else {
        Vec::new()
    };

    let filtered_vector: Vec<RetrievedEpisode> = vector_candidates
        .into_iter()
        .filter(|c| c.similarity >= settings.similarity_threshold)
        .collect();

    let fused = reciprocal_rank_fusion(fts_candidates, filtered_vector, 60.0);

    let max_token_budget = (context_size as f32 * settings.max_context_share) as usize;
    Ok(diversify_and_budget_episodes(
        fused,
        settings.top_k as usize,
        max_token_budget,
    ))
}

/// Main entry point for Episodic Memory RAG retrieval.
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

    let classification = classify_query(query_text);
    if classification.is_generic() {
        tracing::debug!("[Retrieval] Query classified as GENERIC. Bypassing RAG retrieval.");
        return Ok(Vec::new());
    }

    ensure_embedder_loaded(settings.episodic_enabled)?;
    let query_vector = match generate_embedding(query_text)? {
        Some(vec) => vec,
        None => {
            tracing::warn!("[Retrieval] Embedder model not loaded. Skipping retrieval.");
            return Ok(Vec::new());
        }
    };

    let episodes = search_and_diversify_episodes(conn, &query_vector, query_text, current_session_id, settings, context_size).await?;
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
    let query_vector = if (settings.personal_enabled || settings.episodic_enabled) && !query_text.trim().is_empty() {
        ensure_embedder_loaded(true)?;
        generate_embedding(query_text)?
    } else {
        None
    };

    let personal_block = if settings.personal_enabled {
        if let Some(ref q_vec) = query_vector {
            match super::personal_memory::retrieve_personal_context(conn, q_vec, settings, context_size, None).await {
                Ok(block) => block,
                Err(e) => {
                    tracing::warn!("[Retrieval] Failed to load personal profile: {}", e);
                    String::new()
                }
            }
        } else {
            String::new()
        }
    } else {
        String::new()
    };

    let mut episodic_block = String::new();
    if settings.episodic_enabled && !query_text.trim().is_empty() {
        let classification = classify_query(query_text);
        if !classification.is_generic() {
            if let Some(ref q_vec) = query_vector {
                let episodes = search_and_diversify_episodes(
                    conn,
                    q_vec,
                    query_text,
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
