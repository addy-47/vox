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

- **Phase 12 Foundations & Tool Taxonomy (2026-09-21):** Defined `ToolFlow::Terminal` / `NonTerminal` contracts across all specs; implemented Batches 0–7 (canonical types, DB v5 `session_tool_calls`, Invariant 14 synthesis guard latch, tool runtime stage, session capability probe, provider stream normalization).
- **Audit Review & Remediation (2026-09-22):** Resolved all 15 audit findings + 6 residual gaps in Batch 8 (pass-level clause buffering with drop-all-prefix, Invariant 13 barge-in persistence, session-local title gate, loop budget tracking & 5-pass bound, non-blocking capability probe, hybrid RRF episodic retrieval).
- **Flat Harness Subsystem (2026-09-22):** Decomposed and flattened harness into `services/harness/` (`chassis.rs`, `loop.rs`, `steps.rs`, `mod.rs`); unified sequential step execution (Step 1–7), consolidated `enter_non_terminal_phase` with speech normalization, and updated `harness-spec.md` §9.1 and §9.2.
- **Modular Prompt & Tool Directives (2026-09-22):** Injected explicit agentic instructions into `DEFAULT_SYSTEM_PROMPT_MODULAR` for `respond_and_set_title` (session naming) and `search_memory` (episodic retrieval) (0 clippy warnings).
- **Phase 12 Target Integration Test Spec Authored (2026-09-22):** Defined testing boundaries across Seams 6, 8, 9, 11, 20 and established NEW Seam 21 (`agentic_tool_runtime_test.rs`) using `/create-test` standard; integrated directly into canonical `docs/specs/integration-test-spec.md`.
- **Phase 12 Integration Suite Execution & Verification (2026-09-22):** Authored and executed Seams 6, 8, 9, 11, 20 and NEW Seam 21 (`agentic_tool_runtime_test.rs`); resolved runtime `TOKIO_HANDLE` router thread propagation, mock engine reattachment after `EndSession`, canonical barge-in accumulator clearing (`events-spec §5 item 5`), and test stream handle wiring; verified all 122 integration tests passing (122 passed, 4 skipped in 48.94s); updated `docs/tests/integration_test_report.md`.
- **Agentic Tool & Audio Egress Evaluation Harness (2026-09-22):** Authored `evals/agentic_tool_eval.rs` and judge asset `evals/assets/prompts/judge_tool_eval.md`; wired directly to production `Harness::execute_turn` reentrant loop, capturing Turso SQLite `session_tool_calls`, intermediate stream tokens, and real TTS playback into `.wav` clips with markdown judge reports in `evals/results/agentic_tool_eval/<run_id>/`.
- **Phase 12 Mutation Testing Campaign (2026-09-22):** Executed 15 systematic mutations across `steps.rs`, `stages/tools/`, `interrupt.rs`, `session.rs`, and persistence worker against 4 integration suites (`agentic_tool_runtime_test`, `session_lifecycle_test`, `playback_interrupt_test`, `database_persistence_boundary_test`); confirmed 11 killed mutants (73.3%) and isolated 4 surviving mutants (tool flow validation, barge-in token cancel assertion, negative title validation, Invariant 13 partial-text persistence); verified all 23 integration tests passing; documented findings in `docs/tests/mutation_report.md`.
- **Provider-Model Catalog Spec v2 + Adapter Review (2026-09-22):** Renamed `model-capability-catalog-spec.md` to `provider-model-catalog-spec.md` via `git mv` (now owns provider wire contracts + model facts, banning URL-sniff dispatch and dual reasoning emission); audited full `services/llm/` ad-hoc branches and authored `docs/plans/phase12/llm_adapter_review_and_plan.md` (P0–P5 data-driven cleanup plan, testing/eval still paused).
- **Adapter Sourcing Correction (2026-09-22):** Answered community-catalog question with evidence: model facts already sync from `models.dev` (`catalog/sync.rs:9`); cloud wire params vendor from `modelparams.dev` JSON (verified live `nvidia/nemotron-3-super-120b-a12b` entry proves `reasoning_effort`+`max_tokens`, killing dual emission on NIM too); local rows come from official Ollama OpenAPI docs only; no Rust crate used (all surveyed crates hardcode mappings in code). Pinned sourcing rule into spec §2 + plan P0 (`source`+`source_checked` per row).
- **Provider JSON SSOT Implemented — P0 Done (2026-09-22):** Extended `baseline_providers.json` to 14 sourced rows (new `ollama_openai_compat` preset) with full wire policy (`reasoning_wire`, `tool_choice_policy`, `stream_options_policy`, `response_format_wire`, `tool_stream_shape`, `passthrough_top_k`); mirrored in `ProviderPresetMeta` + 5 new enums; `TransportType::OllamaNative` with explicit-match dispatch/health/list (URL-sniff deleted); 2 SSOT unit tests green, `cargo check` + `clippy --lib` clean; drive-by compile repairs in `ollama.rs` stream loop + `chat_completions.rs` import.
- **Provider SSOT Rework — Enums Deleted, Vendors In (2026-09-22):** Replaced 5 policy enums with plain mapping rows (`token_limit` path, `reasoning_off{path,value}|null`, `tool_choice|nil`, `stream_usage`, `response_envelope`, `tool_stream`, `top_k_field|nil`); vendored `modelparams_vendor.json` (7 providers, 188 models) with build-failing cross-check test (caught groq `max_completion_tokens` + google native-vs-compat surface); serializers in `chat_completions.rs`/`ollama.rs` now read resolved `cfg.policy` (dual emission, unconditional `tool_choice`/`stream_options`/bare `top_k` gone); `ConnectionConfig` carries resolved policy; 58/58 lib tests green, clippy clean.
- **llm/ Defragmentation (2026-09-22):** Deleted `TokenLimitField` (400-flip wrote a field serializers no longer read; flip now toggles `policy.token_limit` path) and `CapabilitySource` (duplicated transport name; probe now matches on `transport`); stripped 14 `capability_source` keys from manifest; `modelparams_vendor.json` confirmed test-only fixture (runtime reads manifest; vendor exists so the cross-check test fails the build on drift); 58/58 lib tests green, clippy clean.
- **Seam 21 Wire Tests Fixed (2026-09-22):** Added 2 wire subtests to `agentic_tool_runtime_test.rs` driving real `RemoteTransport` against a std-only mock HTTP server (nvidia request bytes + chunked-SSE ToolCall assembly; ollama `think:false`/`num_predict` + NDJSON ToolCall); 5/5 green `--release`, red-proofed via wrong-value mutant; P5 partially done (live `#[ignore]` tests + eval rerun still open).