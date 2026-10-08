use std::{collections::HashMap, sync::Arc, time::Duration};

use futures_util::future::{BoxFuture, FutureExt};
use serde_json::{json, Value};

use super::{ToolDefinition, ToolDomain, ToolError, ToolExecutionContext, ToolResult};
use crate::{
    core::events::PipelineMode,
    services::{
        llm::ToolFlow,
        memory::ml::{embedder, estimate_tokens},
    },
};

// ─── Level 3 Domain Constants ────────────────────────────────────────────────
pub const DEFAULT_MAX_PASSAGES: usize = 5;
pub const DEFAULT_FETCH_CANDIDATES: usize = 3;
pub const DEFAULT_FETCH_TIMEOUT_MS: u64 = 4000;
pub const DEFAULT_MAX_RESPONSE_BYTES: usize = 512_000;
pub const DEFAULT_CHUNK_SIZE_WORDS: usize = 150;
pub const DEFAULT_CHUNK_OVERLAP_WORDS: usize = 30;

pub const HARD_TOKEN_CEILING: usize = 2000;
pub const MAX_PASSAGE_CHARS: usize = 2000;

pub const TIMEOUT_SPARSE_MS: u64 = 4000;
pub const TIMEOUT_DENSE_MS: u64 = 5000;
pub const TIMEOUT_HYBRID_MS: u64 = 5500;
pub const TIMEOUT_CEILING_MS: u64 = 12000;
pub const TIMEOUT_FLOOR_MS: u64 = 1000;

/// B1 relevance floor: minimum calibrated passage score for evidence admission.
/// Must stay in sync with the nexus default; the AppState singleton sets this
/// explicitly (never rely on `..Default::default()` for this field).
pub const DEFAULT_MIN_SCORE: f32 = 0.12;

/// Adapter bridging Vox's in-process ONNX MiniLM singleton to the `nexus::traits::TextEmbedder` trait.
#[derive(Clone, Copy, Debug, Default)]
pub struct VoxEmbedder;

#[async_trait::async_trait]
impl nexus::traits::TextEmbedder for VoxEmbedder {
    async fn embed_text(&self, text: &str) -> Result<Vec<f32>, nexus::NexusError> {
        let text_owned = text.to_string();
        tokio::task::spawn_blocking(move || {
            let _ = embedder::ensure_embedder_loaded(true);
            embedder::generate_embedding(&text_owned)
                .map_err(|e| nexus::NexusError::Embedding(e.to_string()))?
                .ok_or_else(|| {
                    nexus::NexusError::Embedding("Embedder singleton not available".to_string())
                })
        })
        .await
        .map_err(|e| nexus::NexusError::Embedding(format!("Spawn blocking task failed: {e}")))?
    }

    async fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>, nexus::NexusError> {
        let texts_owned: Vec<String> = texts.iter().map(|s| s.to_string()).collect();
        tokio::task::spawn_blocking(move || {
            let _ = embedder::ensure_embedder_loaded(true);
            let refs: Vec<&str> = texts_owned.iter().map(|s| s.as_str()).collect();
            embedder::generate_embeddings_batch(&refs)
                .map_err(|e| nexus::NexusError::Embedding(e.to_string()))?
                .ok_or_else(|| {
                    nexus::NexusError::Embedding("Embedder singleton not available".to_string())
                })
        })
        .await
        .map_err(|e| nexus::NexusError::Embedding(format!("Spawn blocking task failed: {e}")))?
    }
}

/// Unified cognitive observation tool for multi-engine web search, DOM extraction, and relevance ranking.
pub struct WebSearchTool;

impl ToolDefinition for WebSearchTool {
    fn name(&self) -> &str {
        "web_search"
    }

    fn domain(&self) -> ToolDomain {
        ToolDomain::Modular
    }

    fn description(&self, _mode: PipelineMode) -> &str {
        "Searches the live web and extracts relevant, verified passages from top sources to answer questions about current events, technical facts, or documentation."
    }

