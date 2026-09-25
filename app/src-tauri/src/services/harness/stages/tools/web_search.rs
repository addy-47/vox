use std::{collections::HashMap, sync::Arc, time::Duration};

use futures_util::future::{BoxFuture, FutureExt};
use serde_json::{json, Value};

use super::{ToolDefinition, ToolDomain, ToolError, ToolExecutionContext, ToolResult};
use crate::{core::settings::PipelineMode, services::llm::ToolFlow};

// ─── Level 3 Domain Constants ────────────────────────────────────────────────
pub const DEFAULT_MAX_PASSAGES: usize = 5;
pub const DEFAULT_FETCH_CANDIDATES: usize = 3;
pub const DEFAULT_FETCH_TIMEOUT_MS: u64 = 4000;
pub const DEFAULT_MAX_RESPONSE_BYTES: usize = 512_000;
pub const DEFAULT_CHUNK_SIZE_WORDS: usize = 150;
pub const DEFAULT_CHUNK_OVERLAP_WORDS: usize = 30;

pub const DEFAULT_MIN_REPORTING_ENGINES: usize = 2;
pub const DEFAULT_MIN_DISTINCT_DOMAINS: usize = 3;
pub const DEFAULT_MIN_CANDIDATE_HITS: usize = 8;
pub const DEFAULT_FANOUT_DEADLINE_MS: u64 = 1200;
pub const DEFAULT_TWO_STAGE_RERANK: bool = true;

pub const MAX_CONTEXT_SHARE_CAP: f32 = 0.30;
pub const HARD_TOKEN_CEILING: usize = 2000;
pub const AVERAGE_PASSAGE_TOKENS: usize = 250;

pub const TIMEOUT_SPARSE_MS: u64 = 4000;
pub const TIMEOUT_DENSE_MS: u64 = 5000;
pub const TIMEOUT_HYBRID_MS: u64 = 5500;
pub const TIMEOUT_OUTER_SAFETY_MS: u64 = 10000;

/// Adapter bridging Vox's in-process ONNX MiniLM singleton to the `nexus::traits::TextEmbedder` trait.
#[derive(Clone, Copy, Debug, Default)]
pub struct VoxEmbedder;

#[async_trait::async_trait]
impl nexus::traits::TextEmbedder for VoxEmbedder {
    async fn embed_text(&self, text: &str) -> Result<Vec<f32>, nexus::NexusError> {
        let _ = crate::services::memory::ml::embedder::ensure_embedder_loaded(true);
        crate::services::memory::ml::embedder::generate_embedding(text)
            .map_err(|e| nexus::NexusError::Embedding(e.to_string()))?
            .ok_or_else(|| {
                nexus::NexusError::Embedding("Embedder singleton not available".to_string())
            })
    }

