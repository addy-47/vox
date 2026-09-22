# AGENTS.md — Vox Workspace Rules

---

## 1.. MANDATORY RULE: AGENTS.md Sync Hook

> 🛑 **MANDATORY POST-TASK HOOK (NON-NEGOTIABLE) — TWO STEPS, IN ORDER:**
>
> **Step 1 — Always: Append to `AGENTS.md` Section 5 only.**
> After every completed task, add a concise bullet to Section 5 describing what changed. Do NOT simultaneously write to `docs/`, `recent_work.md`, or any other file — `AGENTS.md` is the only target.
>
> **Step 2 — Only when approaching 175 lines: Migrate Section 5.**
> After appending, check `AGENTS.md` total line count. If it is at or above **125 lines** (the warning threshold before the 175-line ceiling):
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

0. **Zero Backward Compatibility (ZBC):** Backward compatibility is not a requirement unless explicitly stated. Break, replace, or redesign existing interfaces when that produces the better architecture. Never introduce compatibility layers, legacy paths, or transitional abstractions proactively..
1. **Registry-Owned Event Contracts:** `core/events.rs` is the SSOT for all cross-boundary events. Internal pipeline events belong to `VoxEvent`; IPC events belong to `IpcEvent` with strongly typed payloads. Raw string event literals are forbidden at emit and listen sites; frontend mirrors the registry via `IpcEventMap`.
2. **Sacred Audio Hot Path:** Zero allocations, zero locks (`Mutex`/`RwLock`), zero blocking I/O on the CPAL audio thread and VAD inference loop. Ring buffers are lock-free, pre-allocated, and single-consumer (`VadActor` only — never attach secondary readers).
3. **Strict Frontend Service Boundary:** React components and hooks must never directly call `invoke`/`listen`. All backend interactions route through strongly-typed singleton service modules in `src/services/`.
4. **Centralized Monotonic Turn Lifecycle:** Turn IDs come exclusively from `PipelineAtomics::next_turn()` at turn boundaries as a `(turn_id, token)` bundle. Never reset to 0, fragment across actors, or fabricate dummies; subsystems receive the bundle, never advance the counter.
5. **Covered in style guides (not repeated here):** thread placement (inference on OS threads, Tokio for I/O), React 19 memoization + atomic selectors, strict import hoisting (`backend-style-guide.md §2.1`).

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

- **Agentic Runtime Shipped (Batches 0–8):** Tool taxonomy (`Terminal`/`NonTerminal`), reentrant loop, session gating, audit remediation, flattened harness, prompt directives, eval harness.
- **Verification (122 integration + mutation 73% kill):** Seams 6, 8, 9, 11, 20 + NEW Seam 21 green; 4 surviving mutants tracked; integration spec SSOT updated.
- **Provider Wire SSOT + Adapter Cleanup:** `provider-model-catalog-spec.md` v2, 14-row manifest with vendored `modelparams.dev` cross-checks, manifest-driven serializers, `TokenLimitField`/`CapabilitySource` deleted, Seam 21 wire tests green.
- **Next: Live Eval Rerun:** `#[ignore]` live tests (NIM + Ollama `qwen3.5:9b`) + judged `agentic_tool_eval`; testing/eval unpause on green.
- **Eval Green — Set-Title E2E (2026-09-22):** `agentic_tool_eval --no-judge` (server `qwen3.5:9b` native + kokoro) scores **1/1 tool calls**: ledger row + title + spoken wav; root-caused prior 0-score to missing persistence-worker wiring in eval (mirrored `engine.rs`), not model/transport; empty `turns` in eval is expected (eval bypasses pipeline `on_llm_finished`).
- **Eval Green — Search-Memory E2E (2026-09-22):** Same harness scores **1/1** on `search_memory` (non-terminal, 2 LLM passes): filler + RRF observation (`RTX 4090`) + grounded final answer + 13s wav; ledger row `is_error=0`.
- **Prod Tool-Call RCA + Fixes (2026-09-22):** vox2.log no-tool-call root cause = stale `persona.modular_prompt` (641 chars, missing directives; fixed in `settings.json`, 881 chars) + capability-cache key mismatch (`server:` reader vs `openai_compat:` writer → fixed in `session.rs`/`probe.rs`, new `CAP_KIND_*` consts); full harness info logs added (turn banner, gate A–D decisions, attach/exclude reasons, dispatch, terminal finalize); `sessions.updated_at` now only bumps on TurnCompleted (rename/pin/move/session-end/metadata no longer corrupt recency ordering).
- **Follow-up Fixes (2026-09-22):** terminal-tool turns now emit `LlmToken` IPC (missing assistant bubble root cause — tool path never streamed tokens); persistence worker retries retryable errors ×4 with event-named logs (write-write conflict vs title-tool's own conn was dropping TurnCompleted); SessionsChanged success log + frontend logs (`sessions_changed` receipt, header title update, `llm_token`/`commitTurn`); header typewriter path confirmed live (`ActiveSessionHeader` → `sessionTitle` → 28ms/char effect).
- **TTS Threads + Header + Pause Fixes (2026-09-22):** `tts.threads` removed from frontend `requiresRestart` gate (was blocking debounced auto-commit — model-card max-threads never persisted; `settings.json` stuck at 2 → Kokoro RTF 2.4–6.7); Kokoro init log now prints `threads=`; header early-return now logs `{currentActiveId, isMounted}`; `trim_and_fade_samples` appends 150ms inter-chunk silence floor (chunk sentence splits concatenated gaplessly → robotic no-pause speech).
- **Streaming TTS, Realtime Header Sync, Metrics & Import Hoisting (2026-09-22):** Implemented real-time audio chunk streaming in `kokoro.rs` and `chatterbox.rs` via progress callbacks pushing slices to `PlaybackEngine` during generation; lowered chunk 0 sentence split context to 2 words (`terminal_w_min = 2`) in `ClauseChunker` for discourse markers; pre-warmed BPE tokenizer in background at boot eliminating 50ms Turn 1 cold start; added `TurnMetricsCollector` tracking milestone latencies; fixed Home header title sync via `activeSessionId` hydration and 60ms commit delay; hoisted all inline module imports across backend Rust per style guide §2.1; all 5 Seam 21 integration tests and frontend build green.
- **All-Provider TTS Streaming & Header Project Sync (2026-09-22):** Converted `edge_tts.rs` to real-time `raw-24khz-16bit-mono-pcm` WebSocket streaming directly into `PlaybackEngine` (zero MP3 decoding overhead); aligned `supertonic.rs` streaming with atomic sample tracking; resolved Home header project name desync on session move by fetching projects on `sessions_changed` and syncing `activeSessionLabel` in `useSessionPanel`; confirmed Turn 1 dispatch overhead dropped to 1ms and Kokoro RTF improved to 0.748–0.878.
- **Relocated Metrics to Core & End-to-End Pipeline Telemetry (2026-09-22):** Moved `metrics.rs` to `core/metrics.rs`; wired `turn_metrics` into `PlaybackEngine` (recording exact `tts_first_audio` arrival on ingestion, fixing 0ms bug) and `TtsWorker` (tracking total jobs and last chunk synthesis); instrumented `ToolExecutor` (recording tool name, flow, and duration), `MemorySearchTool` (recording semantic search / retrieval latency), and `StreamRoutingStage` (recording token/char throughput); added two-phase hierarchical diagnostic reporting on playback start and playback completion.