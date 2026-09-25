use std::{collections::HashMap, time::Instant};

use futures_util::future::{BoxFuture, FutureExt};
use serde_json::{json, Value};

use super::{ToolDefinition, ToolDomain, ToolError, ToolExecutionContext, ToolResult};
use crate::{
    core::settings::PipelineMode,
    persistence::{fetch_active_episodic_memory, EpisodicFactCandidate},
    services::{
        llm::ToolFlow,
        memory::ml::{cosine_similarity, ensure_embedder_loaded, generate_embedding},
    },
};

const RRF_K: f32 = 60.0;

/// Non-terminal cognitive tool for searching personal memory documents and episodic turns.
pub struct MemorySearchTool;

impl ToolDefinition for MemorySearchTool {
    fn name(&self) -> &str {
        "search_memory"
    }

    fn domain(&self) -> ToolDomain {
        ToolDomain::All
    }

    fn description(&self, _mode: PipelineMode) -> &str {
        "Searches personal cognitive memory for user facts, preferences, background context, or past conversational details."
    }

    fn parameters_schema(&self, mode: PipelineMode) -> Value {
        match mode {
            PipelineMode::Modular => json!({
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "Semantic search query to locate relevant user facts or context in memory."
                    },
                    "spoken_filler": {
                        "type": "string",
                        "description": "A brief, natural 3-5 word spoken phrase delivered while memory search is evaluated."
                    }
                },
                "required": ["query", "spoken_filler"]
            }),
            PipelineMode::Realtime => json!({
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "Semantic search query to locate relevant user facts or context in memory."
                    }
                },
                "required": ["query"]
            }),
        }
    }

    fn flow(&self) -> ToolFlow {
        ToolFlow::NonTerminal
    }

    fn execute<'a>(
        &'a self,
        mode: PipelineMode,
        args: Value,
        ctx: &'a ToolExecutionContext,
    ) -> BoxFuture<'a, Result<ToolResult, ToolError>> {
        async move {
            let query = args
                .get("query")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .trim()
                .to_string();

            if query.is_empty() {
                return Err(ToolError::InvalidArguments(
                    "Parameter 'query' must be a non-empty string".to_string(),
                ));
            }

            log::info!(
                "[MemorySearchTool] Turn {} ({:?}): hybrid episodic search for: '{}'",
                ctx.turn_id,
                mode,
                query
            );

            let retrieval_start = Instant::now();
            let observation = perform_hybrid_search(ctx, &query).await?;
            let retrieval_dur = retrieval_start.elapsed().as_millis() as u64;
            ctx.app_state.turn_metrics.record_retrieval(retrieval_dur);

            let mut result = ToolResult::new(observation);
            log::info!(
                "[MemorySearchTool] Turn {} ({:?}): final observation for LLM: '{}'",
                ctx.turn_id,
                mode,
                result.content
            );
            if mode == PipelineMode::Modular {
                let spoken_filler = args
                    .get("spoken_filler")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .trim()
                    .to_string();
                if !spoken_filler.is_empty() {
                    result = result.with_spoken_filler(spoken_filler);
                }
            }

            Ok(result)
        }
        .boxed()
    }
}

