# Web Search Production Hardening — Forensic Report & Plan

> **Status:** Investigation complete. Phase A harness built. Baseline NOT yet run.
> Corrigendum (2026-10-04): the harness architecture was corrected mid-build. The
> eval drives a *scripted tool call through the real harness* via a stateful mock
> LLM; the harness output boundary (the final request body) is the measurement.
> See §8.8.
> **Scope:** `submodules/nexus-rs/` (the `nexuss` crate) + `app/src-tauri/src/services/harness/stages/tools/` (wiring).
> **Specs requiring amendment *before* code** (AGENTS.md §4.3): `docs/specs/tools-specs/web-search.md`, `docs/specs/tools-specs/lld.md`, `docs/features/web-search-nexus-rs.md`.
> **Constraint carried into every design decision below:** depth, focus and variety are **parameters on the existing `web_search` call**. No `web_search_more`, no `deep_research`, no additional tool round-trips for depth. `web_fetch` is the only new tool.

---

## Table of Contents

- [Part 0 — Direct answers](#part-0--direct-answers)
- [Part 1 — Forensic findings (F1–F9)](#part-1--forensic-findings-f1f9)
- [Part 2 — How this actually improves the LLM's context](#part-2--how-this-actually-improves-the-llms-context)
- [Part 3 — Should we just use DonSeTch?](#part-3--should-we-just-use-donsetch)
- [Part 4 — Why two tools (search + fetch)](#part-4--why-two-tools-search--fetch)
- [Part 5 — The embedding unlock](#part-5--the-embedding-unlock)
- [Part 6 — Target `web_search` parameter surface](#part-6--target-web_search-parameter-surface)
- [Part 7 — Batches](#part-7--batches)
- [Part 8 — Baseline evaluation (Phase A)](#part-8--baseline-evaluation-phase-a)
- [Part 9 — Risks](#part-9--risks)
- [Part 10 — Open questions](#part-10--open-questions)

---

## Part 0 — Direct answers

### 0.1 "Your plan doesn't tell me how this improves web search. How do we get better context for the LLM with more variety? How do we unlock deep-research logic? I don't want more tool calls."

This is the right question and the earlier draft answered it badly. Here is the concrete mechanism.

The LLM's context is bounded by `web-search.md` §4.3:

```
token_ceiling = min(remaining_tokens × 0.30, 2000)
effective_k   = min(max_passages, token_ceiling / 250, ‖scored_passages‖)
```

That is **~8 passages maximum, ever**, inside ~2000 tokens. So "better context" cannot mean *more text*. It can only mean one of exactly four things, and every batch in Part 7 buys one of them:

| Lever | What the LLM gets | Current state | Batches |
|---|---|---|---|
| **1. Variety** | 8 passages = 8 *distinct claims from 8 distinct sources* | 3 candidates, no `max_per_domain`, 2 of 3 slots routinely spent on one apex domain | W1, W3 |
| **2. Density** | Every one of those 2000 tokens carries information | Up to 75 chunks generated, 5 delivered, zero dedup. Boilerplate survives | W3 |
| **3. Coverage (depth)** | Multiple *aspects* of the question inside **one call** | One flat query → one SERP wave → 3 pages. No sub-querying, no second-degree traversal | W1, W3 |
| **4. Precision** | The passages that survive are the ones that actually answer *this* sub-question | Fixed 150-word blind window; no focus, no verification | W3, W6 |

**On deep research without a new tool:** `depth: "deep"` runs **2–4 sub-queries and a second-degree link traversal entirely inside the single `web_search` call**. The model makes one tool call and receives a synthesized multi-source evidence matrix. `web-search.md` §7.1 already architected this as `deep_research`; the correction is that it must be a **parameter**, not a tool, because a tool boundary costs a full LLM round-trip (~1–3s + tokens) and the model cannot know in advance whether it needs depth. Depth becomes a *harness* decision informed by query shape, with the model able to override.

**On "tell me more":** the model re-issues `web_search` with the *same or refined query* plus `focus: "<the specific aspect>"`. The harness recognizes the follow-up (session-scoped corpus cache + semantic similarity) and serves it from the retained corpus with **zero network** — a cache hit, not a new tool call. If the cache misses, it falls through to a normal fanout. The model needs no new tool name in its schema.

**On "same query returns same results, wasteful":** handled three ways inside the same call — semantic query cache (§Part 5 A1), semantic near-duplicate collapse (§Part 5 A2), and MMR diversity (§Part 5 A2). See §Part 2.3.

### 0.2 "We don't use `bge-m3` due to latency issues — but something we can try."

Agreed, and that is exactly the right way to treat it: **measure, don't assume.** `bge-m3` (`model_quantized.onnx`, 543.3MB, 1024-dim) is already on disk with `.verified` sidecars but appears **zero times** in `~/.vox/models/models_manifest.json` — unregistered, unloaded, unreachable from `embedder.rs`, which hardcodes `PRIMARY_EMBEDDING_MODEL_DIR = "minilm-l12-v2"` and `EMBEDDING_DIM = 384`.

Part 8 therefore builds the baseline harness with a **pluggable embedder** so `minilm-l12-v2` vs `bge-m3` is an A/B flag, not a rewrite. If latency rises without a measured relevance gain, `bge-m3` stays unregistered and we lose nothing but the flag.

---

## Part 1 — Forensic findings (F1–F9)

All Vox paths verified against codebase-memory graph `generation 2026-10-03T19:09:41Z` (`check_index_coverage`: `no_recorded_issue` / `metadata_match` for all five cited files) **plus** direct source reads. Engine claims verified by live HTTP probe from this host.

### F1 — Mojeek is a hard CAPTCHA wall. It is not a crash; it is a structural dead engine.

Three failures in one day, `~/.vox/diagnostics/logs/vox.log.2026-10-03` lines **212, 439, 820**:

```
WARN nexus::engines: [Nexus::Engines] mojeek query error after 539ms:
  SERP parsing error for engine 'mojeek': Bot challenge encountered on Mojeek
```

Live probe, real Chrome UA, no TLS impersonation required to reproduce:

```
GET https://www.mojeek.com/search?q=test+query  →  HTTP 200, 5499 bytes
<title>Captcha</title>
```

`is_mojeek_challenge()` (`mojeek.rs:100-118`) detects this **correctly**. Detection is not the bug. The bug is that there is **no quarantine, no health memory, and no circuit breaker**, so Mojeek burns a fanout slot and 539–830 ms on *every query, forever*.

`google_wml` is the same story — `vox.log.2026-10-03:501`:

```
google_wml query error after 677ms: Google WML returned status 403 Forbidden
```

Probe: `google.com/wml/search` → **403 on both a desktop UA and the hardcoded Nokia UA** (`google_wml.rs:10`). The `gbv=1` fallback returns an `enablejs` redirect shell with **zero `<h3>` tags** — no server-rendered results. Google is unavailable without a JS engine.

### F2 — ⚠️ Consensus corroboration is *permanently* dead code.

`model.rs:335-340`:

```rust
Self::Duckduckgo | Self::Bing | Self::Yahoo => "bing",   // ← SAME FAMILY
Self::Mojeek    => "mojeek",   // dead (F1)
Self::GoogleWml => "google",   // dead (F1)
```

`engines/mod.rs:158-170` admits a URL to `consensus_urls` only when `families.len() >= 2`.

With Mojeek and Google dead, **only the `"bing"` family remains**, therefore `consensus_urls` is *always empty*, therefore the `consensus_multiplier = 1.5` boost at `hybrid.rs:82-86` **never fires**. The headline ranking feature documented in both `web-search.md` §7.1 and `web-search-nexus-rs.md` §"2-Stage Re-Ranking" is inert.

> **CORRIGENDUM 2026-10-04 — the collapse above is CORRECT; do not "fix" it.**
> DonSeTch (95.5% measured answer-in-snippet, verified from its README 2026-10-04) lists its
> backends as **"Bing family (Bing, DuckDuckGo, Yahoo), Brave, Mojeek, Google"** — the identical
> collapse, deliberate. Yahoo's index genuinely *is* Bing's; un-collapsing would let DDG↔Bing
> agreement (one upstream) fire the "corroboration" boost, which is worse than inert.
> The true root cause of F2 is **family count, not family mapping**: we have 3 families with 2
> dead, leaving exactly 1 live family. The fix is to **add genuinely independent families**
> (keyless Brave scrape + keyless verticals as independent families — W1), **not** to
> un-collapse `index_family()`. A regression test must assert
> `Duckduckgo == Bing == Yahoo` under `index_family()` so no future reader re-breaks this.

Compounding: `search.yahoo.com` → `curl: (56) Failure when receiving data from the peer`. Yahoo never appears in the logs at all because the quorum early-exit (`engines/mod.rs:126-133`) fires after Bing + DDG and drops it mid-flight. **Correct behavior masking a third dead engine.**

> `grep -n "index_family\|consensus" submodules/nexus-rs/tests/*.rs` → **0 hits.** Entirely untested.

### F3 — ⚠️ The page fetcher has *no* stealth. The SERP layer has all of it.

This is the structural inversion, and it is the single most important finding.

The **engine** layer impersonates Chrome (`engines/mod.rs:239-262`):

```rust
primp::Client::builder()
    .impersonate(primp::Impersonate::ChromeV146)
    .impersonate_os(primp::ImpersonateOS::Windows)
```

The **page fetcher** does not (`fetcher/connector.rs:19-27`):

```rust
reqwest::Client::builder()
    .resolve_to_addrs(host, resolved_addrs)
    .user_agent(DEFAULT_USER_AGENT)   // a UA *string* — trivially fingerprinted
```

`reqwest` + `rustls` produces a generic rustls ClientHello and a generic Akamai h2 fingerprint. Neither resembles Chrome. Worse, `build_pinned_client` constructs a **brand-new `reqwest::Client` per host, per redirect chain** (`redirect.rs:47-58`), so every fetch is a cold TLS handshake with no session resumption and no HTTP/2 connection reuse. Scrapers never resume; browsers always do.

Measured consequence, `vox.log.2026-10-03:228-236`:

```
Fetch: 961ms (3 pages) | Extract: 55ms (1 pages)        ← 2 of 3 pages LOST

[WebSearchTool::Fetch] https://gemini.google.com/            → HTTP transport error
[WebSearchTool::Fetch] https://gemini.google.com/?hl=en-IN  → HTTP transport error
[WebSearchTool::Fetch] https://blog.google/.../gemini-4-argon/ → ok, 405965 bytes, hops=1
```

Two Google properties, one fetcher, one run. `blog.google` succeeded; `gemini.google.com` failed at the **transport** layer (`hops=0`, "error sending request") — TLS/connect, not HTTP status. `curl` with a full Chrome header set over HTTP/2 gets **200** from `gemini.google.com`.

**Hypothesis (repro required, see Part 8 gate V3):** Google's stricter bot detection rejects the rustls ClientHello + h2 fingerprint that curl's OpenSSL stack does not trigger. DonSeTch's entire published thesis is that this exact gap is what separates "works" from "walled".

### F4 — ⚠️ Retrieval silently collapses in both directions.

Full telemetry sweep, `grep "Nexus::Telemetry" ~/.vox/diagnostics/logs/vox.log.2026-10-0*`:

| Query | Fetched | Extracted | Passages | Delivered |
|---|---|---|---|---|
| `Google Gemini latest model 2026 specifications capabilities` | 3 | 3 | 16 | 5 |
| `Google Gemini latest model launch 2025 features` | 3 | 3 | **75** | 5 |
| `Gemini 4 Argon features release date specs` | 3 | **1** | 15 | 5 |
| `Gemini 4 Argon latest model` | 3 | **1** | 15 | 5 |
| `Gemini 4 Argon` | 3 | **2** | 46 | 5 |
| `OpenAI Solana GPU cluster deployment status 2024` | 3 | **1** | **1** | **1** |

Two failure modes, both unacceptable:

- **Starvation.** `max_candidates = 3` is hardcoded at `web_search.rs:17`. Two transient fetch failures = **67% evidence loss**, returned as HTTP 200 with confident framing. The `OpenAI Solana…` row is the pathological case: 3 pages fetched, 1 extracted, **one passage** in the ranked corpus.
- **Flood.** 75 passages generated, 5 delivered. 70 chunks of extraction + embedding + ranking compute, ~93% discarded with **zero deduplication** — no near-duplicate collapse, no diversity selection.

Compounding both: `max_per_domain` (`web-search.md` §7.1.4) is **unimplemented**, so `gemini.google.com/` and `gemini.google.com/?hl=en-IN` both became candidates. `norm_url_key` (`helpers.rs:73-125`) preserves `?hl=en-IN`, so canonical dedup did not catch it either. **Two of three evidence slots spent on a single apex domain.**

### F5 — Mean pooling includes special tokens. The embedding signal is degraded at the source.

`app/src-tauri/src/services/memory/ml/embedder.rs:148`:

```rust
.encode_batch(texts.to_vec(), true)   // ← true == add_special_tokens
```

`[CLS]` and `[SEP]` are then folded into `sum_embeddings` weighted by an attention mask of `1` (`embedder.rs:222-235`). `all-MiniLM-*` was trained with mean pooling over **non-special** tokens (or `[CLS]` pooling). The current output is a blend of both — the worst of the three options.

Every dense and hybrid score in production is computed on this vector, and the **same code path serves `search_memory`**, so personal-memory retrieval is degraded by the same defect.

### F6 — `bge-m3` is on disk, 543 MB, and completely unreachable.

```
~/.vox/models/embedding/bge-m3/model_quantized.onnx   543.3M
~/.vox/models/embedding/bge-m3/tokenizer.json          16.3M
```
Both with `.verified` sidecars, so they passed whatever verification pipeline exists. But `grep "bge-m3" ~/.vox/models/models_manifest.json` → **0 hits**. Not registered, not loaded, not in the manifest, not referenced by `embedder.rs`. A downloaded, verified, orphaned 1024-dim multilingual retrieval asset. See §0.2 and Part 8.

### F7 — Blocking ONNX inside `async` defeats the declared timeout and violates Axiom 4.

`VoxEmbedder::embed_batch` (`web_search.rs:54-61`) calls `embedder::generate_embeddings_batch`, which is **fully synchronous**: holds a `parking_lot::Mutex`, runs ONNX inference, contains **zero `.await`**. It is invoked from inside `tokio::time::timeout(Duration::from_millis(5500), …)`.

Evidence: `vox.log.2026-10-03:228` reports `total=5522ms` against a declared `timeout=5500ms`, with **no timeout log line** and a successful XML render. A future that never yields cannot be preempted by `tokio::time::Timeout`.

Two consequences beyond wall-clock:
1. The adaptive timeout in `web-search.md` §6.1 is **not enforced** for `ranking_mode: dense | hybrid`.
2. A tokio worker is pinned for the duration of ONNX inference. `web-search.md` §1 Axiom 4 requires that "neural passage embedding execute asynchronously on dedicated worker threads, with zero locking or allocation on real-time audio threads." It does not.

**Correction to an earlier claim in this thread:** I initially attributed a per-search cold model load to `engine.rs:417` (`unload_all_onnx_models`). That is wrong — it is inside `stop_audio_engine` (shutdown), and `ensure_memory_embedder` (`engine.rs:553`) pre-warms at startup. Latency is **not** the problem. F7 is a **robustness and hot-path-isolation** defect, and should be framed as such.

### F8 — Spec drift. Must be corrected before code (AGENTS.md §4.3).

| Spec claim | Reality |
|---|---|
| `web-search.md:99` and `web-search-nexus-rs.md:181`: `all-MiniLM-L6-v2` | `minilm-l12-v2` (MiniLM-L12-H384, 112.6 MB int8, 384-dim) |
| `web-search.md:126`: `max_response_bytes = 512,000` | Crate default `524_288`; Vox constant `512_000` — two values, one name |
| `web-search.md` §1 Axiom 1: unified single-pass retrieval | Must be amended to accommodate `web_fetch` |
| `web-search.md` §7.1.2–7.1.5: v2 roadmap | All four unimplemented; items land in this plan as **parameters** |
| `web-search.md` §7.2: "DonSeTch strategies adopted for v2" | None adopted |

### F9 — The eval harness cannot compile or run.

`evals/agentic_tools_eval.rs:24` imports four modules that **do not exist**:

```rust
use common::{audio, db, harness, judge, report};
//                          ^^^^^^ ^^^^^^^ ^^^^^ ^^^^^^
//        common/mod.rs declares only: compaction_eval, consolidation_eval,
//        datasets, db, ingestion_eval, llm_client, reporting
```

Additionally:
- **Not registered in `Cargo.toml`** — no `[[bench]]` entry, so it cannot be invoked at all. Its own doc comment claims `cargo run --release --bin agentic_tool_eval`, which is also wrong (it is neither a bin nor a registered bench).
- `harness.set_tool_registry(...)` at line 180 — **`Harness` has no such method**. `chassis.rs` exposes `tool_registry()` (getter, `:316`), `title_set()` (`:320`) and `set_title_set()` (`:358`) but no registry mutator.
- `#[path = "common/mod.rs"] mod common;` (line 13) vs `mod common;` in `memory_eval.rs:12` — two different module-resolution styles for the same tree.
- `common/mod.rs` exports `compaction_eval`, `ingestion_eval`, `consolidation_eval` at the **top level of `common/`**. These are memory-pipeline-specific, not shared. This is the structural problem: there is no real `common/` layer.
- `audit_run.sh` is memory-pipeline-specific (hardcodes `eval_vox.db`, `raw_llm_traces.json`, and the three memory judges).
- No `web_search` eval exists anywhere.

---

## Part 2 — How this actually improves the LLM's context

This section is the answer to "how does this make web search better", expressed as before → after on real telemetry.

### 2.1 Variety — the 8-passage budget stops being spent on one source

**Today.** `max_candidates = 3`, no `max_per_domain`, consensus dead. Real run (`vox.log.2026-10-03`): candidates were `gemini.google.com/`, `gemini.google.com/?hl=en-IN`, `blog.google/…/gemini-4-argon/`. **Two of three slots, one apex domain.** Two failed to fetch anyway (F3). Net result: **one page, 15 passages, all from one source.**

**After.** Three independent mechanisms:
1. **W1** restores working engines across ≥2 genuinely independent index families, so `consensus_urls` is non-empty and `consensus_multiplier` actually fires — cross-index corroboration becomes a real ranking signal instead of a no-op.
2. **W1** implements `max_per_domain ≤ 1` (spec §7.1.4) at candidate-selection time.
3. **W3** adds **embedding-cluster-aware** domain capping: candidates are clustered by query-embedding similarity and the cap applies per *cluster*, not per domain. This catches `gemini.google.com/?hl=en-IN` (different URL, same content) which string-level dedup misses.

**Net effect on the LLM.** `max_passages: 5` becomes 5 claims from 5 sources, each independently corroborated, instead of 5 paragraphs of one blog post. For a factual voice answer this is the difference between "here is what one site said" and "here is what five sites agree on, and one dissents."

### 2.2 Density — every one of the ~2000 tokens carries information

**Today.** 75 passages generated, 5 delivered, no dedup. Passage text is a blind 150-word window (30 overlap) over 30,000-char pages (`DEFAULT_MAX_PAGE_CHARS`, `extraction`), so it routinely straddles headings and carries nav/boilerplate that survived the selector strip list.

**After.**
1. **W3** — semantic near-duplicate collapse at cosine ≥ 0.95, then **MMR** selection so the 5 delivered passages are 5 *angles*, not 5 adjacent windows of one document.
2. **W3** — block-typed extraction (heading / paragraph / list / table / code / quote with heading breadcrumbs) replacing the blind word-count window as the selection substrate. DonSeTch's `DonSift` reports 80%+ context reduction from block-level focus.
3. **W3** — answer-span windowing: instead of shipping a fixed 150 words, slide a ~40-word window to the locally-maximal query similarity. ~60% token cut at equal-or-better answer density.

**Net effect on the LLM.** The 30% / 2000-token ceiling stops being the binding constraint on *quality*. Today `budget_k = 2000/250 = 8`, but `‖scored_passages‖` and near-duplicate content mean the 8 slots carry maybe 3 distinct facts. After, 8 slots carry 8.

### 2.3 Coverage — depth inside one call

**Today.** One flat query → one SERP wave → top 3 URLs → 3 pages. Nothing more. A comparative or investigative question ("compare X and Y with benchmarks") gets whatever the single query happened to surface.

**After.** `depth: "deep"` triggers, **inside the single `web_search` call**:
1. **Heuristic + embedding intent classification** (`web-search.md` §7.2.1) routes the query: `news` forces `time_filter: day|week` and applies recency decay; `technical` bypasses general engines for developer verticals; `academic` routes to arXiv.
2. **Sub-query decomposition** into 2–4 targeted sub-queries. The model may supply them via `sub_queries: [...]`; otherwise the harness derives them by embedding-similarity clustering of the original query's key aspects.
3. **Second-degree link traversal** (spec §7.1.2): parse outbound `<a href>` from high-ranking pages, score against sub-queries, breadth-first fetch into official docs / whitepapers / primary announcements.
4. **Cross-source evidence matrix** across 6–10 distinct domains with contradiction flagging for the LLM to reconcile.

**Net effect on the LLM.** The model gets a synthesized multi-source matrix from **one** tool call. Today this would cost 4 separate tool round-trips, each a full LLM generation (~1–3s + tokens) that the model has to decide to spend. Making it a parameter means depth is a harness policy decision, not a model decision — and the model can still override with `depth: "quick"`.

### 2.4 Precision — the passages that survive answer *this* sub-question

**Today.** Passages are ranked against the **original** query embedding. A follow-up ("tell me more about the second point's methodology") either re-runs the whole pipeline against a badly-diluted search query, or gets answered from the 5 passages already in context — which may not contain the answer, because they were selected for a different question.

**After.** `focus: "<aspect>"` scores the **retained corpus blocks** against the *follow-up* with **BM25 block relevance** — DonSeTch's approach (12-language tokenizer: CJK uni+bi-grams, stemming, accent folding), zero extra inference, claimed 80%+ context reduction (`web_search.rs` currently discards the corpus entirely at `render_evidence_xml`, `&scored_passages[..effective_k]`). Embedding re-score is reserved as a fallback, gated on eval evidence that BM25 misses. `must_contain: ["..."]` verifies a specific claim against full page text and returns `MATCH` / `NO-MATCH` plus ≤3 excerpts — ~60 tokens instead of 4k.

**Net effect on the LLM.** The 30%-budget evidence block gets *re-used* rather than re-fetched, and re-used *against the actual question being asked*. This is the mechanism that makes "tell me more" work without `web_search_more`.

### 2.5 Summary table

| Failure observed | Evidence | Lever | Batch | Model-visible improvement |
|---|---|---|---|---|
| Mojeek + Google dead, 539–830 ms/query wasted | `vox.log.2026-10-03:212,439,501,820` | Health + quarantine | W1 | Faster, and a live engine pool |
| Consensus never fires | `model.rs:335-340` + `engines/mod.rs:158-170` | Add 4th family (keyless Brave + verticals); `index_family()` deliberately unchanged (see F2 corrigendum) | W1 | Cross-source corroboration becomes a real ranking signal |
| 2/3 pages fail to fetch | `vox.log.2026-10-03:228-236` | Fetcher TLS stealth | W2 | 3 pages of evidence instead of 1 |
| 75 passages → 5, no dedup | `vox.log.2026-10-02:268` | Semantic dedup + MMR | W3 | 5 distinct facts instead of 5 adjacent windows |
| 1 passage returned as 200 OK | `vox.log.2026-02:1465` | `fetch_slack` over-fetch | W2 | Non-empty evidence on hard queries |
| 2 of 3 slots on one domain | `vox.log.2026-10-03:231-232` | `max_per_domain` + cluster cap | W1, W3 | Cross-domain variety |
| Corpus discarded at tool boundary | `web_search.rs:395` | Turn-scoped corpus | W5 | Follow-ups served from cache |
| No query cache | absent from `nexus-rs/src/` | Semantic cache + single-flight | W4 | Repeat/paraphrase = 0 network |
| Mean pooling includes `[CLS]`/`[SEP]` | `embedder.rs:148` | Exclude special tokens | W3 | Better dense ranking **and** better personal memory |
| Blocking ONNX in `async` | `web_search.rs:54-61` | `spawn_blocking` | W3 | Timeout actually enforced; Axiom 4 satisfied |

---

## Part 3 — Should we just use DonSeTch?

**No. Keep `nexus-rs`. Reimplement DonSeTch's algorithms; do not take the dependency.**

| Dimension | `nexus-rs` (keep) | DonSeTch as a dependency | Verdict |
|---|---|---|---|
| **License** | MIT | **AGPL-3.0** | ❌ **Disqualifying.** `app/src-tauri/Cargo.toml` has no `license` field → proprietary. AGPL linking forces Vox open-source. Not a trade-off; a blocker. |
| **Integration shape** | Library crate, `nexuss = "0.1.1"` | **Binary only** (MCP server / CLI). There is no `donsetch = "…"` crate. | ❌ Cannot `cargo add` it. |
| **Dependency weight** | 4,061 LoC, pure Rust | 8k+ LoC **plus** BoringSSL built from source (Go + NASM + CMake), PDFium, optional ONNX Runtime, headless Chromium (~400 MB) | ❌ Vox already owns ONNX Runtime, ASR, TTS, embedder. Second runtime + second Chromium. |
| **Process boundary** | In-process, zero IPC | MCP stdio/HTTP hop | ❌ Subprocess + JSON-RPC hop in the audio-critical turn path. Violates Axiom 4. |
| **Separability** | Clean `TextEmbedder` trait boundary | Search's solve-and-bounce depends on DonShadow's cookie jar; fetch's ghost depends on Chromium. **Not separable.** | ❌ Cannot lift the search half without the transport half. |
| **SSRF posture** | DNS pre-flight + socket pinning + 5-hop manual redirect re-validation | Comparable (tier-1) | ➖ Tie |
| **Keyless search quality** | **2 working engines, 1 index family** (F1, F2) | 6 engines / 4 families + 8 verticals + learned health | ❌ They win, decisively |
| **Prompt-budget authority** | Full. We own the 30% ceiling, XML sandbox, negative inoculation | Opaque; MCP results are model-rendered | ✅ **We win** — this is a voice assistant with a hard context ceiling |
| **Injection sandbox** | `<web_search_evidence>` + system-prompt inoculation | Raw markdown | ✅ We win |
| **Control over rank → evidence** | `render_evidence_xml` | Opaque | ✅ We win |
| **Algorithmic reference value** | — | **Excellent.** Best-in-class public reference | ✅ **Their real value** |
| **Maintenance burden** | Ours | Upstream cadence, plus we still own the glue | ➖ Tie |

**Conclusion.** DonSeTch is an *algorithm catalog*, not a dependency. Its own README states the transport is built from scratch — which is *why* its search quality is high. You cannot lift the search without the transport. Every feature worth having is portable logic: health EWMA (200 LoC), quarantine (80 LoC), cross-encoder rerank (150 LoC), block-typed extraction with focus (600 LoC), keyless verticals (400 LoC), consensus scoring (already present, just broken).

> **VERIFIED 2026-10-04 (from the DonSeTch README, primary source).** Additionally portable, in priority order:
> 1. **`deadline_ms` on every tool + real cancellation + `code`/`errorKind`/`next_action` structured failures** — `deadline.hit` as a *result*, not an exception. This is the deadline contract adopted in Part 11.
> 2. **Honest stop reasons** (`FrontierEmpty`, `MaxPages`, `CharBudget`, `DepthLimit`, `Deadline`, `ThrottledOut`) — adopted in Part 11.
> 3. **Entity-coverage penalty** — anchor entities (versions, years, proper nouns) checked against results; wrong entity → 0.3×. NOT previously in this plan; added to W3. Fixture test must assert no fire on abstract queries.
> 4. **Google profile rotation** — 7 Nokia profiles; the succeeding profile stays preferred per egress; CAPTCHA advances the cursor circularly. Replaces our single hardcoded UA (W1).
> 5. **Warm handoff** — search pre-fetches top results so the next `fetch S1` serves from cache in ~3ms. Validates the W5/W6 corpus design.
> 6. **Token handles** — results as `S1…Sn`, fetchable by handle in 3 tokens instead of 80. Added to W3.
> 7. **`dns_cache_ttl_secs = 30`** — already in W2.
> 8. **Benchmark shape to mirror** — 110 questions / 11 niches / answer-in-snippet vs a keyed baseline (theirs: 95.5% vs Tavily 93.3%, LLM-graded, reproducible script). Our eval's QA subagent plays Tavily's role.

*Optional, out of scope, noted for the record:* a user-installed **optional MCP sidecar** behind a settings toggle, for "deep research" turns only, never on the hot path. AGPL isolation is acceptable for an opt-in separate process. Not in this plan.

---

## Part 4 — Why two tools (search + fetch)

DonSeTch ships `web_search` **and** `web_fetch`. This is not redundancy — they are different axes.

1. **Different computations.** `web_search` ranks *URLs* by query relevance. `web_fetch` ranks *spans inside one known document* by *sub-question* relevance. Nothing is shared.
2. **Snippets are ~200 characters.** Jan's agent guide (merged PR janhq/jan#8460): *"Pick the most relevant result(s) and call web_fetch on their URLs to read the full content — don't rely on snippets alone for anything important."*
3. **Follow-ups are bad search queries but excellent focus queries.** *"Tell me more about the second point's methodology"* dilutes badly through a SERP but is a perfect `focus` string against a document already in hand.
4. **Cost asymmetry.** Search = 5-engine fanout (~900 ms) + N fetches (~1000 ms). Fetch = 1 fetch + local span selection. A second question about a known page must not re-pay the SERP tax.
5. **Different escalation tier.** PDFs, JS shells and Cloudflare interstitials need a browser. A *search* tool must never escalate to one.

**And the Vox-specific sting.** The unified pass (`web-search.md` Axiom 1) fetches 3 pages → chunks into up to 75 passages → delivers 5 → **discards the other 70 and the entire `raw_pages` vector at the tool boundary** (`web_search.rs:395` slices `&scored_passages[..effective_k]`; only the rendered string reaches the scratchpad at `steps.rs:584-590`). The model cannot ask a second question about what it already has. That is the "tell me more" gap: an architectural leak, not a missing heuristic.

**Resolution, matching the stated constraint:** depth stays a **parameter** on `web_search` (`depth`, `focus`, `sub_queries`, `must_contain`). `web_fetch` is added for the genuinely different case — the model holds a URL from a **previous turn or a previous answer** and wants to read it. This requires amending Axiom 1.

---

## Part 5 — The embedding unlock

The embedder is called in **exactly one place** — `score_dense()` at `ranking/dense.rs` — for one cosine similarity per passage. That is the "10%".

### Tier A — no new model, uses the resident MiniLM singleton

| # | Capability | Fixes | Design |
|---|---|---|---|
| **A1** | **Semantic cache lookup** | F4 waste, duplicate queries | Cache keyed on `(intent, query_embedding)`, not string equality. Cosine ≥ ~0.92 against cached query embeddings = hit. A paraphrase costs 0 tokens and 0 network. |
| **A2** | **Semantic near-duplicate collapse + MMR** | F4 flood, §2.2 | Collapse passages with pairwise cosine ≥ 0.95. Then MMR so selected passages are maximally diverse. |
| **A3** | **BM25-scored block focus (DECIDED 2026-10-04)** | §2.2, biggest token win | Score typed blocks against the query/`focus` with 12-language BM25 (CJK uni+bi-grams, stemming, accent folding), keep top-N. DonSeTch's approach, claimed 80%+ cut, zero extra inference. Embedding re-score kept as fallback only, gated on eval evidence that BM25 misses. |
| **A4** | **Corpus re-rank against the focus** | §2.4, "dig deeper" | Embed the follow-up and re-score the **retained corpus embeddings**. Do not reuse the original query embedding. |
| **A5** | **Embedding-cluster `max_per_domain`** | F4 domain waste | Cap per *topical cluster* rather than per domain. Catches `gemini.google.com/?hl=en-IN`. |
| **A6** | **Answer-span windowing** | §2.2 density | Slide a ~40-word window to the locally-maximal query similarity inside the top chunk. |
| **A7** | **Embedding-based intent routing** | §7.2.1 regex classifier | Cosine against intent prototypes (`news` / `technical` / `academic` / `general`) instead of keyword matching. Drives TTL and vertical routing. |
| **A8** | **Cross-turn corpus addressing** | Follow-ups across turns | Persist corpus embeddings keyed `(session_id, turn_id)`. A later user turn ("what was that rate?") semantically hits a prior corpus. |

### Tier B — evaluated in Phase A, adopted only if it earns its place

| # | Capability | Detail |
|---|---|---|
| **B1** | **Cross-encoder rerank** | `Xenova/ms-marco-MiniLM-L-6-v2` ONNX int8, ~23 MB, Apache-2.0. 3rd ranking stage after RRF: `final = 0.6·CE + 0.4·(RRF+BM25+consensus)`. NDCG@10 (TREC DL19) **74.30** vs bi-encoder ~54–60. ~95 pairs/s CPU int8. DonSeTch runs exactly this. **Eval first.** |
| **B2** | **`bge-m3`** | 543 MB asset already on disk, 1024-dim multilingual long-context. **Eval the latency delta in Phase A. Adopt only if latency is acceptable AND relevance measurably improves.** Otherwise it stays unregistered — no loss but a flag. |
| **B3** | **Keyless verticals as engines** | arXiv API, StackExchange API, Wikipedia/Docs REST, GitHub Code/Issues, HN Algolia, MDN. Each is a *real API* — no scraping, no CAPTCHA, no fingerprint battle. Each is its own `index_family()`, which **also fixes F2**. |

---

## Part 6 — Target `web_search` parameter surface

All depth/variety capability expressed as **parameters on the existing call**. No new tools for depth.

```jsonc
{
  "name": "web_search",
  "description": "Searches the live web and extracts relevant, verified passages from top sources to answer questions about current events, technical facts, or documentation.",
  "parameters": {
    "type": "object",
    "properties": {
      "query":        { "type": "string",  "description": "Search query optimized for search engines." },
      "time_filter":  { "type": "string",  "enum": ["any","day","week","month","year"] },
      "ranking_mode": { "type": "string",  "enum": ["sparse","dense","hybrid"], "default": "hybrid" },
      "max_passages": { "type": "integer", "minimum": 1, "maximum": 10, "default": 5 },

      // ── NEW: depth & focus (replaces web_search_more / deep_research) ──
      "depth": {
        "type": "string",
        "enum": ["quick", "standard", "deep"],
        "default": "standard",
        "description": "Retrieval effort. 'quick': single fanout, top 3 pages. 'standard': adaptive quorum + focus extraction. 'deep': 2-4 sub-queries, second-degree link traversal, and a cross-source evidence matrix — all within this single call. Use 'deep' for comparisons, multi-faceted technical questions, and investigative prompts."
      },
      "focus": {
        "type": "string",
        "description": "Optional sub-aspect to narrow extraction and ranking (e.g. 'the methodology section', 'pricing tiers', 'benchmark numbers'). Re-ranks already-retrieved evidence against this specific aspect before spending any network budget. Use this instead of issuing a new search when going deeper on something already found."
      },
      "sub_queries": {
        "type": "array",
        "items": { "type": "string" },
        "maxItems": 4,
        "description": "Optional explicit sub-queries for depth='deep'. If omitted the harness derives them. Only supply these for genuinely multi-faceted questions."
      },
      "must_contain": {
        "type": "array",
        "items": { "type": "string" },
        "maxItems": 3,
        "description": "Optional claims to verify against full page text. Returns MATCH / NO-MATCH plus at most 3 short excerpts per claim, instead of full passages."
      },
      "source_diversity": {
        "type": "boolean",
        "default": true,
        "description": "Enforce cross-source diversity so returned passages come from distinct sources rather than one domain. Leave true unless the user explicitly asks for one source."
      },

      "spoken_filler": { "type": "string", "description": "A natural, brief 3 to 5 word spoken filler phrase to say aloud right now while searching." }
    },
    "required": ["query", "spoken_filler"]
  }
}
```

### The one new tool

```jsonc
{
  "name": "web_fetch",
  "description": "Reads one specific URL as clean markdown. Use when you already know the exact source — from an earlier answer, a link the user mentioned, or a citation — and need its full content. Do not use this to search.",
  "parameters": {
    "type": "object",
    "properties": {
      "url":          { "type": "string",  "description": "Absolute http/https URL to read." },
      "focus":        { "type": "string",  "description": "Optional sub-aspect to extract (e.g. 'installation steps', 'the limitations section')." },
      "must_contain": { "type": "array", "items": { "type": "string" }, "maxItems": 3,
                        "description": "Optional claims to verify against the full page. Returns MATCH / NO-MATCH plus up to 3 short excerpts." },
      "spoken_filler":{ "type": "string",  "description": "A natural, brief 3 to 5 word spoken filler phrase." }
    },
    "required": ["url", "spoken_filler"]
  }
}
```

`web_fetch` reuses the W2-hardened `EgressFetcher` — **zero new attack surface** — and is registered under `ToolDomain::Modular`, `ToolFlow::NonTerminal`.

### Prompt directives (`stages/prompt.rs`)

```
- When asked to compare options, evaluate multiple products, or investigate a
  multi-faceted topic, call web_search with depth="deep".
- When the user asks to go deeper on web evidence already retrieved, call
  web_search again with focus="<the specific aspect>" instead of a broader new query.
- Use web_fetch only when you already know the exact URL. Never use it to search.
- Cite the URLs you rely on. Prefer passages from several distinct sources.
```

---

## Part 7 — Batches

Ordered by blast radius ÷ risk. Each is independently shippable and spec-gated.

```
        ┌─ W1 engine health + verticals + family fix ─┐
W0 ────┤                                              ├─→ W3 semantic ranking ─→ W4 cache ─→ W5 depth params ─→ W6 web_fetch
base-  │                                              │        (+ hoisted client)              (+ corpus retention)
line   └─ W2 fetcher stealth + fetch_slack ──────────┘
```

---

### Batch W0 — Phase A baseline *(do this first; nothing ships before it)*

Build and run the baseline evaluation harness. See **Part 8**. No production code changes.

---

### Batch W1 — Engine health, verticals, and the consensus fix

**Crate only. No Vox changes, no schema change, no model-visible change.**
Spec: amend `web-search-nexus-rs.md` §Core Pipeline to document the health subsystem.

1. **`model.rs`** — **`index_family()` deliberately UNCHANGED** (see F2 corrigendum). `Duckduckgo == Bing == Yahoo` under `index_family()` is correct — Yahoo's index is Bing's. The root cause of F2 is family *count*: add genuinely independent families instead — keyless Brave scrape via the existing `primp` client (the declared-but-unimplemented `Engine::Brave`) plus the W1 verticals below, each its own family. With ≥2 live families, `consensus_multiplier` fires for the first time.
2. **New `engines/health.rs`** — per-engine **EWMA trust** (success / failure / latency) plus **quarantine** with exponential backoff (`15m → 2h` cap). Quarantined engines are skipped *before* the `FuturesUnordered` set is built, so they never consume a fanout slot or deadline. Every quarantine transition logs loudly.
3. **New `engines/verticals/{arxiv,stackexchange,github,hn,mdn}.rs`** — keyless REST/JSON adapters behind the existing `EngineHit` contract. Each is a real API: no scraping, no CAPTCHA, no fingerprint battle.
4. **New `engines/registry.rs`** — per-engine endpoint, rate budget and User-Agent, replacing the hardcoded `NOKIA_USER_AGENT` at `google_wml.rs:10`.
5. **`engines/mod.rs`** — skip quarantined engines pre-dispatch; report `quarantined: Vec<Engine>` in `FanoutOutcome`; enforce `max_per_domain` during candidate selection.
6. **`model.rs`** — `consensus_multiplier` must **degrade to 1.0 with an explicit log** when fewer than 2 families are live, so the next reader does not mistake a silent no-op for a working feature.

**Verification.** New `tests/engine_health_test.rs`: EWMA/quarantine/backoff transitions; `index_family` collapse asserted in the correct direction (`Duckduckgo == Bing == Yahoo`, closing the F2 test gap without re-breaking it); Brave + vertical parsers against recorded fixtures; `max_per_domain`. Live smoke: `examples/basic_search.rs` ×3, assert Mojeek/Google are quarantined after attempt 1 and the fanout budget drops by the reclaimed 539–830 ms.

> **TEST OWNERSHIP (2026-10-04, user directive).** All `nexus-rs` tests are written by a
> **testing subagent** (spawned via the `opencode-subagent` / `agy` / `kilo` skill with a QA
> persona), NOT by the implementing agent. The implementing agent writes production code and
> fixtures only; the testing subagent owns `tests/`, asserts the verification criteria in each
> batch, and must independently reproduce at least one forensic finding (e.g. F2's empty
> `consensus_urls`) as a failing-before-fixed test.

---

### Batch W2 — Fetcher stealth inversion + `fetch_slack`

**Crate only.**
Spec: amend `web-search-nexus-rs.md` §SSRF for the dual-transport egress design.

1. **`fetcher/connector.rs`** — route page fetches through **`primp`** (already a dependency, already proven against these hosts for SERPs) instead of bare `reqwest`+rustls. Keep `reqwest` only where `.resolve_to_addrs()` pinning is load-bearing, or reimplement pinning over a shared `primp` client pool.
2. **Connection reuse** — one `primp::Client` per egress class, cached at `NexusSearch` scope. Kills the per-hop client construction at `redirect.rs:47-58`. Add `dns_cache_ttl_secs` (DonSeTch default 30 s).
3. **Cookie jar** — per-host, restricted to clearance cookies only (`cf_clearance`, `datadome`, `_abck`), never persisted to disk.
4. **`model.rs`** — `PageFetchMetrics` gains `tls_profile`, `reused_connection`, `hops`.
5. **`fetch_slack`** — over-fetch to a configurable count (default 5) and keep the first `max_candidates` that **successfully extract**. Directly kills F4 starvation: today two transient failures cost a third of the evidence.

**Verification.** Reproduce F3 first — a recorded-fixture test asserting `gemini.google.com` and a Cloudflare host now succeed through the fetcher. Network-gated `tests/egress_stealth_test.rs`. Assert the 2-of-3-fetch-loss case drops to 0-of-5 across 20 recorded queries.

---

### Batch W3 — Semantic ranking (Tier A) + the pooling and blocking fixes

**Crate + two narrow Vox seams.**
Spec: amend `web-search.md` §4.2 (chunking and ranking become embedding-aware) and §7.1 (mark 7.1.3 / 7.1.4 implemented).

1. **`traits.rs`** — add a batch-cached embedding interface (`embed_with_cache(&self, texts) -> Vec<Option<Vec<f32>>>`) so A2/A4/A5/A8 never re-embed.
2. **`chunking/passage.rs`** — keep the 150/30 window as the segmentation primitive, but add **block typing** (heading / paragraph / list / table / code / quote) with heading breadcrumbs. This is the substrate A3 and A6 need; a blind word-count window has no structure to focus on.
3. **`ranking/dedupe.rs`** — A2 semantic collapse (cosine ≥ 0.95) + MMR selection.
4. **`ranking/mmr.rs`** — A2 diversity + A5 cluster-based `max_per_domain`.
5. **`extraction/cleaner.rs`** — A3 block focus + A6 answer-span windowing; strip links by default (~30% token reduction).
6. **`ranking/hybrid.rs`** — A1 semantic cache-lookup hook; log `cache: hit_semantic | hit_exact | miss` plus the cosine that decided it.
7. **F5 fix** — `app/src-tauri/src/services/memory/ml/embedder.rs`: exclude special-token positions from the mean. **Isolated commit.** This changes `search_memory` results too; re-run the memory eval before and after.
8. **F7 fix** — `web_search.rs`: `tokio::task::spawn_blocking` around `generate_embeddings_batch` with a bounded pool, so ONNX never occupies a tokio worker (also satisfies Axiom 4). **This is the prerequisite that makes the Part 11 deadline contract enforceable — without it, `deadline_ms` cannot fire during dense/hybrid ranking.**
9. **Entity-coverage penalty (new, from DonSeTch, DECIDED 2026-10-04).** Extract anchor entities (version numbers, years, proper nouns) from the query; multiply passages whose entities contradict the query's anchors by 0.3×. Fixture test must assert the penalty does **not** fire on abstract queries. Small code, high precision value.
10. **Token handles.** Render results with compact handles `S1…Sn` so `focus` follow-ups and `web_fetch` can address a result in ~3 tokens instead of an 80-token URL. Handles are turn-scoped and resolve against the retained corpus (W4/W5).

**Verification.** `tests/semantic_ranking_test.rs`: near-duplicate collapse; MMR diversity invariant (no two selected passages > 0.9 cosine); answer-span token savings; **special-token pooling regression** (an input differing only by explicit special tokens must produce the same vector). Vox-side: the 75-passage case delivers 5 *distinct* passages; the 1-passage case disappears.

---

### Batch W4 — Query cache, single-flight, and client hoisting

**Crate + `AppState`.**
Spec: amend `web-search.md` §4 to add Stage 0. Implements roadmap §7.1.2.

1. **New `cache/`** — LRU (bounded, default 50) keyed on `(intent, query_embedding)` with **intent-aware TTL** (news 15 m / general 1 h / technical-docs 4 h) and **single-flight coalescing** so concurrent identical queries share one in-flight fanout via a keyed mutex. Entries retain the **full `Vec<ScoredPassage>` plus embeddings and `raw_pages`**, not just the rendered string.
2. **`web_search.rs`** — report `cache = hit_exact | hit_semantic | miss` in telemetry and in the XML attributes.
3. **⚠️ Client hoisting (prerequisite, not optimization).** `NexusSearch` is currently **rebuilt on every `execute()` call** (`web_search.rs:203-217`). Without hoisting to `AppState`, the cache dies with the tool call and Batches W5/W6 have nothing to cache. Hoist to a `OnceLock<Arc<NexusSearch>>`.

**Verification.** `tests/query_cache_test.rs`: exact hit; semantic paraphrase hit; TTL expiry per intent; single-flight (N concurrent identical queries → 1 fanout); corpus survival across `execute()` returns.

---

### Batch W5 — Depth as parameters (answers gap #1 and §2.3/§2.4)

**Crate + harness. This is the batch that delivers the stated goal.**
Spec: amend `web-search.md` §3.1 (schema), §4.2 (sub-query + traversal), §7.1.5. **No new tools.**

1. **`AppState`** — `turn_scoped_corpora: HashMap<(session_id, turn_id), Arc<CorpusHandle>>`, LRU + TTL. `CorpusHandle` holds `Vec<ScoredPassage>` + embeddings + the query embedding + `raw_pages` (all of which `NexusSearchResult` already returns and Vox currently discards entirely).
2. **`NexusSearchOptions`** — add `depth: Depth`, `focus: Option<String>`, `sub_queries: Vec<String>`, `must_contain: Vec<String>`, `source_diversity: bool`.
3. **`pipeline.rs`** — `Depth::Deep` runs intent classification → sub-query derivation → **concurrent multi-query fanout with result merging** → second-degree link traversal → cross-source evidence matrix. `Depth::Quick` skips sub-queries and traversal.
4. **`focus`** — A4: on a follow-up call, detect the retained corpus via the hoisted client, re-score its embeddings against the `focus` embedding, and serve **without any network**. Falls through to a normal fanout on a cache miss.
5. **`must_contain`** — verify claims against full page text, return `MATCH` / `NO-MATCH` + ≤3 excerpts.
6. **`web_search.rs`** — surface `depth`, `focus_effective`, `corpus_hit`, `cache` in telemetry and in the `<web_search_evidence>` attributes so every stage is verifiable from a single log line.
7. **`prompt.rs`** — add the directives from Part 6.

**Verification.** `depth="deep"` with the network mocked to a fixture set produces a multi-source matrix from **one** tool call. `focus` on a follow-up returns passages overlapping the focus target and not the original top-5, with `fetch_total_ms == 0`. `must_contain` returns the correct verdict for known-present and known-absent strings.

---

### Batch W6 — `web_fetch`

**Spec: amend Axiom 1 in `web-search.md`** — mandatory pre-modification gate.

1. Reuses `EgressFetcher` (already SSRF-hardened, already extracting) — **zero new attack surface**.
2. `focus` → A3 block scoring. `must_contain` → claim verification.
3. Domain adapters for the cheapest wins: arXiv abs pages, GitHub issue/PR, crates.io / docs.rs, Stack Overflow.
4. `prompt.rs` — the Jan-style workflow guide from Part 6.

**Verification.** `web_fetch` on a URL already in the turn corpus returns from cache with no network. `must_contain` verdicts correct both ways. Full SSRF suite re-run unchanged (no regression).

---

### Batch W7 — Tier B adoption *(conditional, gated on Part 8 evidence)*

Adopt `bge-m3` and/or the cross-encoder **only if** Phase A shows a measured relevance gain that justifies the latency cost. Otherwise this batch does not happen. Design is pre-specified in `nexus-rs` as optional traits (`trait CrossEncoder`) so adoption is configuration, not refactor.

---

## Part 8 — Baseline evaluation (Phase A)

**This runs first. It produces the numbers every later batch is judged against.**

### 8.1 Objectives

1. **Latency baseline** across a multi-query corpus, per pipeline stage.
2. **Retrieved-content quality baseline** — is the evidence actually relevant, diverse, and complete?
3. **Ablations** — `bge-m3` vs `minilm-l12-v2` latency; cross-encoder rerank on/off.
4. **Full artifact persistence** — every stage output written to disk so any claim can be re-verified without a re-run.

### 8.2 Query corpus

A committed dataset of ≥24 queries across 6 categories, chosen to exercise the known failure modes:

| Category | Count | Why |
|---|---|---|
| `news` | 4 | Exercises recency filter + the dead-news-engine path |
| `technical` | 6 | Should route to developer verticals (W1/B3) |
| `comparative` | 4 | The `depth: "deep"` case — multi-facet, currently fails hardest |
| `factual_entity` | 4 | Exercises the F5 pooling defect (entity/attribute binding) |
| `adversarial_walled` | 3 | Cloudflare / Google-hosted targets — exercises F3 |
| `longtail_niche` | 3 | Low-consensus queries — exercises F2's dead corroboration |

Each query carries: `id`, `category`, `query`, `expected_facts: Vec<String>` (for grounded factual scoring), `min_distinct_sources: usize`, `max_acceptable_ms: u64`.

### 8.3 Metrics

**Latency (deterministic, from `NexusSearchMetrics`)**
`fanout_total_ms`, `url_dedup_ms`, `fetch_total_ms`, `extraction_total_ms`, `chunking_total_ms`, `ranking_total_ms`, `total_pipeline_ms`, `sparse_ranking_ms`, `dense_ranking_ms`; plus derived `pages_fetched`, `pages_extracted`, `fetch_success_rate`, `passages_generated`, `passages_delivered`.

**Retrieval quality (deterministic)**
- `answer_in_snippet_rate` — fraction of `expected_facts` present in the delivered evidence (substring + normalized match).
- `source_diversity` — distinct apex domains among delivered sources ÷ delivered sources.
- `evidence_density` — distinct `expected_facts` per 1000 tokens delivered.
- `redundancy` — mean pairwise cosine similarity among delivered passages (**target < 0.6**; F4 predicts ~0.9).
- `context_utilization` — delivered tokens ÷ `token_ceiling` (are we starving the budget?).
- `coverage` — `expected_facts` covered, per query.

**System health**
`engine_success_rate` per engine, `quarantine_events`, `consensus_urls_populated` (**must currently be `false`** — F2), `blocked_fetch_rate`.

**Fact coverage** — `answer_in_snippet_rate` (fraction of `expected_facts` scoring ≥ 0.90 best-window token similarity) and `fact_similarity_mean` (unthresholded mean, so a near-miss is visible rather than rounded to zero).

**No in-run LLM judge.** Judgement and QA are done by the independent subagent (`audit` subcommand) reading the written artifacts, per §8.6.

### 8.4 Ablations

| Ablation | Flag | Question |
|---|---|---|
| `minilm` (baseline) | default | Current shipped behavior |
| `bge-m3` | `--embedder bge-m3` | Latency delta. Adopt only if relevance measurably improves. |
| `cross-encoder` | `--rerank cross-encoder` | NDCG proxy + token cost. Adopt only if the win justifies ~100–200 ms and 23 MB. |
| `sparse` vs `dense` vs `hybrid` | `--ranking-mode` | Does hybrid actually beat sparse? |

### 8.5 Artifact persistence — every stage output on disk

```
evals/results/web_search_eval/<run_id>/
├── manifest.json                  # git SHA, crate version, full argv, env fingerprint
├── corpus.json                    # the resolved query set actually run
├── cases/
│   └── <query_id>/
│       ├── stage_1_fanout.json    # per-engine latency, hits, errors, raw SERP hit list
│       ├── stage_1b_fetch.json    # per-URL DNS ms, bytes, hops, tls_profile, status
│       ├── stage_1c_extract.json  # markdown byte count + first 2000 chars per page
│       ├── stage_2_chunk.json     # passage count, per-passage char/token counts
│       ├── stage_2_rank.json      # per-passage sparse/dense/final scores, rank order
│       ├── evidence.xml           # the EXACT string the LLM received
│       ├── metrics.json           # this case's derived metrics
│       └── raw_nexus_metrics.json # verbatim NexusSearchMetrics
├── summary.json                   # aggregate across all cases, per ablation arm
├── summary.md                     # human-readable tables
└── qa_report.md                   # written by the QA subagent
```

Non-negotiable: **`evidence.xml` must be the byte-exact string handed to the LLM.** Every quality claim must be checkable against it without a re-run.

### 8.6 Verification gates (must pass before any batch ships)

- **V1 — Reproducibility.** Same corpus, two runs, `summary.json` stage latencies within stated variance.
- **V2 — F2 confirmation.** `consensus_urls_populated == false` on every case, with `index_family` values dumped. *If this comes back true, F2 is wrong and must be re-investigated before W1 is designed.*
- **V3 — F3 confirmation.** `blocked_fetch_rate > 0` for `adversarial_walled`, with the failing URLs and error strings recorded. *If `gemini.google.com` fetches fine in the eval, F3's TLS hypothesis is wrong and W2 must be re-scoped.*
- **V4 — F4 quantification.** `redundancy` mean pairwise cosine on the 75-passage case, and the `fetch_success_rate` distribution across ≥20 fetches.
- **V5 — Model tradeoff verdict.** Produced by the `tradeoff` subcommand: signed `Δp50 pipeline ms`, `Δ answer rate`, `Δ redundancy`, `Δ diversity` against the baseline arm, with an explicit verdict. Adoption requires a positive quality delta; a latency win paired with a quality loss is a loss; model size is never a justification. Verdicts per arm: `ADOPT (faster and better)`, `ADOPT (quality gain, bounded cost)` (≤ +25% p50), `MARGINAL (gain does not justify cost)`, `REJECT (no measurable quality gain)`, `REJECT (quality regression)`.
- **V6 — Fact scoring is not keyword matching.** Every `expected_facts` entry is scored by best-window normalized token similarity (threshold 0.90), per the style guide §3 prohibition on keyword-presence assertions. `fact_similarity_mean` is recorded per case alongside the binary match count.

### 8.8 Harness architecture (as built)

The eval sits at the LLM-in / LLM-out boundary only:

```text
INPUT   = the LLM's output to the harness = a scripted tool call
          flags / corpus -> CanonicalToolCall -> mock LLM serves it as SSE
OUTPUT  = the harness's output to the LLM = the context string the model sees
          captured from the FINAL request body the mock LLM received
```

Phase 2's direct `nexus::NexusSearch` driver and the keyless-engine health batch
were cut from the eval scope: the harness under test constructs the real tool
with the real `VoxEmbedder`, so what matters is what the harness returns, not
what the crate contains. Gate V2 (consensus) is superseded — the harness output
carries the `consensus_urls`-equivalent signal only indirectly, and the QA
subagent's independent search is what establishes whether corroboration actually
happened.

Layout:

```text
app/src-tauri/evals/
├── common/                          # shared, no eval-type knowledge
│   ├── mod.rs
│   ├── cli.rs                       # shared clap arg groups (server, corpus, repetition)
│   ├── keys.rs                      # API key resolution (flag → env → temp/.env)
│   ├── db.rs                        # EvalDbGuard (pre-existing)
│   ├── datasets.rs                  # dataset path resolution (pre-existing)
│   ├── llm_client.rs                # RecordingLlmProvider + judge client (pre-existing)
│   ├── reporting.rs                 # run/case dirs, report + summary writing
│   ├── metrics.rs                   # nearest-rank percentiles, token similarity,
│   │                                # stage-timing rollup
│   ├── stage_dump.rs                # per-stage artifacts, manifest, latest.json
│   └── qa_prompts.yaml              # QA prompts for the MAIN AGENT's additional step.
│                                    # NOT launched by code. Each harness type invokes
│                                    # its subagent in its own way; this file is only
│                                    # prompt text, selected by eval id.
├── memory-pipeline/
│   ├── main.rs                      # single entry point (no in-binary audit)
│   ├── compaction.rs
│   ├── ingestion.rs
│   └── consolidation.rs
├── agentic-tool/
│   ├── main.rs                      # TOOL-AGNOSTIC. Mock LLM, harness wiring, TTS,
│   │                                # artifact capture. Knows nothing about any tool.
│   ├── mock_llm.rs                  # stateful mock LLM server. Turn 1 → scripted
│   │                                # tool call as chunked SSE deltas; turn 2+ →
│   │                                # final text. Captures every request body.
│   │                                # The FINAL request body IS the eval output.
│   ├── tts_capture.rs               # REAL TTS (Kokoro). Device-free PlaybackEngine
│   │                                # via from_parts(stream: None); ring buffer
│   │                                # consumer retained and drained to WAV.
│   ├── harness_ctx.rs               # real AppState, mock Tauri handle, real LLM worker
│   └── tools/
│       ├── mod.rs                   # per-tool registry
│       └── web_search.rs            # web_search script: flags/corpus → arguments.
│                                    # ADDING A TOOL = ADDING A FILE HERE.
├── assets/
│   └── web_search_corpus.json       # [{id, query}] only. No expectations; ground
│                                    # truth is established by the QA subagent.
└── results/agentic-tool/<run_id>/
    ├── manifest.json
    ├── cases/<id>/
    │   ├── tool_call_arguments.json
    │   ├── requests/request_<n>.json  # every LLM request the mock received
    │   ├── context_llm_saw.json       # final request body = THE OUTPUT
    │   ├── evidence.xml               # observation extracted from it
    │   ├── tool_calls.json            # ledger: duration_ms + arguments
    │   ├── pipeline_events.json       # state transitions
    │   ├── render_log.json            # TTS jobs
    │   ├── tts_<n>.wav                # real synthesized clips
    │   └── case_summary.json
    └── summary.md
```

Cargo registration (`harness = false`):

```toml
[[bench]]
name = "memory_pipeline_eval"
path = "evals/memory-pipeline/main.rs"
harness = false

[[bench]]
name = "agentic_tool_eval"
path = "evals/agentic-tool/main.rs"
harness = false
```

Invocation:

```bash
# Single query — the primary mode
cargo bench --bench agentic_tool_eval --release -- tool --tool web_search \
  --query "Rust 2024 edition migration guide" --embedder minilm-prod

# Batch over the corpus
cargo bench --bench agentic_tool_eval --release -- batch --tool web_search \
  --embedder minilm-prod

# Ablation: the same query against a different embedding backend.
# (One model per process; the testing-style-guide §3/§7.3 forbids concurrent
# ONNX sessions invalidating latency comparisons.)
cargo bench --bench agentic_tool_eval --release -- tool --tool web_search \
  --query "..." --embedder minilm-fixed
cargo bench --bench agentic_tool_eval --release -- tool --tool web_search \
  --query "..." --embedder bge-m3

# QA pass (additional step, done by the main agent, NOT by code):
# 1. Read evals/common/qa_prompts.yaml, take the prompt whose id matches the eval.
# 2. Substitute {{RUN_DIR}}, {{QUERY}}, {{TOOL_ARGS}}, {{TOOL_LATENCY_MS}}.
# 3. Launch your subagent with it. It runs ITS OWN web search on the same query
#    BEFORE reading our artifacts, then compares. Report → qa_report.md.
```

## How the eval exercises production seams (no stubs)

| Seam | What runs | Proof |
|---|---|---|
| Wire parse | Real `RemoteTransport` against the mock | SSE chunked deltas, `finish_reason: tool_calls` |
| Tool detection | Real `StreamRoutingStage` / normalizer | `ToolCallReceived` → `step6_handle_non_terminal_tool` |
| Execution | Real `ToolExecutor` + real `WebSearchTool` | `duration_ms` in `session_tool_calls` |
| Filler | Real `TtsCommand` → real worker → real Kokoro | `render_log` + WAV |
| Playback state | Real `PlaybackEngine` (device-free) | `PlaybackStarted`/`PlaybackFinished` in `pipeline_events` |
| Budget | Real `ContextBudgetStage` + `render_evidence_xml` | Observation inside `context_llm_saw.json` |

The eval does NOT exercise (and does not claim to): the LLM's query formulation,
the model's answer generation, or the frontend.

## Metrics, re-scoped to the boundary

Per case, all read from artifacts, never asserted in code:

- `tool_duration_ms` (ledger) — the number that matters
- `turn_ms` — full harness overhead around the tool
- `observation_chars`, `sources`, `passages` — parsed from the captured observation
- `tool_args_match` — echoed arguments equal the scripted ones
- `filler_chars`, `tts_jobs`, `audio_clips`, `tts_drained`
- `state_transitions` — did the machine run and return

Aggregate: nearest-rank `LatencyStats` for turn/tool, health counts
(zero-passage cases, single-passage cases, cases with audio, arg mismatches),
`latest.json` mirror. Fact coverage, redundancy and diversity are QA-subagent
territory — the harness records the bytes, the subagent judges them against its
own search.

---

## Part 9 — Risks

1. **F5 (pooling) changes `search_memory` for every user.** Two lines, widest blast radius in the plan. Isolated commit, memory eval re-baselined either side, expect movement.
2. **W2 may not fix F3.** TLS fingerprinting is an arms race; `primp`'s Chrome tables rot. **Gate V3 exists precisely to check this before W2 is built.** If `gemini.google.com` still fails, the honest fallback is to *deprioritize walled hosts at candidate-selection time* — they will fail anyway — and spend the slot on a fetchable source. Budget one repro cycle.
3. **Adding families changes ranking for every query.** Once ≥2 families are live, `consensus_multiplier = 1.5` starts firing for the first time and reorders results. Re-baseline the eval before and after; **do not ship W1's family additions and W3's ranking changes in the same measured window.**
4. **Dead engines are a permanent tax.** Engines die on a schedule — 2 of 5 already have. W1 makes that survivable, not solved. The moment a real search API key is acceptable, BYOK ends the treadmill, at the cost of a key-management surface, third-party egress and per-query cost. **That is a product decision and is deliberately out of scope.**
5. **Burst of new schema surface.** Part 6 adds five parameters plus one tool. Schema tokens cost context on every request. Mitigation: keep descriptions terse; the `depth` enum replaces what would otherwise have been a whole extra tool schema. Measure schema token cost in the eval.

### Explicitly out of scope

BYOK providers (Tavily / Exa / Serper / Brave). DonSeTch-as-MCP-sidecar. Self-hosted SearXNG. PDF/OCR extraction. Headless browser tier. `bge-m3` as a *default*. Adopting the cross-encoder without eval evidence.

---

## Part 10 — Open questions

1. **Axiom 1 amendment for `web_fetch`** — peer tool, or state-gated so it is only reachable *after* a `web_search` in the same turn? State-gating is a tighter contract but harder to prompt.
2. **`depth` default** — `standard` (current behavior plus focus extraction) or `quick`? `standard` costs more per query but should raise answer quality immediately. Recommend `standard`, measure in Phase A.
3. **Intent TTLs** — news 15 m / general 1 h / docs 4 h (DonSeTch's). For a *voice* assistant where a user may ask "what's the latest on X" across three consecutive turns, is 15 m right or too aggressive?
4. **`bge-m3` adoption bar** — what magnitude of `answer_in_snippet_rate` gain justifies a 543 MB resident model? Recommend: adopt only on a **signed, repeatable** gain, judged with the QA subagent, not a single run.
5. **Corpus retention TTL** — how long should a turn-scoped corpus live? One turn (spec §6.1 ephemerality) or the whole session?

> **ANSWERED 2026-10-04 (moved here from open; see Part 11 for the binding form):**
> deadline default/ceiling = **5500ms / 12000ms** · on deadline hit = **partial evidence +
> `next_action`** · `focus` = **BM25 block relevance** (embeddings as gated fallback) ·
> 4th family = **keyless Brave scrape** · ranking default = crate `Sparse`, Vox overrides to
> `Hybrid` (document + validate) · publish = **single 0.1.2 at the end, explicit approval** ·
> Mojeek/GoogleWml = **decide from baseline data** · nexus-rs tests = **testing subagent**.

---

## Part 11 — Consolidated execution plan (2026-10-04, SUPERSEDES the ordering in Part 7)

> **Any agent working this thread: read this Part first.** Parts 0–10 are the forensics and the
> design rationale. This Part is the *order of execution*, the *decisions already taken*, and the
> *rules that bind every implementer*. It supersedes Part 7's ordering wherever they differ.

### 11.1 Decisions already taken (do not re-litigate)

| # | Decision | Value | Effect on the plan |
|---|---|---|---|
| D1 | Deadline is a parameter | `deadline_ms`, default **5500ms**, hard ceiling **12000ms** | §11.2 contract; N1 implements, V1 enforces |
| D2 | On deadline hit | **Partial evidence + `next_action`**, never a bare error | §11.2 outcome enum |
| D3 | `focus` mechanism | **BM25 block relevance** (DonSeTch approach), embeddings as gated fallback | A3, §2.4 amended |
| D4 | 4th index family | **Keyless Brave scrape** via primp, no API key | W1 item 1 amended |
| D5 | Ranking default | Crate `Sparse`, Vox overrides to `Hybrid` — document, plus enum validation against silent fallthrough | G1 spec, V1 code |
| D6 | `nexus-rs` wiring | Bump → publish → bump Vox. **Single publish, version 0.1.2, at the very end. Explicit user approval required before `cargo publish`.** | §11.3 flow |
| D7 | Mojeek / GoogleWml | **Decide from baseline data**, not the forensic estimate | G3 decides |
| D8 | Test ownership | **All `nexus-rs` tests written by a testing subagent**, not the implementing agent | §11.4 rule |

### 11.2 The deadline contract (N1 design, normative)

A timeout is a *result*, not an exception. `NexusSearch::search` returns:

```rust
pub enum RetrievalOutcome {
    Complete { result: NexusSearchResult },
    Partial  { result: NexusSearchResult, degraded: Vec<Degradation>, stop: StopReason },
    Failed   { stop: StopReason, recoverable: bool, next_action: NextAction },
}

pub enum StopReason {  // honest set, mirrors DonSeTch
    Deadline { budget_ms: u64, elapsed_ms: u64, stage: PipelineStage },
    QuorumUnmet { reporting: usize, required: usize },
    AllEnginesDown { per_engine: Vec<EngineStatus> },
    SsrfBlocked { url: String }, BudgetExhausted, Cancelled, FrontierEmpty,
}
// stage = Fanout | Fetch | Extract | Chunk | Rank
```

Vox renders any non-`Complete` outcome as actionable XML the LLM can branch on:

```xml
<web_search_status code="deadline.hit" error_kind="transient" stop="Deadline" stage="dense_ranking">
  <partial>…passages ranked before cutoff…</partial>
  <engine_status>Bing ok 210ms · Yahoo ok 340ms · Mojeek quarantined</engine_status>
  <next_action>Deadline 5500ms hit during dense ranking; 2 of 5 engines answered.
    Retry with ranking_mode="sparse" (no embedding, ~1.4s), or narrow the query.</next_action>
</web_search_status>
```

Mirroring DonSeTch exactly: stable `code`, `errorKind` (`permanent` / `transient` / `walled`),
`next_action`. Transport telemetry (tier, timings, profiles) stays under `_meta`, never in the
model surface. Partial evidence is **kept**, never discarded — F4 already showed we starve for
evidence. The 12s ceiling is load-bearing: without it the LLM can request 600s and stall the
turn. This contract is **unenforceable until V1's `spawn_blocking` lands** — an ONNX call on a
tokio worker cannot be preempted by any timer.

### 11.3 The publish-once flow (normative, no deviations)

```
G0  orphan files deleted, full clippy green (Vox, mechanical)
 └─► G1  spec amendments FIRST (AGENTS.md §4.3 gate — no code until this lands)
      └─► G3  eval baseline: 1 case → 24-case batch → QA subagent report
           │   (decides D7; produces the numbers every later batch is judged against)
           └─► N0+N1  never-crash + deadline contract      (nexus-rs, in-repo)
                └─► V1  Vox seams: spawn_blocking, pooling, enum validation, outcome render
                     └─► N2  health + Brave + verticals + UA rotation
                          └─► N3  fetcher stealth + fetch_slack
                               └─► N4  semantic ranking + entity penalty + handles
                                    └─► V2  depth params + corpora + warm handoff
                                         └─► V3  web_fetch
                                              └─► PUBLISH 0.1.2 (single, explicit approval)
```

**Local-path rule.** From the first nexus-rs code change until the publish gate, Vox builds
against the fork, not crates.io:

> **DEVIATION RECORDED 2026-10-04.** The G3 batch panicked on case 1 before any nexus-rs
> work began: a PDF served as a search result hit P1 (`cleaner.rs:29`, byte index 65536
> inside a multi-byte char — process abort, exit 101). A baseline that crashes on
> real-world input measures nothing, so the path switch + the single N0/P1 fix
> (`floor_char_boundary`, behavior-preserving) moved *ahead* of G3. The fork is
> byte-identical to crates.io 0.1.1 plus that one fix; all other N0–N4 work still follows
> G3. Required regression test (testing-subagent owned): non-ASCII input >64 KB must not
> panic `fast_scan_tag`.

```toml
# app/src-tauri/Cargo.toml — TEMPORARY, dev/eval only. Reverted at the publish gate.
nexus = { package = "nexuss", path = "../../submodules/nexus-rs" }
```

All eval benches run against this path build. At the publish gate the line reverts to
`version = "0.1.2"` and the full `--all-targets --release` clippy + both eval benches re-run
green before `cargo publish` is even asked for.

**Publish gate (ALL must hold, single 0.1.2, one shot):**
1. Vox code + nexus-rs code both complete per W1–W4 + V1.
2. nexus-rs test suite green, written per §11.4, including at least one failing-before-fixed
   reproduction of a forensic finding.
3. `cargo clippy --all-targets --release` zero errors/warnings on the Vox tree with the
   path dep, and again after the version bump.
4. Eval baseline re-run against the new code; QA subagent report filed; no regression vs G3
   numbers outside the expected ranking-reorder window (§9.3).
5. **Explicit user approval.** Then and only then: bump `0.1.1 → 0.1.2` in
   `submodules/nexus-rs/Cargo.toml`, commit + push in the nexus-rs repo
   (`submodules/` is gitignored in Vox — the work survives only there),
   `cargo publish`, bump Vox to `version = "0.1.2"`, re-verify.

**Panic-hardening gate (part of N0, blocks publish).** The crate must be panic-free on
adversarial input: P1 `cleaner.rs:27-32` (`&html[..65536]` byte index — remote panic on any
non-ASCII page >64 KB), P2 `embedder.rs:205` (`shape[2]`, no len check), P3
`embedder.rs:221` (unguarded tensor index), P5 `with_fetch_concurrency` (no ceiling), plus
`catch_unwind` at the Vox tool boundary and an explicit verification of the effective panic
strategy (`panic = "abort"` in the nexus-rs manifest is ignored for non-root crates — verify
what actually governs). No batch past N0 may introduce a new `expect`/`unwrap` on a
network- or model-shaped input.

### 11.4 Implementer rules (bind every agent on this thread)

1. **Spec first.** AGENTS.md §4.3 holds for the whole thread: any behavior in §11.2/§11.3 or the
   batches that is not in `docs/specs/tools-specs/web-search.md` must be written into the spec
   BEFORE code. The crate-side contract lives in `docs/specs/tools-specs/web-search-nexus-rs.md`.
2. **Test ownership.** The implementing agent writes production code + recorded fixtures only.
   All `nexus-rs` `tests/` are owned by a testing subagent (spawned via the
   `opencode-subagent` / `agy` / `kilo` skill with a QA persona), which asserts each batch's
   verification criteria independently.
3. **No silent simplification.** Never bypass a blocker with a mock, stub, or skipped stage.
   Report blocker / cause / attempts / what is needed.
4. **Named evidence only.** Every batch's verification criteria name the log line, the metric,
   or the artifact — no "should improve" prose.
5. **One embedder per process.** The eval's testing-style-guide §3/§7.3 stands: concurrent ONNX
   sessions invalidate latency comparisons. Ablations are separate processes.
6. **No eval runs without the baseline gate.** G3 runs 1 case first; the 24-case batch only
   after the single case's artifacts are inspected and sane.

### 11.6 Round-2 results: fixes landed, QA swept all 24 (2026-10-04)

**Batch:** `evals/results/agentic-tool/20261004_183456_a5512497` — 24/24 ok, **23/24
retrieval_ok**, 127.2s, mean tool 2537ms, 0 timeouts. Built on the path dep with D7 + P0-2/3/4/5/6/8.
**QA:** 4 native subagents × 6 cases = all 24 queries audited, each with own-search-first ground
truth. Reports: `qa_report_full_A/B/C/D.md` (plus round-1 `qa_report.md`, `qa_report_native.md`).

**What the fixes achieved (measured):** `cmp_03` (was 23KB Russian YouTube JS → 5 passages,
6389B), `ent_01` (was 42KB hotel JS → 5 passages, 5844B, exact 384/256 delivered),
`long_01` (was 0 passages → 3 passages), `wall_02` (was redditinc → 5 passages). No observation
exceeds ~6KB (P0-2 clamp holds). Filler overruns negative everywhere except +31/+2/+74ms edges.
49/49 new nexus-rs tests green (testing-subagent authored, `tests/page_quality_gates_test.rs` +
`tests/cleaner_ranking_gates_test.rs`).

**What QA proved still broken (binds the next work — supersedes optimistic readings above):**

| # | Finding | Status |
|---|---|---|
| R2-1 | `retrieval_ok` is a false-positive machine: `news_04` (EU-Wikipedia for ECB), `ent_04` (google.com nav), `long_01` (Blogger nav ×3 locales), `cmp_01` (medical burns for ML frameworks), `tech_05` (w3schools nav) all `retrieval_ok=true` with zero answer content | MUST redefine on query-entity coverage of the body, then recompute the batch |
| R2-2 | Doubled `<web_search_evidence>` envelope open tag in 6/6 (malformed XML to the model); `ent_03` outer/inner counts disagree | MUST fix seam to single envelope, counts from delivered |
| R2-3 | `fanout_ms` = 1200–1202 in 22/24 then ~1201 constantly — the deadline floor, exhausted every call; corroboration truncated before collection | D7 helped (no more 539–830ms waste) but single-sourcing persists: 4th family (Brave, N2) still required |
| R2-4 | `score=0.000` passages delivered (`cmp_02` rank 5, `ent_02`/`ent_04` tails); dual-1.000 ties make order arbitrary | MUST filter `score ≤ ε` + deterministic tie-break (P0-8 follow-up) |
| R2-5 | Byte-identical + near-duplicate passages delivered as distinct (`ent_04` rank1==rank2; wall_01 benchmark table; wall_03 Reference rules) | MUST add post-extraction text-sim dedup (`url_dedup` is URL-level, pre-extraction) |
| R2-6 | Modality substitution: S2S asked, TTS delivered (`wall_02`); product substitution (`wall_01` model page for product query); sense substitution (`cmp_01` burns) | MUST add query→expected-entity-class check with mismatch penalty; abstain-or-clarify path for disjoint-domain tops |
| R2-7 | Staleness: 2023 GPT-4 page dominant for "this week" (`news_03`); no recency demotion under `time_filter=any` | MUST default news-ish queries to week/month or add recency boost |
| R2-8 | Index pages preferred over fact pages (tavily homepage over /pricing; w3schools index over reference) | MUST skip nav-listing blocks / downrank them |
| R2-9 | `audio_summary.json` totals (193090 samples) disagree with clip sums (163043) in 6/6; `render_log.duration_ms` is synthesis wall, not audio length | MUST fix accounting; field already documented, values still wrong |
| R2-10 | `pipeline_events.json` holds only `PlaybackStarted`+`LlmFinished` — Q10 unverifiable | MUST emit Thinking/Working or amend the prompt (E4 partial: glob fixed, emitter not) |
| R2-11 | `max_tokens: 120` raised to 2000 in eval settings; mock final answer realistic-length but still canned — answer path exercised for length, not groundedness | Accepted limitation, recorded |
| R2-12 | Digit-stripper (P0-7): NOT a filter — static HTML contains `<span class="purecounter" …>0</span>M+`; numbers are JS-animated. No code fix; browser tier remains out of scope | Closed as investigated |
| R2-13 | `cmp_04` starved (0 passages) under DDG-challenge pressure + gates; empty results carry zero stage telemetry so RCA is impossible from artifacts | MUST emit per-engine/gate-rejection accounting on empty results |
| R2-14 | Inflated claims delivered as fact (`cmp_03`: 10–15pt BEIR vs published ~4); LaTeX/table fragments unspeakable (`ent_02`); entity anchors miss quoted multiword spans | Backlog for N4 ranking work |

**Consequence for the flow in §11.3:** N2 (Brave 4th family) is now the critical path to diversity
— gates alone cannot conjure a second source. N4 must include: post-extraction dedup, score
floor >0, primary-source preference, recency boost, nav-listing skip, abstain-or-clarify. V2
(depth params) stays gated behind N2+N4 evidence. The single 0.1.2 publish gate is unchanged.

### 11.5 G3 baseline results + QA verdicts (2026-10-04, SUPERSEDES Part 7 priorities)

**Batch:** 24/24 ok, 114.6s wall, mean tool 2441ms (p50 2201, p95 4253), 0 timeouts.
Run dir: `app/src-tauri/evals/results/agentic-tool/20261004_130926_6d0d4ec2/`.
**QA:** two independent auditors, 60/66 agreement. Reports: `qa_report.md`, `qa_report_native.md`.
**Verdicts: 40 FAIL · 15 PARTIAL · 11 PASS.** Real pass rate ≈ 13/24 — the batch headline
measures plumbing, not retrieval.

**What outranks the planned batches.** The following P0 items were NOT in Parts 0–10 and now
take precedence over W1–W4 ordering wherever they conflict:

| # | Finding | Fix location | Supersedes |
|---|---|---|---|
| P0-1 | `ok:true` for 0-answer cases | Eval scoring: separate `retrieval_ok` from `execution_ok` | G3 metrics |
| P0-2 | 42KB observation vs 2000-char ceiling; `ent_01` ≥11,538 tokens vs 8192 window | Vox `render_evidence_xml`: per-passage cap + final clamp + pre-flight assert | §2.2, W3 |
| P0-3 | 22,540 + 41,930 chars of page JavaScript delivered as evidence | nexus-rs extraction: strip `<script>`/`<style>`, reject code-like passages | W3 item 5 |
| P0-4 | `ent_03`: block page ("Your request has been blocked") scored 0.033, delivered | nexus-rs: reject challenge/error pages pre-source | (new) |
| P0-5 | `?hl=ru` for English query; `redditinc.com` for "reddit discussion"; hotel brand on token `all` | nexus-rs: language + domain-intent + entity-anchor filters | W1 verticals |
| P0-6 | `[Skip to main content](#content)`, `&apos;`, `\u002D` in spoken output | nexus-rs cleaner + Vox render: strip link syntax, decode entities once | W3 item 5 |
| P0-7 | Digits stripped from stats (`0M+`/`0K+`/`0B+`) — owner + repro needed | Investigate before fixing | (new) |
| P0-8 | All 103 passages score in [0.030, 0.033] — ranking is decoration | Emit sparse/dense/RRF components; add relevance floor | W3 ranking |
| P0-9 | `fanout_ms` = 1200–1202 in 22/24 (deadline exhausted, 28–83% of every call) | D7: drop Mojeek + GoogleWml (decided from this data) | W1, F1 |
| P0-10 | `chunk_ms=0` + `url_dedup_ms=0` in 23/23 — stages uninstrumented or no-ops | Instrument or delete | W3 |

**Eval integrity fixes (required before the next baseline can measure answers):**
E1. `max_tokens: 120` → raise; mock LLM must emit a real grounded answer, not `"Here is what
I found."` (all 48 WAVs byte-identical).
E2. `render_log.duration_ms` is 203–970ms short of real audio in 48/48; `audio_summary.json`
over-reports 18.4% and is identical across cases — compute per case, define the field.
E3. Filler must be gated on measured tool latency (`wall_02`: filler outlives the tool call).
E4. `pipeline_events.json` holds only `PlaybackStarted` + `LlmFinished` — emit the lifecycle or
update `qa_prompts.yaml` Q10 (and fix its `filler_*.wav` glob, which matches nothing).