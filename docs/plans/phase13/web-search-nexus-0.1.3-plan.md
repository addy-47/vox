# Web Search & `nexus-rs` 0.1.3 — Grounded Plan & Execution Ledger

> **Phase 13 artifact. Single ledger for this work.**
> **Created:** 2026-10-15 | **Revised:** 2026-10-15 (grounded correction)
> **Scope:** `submodules/nexus-rs/` + `app/src-tauri/src/services/harness/stages/tools/web_search.rs` + Vox tool loop/prompts.
> **Goal:** Make the existing single `web_search` call return more usable evidence and make failures explicit. No invented parameters.
> **Status:** Plan approved for implementation. No production code changed yet.

---

## 1. Executive Summary

Current Vox web search is structurally sound on safety (SSRF, budget, answer-presence). It is weak on **supply**: current user-facing reports are rejections/empty answers too often, and when evidence is missing the model still answers from stale memory.

Four root causes dominate:

1. **Provider coverage is narrow and brittle.** The active pool is DuckDuckGo, Bing, Yahoo, Brave, and Wikipedia. Yahoo is routed through Bing-classified family logic, Mojeek/Google paths are effectively dead, and the DuckDuckGo endpoint is the anti-bot-prone HTML endpoint, not the lite endpoint. Keyless verticals used by strong agents (arXiv, Stack Exchange, GitHub, Hacker News, MDN) are not present.
2. **Candidate depth is thin and starved.** Vox requests only 3 top pages. Partial over-fetch exists (`max_candidates + 2`, capped at 5), but it is not configurable and does not continue until enough *extracted* pages exist.
3. **Transport stealth is split.** SERP fetch uses Chrome impersonation. Page fetch uses `primp` with pinned DNS, which is better, but profile rotation, cookie jar scoping, and HTTP/2 reuse policies still need one consolidated owner.
4. **Empty-result handling is advisory only.** The tool returns structured status XML, sometimes with a strong “Do not answer from memory” instruction. The prompt and tool loop do not turn that instruction into enforced behavior, so the model can stop and answer from memory anyway.

`nexus-rs` 0.1.3 should be a **supply-and-honesty release**, not a depth release. Depth (`sub_queries`, `focus`, `web_fetch`, corpus cache) remains Phase 12/14 work. 0.1.3 makes the existing single `web_search` call return more usable evidence and makes failures explicit enough that repetition/no-result behavior can be tracked.

---

## 2. What We Read

### 2.1 Vox side

- `app/src-tauri/src/services/harness/stages/tools/web_search.rs`
  - Tool schema, deadline math, status XML, passage rendering, `VoxEmbedder`, answer-presence gate.
- `app/src-tauri/src/services/harness/loop.rs`
  - `MAX_TOOL_ITERATIONS = 5`, reentrant loop, terminal vs non-terminal handling.
- `app/src-tauri/src/services/harness/stages/prompt.rs`
  - System rules. `web_search` gets only one generic rule. Empty-result guidance is absent.
- `app/src-tauri/src/services/harness/steps.rs`
  - Non-terminal tool path: scratchpad gets tool call + tool result. No automatic retry policy.
- `app/src-tauri/src/core/state.rs`
  - `AppState::nexus_search` is a hoisted `Arc<nexus::NexusSearch>`.
  - Engines currently: DuckDuckGo, Bing, Yahoo, Brave, Wikipedia. Concurrency 5.
- `docs/specs/tools-specs/web-search.md`
  - Tool contract, observation taxonomy, anti-injection guard.
- `docs/plans/phase12/web-search-production-hardening.md`
  - Existing forensic findings F1–F9 and W-batch roadmap. We treat it as the historical source, not as the 0.1.3 target.

### 2.2 Crate side

- `submodules/nexus-rs/Cargo.toml`: package `nexuss` **0.1.2**, library name `nexus`.
- `src/lib.rs`, `src/model.rs`, `src/pipeline.rs`
- `src/engines/{mod.rs,health.rs,helpers.rs}`
- `src/engines/{bing.rs,brave.rs,duckduckgo.rs,yahoo.rs,mojeek.rs,google_wml.rs,wikipedia.rs}`
- `src/fetcher/{mod.rs,connector.rs,redirect.rs,dns.rs,client.rs}`
- `src/extraction/{quality.rs,cleaner.rs,mod.rs}`
- `src/answer_presence.rs`
- `src/chunking/passage.rs`
- `src/ranking/{mod.rs,hybrid.rs,bm25.rs,dense.rs,dedupe.rs}`
- `tests/*.rs`
- `examples/*.rs`

