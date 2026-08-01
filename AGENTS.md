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

## 5. Current Phase — Phase 9: MemoryScope Classifier Fine-Tuning & Gate Validation

**Rules:** No self-audits. Sticky subagents. No shortcuts. Stop on blockers. HITL at phase gates. Update AGENTS.md after milestones.

**Phase 9 Spec:** 4-class MemoryScope (ChitChat/User/Domain/Temporal), tri-lingual ONNX classifier, τ*=0.81 threshold → Domain fallback, 98.08% non-default precision, 25.36ms P50 latency, <50MB RAM.

**Gates:** All 4 PASSED (Gate 1: dedup calibration, Gate 2: NLI precision, Gate 3: edge classifier, Gate 4: MemoryScope calibration). See `docs/benchmarks/`.

**Implementation Status:**
- **Initial review & tests**: Complete. 3/3 integration tests passing, 71/71 unit tests passing. **E2E pending**.
- **Layer 1 (Foundation)**: Schema v7, 4-class classifier τ*=0.81, 15% context cap. **PASSED**.
- **Layer 2 (Pipeline)**: Dedup(128)→Embed(16)→Eval(16, concurrent NLI+Edge)→Commit(32). Soft vector dedup (cos≥0.95) in Stage 2. 3-strike retry. **PASSED**.
- **Layer 3 (Retrieval)**: 4-class scope pruning. Directives vector-searched for Domain, SQL-fetched for Temporal. Narrative SQL for Temporal. Identity idempotent pre-load. 15% budget waterfall. **PASSED**.
- **Layer 4 (NLI & Edges)**: Identity/Directives CONTRADICTION→SUPERSEDES, ENTAILMENT→SUPPORTS. Constraints CONTRADICTION→CONFLICTS. Bidirectional edges. Cross-collection policy: Profile→Entities(SHAPES), Profile→Constraints(restricted_by), Entities→Constraints(DEPENDS_ON). **PASSED**.
- **Audit**: 7 legacy functions pruned. All tests passing.
- **Architecture doc**: `docs/features/memory-architecture.md` (code reality, not spec).