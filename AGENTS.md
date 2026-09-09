# AGENTS.md — Vox Workspace Rules

---

## 0. MANDATORY RULE: AGENTS.md Sync Hook

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

## 1. Project Context

Vox is a **realtime voice AI desktop app** (Tauri v2 / Rust / TypeScript). Constraint: 8GB RAM, CPU-first inference, sub-200ms perceived pipeline latency.

**Crate structure:** Single Rust library crate `vox_lib` at `app/src-tauri/`. `main.rs` is 1 line. `lib.rs` is module declarations + Tauri assembly only. All logic lives in modules.

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
3. **Isolated Test Runner (`cargo-nextest`):** Always use `cargo-nextest run` with explicit thread pool allocation and single-thread isolation:
   ```bash
   RAYON_NUM_THREADS=$(nproc) OMP_NUM_THREADS=$(nproc) cargo nextest run --release --test-threads=1
   ```
   _Single test:_ `cargo nextest run --test <test_file> --release --nocapture --test-threads=1`
   > ⏱️ **Full Suite Baseline Run Time:** ~34.5s test execution (~35s wall-clock with compilation cache hit; ~1m55s cold compile + run) across all 87 tests in 17 binaries (52 unit tests, 35 integration tests across Seams 1–11, 15–17, plus notifications CRUD).
4. **External API Keys (`#[ignore]`):** Cloud provider tests (Nvidia, Gemini Live, Deepgram, OpenAI, ElevenLabs) must be marked `#[ignore]` and run manually only with explicit user approval: `cargo nextest -- --ignored`.

---

## 4. Invariants ,Rules and Specs [MUST FOLLOW]

### 4.1 Critical Architectural & Logical Invariants (Non-Negotiable Concepts)

