# AGENTS.md — Vox Workspace Rules

---

## 1.. MANDATORY RULE: AGENTS.md Sync Hook

> 🛑 **MANDATORY POST-TASK HOOK (NON-NEGOTIABLE) — TWO STEPS, IN ORDER:**
>
> **Step 1 — Always: Append to `AGENTS.md` Section 5 only.**
> After every completed task, add a concise bullet to Section 5 describing what changed. Do NOT simultaneously write to `docs/`, `recent_work.md`, or any other file — `AGENTS.md` is the only target. [Note: SKip this for tasks that lack accountability or can impact a future agent's work]
>
> **Step 2 — Only when approaching 175 lines: Migrate Section 5.**
> After appending, check `AGENTS.md` total line count. If it is at or above **110 lines** (the warning threshold before the 175-line ceiling)
>
> 1. Migrate **only the delta**: append to `docs/plans/<current_phase>/recent_work.md` under a `## Past Work (YYYY-MM-DD)` heading just the Section 5 entries added since the last migration. Dedupe against the file first — never re-archive entries already present there (snapshotting the whole Section 5 duplicates history).
> 2. Replace Section 5 in `AGENTS.md` with a compact 3–5 bullet summary of only the highest-level milestones.
> 3. Keep the deep link at the top of Section 5: `📖 Full History: [recent_work.md](file:///home/addy/projects/apps/vox/docs/plans/<current_phase>/recent_work.md)`.
>
> **This is the complete hook. Nothing else is mandatory on every task.**

---

## 2. Workspace Directory Map

| Path                      | Purpose                                             | Rules                                                                                                 |
| ------------------------- | --------------------------------------------------- | ----------------------------------------------------------------------------------------------------- |
| `app/src-tauri/src/`      | Purpose Rust source                                 | No test logic. No benchmarks.                                                                         |
| `app/src-tauri/tests/`    | Integration tests                                   | Named `<feature>_test.rs`. Tests public API only.                                                     |
| `app/src-tauri/benches/`  | Performance benchmarks                              | Named `<feature>_bench.rs`. `harness = false` + custom `fn main()`.                                   |
| `.agents/rules/`          | Role-specific agent instruction files               | Read relevant file before acting in that role.                                                        |
| `docs/plans/`             | Architecture specs and phase plans                  | Source of truth for specs. Do not contradict.                                                         |
| `docs/features/`          | Implemented feature ledgers                         | Update after completing features.                                                                     |
| `sandbox/`                | Scratch space for experiments, evaluations, scripts | Non-production code. Results in `sandbox/results/`. Datasets in `sandbox/datasets/`.                  |
| `temp/`                   | Ephemeral runtime files: logs, raw LLM outputs      | `temp/.env` (API keys). `temp/server.txt` (remote GPU server creds). Not versioned.                   |
| `submodules/`             | Git submodules                                      | `chatterbox-rs`, `query-sieve-rs`, `distilbert-query-classifier`, `vox-models`, `nexus-rs`.       |
| `~/.vox/models/`          | Local model weights                                 | Canonical manifest: `~/.vox/models/models_manifest.json`.                                             |

**Remote GPU server:** `root@[IP_ADDRESS]` (creds in `temp/server.txt`). Ollama . **Never kill running server processes.**

---

## 3 Execution & Testing Invariants (All Agents)

0. **Command Context Gate:** Only use `cargo clippy --all-targets` this to verify syntax no other cmds. Test cmds must not be run without explicit user approval.

1. **Sequential Execution with Release flag:** Run benchmarks, evals ,test suites or any cargo command strictly one at a time to prevent CPU, memory, and I/O contention and always use the `--release` flag.
2. **Isolated Test Runner (`cargo-nextest`):** Use the cmd listed below and its variations . 
   ```bash
   RAYON_NUM_THREADS=$(nproc) OMP_NUM_THREADS=$(nproc) cargo nextest run --release --test-threads=1 --no-fail-fast
   ```
   > ⏱️ **Full Suite Baseline Run Time:** ~45.5s test execution without cold compilation , always use a timeout or cron to monitor test runs and avoid hangs.
4. **Test with External Dependencies(`#[ignore]`):** Cloud or remote server tests must be run manually only with explicit user approval: `cargo nextest -- --ignored`.
---

## 4. Invariants ,Rules and Specs [MUST FOLLOW]

### 4.1 Critical Architectural & Logical Invariants (Non-Negotiable Concepts)
**Zero Backward Compatibility (ZBC):** Backward compatibility is not a requirement unless explicitly stated. Break, replace, or redesign existing interfaces when that produces the better architecture. Never introduce compatibility layers, legacy paths, 
> 🛑 **MANDATORY PRE-EXPLORATION HOOK:**
> - Before any codebase exploration, architecture lookup, graph query, or broad search, read `.agents/rules/codebase-memory-mcp.md` and follow its graph-first workflow.
>
**Local Turso Database CLI (`tursodb`):** For inspecting or querying the local Turso SQLite database (`~/.vox/vox.db`), use the native `tursodb` CLI tool:
   ```bash
   tursodb ~/.vox/vox.db "SELECT name FROM sqlite_master WHERE type='table';"
   ```

### 4.2. HARD GATE: Code Modification Gate

> 🛑 **MANDATORY PRE-MODIFICATION HOOK:**
> - You MUST read the corresponding style guide and engineer rule file for the specific area you are working on located in .agents/rules/.

### 4.3 Specifications, Behavioral Contracts & Non-Drift Hook [MANDATORY]

> 🛑 **MANDATORY SPEC ALIGNMENT HOOK:**
> Every agent working on Vox must adhere strictly to the approved specifications.
> 1. **Specification Divergence / Additions**: If an agent needs to implement, modify, or add behavior, commands, or schemas that diverge from or are not defined in the relevant spec, it MUST STOP and ask the user for approval. If approved, the agent MUST update the spec artifact FIRST before authoring or modifying code. Specifications must NEVER quietly drift from code.
> 2. **Code Divergence / Legacy Code**: If existing code implements nuances or legacy behaviors not defined in the spec, the agent MUST confirm with the user first before either pruning the code or updating the spec to capture the behavior.

### 4.4 Active Specifications Ledger
Authoritative system specifications reside in [`docs/specs/`](file:///home/addy/projects/apps/vox/docs/specs/). Key target specs include **[storage-spec.md](file:///home/addy/projects/apps/vox/docs/specs/storage-spec.md)** (SSOT filesystem, 3-way config decomposition, and runtime paths), **[events-spec.md](file:///home/addy/projects/apps/vox/docs/specs/events-spec.md)** (SSOT pipeline & 6-domain contracts), **[dictation-spec.md](file:///home/addy/projects/apps/vox/docs/specs/dictation-spec.md)** (dictation & OS output), **[harness-spec.md](file:///home/addy/projects/apps/vox/docs/specs/harness-spec.md)** (LLM runtime), **[db-spec.md](file:///home/addy/projects/apps/vox/docs/specs/db-spec.md)**, **[memory-spec.md](file:///home/addy/projects/apps/vox/docs/specs/memory-spec.md)**, **[ownership-spec.md](file:///home/addy/projects/apps/vox/docs/specs/ownership-spec.md)**, etc.

---

## 5. Phase 12 — Recent Work Summary

> 📖 **Full History:** [recent_work.md](file:///home/addy/projects/apps/vox/docs/plans/phase12/recent_work.md) | Phase 11 Archive: [phase11/recent_work.md](file:///home/addy/projects/apps/vox/docs/plans/phase11/recent_work.md)

- **Agentic Vox Runtime:** Tool-calling runtime with `respond_and_set_title` (terminal) and `search_memory`/`web_search` (non-terminal) tools, reentrant loop (max 5 iterations), `session_tool_calls` DB table (Schema v5), capability gating via `model_capabilities.json` cache, and `turn_open`/`drained_while_open` synthesis guard latch.
- **Semantic Structured Personal Memory:** Canonical JSON semantic model replacing positional Markdown, personal memory versioning, Google Docs inline suggestion review, indexed-block consolidation, and full memory evaluation framework with NVIDIA NIM 120B judge.
- **Dictation Overhaul:** Owner-stamped `VoxEvent` pipeline eliminating TOCTOU races, native OS notifications replacing GTK toast, 1.2s silence auto-stop watchdog, and live STT partials streaming — with 15k disfluency dataset (Gate 2 passed: 1.734% harmful, 0.0% over-deletion).
- **Theme & Performance:** rAF token interpolation, scoped blur suspension, compact-viewport SSOT (1024px), `VoxLogoLoader` SVG, and wizard hardening (lazy webview, reveal-after-paint, de-generic icons).
- **Web Search Hardening & Multi-Turn Retention:** Panic recovery boundary (`catch_unwind`), transparent error reporting, silent harness deadline clamp (12s ceiling, 1s floor), Option 2 compacted tool observation retention in working history, `TurnCorpus` zero-network follow-ups, intent-aware query caching, keyless Brave and Wikipedia REST engines, egress TLS browser spoofing, over-fetch candidate slack, DonSeTch entity-coverage penalty (0.3x), zero-anchored score calibration, and Jaccard passage deduplication.
- **Web Retrieval Architecture (v0.1.2 Complete):** Targeted document ingestion via `web_fetch` tool, engine health EWMA & 10m circuit breaker quarantine, candidate domain diversity (`max_per_domain <= 1`), token handles (`S1...Sn`) for compact 3-token referencing, single-flight in-flight request coalescing, and system prompt routing directives.
- **Web Retrieval Boundary Decoupling:** Re-aligned architectural responsibilities across nexus-rs, tool definitions, and harness chassis: query caching, single-flight coalescing, single-page egress ingestion, and focused BM25 passage extraction consolidated inside nexus-rs; ToolDefinition::compact_observation trait added for modular observation pruning; tool-specific XML dependencies removed from harness history; reverted tool capability detection bypass.
- **Web Retrieval QA Hardening (Batch 10 Complete):** Resolved gate blockers B1–B5 and gaps O1–O5: default relevance floor (min_score=0.12), harness outer backstop aligned to 13s with structured deadline status, RAII InFlightGuard cancellation recovery & bounded follower wait, mutex poisoning resilience, per-engine catch_unwind isolation, focus lexical extraction parameter, and hermetic tests for circuit breakers, parsing, and coalescing.
- **Eval Gate QA Remediations & Mutation-Tested Verification:** Replaced false-green tests with hermetic, mutation-killing tests in nexus-rs and Vox harness: B3 InFlightGuard drop cleanup & bounded follower wait (Mutants 1 & 2 killed), B2 13s harness timeout ceiling & structured XML status assertions, B4 cache mutex poison recovery via into_inner(), and B5 engine panic isolation in fanout (all 92 nextest tests passing).
- **Eval Gate Opened & Smoke Test (3 queries):** Re-verified all B1–B5 fixes in source, killed both B3 mutants against the new tests (both went red), confirmed 92/92 green + vox clippy clean, deleted `EVAL-GATE-REVIEW.md`. Smoke results: `news_04` fixed (correct ECB +25bp content, 2 sources incl. ecb.europa.eu, calibrated 0.62–0.66 scores); `cmp_01` medical-burn false positive gone but via withhold-all on 2% ceiling overshoot (brittle — needs trim-one-more); `ent_04` still returns nav/marketing noise as `retrieval_ok=true` (0.12 floor too low for this class). Flagged: `__test_` magic-query hooks in `engines/mod.rs` ship ungated in prod — must be env-gated before 0.1.2 publish.
- **Extended Smoke (9 queries) & Root Report:** Ran 6 more queries (`tech_05/03`, `wall_02`, `long_01`, `cmp_04`, `ent_02`); personally verified all evidence. Results: `ent_02` fixed (1024 present, primary sources); `tech_05`/`long_01`/`cmp_04` hit 5.5s deadline honestly (latency regressed vs round 2); `tech_03`/`wall_02` still false-ok (floor too low, Wikipedia engine amplifies company-page bias). Wrote `EVAL-SMOKE-REPORT.md` (root) with per-query verdicts, latency table, and 8 flagged issues S1–S8 for backend discussion.
- **Full 24-Case Batch & Review Fixes:** Re-reviewed S1–S8 fixes in crate + Vox (S1 adaptive fanout, S3 reword, S4 trim loop, S5 entity penalty, S6 discussion-intent, S7 env-gated hooks all verified; fixed B1 regression where AppState singleton dropped explicit `min_score`, stale model.rs comment, 2 red backend tests via shared `cache_key_for` helper). Verified clippy clean both trees + 91/91 nextest green. Batch `20261007_180849_e8967217`: 24/24 ok plumbing, flag 16/24 vs hand-verified ~9/24 answer-bearing; found run-to-run instability (news_04 fixed→broken), 7 false-ok, 3 suspect over-filters, 2 cases exceeding 5500ms without deadline firing (blocking-stage starves timer hypothesis). Rewrote `EVAL-SMOKE-REPORT.md` with full verdicts + F1–F8.
- **Deterministic Multi-Domain Corpus & Precision Report:** Replaced `evals/assets/web_search_corpus.json` with 24 stable-fact queries across 14 domains (geography, science, math, units, history, literature, law, medicine, sport, music, computing) and 4 query types. Independently verified ground truth for all 24 via own web searches, then machine-scored evidence XML. Batch `20261007_190142_0435a672`: 24/24 `retrieval_ok` (100%) but only **14/24 (58%) answer-bearing** — 42-pt false-confidence gap. Latency recovered (mean 3261ms, max 4893ms, **0** over the 5500ms deadline vs 3 previously; fetch 1804ms dominant, rank down to 421ms). Two clean failure mechanisms: (A) engine resolved query head noun to a Wikipedia article about that word (`year the Berlin Wall fell` → `wiki/Year`, `number of justices` → `wiki/Number` returning Euler's *e* chunk); (B) right page/domain but answer absent from all 5 selected chunks (MDN `/Web/HTTP` index instead of the 429 page). No hallucinations observed — evidence is verbatim; failure is omission, which invites the model to guess from parametric memory. Top ask: answer-presence gate.
- **Post-Fix Deterministic Re-Run (58%→79%):** Backend agent shipped generic-head-stub denylist (`quality.rs` `GENERIC_HEAD_STUBS`, `is_generic_head_stub`), stopword-aware anchors (proper nouns + acronyms), and substantive-token candidate boosting. Batch `20261008_050729_38493ac8` on the same 24-query corpus with identical self-verified ground truth: **precision 14/24 (58%) → 19/24 (79%)**, flag gap +42pts → +17pts, mean sources 1.6→2.0 (3+ sources in 8 cases), Wikipedia share 75%→54%. All four head-noun→wiki-stub failures fixed. Latency mean 3261→3791ms, max 5380ms, **still 0 over deadline**; +592ms all in rank (421→1013ms, denser corpus). **Two regressions found:** geo_01 (Everest) HIT→empty_results and math_01 HIT→`wiki/Triacontagon` — new `+2`-per-substantive-token boost in `order_candidates` over-promotes pages matching generic query words and over-filters correct ones. Answer-presence gate still unimplemented (`retrieval_gate` naive at `main.rs:674`). **Suite not green:** 2 stale failures in `chunking_ranking_test.rs` asserting old position-RRF math after `hybrid.rs` moved to direct cosine+normalized BM25. Report updated at `EVAL-SMOKE-REPORT.md` (root).
- **`nexuss` 0.1.2 Published (Final Iteration, 62%→88%):** Measured the real 0.1.1 baseline by checking out `ceff55e` (the tag `v0.1.1` predates the crate rename to `nexuss`) behind a 3-site Vox shim: **15/24 = 62%**. Progression on the identical corpus and ground truth: 62% → 58% → 79% → 83% → **21/24 = 88%**, flag gap 42pts → 0, mean latency 3333→3208ms, 0 over deadline. Implemented `answer_presence` module in nexuss (answer-shape classification + candidate-answer verification on `NexusSearchMetrics`), replacing Vox's too-weak inline `is_numeric_query` check; added eval-harness `--case-delay-ms` pacing and `--transient-retries` with a distinct `transient_failure` field after discovering back-to-back batches trip provider throttling and cost a full run (62% vs 83% on identical code). Corrected stale "Hybrid RRF" claims across README/metadata after RRF was replaced by 65/35 dense+BM25. Committed, tagged `v0.1.2`, pushed, published to crates.io, re-pointed Vox to the registry version, and re-verified parity (88%). 102/102 nexuss tests green, clippy clean both trees. Final report at `docs/benchmarks/nexuss-web-search-benchmark.md`; root `EVAL-SMOKE-REPORT.md` deleted. **3 mechanism-B misses remain** (`lit_01`, `comp_01`, `comp_02`) — the gate detects absence of a candidate answer, not presence of a wrong one, so entailment-level verification is the real next step.
- **Web Retrieval Hardening & Scratchpad Implementation (Batches 1 & 2 Complete):** Added non-blocking fanout collection with background engine health updates and deadline in `nexus-rs`; gated `__test_` hooks behind `NEXUS_TEST_HOOKS`; added acronym anchor extraction (`WML`, `S2S`, `TREC`) and Wikipedia discussion penalty; fixed zero-score normalization bug (`0.0` baseline) and capped non-anchored passages below relevance floor; implemented session-monotonic `task_{:02}` IDs; implemented `query_scratchpad` tool for $<20$ms silent surplus evidence inspection; stripped `focus` from `web_fetch` and aligned with standard schemas; implemented Option 1 working context retention; verified clean clippy (0 warnings) across both crates.
- **Clean Single-Tool Architecture & Nexus-RS Boundary Decoupling:** Stripped premature sibling tools (`web_fetch`, `query_scratchpad`), surplus scratchpads, synthetic IDs, and late hits from `nexus-rs` while preserving acronym anchors, Wikipedia penalty, cleaner UTF-8 bounds, and score calibrations; added `nexus_search` singleton to `AppState`; moved context budget ceiling calculation to `ContextBudgetStage` (`calculate_max_observation_tokens`) and passed via `ToolExecutionContext`; implemented progressive passage popping (anti-withhold) in `WebSearchTool`; added `catch_unwind` panic isolation and structured `<web_search_status>` XML responses; verified 0 errors and 0 warnings with `cargo clippy --all-targets` across `nexus-rs` and `Vox`.
- **Retrieval Quorum & Pipeline Hardening (Eval Verified):** Excluded Wikipedia from engine quorum early-exit in `nexus-rs` fanout; gated zero-sparse/zero-dense inputs and cosine-scaled RRF rankings to prevent artificial score inflation; moved ONNX embeddings to `tokio::task::spawn_blocking` to prevent Tokio worker thread starvation; raised observation ceiling floor to 500 tokens in `ContextBudgetStage`; increased page fetch concurrency to 5 and introduced dynamic deadline budgeting for page fetch; verified 100% retrieval success across test queries with high semantic grounding (`3598ms`, `2907ms`, `3723ms`).
- **Retrieval Quality & Ranking Overhaul (Direct Cosine & Head-Noun Penalty):** Added generic head-noun stub penalty (`-20`) and substantive query token extraction to eliminate dictionary/disambiguation stubs (`/wiki/Year`, `/wiki/Number`); upgraded hybrid scoring from position RRF to direct dense cosine similarity ($65\%$) and normalized BM25 ($35\%$); enhanced passage chunking with paragraph/block preservation (`\n\n`) to protect tables and lists; added factual/numeric answer-presence verification gate in `WebSearchTool`; verified 0 warnings with `cargo clippy --all-targets` across `nexus-rs` and `app/src-tauri`.
- **Subject-Anchor Reordering & Test Suite Repair:** Weighted primary subject entity anchors higher in `order_candidates` (+6 for title matches) while damping generic measurement tokens; added anchor/substantive overlap boost to 2-stage reranking candidate pre-filtering to protect single-sentence facts; aligned eval benchmark `retrieval_gate` with numeric/status validity; repaired `chunking_ranking_test.rs` unit tests for direct cosine+BM25 hybrid contract; verified 0 errors and 0 warnings with `cargo clippy --all-targets` across `nexus-rs` and `app/src-tauri`.
- **Tooltip Redesign & Overlay Radius Scale + Android Frontend Prep (Batch 5–6):** Ran dual-agent impeccable critique (design review 23/40 Acceptable + detector evidence) on tooltips/popovers/menus; redesigned `Tooltip` to compact 6px-radius dark tooltip (wrap not truncate, theme-aware kbd, no mobile-GPU blur); locked overlay radius scale (`rounded-3xl` removed from MemoryNodeTooltip/Monitoring popover/Modal/MemoryStates, menus to `rounded-lg`, TitleBar cards to `rounded-lg` + focus-within, faux-pill badges to `rounded-md`); fixed `MessageContent` dead-ternary bug hiding the assistant variant. Implemented Android plan frontend batches: new `src/lib/capabilities.ts`, desktop-gated gesture suppression + `touch-action` in `index.html` (`viewport-fit=cover`), safe-area insets + coarse-pointer guards in `index.css`, TitleBar window-controls hidden on touch, Ctrl+W/Q gated in `ResponsiveLayout`, EdgeNav/monitor safe-area offsets, wizard phone treatments (scrollable shells, hidden sidebar <md, responsive footer with safe-area, wrapped device names). Verified `tsc --noEmit` clean, `pnpm build` green, impeccable detect clean.