### 2.3 Repos pulled for comparison

- `temp/donsetch` (`dondai44423/donsetch`, current commit as of clone 2026-10-15)
  - Rust web fetch/search/crawl stack. Borrow from, do not depend on it. License blocker: AGPL-3.0.
- `temp/hermes` (`NousResearch/hermes-agent` shallow clone)
  - Python keyless DDGS provider architecture, backend selection, managed routing, and keyless fallback ring.
- Plus registry/docs for Rust crates:
  - `web-search` (multi-provider merge, 22 providers)
  - `rust-websearch` (DuckDuckGo lite/Instant Answer, SearXNG, anti-detection, Servo fetch)
  - `nexuss` 0.1.1 on crates.io
- OpenCode source was checked in the local install. No web-search tool implementation source was available to copy patterns from, so comparison is limited to what Vox already integrates.

---

## 3. Observed User-Facing Symptoms

| Symptom | Current behavior | Where it shows |
|---|---|---|
| Rejections are frequent | Hard-coded small candidate count and engine-weighted quorum can starve the corpus | `web_search.rs` uses 3 top candidates |
| Model barely gets info | 2 of 3 fetch failures remove most evidence, and single-domain dominance occurred despite fixes | `order_candidates` and fetch filtering |
| Follow-up questions return stale answers | Evidence removed at tool boundary, and model uses memory when no matching snippet survives | `web_search.rs` render + loop |
| “No relevant passage found” does not reliably trigger another search | Tool status XML is informational only | prompt + loop |
| DDG-style results can be missing | HTML endpoint is more bot-prone than DDG Lite | `duckduckgo.rs` |
| Keyless scraping remains fragile | Endpoint profile rotation and cookie scoping are partial | `engines`, `fetcher` |

### 3.1 What the model gets when no passage is found

There are two empty paths:

1. `scored_passages.is_empty()`:
   - `<web_search_status code="empty_results" error_kind="not_found">`
   - Message: `Web search completed for '...' but no relevant passages were found.`
   - `next_action`: `Try rephrasing the query with different keywords or broader terms.`
2. `answer_presence == AnswerPresence::Absent`:
   - `<web_search_status code="empty_results" error_kind="missing_factual_answer">`
   - Message: `Web search found pages about '...' but none of them state the specific answer.`
   - Extra: `Do not answer from memory; report that the source was not found.`
   - `next_action`: `Try rephrasing with the concrete entity name, or search for the specific quantity.`

So the strongest guard exists only on `Absent`. On the ordinary `not_found` path, the model is invited to retry with different wording, but nothing forces it. On both paths, the tool call is never blocked from being followed by a final answer.

---

## 4. Where the Gap Is

### 4.1 Gate level

The tool is correct in returning a status. The gap is in **how that status is used**:

- The system prompt only says when to call `web_search`.
- There is no rule for a missing-answer status.
- The reentrant loop allows one more model turn after a tool status. If the model does not emit another `web_search`, the pipeline commits the current text.
- The tool never sets an error flag that later code can use to forbid committing. `ToolResult` is always `Ok`, even for empty results.
- The user can see filler audio and then a confident answer. That makes empty results read as success.

### 4.2 Retrieval level

The second gap is that an empty result is often not the right product of the system:

1. DDG HTML requests can fail before retrieval starts.
2. Yahoo can be dropped by quorum or blocked by peer data.
3. Only 3 pages are kept, even when 1 or 2 fetches fail.
4. The first fetched candidate can be a bad SERP-relative or low-value page.
5. A correct page can have the answer only in the first few characters while the selected passage is a blind chunk.
6. Some things that users describe generically (“what is X?”) are not value-seeking queries. Those should be handled as general retrieval, not answer presence.

### 4.3 Telemetry level

Vox already records metrics. The gap is that the last release does not report enough failure modes to distinguish the causes above. 0.1.3 should make logs show:

