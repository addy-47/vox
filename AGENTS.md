# AGENTS.md — Vox Workspace Rules

---

## 1.. MANDATORY RULE: AGENTS.md Sync Hook

> 🛑 **MANDATORY POST-TASK HOOK (NON-NEGOTIABLE) — TWO STEPS, IN ORDER:**
>
> **Step 1 — Always: Append to `AGENTS.md` Section 5 only.**
> After every completed task, add a concise bullet to Section 5 describing what changed. Do NOT simultaneously write to `docs/`, `recent_work.md`, or any other file — `AGENTS.md` is the only target.
>
> **Step 2 — Only when approaching 175 lines: Migrate Section 5.**
> After appending, check `AGENTS.md` total line count. If it is at or above **110 lines** (the warning threshold before the 175-line ceiling):
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
| `submodules/`             | Git submodules                                      | `chatterbox-rs`, `query-sieve-rs`, `distilbert-query-classifier`, `vox-models`. Do not edit directly. |
| `~/.vox/models/`          | Local model weights                                 | Canonical manifest: `~/.vox/models/models_manifest.json`.                                             |

**Remote GPU server:** `root@[IP_ADDRESS]` (creds in `temp/server.txt`). Ollama . **Never kill running server processes.**

---

## 3 Execution & Testing Invariants (All Agents)

1. **Sequential Execution:** Run performance-sensitive tasks (benchmarks, evals, test suites) strictly one at a time to prevent CPU, memory, and I/O contention.
2. **Release / Optimized Mode:** Always run performance measurements and benchmarks under release mode (`--release`). Debug builds produce invalid metrics.
3. **Isolated Test Runner (`cargo-nextest`):** Always use `cargo-nextest run` with explicit thread pool allocation and single-thread isolation. Nextest defaults to fail-fast (cancels the suite on first failure). Use `--no-fail-fast` to execute the full suite without aborting on failure:
   ```bash
   RAYON_NUM_THREADS=$(nproc) OMP_NUM_THREADS=$(nproc) cargo nextest run --release --test-threads=1 --no-fail-fast
   ```
   _Isolate single test:_ `cargo nextest run -E 'test(<test_fn_name>)' --release --nocapture --test-threads=1`
   _Isolate test file:_ `cargo nextest run --test <test_file> --release --nocapture --test-threads=1`
   > ⏱️ **Full Suite Baseline Run Time:** ~45.5s test execution (~45s wall-clock with compilation cache hit; ~2m30s cold compile + run) across all 105 tests in 21 binaries (across Seams 1–20).
4. **External API Keys (`#[ignore]`):** Cloud provider tests (Nvidia, Gemini Live, Deepgram, OpenAI, ElevenLabs) must be marked `#[ignore]` and run manually only with explicit user approval: `cargo nextest -- --ignored`.
5. **Local Turso Database CLI (`tursodb`):** For inspecting or querying the local Turso SQLite database (`~/.vox/vox.db`), use the native `tursodb` CLI tool:
   ```bash
   tursodb ~/.vox/vox.db "SELECT name FROM sqlite_master WHERE type='table';"
   ```

---

## 4. Invariants ,Rules and Specs [MUST FOLLOW]

### 4.1 Critical Architectural & Logical Invariants (Non-Negotiable Concepts)
0. **Zero Backward Compatibility (ZBC):** Backward compatibility is not a requirement unless explicitly stated. Break, replace, or redesign existing interfaces when that produces the better architecture. Never introduce compatibility layers, legacy paths, 
1. **Exploration hook:** Before any codebase exploration, architecture lookup, graph query, or broad search, read `.agents/rules/codebase-memory-mcp.md` and follow its graph-first workflow.

### 4.2. HARD GATE: Code Modification Gate

> 🛑 **MANDATORY CONTEXT GATE:**
>
> - **WRITE TASK (Backend Rust):** You MUST read `.agents/rules/backend-style-guide.md` AND `.agents/rules/backend-engineer.md` BEFORE modifying Rust backend code.
> - **WRITE TASK (Frontend React/TS):** You MUST read `.agents/rules/frontend-style-guide.md` AND `.agents/rules/frontend-engineer.md` BEFORE modifying frontend code.
> - **WRITE TASK (Tests/Benches/Evals):** You MUST read `.agents/rules/testing-style-guide.md` AND `.agents/rules/test-engineer.md` BEFORE authoring tests or benchmarks.
> - **READ-ONLY TASK (Auditing, answering questions, running tests/benchmarks, searching code):** DO NOT read code style files. Save context tokens.


### 4.3 Specifications, Behavioral Contracts & Non-Drift Hook [MANDATORY]

> 🛑 **MANDATORY SPEC ALIGNMENT HOOK (NON-NEGOTIABLE):**
> Every agent working on Vox must adhere strictly to the approved specifications.
> 1. **Specification Divergence / Additions**: If an agent needs to implement, modify, or add behavior, commands, or schemas that diverge from or are not defined in the relevant spec, it MUST STOP and ask the user for approval. If approved, the agent MUST update the spec artifact FIRST before authoring or modifying code. Specifications must NEVER quietly drift from code.
> 2. **Code Divergence / Legacy Code**: If existing code implements nuances or legacy behaviors not defined in the spec, the agent MUST confirm with the user first before either pruning the code or updating the spec to capture the behavior.