    async fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>, nexus::NexusError> {
        let _ = crate::services::memory::ml::embedder::ensure_embedder_loaded(true);
        crate::services::memory::ml::embedder::generate_embeddings_batch(texts)
            .map_err(|e| nexus::NexusError::Embedding(e.to_string()))?
            .ok_or_else(|| {
                nexus::NexusError::Embedding("Embedder singleton not available".to_string())
            })
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
                "max_passages": {
                    "type": "integer",
                    "minimum": 1,
                    "maximum": 10,
                    "description": "Maximum number of distinct evidence passages to return (1-10). Default: 5. Actual returned count may be reduced based on available context budget."
                },
                "spoken_filler": {
                    "type": "string",
                    "description": "A natural, brief 3 to 5 word spoken filler phrase to say aloud right now while searching (e.g., 'Searching the web now...')."
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

            let adaptive_timeout_ms = match ranking_mode {
                nexus::RankingMode::Sparse => TIMEOUT_SPARSE_MS,
                nexus::RankingMode::Dense => TIMEOUT_DENSE_MS,
                nexus::RankingMode::Hybrid => TIMEOUT_HYBRID_MS,
            }
            .min(TIMEOUT_OUTER_SAFETY_MS);

            log::info!(
                "[WebSearchTool] Turn {} executing web_search for '{}' (time_filter={:?}, ranking_mode={:?}, max_passages={}, timeout={}ms)",
                ctx.turn_id,
                query,
                time_filter,
                ranking_mode,
                max_passages,
                adaptive_timeout_ms
            );

            let options = nexus::NexusSearchOptions {
                time_filter,
                ranking_mode,
                max_candidates: DEFAULT_FETCH_CANDIDATES,
                fetch_timeout_ms: DEFAULT_FETCH_TIMEOUT_MS,
                max_response_bytes: DEFAULT_MAX_RESPONSE_BYTES,
                chunk_size_words: DEFAULT_CHUNK_SIZE_WORDS,
                chunk_overlap_words: DEFAULT_CHUNK_OVERLAP_WORDS,
            };

            let fanout_policy = nexus::model::FanoutPolicy {
                min_reporting_engines: DEFAULT_MIN_REPORTING_ENGINES,
                min_distinct_domains: DEFAULT_MIN_DISTINCT_DOMAINS,
                min_candidate_hits: DEFAULT_MIN_CANDIDATE_HITS,
                max_fanout_deadline_ms: DEFAULT_FANOUT_DEADLINE_MS,
            };

            let ranking_policy = nexus::model::RankingPolicy {
                two_stage_reranking: DEFAULT_TWO_STAGE_RERANK,
                ..Default::default()
            };

            let search_client = match nexus::NexusSearch::builder()
                .with_engines(vec![
                    nexus::Engine::Duckduckgo,
                    nexus::Engine::Bing,
                    nexus::Engine::GoogleWml,
                    nexus::Engine::Mojeek,
                    nexus::Engine::Yahoo,
                ])
                .with_fanout_policy(fanout_policy)
                .with_ranking_policy(ranking_policy)
                .with_embedder(Arc::new(VoxEmbedder))
                .build()
            {
                Ok(client) => client,
                Err(err) => {
                    log::error!("[WebSearchTool] Failed to initialize NexusSearch client: {err}");
                    return Ok(ToolResult::new(
                        "Web search unavailable: network connection could not be established.",
                    )
                    .with_spoken_filler(spoken_filler));
                }
            };

            let search_and_render = async {
                let result = search_client.search(&query, &options).await?;
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
                        eng.engine, eng.latency_ms, eng.hit_count, eng.success, eng.error
                    );
                }
                for page in &m.pages_fetched {
                    log::info!(
                        "[WebSearchTool::Fetch] {} -> duration={}ms, bytes={}, hops={}, ok={}, err={:?}",
                        page.requested_url, page.total_fetch_ms, page.bytes_read, page.hop_count, page.success, page.error
                    );
                }
                for page in &m.pages_extracted {
                    log::info!(
                        "[WebSearchTool::Extract] {} -> duration={}ms, markdown_bytes={}",
                        page.url, page.extraction_ms, page.markdown_bytes
                    );
                }

                if result.scored_passages.is_empty() {
                    log::info!("[WebSearchTool] Web search returned zero passages for '{}'", query);
                    Ok(format!(
                        "Web search completed for '{}'. No relevant web results could be retrieved.",
                        xml_escape(&query)
                    ))
                } else {
                    let xml_start = std::time::Instant::now();
                    let rendered = render_evidence_xml(
                        &query,
                        ranking_mode,
                        &result.scored_passages,
                        max_passages,
                        ctx,
                        Some(&result.metrics),
                    )
                    .await;
                    let xml_render_ms = xml_start.elapsed().as_millis() as u64;
                    log::info!(
                        "[WebSearchTool::XML] Rendered in {}ms (total observation bytes={})",
                        xml_render_ms,
                        rendered.len()
                    );
                    Ok(rendered)
                }
            };

            let search_result = tokio::time::timeout(
                Duration::from_millis(adaptive_timeout_ms),
                search_and_render,
            )
            .await;