- Empty due to no hits.
- Empty due to fetch starvation.
- Empty due to answer-presence absence.
- Empty due to a minimum score floor.
- Empty due to budget collapse.
- Whether the query is a follow-up or fresh.
- Whether a retry actually happened.

---

## 5. What Major Projects Do

### 5.1 DonSeTch (Rust, AGPL-3.0)

Borrow ideas only, not the dependency.

| DonSeTch pattern | Why it matters | Borrow decision |
|---|---|---|
| DDG Lite as primary endpoint | HTML endpoint is CAPTCHA-prone from proxy IPs | **Adopt** in Nexus DDG engine |
| Chrome-true TLS plus profile rotation | Transport failures separate successful and broken hosts | **Adopt** generalized profile rotation |
| Keyless verticals (arXiv, StackExchange, etc.) | Real APIs avoid SERP fragility and add independent families | **Adopt** as `Engine` variants |
| Health EWMA/quarantine | Avoid wasting repeated budget on dead engines | Already partly present; refine lifecycle |
| Block-level extraction with focus | Massive token reduction while keeping answer spans | **Adopt** in chunking/extraction |
| Entity-coverage penalty (0.3x on contradiction) | Stops wrong entity pages from looking like evidence | **Adopt** |
| Token handles (`S1..Sn`) | Follow-ups address evidence cheaply | **Adopt** in later corpus phase |
| Honest deadline/stop reasons | Makes empty results actionable | **Adopt** as status and log vocabulary |
| Warm handoff search->fetch | Cache top results for next step | **Adopt** in cache phase |
| Browser escalation | Higher cost, only for walled targets | **Defer** to opt-in sidecar |

**License constraint:** DonSeTch is AGPL-3.0 and is a binary/MCP server, not a crate. Vox remains proprietary-oriented. Do not link it. Only copy algorithmic patterns.

### 5.2 Hermes agent (Python + plugins)

Borrow operational patterns, especially around keyless tiers and process isolation.

| Hermes pattern | Why it matters | Borrow decision |
|---|---|---|
| DDGS runs in a disposable child process | Native code can hold the Python GIL and defeat parent timeouts | **Pattern is relevant**; Rust equivalent: use process/timeout escape hatches only where native code might hang |
| Keyless fallback ring plus managed backend | Free tier gives the model a working search even with no API key | **Adopt** as engine selection policy and diagnostics |
| Provider-first result rows (`title`, `url`, `description`, ... contract) | Keeps tool output stable and testable | **Adopt** in Vox persistence/logging |
| Per-backend unavailable messaging | Users can see exactly why a backend failed | **Adopt** in status and debug output |
| Provider-specific env var and key metadata | Better setup surface | **Adopt** only if Vox adds managed keys |
| Keyless variant exposed as separate “free” rows | Makes operator choice clear | **Adopt** in settings/health diagnostics |
| Empty results returned as success=false data | The model receives a clear failure signal | **Adopt** partially; keep Vox’s current XML status richness |

The biggest applicable lesson from Hermes is: **never let an opaque native search dependency freeze the whole agent loop**. Vox already avoids this by using Rust and `tokio::time::timeout`, but F7 shows ONNX embedding can still evade the deadline because it holds a tokio worker. Fix that in 0.1.3.

### 5.3 Rust ecosystem (`web-search`, `rust-websearch`, ddgs)

| Pattern | Why it matters | Borrow decision |
|---|---|---|
| DDG Lite endpoint | Less anti-bot than HTML SERP | **Adopt** |
| DDG Instant Answer API for knowledge queries | Better for simple factual questions | **Adopt** as optional engine |
| SearXNG client support | Aggregates 70+ engines, no key | **Adopt as optional backend path**, not default |
| `rf craw` / strong content extraction? | Often better page extraction than raw HTML | **Evaluate parity first** |
| User-agent rotation and jitter | Better survival on bot-sensitive hosts | **Adopt** already partially, expand |
| Rust library with clear `SearchResult` shape | Small and testable interface | Already has this in Nexus |
| Multiple merge strategies | Explicit control over diversity vs depth | **Adopt** selection in later corpus phase |

### 5.4 OpenCode (verified from source clone `/tmp/opencode/opencode-src`)