    fn parameters_schema(&self, _mode: PipelineMode) -> Value {
        json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "The search query optimized for search engines (e.g., 'Federal Reserve interest rate decision September 2026')."
                },
                "spoken_filler": {
                    "type": "string",
                    "description": "A natural, brief 3 to 5 word spoken filler phrase to say aloud right now while searching (e.g., 'Searching the web now...')."
                },
                "time_filter": {
                    "type": "string",
                    "enum": ["any", "day", "week", "month", "year"],
                    "description": "Optional recency filter. Use 'day' or 'week' for breaking news or recent events. Default: 'any'."
                },
                "ranking_mode": {
                    "type": "string",
                    "enum": ["sparse", "dense", "hybrid"],
                    "description": "Passage relevance ranking strategy: 'sparse' (BM25 keyword matching), 'dense' (neural semantic embedding), or 'hybrid' (RRF fusion of sparse + dense). Default: 'hybrid'."
                },
                "deadline_ms": {
                    "type": "integer",
                    "description": "Optional execution deadline in milliseconds for this search request (e.g., 5000)."
                },
                "max_passages": {
                    "type": "integer",
                    "minimum": 1,
                    "maximum": 10,
                    "description": "Maximum number of distinct evidence passages to return (1-10). Default: 5. Actual returned count may be reduced based on available context budget."
                }
            },
            "required": ["query", "spoken_filler"]
        })
    }

    fn flow(&self) -> ToolFlow {
        ToolFlow::NonTerminal
    }

    fn execute<'a>(
        &'a self,
        _mode: PipelineMode,
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

            let spoken_filler = args
                .get("spoken_filler")
                .and_then(|v| v.as_str())
                .unwrap_or("Searching the web now...")
                .trim()
                .to_string();

            let time_filter = match args.get("time_filter").and_then(|v| v.as_str()) {
                Some("day") => nexus::TimeFilter::Day,
                Some("week") => nexus::TimeFilter::Week,
                Some("month") => nexus::TimeFilter::Month,
                Some("year") => nexus::TimeFilter::Year,
                _ => nexus::TimeFilter::Any,
            };

            let ranking_mode = match args.get("ranking_mode").and_then(|v| v.as_str()) {
                Some("sparse") => nexus::RankingMode::Sparse,
                Some("dense") => nexus::RankingMode::Dense,
                _ => nexus::RankingMode::Hybrid,
            };

            let max_passages = args
                .get("max_passages")
                .and_then(|v| v.as_u64())
                .map(|v| (v as usize).clamp(1, 10))
                .unwrap_or(DEFAULT_MAX_PASSAGES);

            let requested_deadline = args.get("deadline_ms").and_then(|v| v.as_u64());
            let default_timeout_ms = match ranking_mode {
                nexus::RankingMode::Sparse => TIMEOUT_SPARSE_MS,
                nexus::RankingMode::Dense => TIMEOUT_DENSE_MS,
                nexus::RankingMode::Hybrid => TIMEOUT_HYBRID_MS,
            };
            let effective_deadline_ms = requested_deadline
                .unwrap_or(default_timeout_ms)
                .clamp(TIMEOUT_FLOOR_MS, TIMEOUT_CEILING_MS);
            let fanout_budget_ms =
                ((effective_deadline_ms as f32 * 0.18) as u64).clamp(1000, 3000);
            let fetch_budget_ms = (effective_deadline_ms.saturating_sub(fanout_budget_ms + 1200))
                .clamp(1500, DEFAULT_FETCH_TIMEOUT_MS);

            log::info!(
                "[WebSearchTool] Turn {} executing web_search for '{}' (time_filter={:?}, ranking_mode={:?}, max_passages={}, deadline={}ms, fanout={}ms, fetch={}ms)",
                ctx.turn_id,
                query,
                time_filter,
                ranking_mode,
                max_passages,
                effective_deadline_ms,
                fanout_budget_ms,
                fetch_budget_ms
            );

            let options = nexus::NexusSearchOptions {
                time_filter,
                ranking_mode,
                max_candidates: DEFAULT_FETCH_CANDIDATES,
                fetch_timeout_ms: fetch_budget_ms,
                max_response_bytes: DEFAULT_MAX_RESPONSE_BYTES,
                chunk_size_words: DEFAULT_CHUNK_SIZE_WORDS,
                chunk_overlap_words: DEFAULT_CHUNK_OVERLAP_WORDS,
                fanout_deadline_ms: Some(fanout_budget_ms),
            };

            let search_client = Arc::clone(&ctx.app_state.nexus_search);
            let query_cloned = query.clone();
            let token_ceiling = ctx.max_observation_tokens.unwrap_or(HARD_TOKEN_CEILING);

            let search_start = std::time::Instant::now();
            let execute_search = async move {
                let result = search_client.search(&query_cloned, &options).await?;
                let m = &result.metrics;
                log::info!(
                    "[WebSearchTool::Metrics] Total: {}ms | Fanout: {}ms (raw={}, dedup={} in {}ms) | Fetch: {}ms ({} pages) | Extract: {}ms ({} pages) | Chunk: {}ms ({} passages) | Rank: {}ms (mode={:?}, sparse={:?}, dense={:?}, rrf={:?})",
                    m.total_pipeline_ms,
                    m.fanout_total_ms,
                    m.total_raw_hits,
                    m.deduplicated_hits,
                    m.url_dedup_ms,
                    m.fetch_total_ms,
                    m.pages_fetched.len(),
                    m.extraction_total_ms,
                    m.pages_extracted.len(),
                    m.chunking_total_ms,
                    m.total_passages_generated,
                    m.ranking_total_ms,
                    m.ranking_mode,
                    m.sparse_ranking_ms,
                    m.dense_ranking_ms,
                    m.rrf_fusion_ms,
                );
                for eng in &m.engines {
                    log::info!(
                        "[WebSearchTool::Engine] {:?}: duration={}ms, hits={}, ok={}, err={:?}",
                        eng.engine,
                        eng.latency_ms,
                        eng.hit_count,
                        eng.success,
                        eng.error
                    );
                }
                for page in &m.pages_fetched {
                    log::info!(
                        "[WebSearchTool::Fetch] {} -> duration={}ms, bytes={}, hops={}, ok={}, err={:?}",
                        page.requested_url,
                        page.total_fetch_ms,
                        page.bytes_read,
                        page.hop_count,
                        page.success,
                        page.error
                    );
                }
                for page in &m.pages_extracted {
                    log::info!(
                        "[WebSearchTool::Extract] {} -> duration={}ms, markdown_bytes={}",
                        page.url,
                        page.extraction_ms,
                        page.markdown_bytes
                    );
                }

                if result.scored_passages.is_empty() {
                    log::info!(
                        "[WebSearchTool] Web search returned zero passages for '{}'",
                        query_cloned
                    );
                    return Ok(render_status_xml(
                        "empty_results",
                        "not_found",
                        &format!(
                            "Web search completed for '{}' but no relevant passages were found.",
                            xml_escape(&query_cloned)
                        ),
                        "Try rephrasing the query with different keywords or broader terms.",
                        None,
                    ));
                }

                // Answer-presence verification gate.
                //
                // Mechanism B: the correct page is fetched and ranked, but the answer
                // sentence never survives chunk selection, so the evidence is topical
                // background with no answer. Reporting success there invites the model to
                // answer from parametric memory. The verdict comes from nexuss, which has
                // the full passage text; this layer only decides what to tell the model.
                //
                // The old inline check was too weak: it matched a hardcoded substring list
                // and accepted any digit anywhere, so a page full of unrelated numerals
                // passed. See `nexus::answer_presence`.
                if result.metrics.answer_presence == Some(nexus::AnswerPresence::Absent) {
                    log::warn!(
                        "[WebSearchTool] Query '{}' shape={:?} returned {} passages but none carry a candidate answer.",
                        query_cloned,
                        result.metrics.answer_shape,
                        result.scored_passages.len()
                    );
                    return Ok(render_status_xml(
                        "empty_results",
                        "missing_factual_answer",
                        &format!(
                            "Web search found pages about '{}' but none of them state the specific answer. Do not answer from memory; report that the source was not found.",
                            xml_escape(&query_cloned)
                        ),
                        "Try rephrasing with the concrete entity name, or search for the specific quantity.",
                        None,
                    ));
                }

                let xml_start = std::time::Instant::now();
                let rendered = render_evidence_xml(
                    &query_cloned,
                    ranking_mode,
                    &result.scored_passages,
                    max_passages,
                    token_ceiling,
                    Some(&result.metrics),
                );
                let xml_render_ms = xml_start.elapsed().as_millis() as u64;
                log::info!(
                    "[WebSearchTool::XML] Rendered in {}ms (total observation bytes={})",
                    xml_render_ms,
                    rendered.len()
                );
                Ok(rendered)
            };

            let wrapped_fut = std::panic::AssertUnwindSafe(async {
                tokio::time::timeout(
                    Duration::from_millis(effective_deadline_ms),
                    execute_search,
                )
                .await
            });

            let observation = match wrapped_fut.catch_unwind().await {
                Ok(Ok(Ok(rendered))) => rendered,
                Ok(Ok(Err(err))) => {
                    log::warn!("[WebSearchTool] Web search pipeline failed: {err}");
                    match err {
                        nexus::NexusError::PrivateIpBlocked(_)
                        | nexus::NexusError::DnsResolution { .. }
                        | nexus::NexusError::Http(_)
                        | nexus::NexusError::ScraperTransport(_)
                        | nexus::NexusError::AllProvidersFailed { .. } => render_status_xml(
                            "network.failed",
                            "transient",
                            "The external search providers could not be reached.",
                            "Inform the user that web search is currently unreachable or try again with a simpler query.",
                            None,
                        ),
                        _ => render_status_xml(
                            "empty_results",
                            "not_found",
                            &format!(
                                "Web search completed for '{}' but no relevant passages were found.",
                                xml_escape(&query)
                            ),
                            "Try rephrasing the query with different keywords or broader terms.",
                            None,
                        ),
                    }
                }
                Ok(Err(_timeout)) => {
                    let elapsed_ms = search_start.elapsed().as_millis() as u64;
                    log::warn!(
                        "[WebSearchTool] Web search timed out after {}ms for '{}'",
                        effective_deadline_ms,
                        query
                    );
                    render_status_xml(
                        "deadline.hit",
                        "timeout",
                        "Web search reached the execution deadline before completing within the deadline.",
                        "Inform the user or retry with ranking_mode=\"sparse\" for faster results.",
                        Some(elapsed_ms),
                    )
                }
                Err(_panic) => {
                    log::error!(
                        "[WebSearchTool] Panic recovered in web_search for '{}'",
                        query
                    );
                    render_status_xml(
                        "panic_recovered",
                        "fatal",
                        "Web search encountered an internal error and was recovered safely.",
                        "Rephrase or simplify the search query.",
                        None,
                    )
                }
            };

            Ok(ToolResult::new(observation).with_spoken_filler(spoken_filler))
        }
        .boxed()
    }
}