/// Orchestrates DB candidate retrieval, embedding generation, and ranking.
async fn perform_hybrid_search(
    ctx: &ToolExecutionContext,
    query: &str,
) -> Result<String, ToolError> {
    let conn = ctx
        .app_state
        .db
        .connect()
        .map_err(|e| ToolError::ExecutionFailed(format!("Database connect error: {}", e)))?;

    let candidates = fetch_active_episodic_memory(&conn)
        .await
        .map_err(|e| ToolError::ExecutionFailed(format!("Episodic memory query error: {}", e)))?;

    log::info!(
        "[MemorySearchTool] Retrieved {} active episodic candidates for query '{}'",
        candidates.len(),
        query
    );
    for candidate in &candidates {
        log::info!(
            "[MemorySearchTool] Candidate id={} type={} embedding_present={} text='{}'",
            candidate.id,
            candidate.fact_type,
            candidate.embedding.is_some(),
            candidate.text
        );
    }

    if candidates.is_empty() {
        return Ok(format!(
            "Memory search completed for query '{}'. No relevant historical records found.",
            query
        ));
    }

    let (top_k, cutoff) = {
        let guard = ctx
            .app_state
            .settings
            .read()
            .map_err(|e| ToolError::ExecutionFailed(format!("Settings lock error: {}", e)))?;
        (
            guard.personal_memory.top_k_facts as usize,
            guard.personal_memory.semantic_similarity_cutoff,
        )
    };

    let query_embedding = {
        if ensure_embedder_loaded(true).unwrap_or(false) {
            generate_embedding(query).unwrap_or(None)
        } else {
            None
        }
    };
    log::info!(
        "[MemorySearchTool] Query embedding available={} dimensions={}",
        query_embedding.is_some(),
        query_embedding.as_ref().map_or(0, Vec::len)
    );

    let results = rank_candidates(
        &candidates,
        query,
        query_embedding.as_deref(),
        top_k,
        cutoff,
    );
    if results.is_empty() {
        Ok(format!(
            "Memory search completed for query '{}'. No relevant historical records found.",
            query
        ))
    } else {
        let lines: Vec<String> = results
            .into_iter()
            .map(|f| format!("- [{}] {}", f.fact_type, f.text))
            .collect();
        Ok(format!(
            "Found relevant memory records:\n{}",
            lines.join("\n")
        ))
    }
}

/// Fuses dense cosine similarity and lexical token matches via Reciprocal Rank Fusion.
fn rank_candidates<'a>(
    candidates: &'a [EpisodicFactCandidate],
    query: &str,
    query_embedding: Option<&[f32]>,
    top_k: usize,
    cutoff: f32,
) -> Vec<&'a EpisodicFactCandidate> {
    let mut vector_scores: HashMap<String, f32> = HashMap::new();
    if let Some(q_vec) = query_embedding {
        for c in candidates {
            if let Some(ref c_vec) = c.embedding {
                let sim = cosine_similarity(q_vec, c_vec);
                log::info!(
                    "[MemorySearchTool] Dense candidate id={} similarity={} cutoff={} admitted={}",
                    c.id,
                    sim,
                    cutoff,
                    sim >= cutoff
                );
                if sim >= cutoff {
                    vector_scores.insert(c.id.clone(), sim);
                }
            }
        }
    }

    let query_terms: Vec<String> = query
        .split_whitespace()
        .map(|w| w.to_lowercase())
        .filter(|w| w.len() >= 3)
        .collect();

    let mut lexical_scores: HashMap<String, usize> = HashMap::new();
    for c in candidates {
        let lower = c.text.to_lowercase();
        let matches: usize = query_terms
            .iter()
            .filter(|&term| lower.contains(term))
            .count();
        if matches > 0 {
            lexical_scores.insert(c.id.clone(), matches);
        }
    }

    let mut rrf_scores: Vec<(&'a EpisodicFactCandidate, f32)> = Vec::new();
    for c in candidates {
        let mut score = 0.0f32;
        if let Some(vec_score) = vector_scores.get(&c.id) {
            score += vec_score / (RRF_K + 1.0);
        }
        if let Some(lex_count) = lexical_scores.get(&c.id) {
            score += (*lex_count as f32) / (RRF_K + 1.0);
        }
        let dense_admitted = vector_scores.contains_key(&c.id);
        let lexical_matches = lexical_scores.get(&c.id).copied().unwrap_or(0);
        log::info!(
            "[MemorySearchTool] Fused candidate id={} type={} dense_admitted={} lexical_matches={} rrf_score={} final_admitted={}",
            c.id,
            c.fact_type,
            dense_admitted,
            lexical_matches,
            score,
            score > 0.0
        );
        if score > 0.0 {
            rrf_scores.push((c, score));
        }
    }

    rrf_scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    let selected = rrf_scores
        .into_iter()
        .take(top_k.max(1))
        .map(|(c, _)| c)
        .collect::<Vec<_>>();
    log::info!(
        "[MemorySearchTool] Ranking complete: selected={} top_k={} ids={:?}",
        selected.len(),
        top_k.max(1),
        selected
            .iter()
            .map(|candidate| candidate.id.as_str())
            .collect::<Vec<_>>()
    );
    selected
}
