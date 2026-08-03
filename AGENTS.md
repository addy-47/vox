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

## 5. Current Phase — Phase 11: Memory Pipeline Stage-by-Stage Evaluation & QA

**Global Evaluation Configuration:**
- **Context Window Cap:** Set to **8192 tokens** across all stage evaluation runs (`eval_compaction.rs`, `eval_pipeline.rs`, `eval_retrieval.rs`).

---

### Phase 11 QA Evaluation Focus (Semantic Quality & System Behavior)

Deterministic checks (e.g. table creation, schema existence, non-null values, non-crashing execution) belong in automated unit tests. **QA Evaluation focuses strictly on semantic validity, information coverage, false positive/negative detection, edge correctness, and latency dynamics.**

#### 1. Eval 1 — LLM Compaction & Fact Extraction (`eval_compaction.rs`)
- **Information Coverage:** Do extracted facts capture all critical user information across conversation turns, or was vital context silently dropped?
- **Redundancy & Over-Extraction:** Is there semantic redundancy or over-extraction across or within collections?
- **Collection Disambiguation:** Are extracted facts correctly assigned to their true semantic domain (`Identity` vs `Profile`, `Entities` vs `Constraints`), or miscategorized?
- **Failure Analysis:** If compaction fails or requires retries, why did it fail (prompt length overflow, schema confusion, LLM degradation)?

#### 2. Eval 2 — 4-Stage Ingestion Pipeline & State Resolution (`eval_pipeline.rs`)
- **Deduplication Semantic Validity:** Exactly which facts were merged in Stage 1 (Jaccard) and Stage 2 (Soft Vector)? Were merged facts genuinely identical, or did soft-dedup cause false positives that destroyed distinct facts?
- **Candidate Retrieval & Edge Soundness:** Is candidate selection missing valid fact pairs? Are generated NLI edges (`SUPERSEDES`, `SUPPORTS`, `CONFLICTS`) and cross-collection edges (`SHAPES`, `DEPENDS_ON`, `CONFLICTS_WITH`) logically sound?
- **Grouped Stage Latency & Throughput:** Measure per-stage grouped latencies per batch. Track end-to-end processing time for a single fact vs. a full batch of 128 facts. Verify metric population in `memory_pipeline_metrics`.

#### 3. Eval 3 — Pre-Retrieval Scope Classifier & Dynamic Token Waterfall (`eval_retrieval.rs`) [CRITICAL]
- **Scope Classification Validity:** Did `query-sieve-rs` classify test queries into the correct `MemoryScope` (`ChitChat`, `User`, `Domain`, `Temporal`)?
- **Retrieval Precision & Noise Ratio:** For each query, what facts were retrieved? Were they genuinely relevant? How many irrelevant facts were retrieved, and why?
- **Adversarial & Vague Query Dataset:** Benchmark queries must be purposefully engineered by analyzing stored facts and creating vague, ambiguous, or negative queries designed to stress-test vector search and BFS graph expansion.
- **Context Budget Enforcement:** Verify dynamic waterfall rendering respects the 15% budget cap under an **8192 token context window**.

---

**Phase 11 Goal:** Evaluate each stage of the Vox v7 Memory System independently against semantic quality, coverage, false positive rates, and latency dynamics using empirical datasets (`app/src-tauri/evals/datasets/`), logging all evidence, and executing independent subagent reviews via `agy-subagent`.