/// Renders structured XML status blocks for tool errors, timeouts, or empty states.
fn render_status_xml(
    code: &str,
    error_kind: &str,
    message: &str,
    next_action: &str,
    elapsed_ms: Option<u64>,
) -> String {
    let elapsed_attr = match elapsed_ms {
        Some(ms) => format!(" elapsed_ms=\"{}\"", ms),
        None => String::new(),
    };
    format!(
        "<web_search_status code=\"{}\" error_kind=\"{}\"{}>\n  <message>{}</message>\n  <next_action>{}</next_action>\n</web_search_status>",
        code, error_kind, elapsed_attr, message, next_action
    )
}

/// Computes dynamic context budget clamping with progressive passage popping and renders bounded XML evidence.
fn render_evidence_xml(
    query: &str,
    ranking_mode: nexus::RankingMode,
    scored_passages: &[nexus::ScoredPassage],
    max_passages: usize,
    max_observation_tokens: usize,
    metrics: Option<&nexus::NexusSearchMetrics>,
) -> String {
    let token_ceiling = max_observation_tokens.min(HARD_TOKEN_CEILING);
    let mut admitted_k = max_passages.min(scored_passages.len());
    log::info!(
        "[WebSearchTool::XML] Rendering evidence for query='{}': scored_passages={}, max_passages={}, admitted_k={}, token_ceiling={}",
        query,
        scored_passages.len(),
        max_passages,
        admitted_k,
        token_ceiling
    );

    let ranking_str = match ranking_mode {
        nexus::RankingMode::Sparse => "sparse",
        nexus::RankingMode::Dense => "dense",
        nexus::RankingMode::Hybrid => "hybrid",
    };

    let metrics_attr = if let Some(m) = metrics {
        format!(
            " total_duration_ms=\"{}\" fanout_ms=\"{}\" url_dedup_ms=\"{}\" fetch_ms=\"{}\" extract_ms=\"{}\" chunk_ms=\"{}\" rank_ms=\"{}\"",
            m.total_pipeline_ms,
            m.fanout_total_ms,
            m.url_dedup_ms,
            m.fetch_total_ms,
            m.extraction_total_ms,
            m.chunking_total_ms,
            m.ranking_total_ms,
        )
    } else {
        String::new()
    };

    // Progressive passage popping loop:
    // If the assembled XML observation exceeds token_ceiling, pop the lowest-ranking admitted passage
    // and re-evaluate until it fits or no passages remain.
    while admitted_k > 0 {
        let selected = &scored_passages[..admitted_k];

        // Group selected passages by source URL while strictly preserving rank order
        struct SourceGroup<'a> {
            id: usize,
            title: &'a str,
            url: &'a str,
            passages: Vec<(usize, f32, String)>, // (rank, score, capped_text)
        }

        let mut sources_list: Vec<SourceGroup> = Vec::new();
        let mut url_to_idx: HashMap<&str, usize> = HashMap::new();

        for (idx, passage) in selected.iter().enumerate() {
            let rank = idx + 1;
            let url = passage.source_url.as_str();
            let title = passage.source_title.as_str();
            let capped = cap_passage_text(&passage.text);

            if let Some(&source_idx) = url_to_idx.get(url) {
                sources_list[source_idx]
                    .passages
                    .push((rank, passage.score, capped));
            } else {
                let new_id = sources_list.len() + 1;
                url_to_idx.insert(url, sources_list.len());
                sources_list.push(SourceGroup {
                    id: new_id,
                    title,
                    url,
                    passages: vec![(rank, passage.score, capped)],
                });
            }
        }

        let header = format!(
            "<web_search_evidence query=\"{}\" ranking_mode=\"{}\" total_sources=\"{}\" total_passages=\"{}\"{}>\n",
            xml_escape(query),
            ranking_str,
            sources_list.len(),
            admitted_k,
            metrics_attr
        );
        let footer = "</web_search_evidence>";

        let mut xml = String::with_capacity(token_ceiling * 4);
        xml.push_str(&header);
        for source in &sources_list {
            xml.push_str(&format!(
                "  <source id=\"{}\" title=\"{}\" url=\"{}\">\n",
                source.id,
                xml_escape(source.title),
                xml_escape(source.url)
            ));
            for (rank, score, text) in &source.passages {
                xml.push_str(&format!(
                    "    <passage rank=\"{}\" score=\"{:.3}\">\n      {}\n    </passage>\n",
                    rank,
                    score,
                    xml_escape(text)
                ));
            }
            xml.push_str("  </source>\n");
        }
        xml.push_str(footer);

        let final_tokens = estimate_tokens(&xml);
        log::info!(
            "[WebSearchTool::XML] Candidate admitted_k={} produced {} tokens (token_ceiling={})",
            admitted_k,
            final_tokens,
            token_ceiling
        );
        if final_tokens <= token_ceiling {
            return xml;
        }

        // Exceeded token_ceiling: pop the lowest-ranking passage
        admitted_k -= 1;
    }

    // If zero passages can fit within the context budget:
    render_status_xml(
        "empty_results",
        "budget_constrained",
        "Web search completed but available context was insufficient to fit results.",
        "Try a more specific query.",
        None,
    )
}

