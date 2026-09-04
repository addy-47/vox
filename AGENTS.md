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
| `app/src-tauri/tests/`    | Integration tests (`cargo test --tests`)            | Named `<feature>_test.rs`. Tests public API only.                                                     |
| `app/src-tauri/benches/`  | Performance benchmarks (`cargo test --benches`)     | Named `<feature>_bench.rs`. `harness = false` + custom `fn main()`.                                   |
| `app/src-tauri/examples/` | Utility CLI tools (`cargo run --example <name>`)    | Standalone tools. No `#[test]`. No assertions.                                                        |
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
4. **External API Keys (`#[ignore]`):** Cloud provider tests (Nvidia, Gemini Live, Deepgram, OpenAI, ElevenLabs) must be marked `#[ignore]` and run manually only with explicit user approval: `cargo nextest -- --ignored`.

---

## 4. Invariants and Workflow Gates

### 4.1 Critical Architectural & Logical Invariants (Non-Negotiable Concepts)

0. **Zero Backward Compatibility (ZBC):** Backward compatibility is not a requirement unless explicitly stated. Break, replace, or redesign existing interfaces when that produces the better architecture. Never introduce compatibility layers, legacy paths, or transitional abstractions proactively..
1. **Single Source of Truth for State:** `InteractionState` (`Idle, Ready, Listening, Thinking, Speaking, Paused, Error`) and `DictationState` (`Idle, Recording, Transcribing, Error`) are the sole sources of truth. Synthetic lifecycle booleans (`is_engaged`, `is_recording`, `is_connected`, `is_speaking`, `is_sleeping` are strictly banned across Rust and TypeScript.
2. **Registry-Owned Event Contracts:** `core/events.rs` is the SSOT for all cross-boundary events. Internal pipeline events belong to `VoxEvent`; IPC events belong to `IpcEvent` with strongly typed payloads. Raw string event literals are forbidden at emit and listen sites; frontend mirrors the registry via `IpcEventMap`.
3. **Sacred Audio Hot Path:** Zero dynamic memory allocations, zero lock acquisitions (`Mutex`/`RwLock`), and zero blocking I/O on the CPAL audio thread and VAD inference loop. Ring buffers must be lock-free and pre-allocated.
4. **Actor-Engine Separation & Thread Isolation:** CPU/GPU-heavy model inference (Whisper STT, ONNX VAD, Llama LLM, Chatterbox TTS) runs strictly on dedicated background OS threads (`std::thread`). Tokio runtime is reserved strictly for async I/O, IPC routing, and network WebSockets.
5. **Strict Frontend Service Boundary:** React components and hooks must never directly call `@tauri-apps/api/core` (`invoke`) or `@tauri-apps/api/event` (`listen`). All backend interactions must route through strongly-typed singleton service modules in `src/services/`.
6. **React 19 Context Memoization & Selector Discipline:** Provider values must be wrapped in `useMemo`. Zustand store state must be queried via fine-grained atomic selectors (`(s) => s.field`) rather than consuming entire store snapshots, preventing cascading render loops.
7. **Centralized Monotonic Turn Lifecycle:** Monotonic turn IDs must be generated exclusively at turn boundaries via `PipelineAtomics::next_turn()`, which atomically advances the turn ID, renews the `CancellationToken`, and returns `(turn_id, token)` as a bundle. Turn IDs must never be reset to 0, fragmented across parallel actors, or fabricated with dummy values. Subsystems receive `(turn_id, token)` at the turn boundary — they never own or directly advance the underlying `AtomicU32`.
8. **Single-Consumer Audio Stream Invariant:** Audio ring buffers and input channels must have exactly one consumer (`VadActor`). Never attach secondary or ad-hoc readers to production audio streams.

### 4.2. HARD GATE: Code Modification Gate

> 🛑 **MANDATORY CONTEXT GATE:**
>
> - **WRITE TASK (Backend Rust):** You MUST read `.agents/rules/backend-style-guide.md` AND `.agents/rules/backend-engineer.md` BEFORE modifying Rust backend code.
> - **WRITE TASK (Frontend React/TS):** You MUST read `.agents/rules/frontend-style-guide.md` AND `.agents/rules/frontend-engineer.md` BEFORE modifying frontend code.
> - **WRITE TASK (Tests/Benches/Evals):** You MUST read `.agents/rules/testing-style-guide.md` AND `.agents/rules/test-engineer.md` BEFORE authoring tests or benchmarks.
> - **READ-ONLY TASK (Auditing, answering questions, running tests/benchmarks, searching code):** DO NOT read code style files. Save context tokens.

---

### 4.3 Agent Roles

| Role                 | Rule File                               | Scope                                                            |
| -------------------- | --------------------------------------- | ---------------------------------------------------------------- |
| System Architect     | `.agents/rules/system-architect.md`     | Strategy, gates, plan approval                                   |
| Backend Engineer     | `.agents/rules/backend-engineer.md`     | `app/src-tauri/src/` implementation                              |
| Frontend Engineer    | `.agents/rules/frontend-engineer.md`    | `app/src/` implementation                                        |
| QA Engineer          | `.agents/rules/qa-engineer.md`          | Test audit, benchmark validation                                 |
| ML Research Engineer | `.agents/rules/ml-research-engineer.md` | ML model research, evaluation, and fine-tuning dataset curation  |
| Test Engineer        | `.agents/rules/test-engineer.md`        | Test case design, benchmark validation, and performance analysis |

---

## 5. Phase 11 Test Suite & Verification Ledger

> 📖 **Full History: [recent_work.md](file:///home/addy/projects/apps/vox/docs/plans/phase11/recent_work.md)** | Phase 10 Archive: [phase10/recent_work.md](file:///home/addy/projects/apps/vox/docs/plans/phase10/recent_work.md)

- **Phase 10 → 11: pipeline refactor to test-suite engineering:** 4-domain refactor, realtime driver decomposition, hot-path zero-alloc pools, `services/audio/` separation (zero clippy warnings); Phase 11 scope is UT/IT/benchmarks/evals, with Model Hub v1.6.0 + `ProviderCaps`, Memory Compaction (Schema v2, `CompactionCoordinator`, Bell cards), and IPC consolidation landed.
- **Backend dictation pipeline + hardening:** unified `InteractionState` (+`Sleeping=7`), Option-C `ingestion_gate` 2-layer defense, `dictation/{mod,ptt,speech,transcript,error}` split, event-driven hotkey via `event_tx`, owner handover + VAD sync + CPAL gate, `try_lock()` discipline, always-emit STT transcripts; `cargo check`/`clippy` clean.
- **Frontend wiring + state authority:** 11 IPC arg-case fixes, hook-discipline/event-contract/selector sprints, backend-driven `InteractionState` (tray cleared on disengage, `pttStatus` eradicated), notification alignment, HelpDrawer; audits in `voice_wiring_audit.md`, `frontend_review.md`, app-state/telemetry/atomics report; `tsc`/`pnpm build` green.
- **Test-suite specs (no code changed):** IT spec v2.2 — all 17 seams defined with 17 false-green tables (dictation seam rewritten for post-refactor truth); session-continuation spec draft; `tests/common/` harness contract (TO-BUILD).