#### Active Specifications Ledger
1. **[Event-Domain Architectural Specification (Ground Truth)](file:///home/addy/projects/apps/vox/docs/specs/events-spec.md)** — *Status: Approved SSOT*. Golden source of truth for pipeline behavior, state transitions, and 6-domain contracts.
2. **[Database & Persistence Specification (v2)](file:///home/addy/projects/apps/vox/docs/specs/db-spec.md)** — *Status: Approved Target Spec*. Strict persistence boundary, native Turso engine invariants, and normalized v2 schema.
3. **[Minimal Cognitive Memory & Session Continuation Spec (v2)](file:///home/addy/projects/apps/vox/docs/specs/memory-spec.md)** — *Status: Approved Target Spec*. 2-stage dedup, single evolving personal memory document, rolling working compaction, and deferred episodic tool retrieval.
4. **[IPC Command & Event Specification (v2)](file:///home/addy/projects/apps/vox/docs/specs/ipc-spec.md)** — *Status: Approved / Target Spec*. Grouped frontend-to-backend commands and backend-to-frontend IPC events.
5. **[LLM Agent Harness & Plugin Runtime Specification (v2)](file:///home/addy/projects/apps/vox/docs/specs/harness-spec.md)** — *Status: Approved Target Spec*. Plugin chassis, 1:1 session lifecycle (`start_session(Option<sessionId>)`), decoupled LLM actor with duplex dialogue pipe, `InteractionState::Working` with audio intent gating, and clean-slate deletion of legacy harness code.
6. **[Notification Center Behavioral & Interface Specification (v2)](file:///home/addy/projects/apps/vox/docs/specs/notifications-spec.md)** — *Status: Proposed Target Spec*. Append storage with correlation key, task idempotency, and frontend stream rollup.
7. **[Agentic Tools & Tool Runtime Specification (v1)](file:///home/addy/projects/apps/vox/docs/specs/tools-spec.md)** — *Status: Approved Target Spec*. Dual classification (`SideEffect` vs `Observation`), capability gating, RRF hybrid retrieval, and scratchpad rollback.

---

## 5. Phase 12 — Recent Work Summary

> 📖 **Full History:** [recent_work.md](file:///home/addy/projects/apps/vox/docs/plans/phase12/recent_work.md) | Phase 11 Archive: [phase11/recent_work.md](file:///home/addy/projects/apps/vox/docs/plans/phase11/recent_work.md)

- **Agentic runtime & provider SSOT shipped:** Full tool taxonomy, reentrant harness loop, session gating, provider wire policy mapping, and 122 integration tests with 73% mutation kill rate.
- **Realtime, memory, and UX foundations:** Streaming TTS/metrics, realtime tool translation, session continuation, boundary compaction fixes, and deterministic frontend invariant/stress tooling.
- **Memory pipeline evaluation & semantic audit:** Built 14-case eval suite, executed operational baseline (Case 1), audited memory grounding overclaims, hardened extraction prompts, and stabilized `evals/memory_pipeline_eval.rs`.
- **Web search tool integration & adversarial audits:** Conducted production-scale architectural, security, and contract audits of `submodules/nexus-rs` and `web_search.rs` to prepare verified implementation.
- **Web search remediation & live eval passed:** Resolved all findings from adversarial audit across `submodules/nexus-rs` (body-cap prefix retention, 5-hop redirect bound, bounded SERP chunk stream, engine time_filter mapping) and `web_search.rs` (live turn-budget integration, persistent injection guard across evidence passes, C0 control stripping, XML query escaping, Stage 3 timeout encapsulation, runtime IPC settings); 6/6 Seam 21 integration tests green; executed live OpenRouter eval with `inclusionai/ling-3.0-flash-sante:free` achieving 1/1 tool execution in 4.7s and grounded synthesis in 12.4s.
- **Agentic tool eval genericization & step telemetry:** Refactored `evals/agentic_tool_eval.rs` to be generic (zero temp/.env or tool-specific logic, queries fed via CLI, Kokoro TTS audio capture, shared `evals/common/` helpers); instrumented granular step latencies across `submodules/nexus-rs` (per-engine fanout, URL dedup, per-page fetch/extract) and `WebSearchTool`; verified September 2026 Apple event query producing grounded synthesis and 29.86s Kokoro audio in `response.wav`.
- **10-query web search benchmark & latency profiling:** Executed full 10-query September 2026 evaluation with pre-warmed MiniLM embedder and Kokoro TTS; achieved average 3.37s tool latency (Fanout: 1.82s, Fetch: 0.85s, Extract: 25ms, Rank: 0.68s) and 9.1/10 factual grounding score; isolated Yahoo fanout tail (1.81s vs Bing 0.44s/DDG 0.32s) and dense ONNX batch sizing as primary bottlenecks with quorum early-exit and 2-stage reranking optimization plan.
- **Adaptive fanout quorum, fast extraction & 2-stage reranking:** Enhanced `submodules/nexus-rs` with configurable `FanoutPolicy` and `RankingPolicy`, Google WML scraper, URL consensus multiplier, and single-pass title extraction; kept tuning parameters as internal Level 3 domain constants in `web_search.rs` with only `web_search_enabled` user-facing; all 25 nexus tests and 6 Seam 21 integration tests green; reduced live tool latency from 4.75s to 2.16s.