            let observation = match search_result {
                Ok(Ok(rendered)) => rendered,
                Ok(Err(err)) => {
                    log::warn!("[WebSearchTool] Web search pipeline failed: {err}");
                    match err {
                        nexus::NexusError::PrivateIpBlocked(_)
                        | nexus::NexusError::DnsResolution { .. }
                        | nexus::NexusError::Http(_)
                        | nexus::NexusError::ScraperTransport(_)
                        | nexus::NexusError::AllProvidersFailed { .. } => {
                            "Web search unavailable: network connection could not be established.".to_string()
                        }
                        _ => {
                            format!(
                                "Web search completed for '{}'. No relevant web results could be retrieved.",
                                xml_escape(&query)
                            )
                        }
                    }
                }
                Err(_) => {
                    log::warn!("[WebSearchTool] Web search timed out after {}ms for '{}'", adaptive_timeout_ms, query);
                    format!(
                        "Web search completed for '{}'. No relevant web results could be retrieved.",
                        xml_escape(&query)
                    )
                }
            };

            Ok(ToolResult::new(observation).with_spoken_filler(spoken_filler))
        }
        .boxed()
    }
}

/// Computes dynamic context budget clamping and renders bounded XML evidence per Section 4.3 & 5.1.
async fn render_evidence_xml(
    query: &str,
    ranking_mode: nexus::RankingMode,
    scored_passages: &[nexus::ScoredPassage],
    max_passages: usize,
    ctx: &ToolExecutionContext,
    metrics: Option<&nexus::NexusSearchMetrics>,
) -> String {
    // 1. Stage 3 dynamic context ceiling computation
    let (context_window, max_output) = {
        if let Ok(guard) = ctx.app_state.settings.read() {
            (
                guard.llm.context_window as usize,
                guard.llm.max_output_tokens as usize,
            )
        } else {
            (8192, 120)
        }
    };
    let usable_tokens = context_window.saturating_sub(max_output).max(1);

    let live_tracked = ctx.app_state.turn_metrics.context_tokens_used();
    let current_tracked_tokens = if live_tracked > 0 {
        live_tracked
    } else if let Ok(conn) = ctx.app_state.db.connect() {
        if let Ok(turns) = crate::persistence::compactions::fetch_turns_for_compaction(
            &conn,
            ctx.session_id,
            1,
            ctx.turn_id,
        )
        .await
        {
            turns
                .iter()
                .map(|t| {
                    crate::services::memory::ml::estimate_tokens(&t.user_text)
                        + crate::services::memory::ml::estimate_tokens(&t.assistant_text)
                })
                .sum()
        } else {
            0
        }
    } else {
        0
    };

    let remaining_tokens = usable_tokens.saturating_sub(current_tracked_tokens).max(1);
    let token_ceiling =
        (((remaining_tokens as f32) * MAX_CONTEXT_SHARE_CAP) as usize).min(HARD_TOKEN_CEILING);
    let budget_k = (token_ceiling / AVERAGE_PASSAGE_TOKENS).max(1);
    let effective_k = max_passages.min(budget_k).min(scored_passages.len());

    let selected = &scored_passages[..effective_k];

    // Group selected passages by source URL while strictly preserving rank order
    struct SourceGroup<'a> {
        id: usize,
        title: &'a str,
        url: &'a str,
        passages: Vec<(usize, f32, &'a str)>, // (rank, score, text)
    }

    let mut sources_list: Vec<SourceGroup> = Vec::new();
    let mut url_to_idx: HashMap<&str, usize> = HashMap::new();

    for (idx, passage) in selected.iter().enumerate() {
        let rank = idx + 1;
        let url = passage.source_url.as_str();
        let title = passage.source_title.as_str();

        if let Some(&source_idx) = url_to_idx.get(url) {
            sources_list[source_idx]
                .passages
                .push((rank, passage.score, &passage.text));
        } else {
            let new_id = sources_list.len() + 1;
            url_to_idx.insert(url, sources_list.len());
            sources_list.push(SourceGroup {
                id: new_id,
                title,
                url,
                passages: vec![(rank, passage.score, &passage.text)],
            });
        }
    }

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

    let mut xml = String::new();
    xml.push_str(&format!(
        "<web_search_evidence query=\"{}\" ranking_mode=\"{}\" total_sources=\"{}\" total_passages=\"{}\"{}>\n",
        xml_escape(query),
        ranking_str,
        sources_list.len(),
        selected.len(),
        metrics_attr
    ));

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
                xml_escape(text.trim())
            ));
        }

        xml.push_str("  </source>\n");
    }

    xml.push_str("</web_search_evidence>");
    xml
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