/// Caps a single passage at [`MAX_PASSAGE_CHARS`], cutting at a word boundary
/// and marking the cut. Unbounded passages are truncated cleanly.
fn cap_passage_text(text: &str) -> String {
    let trimmed = text.trim();
    if trimmed.chars().count() <= MAX_PASSAGE_CHARS {
        return trimmed.to_string();
    }
    let mut end = 0usize;
    for (count, (i, c)) in trimmed.char_indices().enumerate() {
        if count >= MAX_PASSAGE_CHARS {
            break;
        }
        end = i + c.len_utf8();
    }
    let mut cut = trimmed[..end].to_string();
    // Back off to the last word boundary so we never end mid-token.
    if let Some(ws) = cut.rfind(char::is_whitespace) {
        cut.truncate(ws);
    }
    cut.push('…');
    cut
}

/// Escapes standard XML entity delimiters to prevent structural corruption and prompt boundary attacks.
fn xml_escape(input: &str) -> String {
    let mut escaped = String::with_capacity(input.len());
    for ch in input.chars() {
        match ch {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&apos;"),
            '\u{0000}'..='\u{0008}' | '\u{000B}'..='\u{000C}' | '\u{000E}'..='\u{001F}' => {
                // Strip XML-invalid C0 control characters
            }
            _ => escaped.push(ch),
        }
    }
    escaped
}
