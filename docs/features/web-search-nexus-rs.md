# Web Search — `nexuss` Crate Architecture

**Last Updated:** 2026-09-25

The `nexuss` crate (published as `nexuss` on crates.io) is a standalone, zero-bloat Rust library powering Vox's `web_search` tool. It handles the full lifecycle from multi-engine query fanout through SSRF-hardened page extraction to tri-mode relevance ranking. This document focuses on the crate's internal logic and module responsibilities; the Vox-side wiring (tool definition, context budget clamping, XML evidence rendering) is covered in the [tools specification](file:///home/addy/projects/apps/vox/docs/specs/tools-spec.md).

---

## Crate Overview

| Property | Value |
|----------|-------|
| **Name** | `nexus-rs` |
| **Library crate** | `nexus` |
| **Edition** | 2024 |
| **Rust minimum** | 1.89 |
| **Location** | `submodules/nexus-rs/` |
| **License** | MIT |
| **Repository** | `https://github.com/addy-47/nexus-rs.git` |

### Public API Surface

The crate exposes a single entry point — `NexusSearch::builder()` — which returns a `NexusSearchBuilder`. The builder validates configuration and constructs a `NexusSearch` orchestrator. All other types are re-exported from `lib.rs`:

```rust
use nexus::{
    NexusSearch, NexusSearchBuilder, NexusSearchOptions, NexusSearchResult, NexusSearchMetrics,
    Engine, RankingMode, TimeFilter, FanoutPolicy, RankingPolicy, TextEmbedder, NexusError,
    RawPage, ScoredPassage, EgressFetcher,
};
```

---

## Module Architecture

```
nexus-rs/
├── src/
│   ├── lib.rs              ← Public re-exports and module declarations
│   ├── builder.rs          ← Fluent NexusSearchBuilder with validation
│   ├── pipeline.rs         ← NexusSearch orchestrator: 3-stage search pipeline
│   ├── model.rs            ← All domain structs, enums, and configuration
│   ├── error.rs            ← Strongly typed NexusError enum
│   ├── traits.rs           ← TextEmbedder async trait (dense ranking decoupling)
│   ├── engines/            ← Provider scrapers and fanout orchestration
│   │   ├── mod.rs          ← EngineFanout, consensus corroboration, TLS client
│   │   ├── helpers.rs      ← URL normalization, domain extraction, SERP streaming
│   │   ├── duckduckgo.rs   ← DuckDuckGo HTML scraper
│   │   ├── bing.rs         ← Bing HTML scraper
│   │   ├── yahoo.rs        ← Yahoo HTML scraper
│   │   ├── mojeek.rs       ← Mojeek HTML scraper
│   │   └── google_wml.rs   ← Google mobile WML scraper (no-JS)
│   ├── fetcher/            ← SSRF-hardened HTTP egress
│   │   ├── mod.rs          ← EgressFetcher, concurrent fetch with semaphore
│   │   ├── client.rs       ← Bounded body reader
│   │   ├── connector.rs    ← Socket-pinned reqwest client builder
│   │   ├── dns.rs          ← IP safety validation (RFC1918, link-local, etc.)
│   │   └── redirect.rs     ← Manual redirect loop with per-hop DNS pre-flight
│   ├── chunking/           ← Sliding-window passage segmentation
│   │   ├── mod.rs          ← chunk_pages entry point
│   │   └── passage.rs      ← Per-document sliding window chunker
│   ├── ranking/            ← Tri-mode relevance scoring
│   │   ├── mod.rs          ← rank_passages dispatcher
│   │   ├── bm25.rs         ← Lexical BM25 scorer (k1=1.5, b=0.75)
│   │   ├── dense.rs        ← Neural cosine similarity scorer
│   │   └── hybrid.rs       ← RRF fusion with 2-stage re-ranking
│   └── extraction/         ← DOM parsing and Markdown normalization
│       ├── mod.rs          ← extract_document entry point
│       └── cleaner.rs      ← Readability extraction, HTML-to-Markdown, title parsing
```

---

## Core Pipeline: `NexusSearch::search()`

The `search()` method in `pipeline.rs` executes three sequential stages with granular telemetry:

### Stage 1 — Multi-Engine Fanout (`EngineFanout::query_all`)

All enabled engines are queried concurrently via `FuturesUnordered` with a deadline-based early-exit quorum. Each engine dispatches through `primp` (TLS-impersonated HTTP client) with randomized browser fingerprint (Chrome/Firefox × Windows/macOS).

**Consensus Corroboration:** After all queries complete, URLs are normalized via `helpers::norm_url_key()` (strips `www.`, trailing slashes, UTM parameters, tracking fragments). A URL enters the `consensus_urls` set if it appears across ≥2 distinct `Engine::index_family()` groups. This consensus set later drives the `consensus_multiplier` in ranking.

**Adaptive Quorum Early-Exit:** The fanout loop checks after each completed engine whether:
- `successful_queries >= min_reporting_engines` (default: 2)
- `seen_domains.len() >= min_distinct_domains` (default: 3)
- `seen_canonical.len() >= min_candidate_hits` (default: 8)

If all three conditions are met, remaining engine queries are dropped. A hard deadline (`max_fanout_deadline_ms`, default: 1200ms) also terminates the loop via `tokio::select!` timeout.

**Interleave-and-Deduplicate:** Hits from all engines are merged in round-robin order by index position, deduplicating on canonical URL keys. This preserves engine diversity while eliminating duplicate results.

### Stage 1B — Hardened Fetch & Extraction (`EgressFetcher::fetch_all_concurrent`)

Candidate URLs are fetched concurrently with a `Semaphore` bounding concurrency (default: 3). Each fetch goes through the full SSRF egress pipeline:

1. **Scheme Validation:** Only `http`/`https` accepted.
2. **DNS Pre-flight:** `resolve_and_validate_host()` resolves the host and checks every resulting IP against the SSRF policy.
3. **Socket Pinning:** `build_pinned_client()` creates a `reqwest::Client` bound to the validated socket addresses via `.resolve_to_addrs()`, preventing TOCTOU DNS rebinding.
4. **Manual Redirect Loop:** Reqwest's automatic redirect following is disabled (`Policy::none()`). Up to 5 redirect hops are followed manually, each requiring fresh DNS validation and socket pinning.
5. **Bounded Body Reading:** Response streams are terminated if payload exceeds `max_response_bytes` (default: 512 KB).

After fetching, `extraction::extract_document()` uses `scraper` for DOM parsing and `html-to-markdown-rs` (v3.14.1) for HTML-to-Markdown conversion. Pages are truncated to `DEFAULT_MAX_PAGE_CHARS` (30,000 chars) to prevent token explosion.

### Stage 2 — Chunking & Ranking (`chunk_and_rank_passages`)

**Sliding-Window Chunking:** Each `RawPage.markdown` is segmented into overlapping passages of `chunk_size_words` (default: 150) with `chunk_overlap_words` (default: 30) overlap. Each passage is tagged with its source URL, page title, and deterministic index.

**Tri-Mode Ranking** (`ranking::rank_passages`):

| Mode | Algorithm | Embedder Required | Description |
|------|-----------|-------------------|-------------|
| `Sparse` | BM25 lexical | No | Term-frequency scoring with `k1=1.5, b=0.75` |
| `Dense` | Cosine similarity | Yes | Neural embedding via `TextEmbedder` trait |
| `Hybrid` | RRF fusion | Yes | Combines BM25 + Dense via Reciprocal Rank Fusion |

**2-Stage Re-Ranking (Hybrid only):** When `RankingPolicy.two_stage_reranking` is enabled and passages exceed `max_candidates_to_rerank` (default: 10), the ranker first sorts by BM25 score, then only embeds the top-k candidates. This avoids expensive neural inference on all passages. The `consensus_multiplier` (default: 1.5) boosts passages whose source URL was corroborated by ≥2 index families.

**RRF Formula:** `score = 1/(k + sparse_rank) + 1/(k + dense_rank)`, then multiplied by `consensus_multiplier` if applicable. Final sort is descending by `score`.

---

## SSRF Security: `polyc-egress` Pattern

The fetcher module implements a defense-in-depth strategy against server-side request forgery:

- **Pre-flight DNS Evaluation:** Every hostname resolves before any connection. All resolved IPs are checked against blocked ranges: loopback (`127.0.0.0/8`, `::1`), RFC 1918 private (`10.0.0.0/8`, `172.16.0.0/12`, `192.168.0.0/16`), link-local/cloud metadata (`169.254.0.0/16`), carrier-grade NAT (`100.64.0.0/10`), multicast, documentation nets, IPv6 ULA (`fc00::/7`), and IPv4-mapped IPv6.
- **DNS Rebinding Protection:** Socket pinning via `.resolve_to_addrs()` binds the client to the pre-flight validated IPs. Even if DNS changes between resolution and connection, the client connects only to the pinned addresses.
- **Manual Redirect Validation:** Each redirect hop re-executes DNS pre-flight and socket pinning. The 5-hop limit prevents redirect loops.
- **System Proxy Isolation:** `.no_proxy()` disables ambient system/proxy configuration.
- **Strict Scheme Whitelist:** Only `http` and `https` schemes pass validation.

---

## Key Types and Configuration

### `NexusSearchOptions`

Controls a single search execution. Defaults are optimized for latency and precision:

| Field | Default | Description |
|-------|---------|-------------|
| `time_filter` | `TimeFilter::Any` | Provider recency filter |
| `ranking_mode` | `RankingMode::Sparse` | Scoring algorithm |
| `max_candidates` | 3 | Max candidate URLs to fetch |
| `chunk_size_words` | 150 | Passage chunk size |
| `chunk_overlap_words` | 30 | Overlap between chunks |
| `fetch_timeout_ms` | 4000 | Per-page fetch timeout |
| `max_response_bytes` | 524,288 | Byte cap per response |

### `FanoutPolicy`

Controls the adaptive quorum early-exit behavior:

| Field | Default | Description |
|-------|---------|-------------|
| `min_reporting_engines` | 2 | Minimum engines responding before early exit |
| `min_distinct_domains` | 3 | Minimum distinct domains for quorum |
| `min_candidate_hits` | 8 | Minimum deduplicated URLs for quorum |
| `max_fanout_deadline_ms` | 1200 | Hard deadline for fanout wave |

### `RankingPolicy`

Controls the ranking behavior:

| Field | Default | Description |
|-------|---------|-------------|
| `two_stage_reranking` | `true` | Prune candidates before dense embedding |
| `max_candidates_to_rerank` | 10 | Top-k passages for neural scoring |
| `consensus_multiplier` | 1.5 | Score boost for corroborated URLs |

### `Engine` Enum

Six providers are supported: `Duckduckgo`, `Bing`, `Yahoo`, `Mojeek`, `GoogleWml`, `Brave`. Each has an `index_family()` method used for consensus corroboration. `Brave` is defined but returns `InvalidConfiguration` at dispatch time (not yet implemented).

---

## The `TextEmbedder` Trait

Dense and hybrid ranking are decoupled through the `TextEmbedder` async trait. The host application provides the implementation — this allows Vox to use its existing ONNX MiniLM singleton (`all-MiniLM-L6-v2`, 384-dim) without the crate depending on any ML framework.

```rust
#[async_trait]
pub trait TextEmbedder: Send + Sync {
    async fn embed_text(&self, text: &str) -> Result<Vec<f32>, NexusError>;
    async fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>, NexusError>;
}
```

**Vox Adapter (`VoxEmbedder`):** In `web_search.rs`, `VoxEmbedder` wraps the in-process ONNX singleton. It calls `ensure_embedder_loaded(true)` to lazily initialize the model, then delegates to `generate_embedding()` / `generate_embeddings_batch()`. Any embedder error is mapped to `NexusError::Embedding`.

---

## Telemetry & Metrics

Every pipeline stage produces granular timing metrics collected in `NexusSearchMetrics`:

- **Fanout:** `fanout_total_ms`, per-engine `EngineQueryMetrics` (latency, hit count, success, error)
- **Dedup:** `url_dedup_ms`, `deduplicated_hits`, `total_raw_hits`
- **Fetch:** `fetch_total_ms`, per-URL `PageFetchMetrics` (DNS ms, bytes, hop count, success)
- **Extraction:** `extraction_total_ms`, per-page `PageExtractMetrics` (extraction ms, markdown bytes)
- **Chunking:** `chunking_total_ms`, `total_passages_generated`
- **Ranking:** `ranking_total_ms`, `sparse_ranking_ms`, `dense_ranking_ms`, `rrf_fusion_ms`

These metrics are logged by `WebSearchTool` at `info` level and embedded in the `<web_search_evidence>` XML output as attributes.

---

## Error Taxonomy

`NexusError` is a strongly typed enum covering the full failure surface:

| Variant | Origin | Meaning |
|---------|--------|---------|
| `InvalidUrl(String)` | URL parsing / scheme validation | Malformed or unsupported scheme |
| `DnsResolution { host, message }` | DNS pre-flight | Host could not be resolved |
| `PrivateIpBlocked(IpAddr)` | SSRF guard | Resolved IP is private/loopback/reserved |
| `TooManyRedirects { max_hops, url }` | Redirect loop | Exceeded 5-hop redirect limit |
| `ResponseTooLarge { limit_bytes, url }` | Body bounded | Payload exceeded byte cap |
| `Http(reqwest::Error)` | Transport | HTTP transport error |
| `ScraperTransport(String)` | Engine query | SERP fetch via primp failed |
| `SerpParse { engine, message }` | Engine query | SERP HTML structure unrecognized |
| `HttpStatus { status, url }` | Fetch | Non-success HTTP response |
| `AllProvidersFailed { attempted }` | Fanout | Zero engines returned results |
| `Timeout { url, timeout_ms }` | Fetch | Request exceeded deadline |
| `Embedding(String)` | TextEmbedder | Downstream embedding failure |
| `InvalidConfiguration(String)` | Builder / ranking | Invalid parameter configuration |
| `Io(std::io::Error)` | I/O | Standard I/O failure |

---

## Vox Integration Summary

The `web_search.rs` tool adapter in Vox:

1. **Constructs** a `NexusSearch` via `NexusSearch::builder()` with 5 engines (DuckDuckGo, Bing, GoogleWML, Mojeek, Yahoo), a `VoxEmbedder`, and `FanoutPolicy`/`RankingPolicy` defaults.
2. **Maps** user-facing parameters (`time_filter`, `ranking_mode`, `max_passages`) to `NexusSearchOptions`.
3. **Executes** `search_client.search(&query, &options)` with an adaptive timeout (4–5.5s depending on ranking mode, capped at 10s).
4. **Renders** results into `<web_search_evidence>` XML with context-budget clamping (30% of remaining tokens, hard cap at 2000 tokens).
5. **Handles** errors by mapping `NexusError` variants to user-facing messages, with network errors producing a "unavailable" response and empty results producing a "no relevant results" response.

Level 3 domain constants in `web_search.rs` (`DEFAULT_MAX_PASSAGES`, `DEFAULT_FETCH_CANDIDATES`, `DEFAULT_CHUNK_SIZE_WORDS`, etc.) mirror the crate defaults and serve as the Vox-side configuration layer.