0. **Zero Backward Compatibility (ZBC):** Backward compatibility is not a requirement unless explicitly stated. Break, replace, or redesign existing interfaces when that produces the better architecture. Never introduce compatibility layers, legacy paths, or transitional abstractions proactively..
1. **Registry-Owned Event Contracts:** `core/events.rs` is the SSOT for all cross-boundary events. Internal pipeline events belong to `VoxEvent`; IPC events belong to `IpcEvent` with strongly typed payloads. Raw string event literals are forbidden at emit and listen sites; frontend mirrors the registry via `IpcEventMap`.
2. **Sacred Audio Hot Path:** Zero dynamic memory allocations, zero lock acquisitions (`Mutex`/`RwLock`), and zero blocking I/O on the CPAL audio thread and VAD inference loop. Ring buffers must be lock-free and pre-allocated.
3. **Actor-Engine Separation & Thread Isolation:** CPU/GPU-heavy model inference (STT,  VAD, LLM, TTS etc.) runs strictly on dedicated background OS threads (`std::thread`). Tokio runtime is reserved strictly for async I/O, IPC routing, and network WebSockets.
4. **Strict Frontend Service Boundary:** React components and hooks must never directly call `@tauri-apps/api/core` (`invoke`) or `@tauri-apps/api/event` (`listen`). All backend interactions must route through strongly-typed singleton service modules in `src/services/`.
5. **React 19 Context Memoization & Selector Discipline:** Provider values must be wrapped in `useMemo`. Zustand store state must be queried via fine-grained atomic selectors (`(s) => s.field`) rather than consuming entire store snapshots, preventing cascading render loops.
6. **Centralized Monotonic Turn Lifecycle:** Monotonic turn IDs must be generated exclusively at turn boundaries via `PipelineAtomics::next_turn()`, which atomically advances the turn ID, renews the `CancellationToken`, and returns `(turn_id, token)` as a bundle. Turn IDs must never be reset to 0, fragmented across parallel actors, or fabricated with dummy values. Subsystems receive `(turn_id, token)` at the turn boundary — they never own or directly advance the underlying `AtomicU32`.
7. **Single-Consumer Audio Stream Invariant:** Audio ring buffers and input channels must have exactly one consumer (`VadActor`). Never attach secondary or ad-hoc readers to production audio streams.
8. **No Inline Crate Qualifiers (Strict Import Hoisting):** Inline `crate::` path qualifiers in function bodies, structs, or signatures are strictly prohibited. All imports must be cleanly grouped and hoisted at the top of the file according to `backend-style-guide.md §2.1`.

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
1. **[Event-Domain Architectural Specification (Ground Truth)](file:///home/addy/projects/apps/vox/docs/specs/event-domain-matrix.md)** — *Status: Approved SSOT*. Golden source of truth for pipeline behavior, state transitions, and 6-domain contracts.
2. **[Database & Persistence Specification (v2)](file:///home/addy/projects/apps/vox/docs/specs/db-spec.md)** — *Status: Approved Target Spec*. Strict persistence boundary, native Turso engine invariants, and normalized v2 schema.
3. **[Minimal Cognitive Memory & Session Continuation Spec (v2)](file:///home/addy/projects/apps/vox/docs/specs/memory-spec.md)** — *Status: Approved Target Spec*. 2-stage dedup, single evolving personal memory document, rolling working compaction, and deferred episodic tool retrieval.
4. **[IPC Command & Event Specification (v2)](file:///home/addy/projects/apps/vox/docs/specs/ipc-spec.md)** — *Status: Proposed / Target Spec*. Grouped frontend-to-backend commands and backend-to-frontend IPC events.
5. **[LLM Agent Harness & Dual-Stream Demuxer Spec (v2)](file:///home/addy/projects/apps/vox/docs/specs/harness-spec.md)** — *Status: DRAFT / Under Active Architectural Discussion*. Dynamic streaming tag demuxing, `InteractionState::Working`, and decoupled LLM actor.

---

## 5. Phase 11 Test Suite & Verification Ledger

> 📖 **Full History: [recent_work.md](file:///home/addy/projects/apps/vox/docs/plans/phase11/recent_work.md)** | Phase 10 Archive: [phase10/recent_work.md](file:///home/addy/projects/apps/vox/docs/plans/phase10/recent_work.md)

- **Integration & Seam Verification:** Seams 1–11, 15–17 green & mutate-verified; isolated test DB fixtures; single-turn latency optimized (NVIDIA NIM 2.59s vs Local Qwen 8.67s); 100% release test suite clean.
- **Frontend IPC & UI Decomposition:** Replaced fragile IPC contracts with canonical session IDs; modularized `Settings.tsx`, `RealtimeCard.tsx`, and `ModelsCard.tsx`; stabilized Three.js memory graph.
- **Minimal Memory & Pure-Rust Turso Persistence (Batches 1–5):** Rebuilt clean v2 DDL schema for 10 tables; purged 14 legacy graph/NLI files; implemented non-blocking persistence facade, 2-stage dedup engine (Jaccard + cosine), batched ONNX tensor embeddings, unified compaction coordinator, and session continuation.
- **Architect Review Remediation & Clippy Zero-Tolerance:** Wired background quiet ingestion observer (30s) and quiet debounced soft compaction (20s); eliminated ad-hoc DB open calls across actors and IPC; resolved `MutexGuard` across await points in session continuation; verified zero warnings on `cargo clippy --all-targets`.
- **Architect Review Items 7 &amp; 8 Closed:** Purged remaining `VoxDb::open`/`open_readonly` in `transcript.rs`, `error.rs`, `compaction/mod.rs` — replaced with `Arc::clone(&state.db)`; added `deactivate_facts_batch` to `persistence/facts.rs` and wired Stage 1 dedup to batch-deactivate duplicates in a single `BEGIN IMMEDIATE` transaction, eliminating N+1 write overhead. Clippy `-- -D warnings` clean.
- **Batch 6 Frontend Service Layer Complete:** Updated `historyService.ts` (v2 `SessionRow`/`TurnRow` shape, `createSession`, `continueSession`, `getSessions`, `getTurns`, `deleteSession`, `getTranscriptHistory`); `memoryService.ts` (5 personal memory commands + new `getActiveFacts`/`FactRecord`); `eventsService.ts` (pruned dead `NotificationDismissed`, `NotificationsMarkedRead`, `SessionTitleUpdated` events; added `PersonalMemoryUpdated`, `SessionsChanged`); `notificationService.ts`, `projectsService.ts`. Fixed 71 → 0 TypeScript errors across 16 consuming files (`useHistory`, `VoiceSessionContext`, `TrayApp`, `notificationStore`, history components).
- **Memory Page Full Redesign:** SVG-based spatial graph — central gold sphere (PersonalMemory, click → bottom drawer), Ring 1 identity facts (cyan), Ring 2 session clusters (5 fact types, distinct colors). Added `get_active_facts` IPC command + `fetch_all_active_facts` persistence query; spec updated per §4.3. Deprecated `MemoryPipelineDrawer` with `@deprecated` JSDoc + `@ts-nocheck`; 7 legacy graph components tombstoned without deletion. `pnpm tsc --noEmit` → 0 errors.
- **IPC Drift Sprint (4 fixes + spec):** Fixed `triggerSessionCompaction` arg key (`sessionId`); implemented `get_active_facts` project scoping (JOIN via sessions); refactored `ipc/voices.rs` (5 commands) + persistence worker onto shared `Arc<Connection>` (bootstrap now open→migrate→spawn); mapped `delete_project` guard failures to `InvalidArgument`/`NotFound`; `update_session` all-`None` no-op guard; `ipc-spec.md` corrected (`SessionsChanged` void, camelCase contract, `SessionRow` extras, full `PersonalMemoryRecord`). Repaired 2 unit-test FK setups (`facts` lifecycle, crash reconciliation). `cargo check`/`clippy -D warnings`/`tsc` clean, 57/57 lib unit green. Integration suite red on pre-existing harness `block_on`-in-runtime (`tests/common/harness.rs:295`) — awaits harness design decision.
- **Harness Async Fix & Test-Setup Repairs:** Made `get_test_app_and_state`/`get_test_app_state` async (direct `await VoxDb::open`, plus `get_test_app_and_state_sync` wrapper for 2 sync VAD-ducking tests); updated 21 call sites. Repaired 2 more FK setups (`crash_reconciliation`, `notifications` lifecycle). Suite now 86/93 with `cargo check`/`clippy -D warnings` clean. Remaining red is pre-existing: `compaction_ledger` test targets pre-v2 schema/API (`started_at` column) and 7 model tests SIGSEGV on first ONNX load (env/infra, untouched paths).
- **Ledger v2 Rework & ONNX RCA:** Reworked `compaction_ledger` test to v2 (`create_session_with_id`, `manual` trigger kind) — notifications binary 2/2 green. gdb localized the 7 SIGSEGVs to ORT 1.24.2's `ReshapeFusion` optimizer crashing on the MiniLM INT8 model during first session creation; same file loads clean under ORT 1.27 (Python). Verdict: library-version defect, not Vox code — needs dep/architecture decision (bump `ort` vs lower graph-opt level).
- **ORT Bump (rc.12 → rc.13, ORT 1.28):** Root-caused SIGSEGVs to ORT 1.24.2 `ReshapeFusion`; bumped `ort` to `2.0.0-rc.13` (zero API breakage, lock shows only `ort`/`ort-sys` moved). Full release suite **93/93 green**, `clippy -D warnings` clean.
- **Consolidation Scheduler Rework (per user review):** Deleted unapproved `memory_compaction_test.rs`; degraded-compaction event now emitted inside `prepare_turn_context` via new `pipeline_tx` param (2-tuple return; transcript is passthrough); scheduler sleeps until next `consolidation_time` via `duration_until_next` (no polling, DST-safe, 24h fallback); `session_close` cadence removed everywhere (mutation validates `manual|daily` only); deferred daily runs retry at next scheduled time; notification enum skipped (user-owned). `clippy -D warnings` clean, `tts_transition_test` 2/2 green, spec §5.4 updated.