- `packages/core/src/tool/plugin/websearch.ts`: schema is `query` only. Output is provider + results array. Empty case returns constant `NO_RESULTS = "No search results found. Please try a different query."`
- `packages/core/src/tool/plugin/webfetch.ts`: schema is `url`, optional `format` (`text`/`markdown`/`html`), optional `timeout` (max 120s). Has byte cap, MIME guards, Cloudflare 403 challenge retry with alternate UA.
- Lesson for Vox: keep model schema minimal. Put depth/fallback logic inside the implementation, not in new model params. Fix must include prompt and loop guards, not just better search code.

### 5.5 jcode-Rust (verified from source clone `/tmp/opencode/jcode-rs`)

- `crates/jcode-app-core/src/tool/websearch.rs`: schema is `query`, optional `num_results` (default 8, max 20), optional `engine` (`duckduckgo`/`bing`/`searxng`/`custom`), optional `bing_market`. Explicit fallback loop over `local_engine_order`.
- DuckDuckGo uses POST form to `html.duckduckgo.com/html/` with Chrome UA; empty results trigger anti-bot detection with named reason and fallback to next engine.
- `crates/jcode-app-core/src/tool/webfetch.rs`: schema is `url`, optional `format`, optional `timeout`. 5MB cap, 40k char output cap with truncation note.
- Lesson for Vox: fallback engine order and SearXNG/custom endpoints are proven patterns. Adopt these, not invented `depth`/`focus`/`must_contain` params.

---

## 6. Grounded 0.1.3 Scope (correction: no invented params)

0.1.3 ships only patterns with direct source evidence. `depth`, `focus`, `sub_queries`, `must_contain`, `source_diversity`, token handles, corpus retention, and warm handoff are **withdrawn from 0.1.3** and moved to §12 as unvalidated research.

### 6.1 Must ship (`0.1.3-core`)

1. **DDG Lite primary + challenge detection (evidence: DonSeTch, jcode-Rust, rust-websearch).**
   - Add Lite endpoint as primary; keep HTML parser as fallback.
   - Detect challenge/empty SERP and report named reason + fallback taken.
   - DDG Instant Answer: evaluate only; ship only if parser proves stable on fixtures.
2. **Explicit fallback engine order (evidence: jcode-Rust `local_engine_order`).**
   - Configurable order, default `DuckDuckGo -> Bing -> SearXNG -> custom`.
   - Record per-engine success/failure in `FanoutOutcome`/metrics.
3. **SearXNG + custom endpoints (evidence: jcode-Rust, Hermes SearXNG backend).**
   - New optional `Engine::Searxng` and `Engine::Custom` behind config; off by default.
   - Each vertical/API declares its own `index_family()` so consensus can fire.
4. **Fetch starvation fix (evidence: Vox F4 telemetry, OpenCode/jcode byte caps).**
   - Configurable `fetch_slack` (over-fetch N, keep first K extracted).
   - Default Vox: request enough candidates for >=3 extracted pages.
   - Keep existing byte caps and SSRF pinning; no new attack surface.
5. **Consolidated transport + Cloudflare UA fallback (evidence: OpenCode `webfetch.ts`, jcode Chrome UA).**
   - One profile owner for SERP + page fetch.
   - Retry known 403 challenge once with alternate UA; log profile used.
   - Scoped clearance-only cookie jar; no disk persistence.
6. **Answer-presence honesty (already in crate; extend taxonomy).**
   - Keep current absent check.
   - Add an explicit status field `source_failure_class`:
     - `no_hits`
     - `fetch_starved`
     - `answer_absent`
     - `low_score_floor`
     - `budget_exhausted`
   - Ensure all of these produce a `next_action` the model can act on.
7. **Matrix and logs.**
   - Log whether a result is fresh, network fallback, or cache hit.
   - Log candidates fetched vs extracted vs delivered.
8. **ONNX blocking fix.**
   - Wrap `generate_embeddings_batch` in `spawn_blocking` with a bounded pool, matching the existing Vox embedder adapter.
   - Ensure `deadline_ms` can actually fire during dense/hybrid ranking.

### 6.2 Should ship (`0.1.3-hardening`)

1. arXiv + Stack Exchange adapters first (real JSON APIs, no scraping). GitHub/HN/MDN only after 0.1.3 if fixtures prove stable.
2. Domain-cap two-pass selection (already partly in `order_candidates`; harden + test).
3. Query cache + single-flight verification under Vox (already present in crate; prove TTL/hit logging).
4. Fetch metrics: TLS profile, reused connection, hops.

