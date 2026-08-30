# AGENTS.md — Vox Workspace Rules

---

## 0. MANDATORY RULE: Automatic Documentation & AGENTS.md Sync Hook

> 🛑 **MANDATORY POST-TASK DOCUMENTATION HOOK (NON-NEGOTIABLE):**
> 1. You **MUST** automatically update `AGENTS.md` (specifically Section 5) directly with all recent work, milestones, implementation changes, model configurations, and threshold updates.
>    - **Line-Count Threshold (175 Lines Max):** All recent work is recorded in `AGENTS.md` first. When `AGENTS.md` approaches or exceeds the 175-line ceiling, you MUST compact Section 5 into high-level critical milestones, move the uncompacted detailed history into `docs/plans/<current_phase>/recent_work.md` (e.g. [`recent_work.md`](file:///home/addy/projects/apps/vox/docs/plans/<current_phase>/recent_work.md)), and keep a deep link to it.
> 2. You **MUST** automatically update any relevant feature, component, design, or architecture documentation in docs/ to match the actual code state, key files include `backend.md`, `models.md`, `frontend.md` and `docs/features/*`.
> 3. This is a **mandatory post-task completion hook** — do NOT wait for the user to explicitly remind you to sync documentation.

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

## 2.1 Execution & Testing Invariants (All Agents)

1. **Sequential Execution:** Run performance-sensitive tasks (benchmarks, evals, test suites) strictly one at a time to prevent CPU, memory, and I/O contention.
2. **Release / Optimized Mode:** Always run performance measurements and benchmarks under release mode (`--release`). Debug builds produce invalid metrics.
3. **Isolated Test Runner (`cargo-nextest`):** Always use `cargo-nextest run` with explicit thread pool allocation and single-thread isolation:
   ```bash
   RAYON_NUM_THREADS=$(nproc) OMP_NUM_THREADS=$(nproc) cargo nextest run --release --test-threads=1
   ```
   _Single test:_ `cargo nextest run --test <test_file> --release --nocapture --test-threads=1`  
   _Timeouts:_ 60s per individual test, 90s full suite (baseline runtime is ~28.8s).
4. **External API Keys (`#[ignore]`):** Cloud provider tests (Nvidia, Gemini Live, Deepgram, OpenAI, ElevenLabs) must be marked `#[ignore]` and run manually only with explicit user approval: `cargo nextest -- --ignored`.

---

## 3. HARD GATE: Code Modification Gate

> 🛑 **MANDATORY CONTEXT GATE:**
>
> - **WRITE TASK (Backend Rust):** You MUST read `.agents/rules/backend-style-guide.md` AND `.agents/rules/backend-engineer.md` BEFORE modifying Rust backend code.
> - **WRITE TASK (Frontend React/TS):** You MUST read `.agents/rules/frontend-style-guide.md` AND `.agents/rules/frontend-engineer.md` BEFORE modifying frontend code.
> - **WRITE TASK (Tests/Benches/Evals):** You MUST read `.agents/rules/testing-style-guide.md` AND `.agents/rules/test-engineer.md` BEFORE authoring tests or benchmarks.
> - **READ-ONLY TASK (Auditing, answering questions, running tests/benchmarks, searching code):** DO NOT read code style files. Save context tokens.

---

## 4. Agent Roles

| Role                 | Rule File                               | Scope                                                            |
| -------------------- | --------------------------------------- | ---------------------------------------------------------------- |
| System Architect     | `.agents/rules/system-architect.md`     | Strategy, gates, plan approval                                   |
| Backend Engineer     | `.agents/rules/backend-engineer.md`     | `app/src-tauri/src/` implementation                              |
| Frontend Engineer    | `.agents/rules/frontend-engineer.md`    | `app/src/` implementation                                        |
| QA Engineer          | `.agents/rules/qa-engineer.md`          | Test audit, benchmark validation                                 |
| ML Research Engineer | `.agents/rules/ml-research-engineer.md` | ML model research, evaluation, and fine-tuning dataset curation  |
| Test Engineer        | `.agents/rules/test-engineer.md`        | Test case design, benchmark validation, and performance analysis |

---

## 5. Phase 10 Architecture & Orchestration Refactor Ledger

> 📖 **Full History & Sprint Details:** See [`docs/plans/phase10/recent_work.md`](file:///home/addy/projects/apps/vox/docs/plans/phase10/recent_work.md) for full chronological storyline and sprint breakdowns.

- **State & Flag Purge:** Purged synthetic flags (`is_sleeping`, `is_engaged`, `is_recording`, etc.); switched to SSOT state enum queries.
- **Monotonic Turn ID & Tokio standard:** Centralized turn IDs in `PipelineAtomics`; unified cancellation tokens with `tokio_util::sync::CancellationToken`.
- **Test Suite Realignment (S01):** Production capture seam integration across all test suites (`cargo nextest run --release` 40/40 green).
- **Concurrency & Dead Code Purge (S02-S05):** Removed dead `audio_sink` and unread atomics; promoted PTT workers to async locks; wired realtime activity signals.
- **Lifecycle & TurnAccumulator (S07-S11):** Idempotent transitions, barge-in memory cleanup, and domain-scoped `TurnAccumulator` structs.
- **Subsystem Promotion & Path Alignment:** Promoted `services/pipeline/` $\to$ `src/pipeline/` (`vox_lib::pipeline::*`) and `services/memory/harness/` $\to$ `services/harness/` (`vox_lib::services::harness::*`), with all integration tests and benchmarks aligned.
- **SSOT Model Residency & Dead State Audit:** Eliminated 9 redundant `is_*_loaded` AtomicBools across `AppState` and actor handles; derived residency directly from engine channels & static locks in `monitoring::collector`; verified all state atomics across backend and frontend.
- **Realtime Architecture & Lifecycle Refactor:** Decoupled session boot/teardown into IPC SSOT orchestrators; unified 7-min idle monitor (`REALTIME_IDLE_TIMEOUT=420s`) across all assistant pipelines in `pipeline/mod.rs`; enforced 2-hr session resumption cache validation and graceful eviction; wired `PlaybackStarted` on first audio chunk; ensured guaranteed turn persistence on barge-in across modular and realtime modes with zero context-compaction leakage.
- **Realtime Passive Review & Hardening:** Fixed audio passthrough leak on `pause_session` by dispatching `VadCommand::StopRealtime`; wired Hindi/Hinglish `transliterate_if_hi` on partial/final realtime transcripts before UI emit; resolved barge-in turn ID desync to persist interrupted turns under their actual turn ID.
- **ConversationManager & ContextHarness Decoupling:** Cleanly separated pure dialog turn & prompt state (`ConversationManager`) from modular LLM token budgeting, sliding-window FIFO, and compaction state (`ContextHarness`), ensuring Realtime S2S domains have zero coupling or overhead to context compaction machinery. Added `#![recursion_limit = "256"]` to `lib.rs` for deep async Tauri handler state machines.
- **End-to-End Trace Alignment across 4 Voice Domains:** Reconstructed runtime flow artifacts across all four pipeline domains (Modular Passive, Modular PTT, Realtime Passive, Realtime PTT) using the strict `trace` skill discipline, grounding every lifecycle step with verified code citations, actor boundary directions, triggers, and subsystem owners.
