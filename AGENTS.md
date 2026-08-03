# AGENTS.md — Vox Workspace Rules

---

## 1. Project Context

Vox is a **realtime voice AI desktop app** (Tauri v2 / Rust / TypeScript). Constraint: 8GB RAM, CPU-first inference, sub-200ms perceived pipeline latency.

**Crate structure:** Single Rust library crate `vox_lib` at `app/src-tauri/`. `main.rs` is 1 line. `lib.rs` is module declarations + Tauri assembly only. All logic lives in modules.

---

## 2. Workspace Directory Map

| Path | Purpose | Rules |
|---|---|---|
| `app/src-tauri/src/` | Production Rust source | No test logic. No benchmarks. |
| `app/src-tauri/tests/` | Integration tests (`cargo test --tests`) | Named `<feature>_test.rs`. Tests public API only. |
| `app/src-tauri/benches/` | Performance benchmarks (`cargo test --benches`) | Named `<feature>_bench.rs`. `harness = false` + custom `fn main()`. |
| `app/src-tauri/examples/` | Utility CLI tools (`cargo run --example <name>`) | Standalone tools. No `#[test]`. No assertions. |
| `.agents/rules/` | Role-specific agent instruction files | Read relevant file before acting in that role. |
| `docs/plans/` | Architecture specs and phase plans | Source of truth for specs. Do not contradict. |
| `docs/features/` | Implemented feature ledgers | Update after completing features. |
| `sandbox/` | Scratch space for experiments, evaluations, scripts | Non-production code. Results in `sandbox/results/`. Datasets in `sandbox/datasets/`. |
| `temp/` | Ephemeral runtime files: logs, raw LLM outputs | `temp/.env` (API keys). `temp/server.txt` (remote GPU server creds). Not versioned. |
| `submodules/` | Git submodules | `chatterbox-rs`, `query-sieve-rs`, `distilbert-query-classifier`, `vox-models`. Do not edit directly. |
| `~/.vox/models/` | Local model weights | Canonical manifest: `~/.vox/models/models_manifest.json`. |

**Remote GPU server:** `hypr4@100.86.62.14` (creds in `temp/server.txt`). Ollama + LMS available. **Never kill running server processes.**

---

## 2.1 Benchmark & Latency Execution Rules (MANDATORY)

1. **NEVER RUN BENCHMARK PROBES IN PARALLEL**:
   - Running multiple GGUF or ONNX inference commands concurrently causes CPU thread contention and invalidates per-pair latency metrics.
   - Always execute benchmark probes **strictly sequentially, one model at a time**.

---

## 3. HARD GATE: Code Modification Gate

> 🛑 **MANDATORY CONTEXT GATE:**
> - **WRITE TASK (Adding/editing code, refactoring, fixing bugs):** You MUST read `.agents/rules/code-style-guide.md` AND the relevant role rule file (e.g. `.agents/rules/backend-engineer.md` or `frontend-engineer.md`) BEFORE modifying code.
> - **READ-ONLY TASK (Auditing, answering questions, running tests/benchmarks, searching code):** DO NOT read code style files. Save context tokens.

---

## 4. Agent Roles

| Role | Rule File | Scope |
|---|---|---|
| System Architect | `.agents/rules/system-architect.md` | Strategy, gates, plan approval |
| Backend Engineer | `.agents/rules/backend-engineer.md` | `app/src-tauri/src/` implementation |
| Frontend Engineer | `.agents/rules/frontend-engineer.md` | `app/src/` implementation |
| QA Engineer | `.agents/rules/qa-engineer.md` | Test audit, benchmark validation |

**Subagent reuse rule:** Re-use existing subagent conversation IDs via `send_message`. Do not spawn duplicate subagents per turn.

---

---

## 5. Current Phase — Phase 10: Frontend Refactor & Restructuring

**Refactor Mindset & Execution Rules (MANDATORY FOR ALL AGENTS):**
1. **Subagent Delegation & Parallelization:** Utilize subagents for modular tasks (e.g., page-by-page auditing, service layer decoupling, component extraction). Parallelize non-interfering sub-tasks whenever possible.
2. **CLI-Driven & Instant Feedback:** Validate all edits via `pnpm lint` and `pnpm build` in terminal. Never declare a frontend step complete without clean CLI verification.
3. **Sticky Subagent Continuity:** Re-use persistent subagent conversation threads across multi-stage reviews to retain contextual history and avoid redundant audits.
4. **Incremental Layering & Gated Approval:** Focus on one layer at a time (e.g. Layer 1: Wiring data/services). Do NOT frontload detail for future layers. Execute subagent review gates after each layer, followed by HITL sign-off.

**Phase 10 Goal:** Eliminate bloat, tech debt, hardcoded text, and monolithic files in `app/src/`. Re-architect into clean, modular layouts, page-specific component subdirectories, centralized `src/services/`, and static `src/data/`.

**Implementation Progress:**
- **Pre-requisites**: `src/services/` and `src/data/` directories established.
- **Layer 1 (IPC Service Decoupling, Data Wiring & Custom Page Hooks)**: Completed ✅
  - All direct Tauri `invoke()` calls migrated to `src/services/`.
  - Static configs and metadata bound to `src/data/`.
  - Page-level event listeners, telemetry polling loops, and radial geometry consolidated into 1 custom hook per page (`useHomePage.ts`, `useSettingsPage.ts`, `useMonitoringMetrics.ts`).
  - Verified clean `pnpm build` (`tsc && vite build`) in 11.43s.
- **Phase 1 Settings Cards Refactoring**: Completed ✅
  - Extracted 6 shared UI & settings primitives (`ApiKeyField`, `VendorLogos`, `ProviderSelector`, `PipelineFlow`, `VoiceCarousel`, `ModelOptionRow`).
  - Refactored `ModelsCard.tsx` (2,349 ➔ 120 LOC), `InteractionCard.tsx` (1,336 ➔ 120 LOC), and `RealtimeCard.tsx` (863 ➔ 101 LOC).
  - Reduced total frontend LOC from **17,435 LOC ➔ 13,729 LOC** (**-3,706 lines removed!**).
  - Reduced Settings production JS bundle asset from **163.83 kB ➔ 70.64 kB** (**56.9% asset size reduction**).
  - Verified clean `pnpm build` (`tsc && vite build`) in 14.20s.
- **Subagent Review Gate**: Passed ✅