### 6.3 Explicitly out of scope for 0.1.3 (withdrawn as ungrounded)

`sub_queries`, `depth`, `focus`, corpus retention, token handles, warm handoff, cross-encoder/`bge-m3` adoption, block-level focus extraction. Moved to §12 research. `web_fetch` as a Vox tool is allowed only as a bounded `url`/`format`/`timeout` tool per OpenCode/jcode evidence — no `focus`/`must_contain` params.

---

## 7. Web Search “No Result” Gap: Recommended Fix Plan

This is the specific user complaint. The fix should be distributed across prompt, tool result, and loop.

### 7.1 Tool result contract

The tool already returns usable XML. 0.1.3 should standardize the status so the model can tell what happened:

```xml
<web_search_status code="empty_results" error_kind="not_found"
                 failure_class="fetch_starved"
                 query="...">
  <message>Web search found results, but not enough high-quality readable text.</message>
  <next_action>Retry with a more specific query or a narrower source type.</next_action>
</web_search_status>
```

For normal empty results:

- `failure_class="no_hits"`
- Message and next action as today.
- Add `retry_recommended="true"`.

For answer-absent results:

- `failure_class="answer_absent"`
- Keep `Do not answer from memory; report that the source was not found.`
- Add `retry_recommended="true"`.
- Guide the model to search with an original entity form, version, date, or exact number.

For budget exhaustion:

- `failure_class="budget_exhausted"`
- Message says available evidence did not fit.
- Next action says “answer with the facts you already have and state it as provisional.”

### 7.2 Prompt rule additions

`assemble_with_tools_and_evidence` should include these explicit rules when web search is active:

- `If web_search returns empty_results, do not answer from memory; summarize the failure and ask for or propose a narrower search.`
- `If the empty result says answer was not found, state clearly that the source did not contain the specific answer. Mention any related context briefly, then ask for a more specific query term.`
- `A second web_search is allowed, but only after changing the query to target a concrete entity, version, date, number, or source name.`
- `Do not repeat the same query more than once.`
- `If two different attempts fail, stop searching and tell the user what was tried.`

This changes behavior even if retrieval quality stays the same.

### 7.3 Loop behavior

Do not add an automatic retry in the harness. Query refinement is a model responsibility and automatic retries burn latency.

Instead:

1. Keep the current non-terminal flow.
2. Add an iteration-scoped counter of distinct tool failures.
3. On the 2nd unique failure, set `allow_tools = false` so the model forms the final answer.
4. Log the counter.
5. Persist the failure type with `ToolCallExecuted`.

This is a small change and avoids hidden work.

### 7.4 Model instructions

The tool description should be revised to state the expected behavior on failure:

> “Returns evidence or a structured status. If the status says the source was not found, do not answer from memory. Say that and either ask for a more specific term or propose the next query.”

This is stronger than the current generic comment.

### 7.5 UI behavior

For a voiced assistant, an empty result must not sound like success:

- If status is `missing_factual_answer`, do not play a success filler.
- Say something like: `I could not find that specific number in the sources.`
- If no result at all: `I could not find anything usable yet.`

This needs a frontend/backend coordination review, but the plan should include it.

---

## 8. Detailed Change List (crate vs Vox)

### 8.1 `nexus-rs` (`submodules/nexus-rs/`)

| Area | Change | Files |
|---|---|---|
| engines | DDG Lite primary + HTML fallback + challenge detection | `src/engines/duckduckgo.rs` (extend), or new `src/engines/duckduckgo_lite.rs` + `mod.rs` wiring |
| engines | SearXNG + custom endpoint support | new `src/engines/searxng.rs`, new `src/engines/custom.rs`, `src/engines/mod.rs` |
| model/builder | `Engine::Searxng`, `Engine::Custom`, fallback order, `fetch_slack`, SearXNG URL | `src/model.rs`, `src/builder.rs` |
| pipeline | Over-fetch until K extracted pages; per-engine failure surface | `src/pipeline.rs` |
| fetcher | Shared profile + 403 UA fallback + clearance cookie jar | `src/fetcher/connector.rs`, `src/fetcher/redirect.rs`, `src/fetcher/client.rs` |
| quality | Harden two-pass domain caps; vertical host ordering | `src/extraction/quality.rs` |
| answer_presence | Failure-class helper (`no_hits`, `fetch_starved`, `answer_absent`, `low_score_floor`, `budget_exhausted`) | `src/answer_presence.rs`, `src/model.rs` |
| verticals | arXiv + Stack Exchange JSON adapters | new `src/engines/verticals/arxiv.rs`, `src/engines/verticals/stackexchange.rs` |
| tests | Parser, fallback, starvation, quarantine tests | `tests/` (testing subagent owns assertions) |

