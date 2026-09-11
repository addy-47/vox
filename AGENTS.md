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

---

## 5. Phase 11 Test Suite & Verification Ledger

> 📖 **Full History: [recent_work.md](file:///home/addy/projects/apps/vox/docs/plans/phase11/recent_work.md)** | Phase 10 Archive: [phase10/recent_work.md](file:///home/addy/projects/apps/vox/docs/plans/phase10/recent_work.md)

- **Suite green:** Full release suite 93/93 (post `ort` rc.13 bump fixing ReshapeFusion SIGSEGVs); `clippy -D warnings` clean; Seams 1–11, 15–17 mutate-verified.
- **Memory v2 complete:** 10-table Turso schema, 2-stage dedup, compaction coordinator + ledger, session continuation, IPC drift fixed, sleep-until consolidation scheduler (`manual|daily` only).
- **Edge panels & shell unified:** Full-vh docked side panels with framer-motion slide, persistent IDE layout, null safety, and top-right clusters across desktop pages.
- **Session lifecycle & prompt resolution:** Centralized prompt resolution in `AppState`, lazy session persistence without echo events, and clean voice pipeline teardown on reset.
- **Backend architecture & dead code cleanup:** Modularized `ipc/settings` and `pipeline/mod.rs`, wired dynamic TTS speed/quality-steps hot-reloading, fixed opportunistic compaction commit, and pruned dead code and legacy Memory v1 XML artifacts.
- **Harness v2 architecture spec approved:** Re-architected `services/harness/` into plugin runtime chassis with 1:1 session lifecycle, `start_session(Option<sessionId>)`, duplex actor pipe, and `Working` state; aligned `events-spec.md` and `ipc-spec.md`.
- **Harness v2 Batches 1 & 2 verified:** AudioIntent contracts & gating in `core/events.rs`, `InteractionState::Working=8`, LLM actor decoupled with `LlmResponse` duplex pipe, `StreamRoutingPlugin` & `SynthesisContext` bundled handles, zero warnings on `clippy -D warnings` and `pnpm build`.
- **Harness v2 Chassis & Clean Deletion complete:** Legacy files deleted clean-slate (`accountant`, `buffer`, `facade`, `manager`, `prompt_builder`); 5-plugin runtime assembled (`history`, `prompt`, `budget`, `compaction`, `stream`) in `HarnessSession`; reactive 20s quiet watcher implemented; strict import hoisting enforced; `clippy --release --lib` and `pnpm build` clean.
- **Harness v2 Batch 6 Hardening complete:** Encapsulated compaction execution & Turso DB fact staging inside `CompactionPlugin`; embedded `QuietCompactionWatcher` into session lifecycle with barge-in cancellation in `speech.rs`, `ptt.rs`, `playback.rs`, and `interrupt.rs`; fixed `PromptBuilderPlugin` memory ceiling allocation; verified with 0 warnings on `cargo clippy --release --all-targets -- -D warnings` and `pnpm build`.
- **Harness v2 Review & Architectural Audit complete:** Formatted `HARNESS_REVIEW_REPORT.md` in root; identified 2 critical defects (UTF-8 slice panic, unpruned soft compaction history), mapped 11 upstream consumers, verified style guide and invariant compliance; `cargo check --release` clean.
- **Harness v2 Spec Aligned (Invariants 6 & 7):** Surgically updated `docs/specs/harness-spec.md` with strict Two-Door Chassis Encapsulation and Autonomous Reactive Quiet Watcher & History Pruning invariants.
- **Harness v2 Batch 7 complete:** Fixed `context_summary` → `session_context` rename across `coordinator.rs`, `watcher.rs`, `transcript.rs`; restored `stream: StreamRoutingPlugin` to `HarnessSession` with `route_stream()` as the single Two-Door gateway; `clone_stream_plugin()` allows `spawn_blocking` without holding the lock; added `history_mut()` test seam; fixed all 4 integration tests (`session_lifecycle`, `tts_transition`, `transcript_to_llm`, `llm_to_tts`) to use public accessors instead of private fields; `cargo check --tests` and `clippy --release --lib -D warnings` both clean.