### 8.2 Vox (`app/src-tauri/`)

| Area | Change | Files |
|---|---|---|
| dep | Temp local path dep, then publish + version bump | `app/src-tauri/Cargo.toml` |
| state | Builder: engine list + fallback order + SearXNG URL + concurrency + `fetch_slack` | `src/core/state.rs` |
| tool | Request 5+slack candidates; map `failure_class`/`retry_recommended` into status XML | `src/services/harness/stages/tools/web_search.rs` |
| prompt | Empty-result + one-retry rules when `web_search` active | `src/services/harness/stages/prompt.rs` |
| loop | Distinct failure counter; `allow_tools=false` after 2nd unique failure; log + persist | `src/services/harness/loop.rs`, `src/services/harness/steps.rs` |
| fetch tool | Bounded `web_fetch` (`url`/`format`/`timeout` only) reusing `EgressFetcher` | new `src/services/harness/stages/tools/web_fetch.rs`, `registry.rs`, `mod.rs` |
| eval | Expand query set; stage latency + quality harness; QA compare | `evals/`, `evals/assets/web_search_corpus.json`, `evals/agentic-tool/` |
| specs (end of task) | Update `web-search.md` status contract; amend `lld.md`; update feature ledger | `docs/specs/tools-specs/web-search.md`, `docs/specs/tools-specs/lld.md`, `docs/features/nexus-web-scraper.md` or `web-search-nexus-rs.md` |

---

## 9. Verification Plan (baseline-first, QA-scored)

Order is load-bearing. No implementation before baseline.

### 9.0 Step 0 — expand eval set + baseline (first task)

1. Expand `evals/assets/web_search_corpus.json` to >=24 queries across news, technical, comparative, factual-entity, walled/adversarial, longtail.
2. Each item: `id`, `category`, `query`, `expected_facts`, `min_distinct_sources`, `max_acceptable_ms`.
3. Run baseline on current published `nexuss 0.1.2` with local path unchanged. Capture per-stage `NexusSearchMetrics` + Vox XML output.
4. Spawn QA subagent: independently web-searches each query, records its own evidence/URLs, scores crate result vs independent result on relevance, source diversity, answer presence, and latency.
5. Baseline report is the gate. No 0.1.3 code until baseline lands.

### 9.1 Crate

Run:

```bash
cd submodules/nexus-rs && cargo test --release
cd ../app/src-tauri && ./scripts/verify-targets.sh
```

Add tests:

1. DDG Lite parser returns at least one organic result for common queries.
2. Candidate over-fetch keeps enough results when 2 of 6 fetches fail.
3. Domain capping prevents more than 2 slots per apex domain.
4. Vertical engines return non-empty arrays for their API responses.
5. Answer-presence classification has no false fire on abstract queries.
6. Fetch timeout closes, even when ONNX is heavy.
7. Status failure class is correctly emitted on empty, starved, and absent cases.

### 9.2 Vox

Run:

```bash
cd app/src-tauri && ./scripts/verify-targets.sh
pnpm test
pnpm tsc --noEmit
```

Add eval cases:

1. Search succeeds where it currently gets 1 passage.
2. Search over-fetch catches the failure state in a recorded network fixture.
3. Empty status triggers one retry with a clarified query.
4. After two failures, final text states failure rather than memory.
5. A simple factual query returns evidence in under 8s.

### 9.3 Metrics threshold (baseline vs 0.1.3 delta required)

For the same expanded query set, before vs after:

- Extracted pages >=3 or clearly declared empty.
- Delivered passages >0 rate improves; target >=12/20 non-empty queries.
- No `answer_absent` result cites fabricated facts.
- Fetch success rate, fanout/fetch/extract/chunk/rank ms, and total ms recorded per query.
- QA subagent re-scores after; 0.1.3 ships only on measured win.

---

## 12. Deferred research (not committed)

Corpus retention, `focus`, `must_contain`, sub-query decomposition, token handles, warm handoff, cross-encoder ranking, `bge-m3`. Each needs its own grounded proposal with external evidence before entering any release plan. `web_fetch` beyond bounded `url`/`format`/`timeout` also stays here.

---

## 13. Execution TODO (single ledger)

- [ ] 0. Expand eval corpus + baseline on `nexuss 0.1.2` (gate).
- [ ] 1. QA subagent independent search + scoring (baseline).
- [ ] 2. Crate: DDG Lite + challenge detection + fallback order.
- [ ] 3. Crate: SearXNG/custom + arXiv/StackExchange verticals.
- [ ] 4. Crate: `fetch_slack` over-fetch + domain-cap hardening + transport fallback.
- [ ] 5. Crate: failure-class taxonomy + metrics/logging.
- [ ] 6. Vox: temp path dep + `state.rs` builder wiring.
- [ ] 7. Vox: `web_search.rs` status contract + `prompt.rs` + loop failure counter.
- [ ] 8. Vox: bounded `web_fetch` (only if approved in this release).
- [ ] 9. Re-run eval + QA scoring; final delta report for user approval.
- [ ] 10. User manual test.
- [ ] 11. `cargo publish` nexus-rs 0.1.3; Vox `Cargo.toml` to new version + `cargo update`.
- [ ] 12. Spec updates (mandatory close-out): `docs/specs/tools-specs/web-search.md`, `docs/specs/tools-specs/lld.md`, feature ledger.
- [ ] 13. Verify: `verify-targets.sh`, `cargo test --release` (crate), `pnpm test`, `tsc --noEmit`.

## 14. Research status

No further broad research needed before Step 0. Remaining unknowns are implementation-time checks: SearXNG response shape, arXiv/StackExchange API fields, DDG Lite markup drift, Cloudflare retry scope. Hypothesis: fallback order + Lite + over-fetch fixes most empties. If baseline shows otherwise, report before adapting (ponytail-guy §4).

---

## 10. Risks & Trade-Offs

| Risk | Cost | Mitigation |
|---|---|---|
| Adding keyless verticals takes time | Each new parser requires fixtures and test ownership | Ship arXiv + StackExchange first, then others |
| DDG Lite changes endpoints and anti-bot rules | Provider instability | Health quarantine already exists; add instant-answer only after testing |
| Over-fetch increases latency | 2 extra requests | Cap at 5 by default and cancel outstanding early |
| Prompt changes may make the assistant too tentative | User feels it gave up too early | Allow exactly one retry; otherwise explain what happened |
| Embedded ONNX unblocks tokio but may still be slow | Pool resource cost | Use bounded pool (2 threads) and log pool exhaustion |
| More candidate count means context pressure | Passage budget | Existing progressive popping handles it |
| Status XML changes break consumers | Tool schema change | Update Vox tests/specs at same time |

---

## 11. Deliverable Definition of Done for 0.1.3

0.1.3 is “done” when:

1. Nexus version in `Cargo.toml` and Vox dependency is updated to **0.1.3**.
2. Fallback engine order is explicit, logged, and tested.
3. Fetching over-fetches and recovers from at least one failed candidate in recorded tests.
4. DDG Lite is primary with HTML fallback; challenge pages are detected and reported.
5. At least arXiv and Stack Exchange verticals are functional, if approved.
6. Empty and answer-absent statuses include `failure_class` and `retry_recommended`.
7. Prompt changes force honest failure reporting and one optional retry.
8. Harness code logs distinguish not_found, fetch-starved, answer-absent, and budget cases.
9. Baseline vs 0.1.3 QA delta is recorded and shows a measured win.
10. Specs in §13 TODO-12 are updated.

---

## 15. Retired 0.1.4 depth list (no evidence, do not implement)

Retired without evidence: old corpus retention, `focus`, `must_contain`, sub-query `depth`, token handles, warm handoff, cross-encoder/`bge-m3` plan. Each needs its own grounded proposal before entering any release.
