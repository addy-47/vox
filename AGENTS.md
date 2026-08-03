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

**Testing & QA Invariants (MANDATORY FOR ALL AGENTS):**
1. **Execution != Verification:** A zero exit code (`0`) or non-crashing script is NEVER evidence of success. Success requires empirical verification of values, state changes, graph edges, and log consistency against the spec (`docs/plans/memory-spec-v7.md`).
2. **No Fake Success or Mocks:** Never use mock data, hardcoded fallbacks, or hidden recovery paths. Always test against the real system, real embeddings, real ONNX models, and real LLM evaluation pipelines.
3. **Independent Subagent Verification (`agy-subagent`):** The agent writing or running a test must NEVER approve it. Independent evaluation gates must be executed via persistent subagents using `agy-subagent` CLI (`--model gemini-3.6-flash-high --dangerously-skip-permissions`).
4. **Stage-by-Stage Before E2E:** Each stage (Stage 1 Dedup, Stage 2 Embedding/Soft Dedup, Stage 3 NLI/Edge Eval, Stage 4 Commit/Prune, and Retrieval Waterfall) must pass individual ground-truth evaluation before E2E testing begins.

---

### What Success Looks Like for Each Eval Stage

#### 1. Eval 1: Multi-Window Ingestion & LLM Compaction (`eval_compaction.rs`)
- **JSON Schema Conformance:** 100% valid JSON responses conforming to the 6 collections (`Identity`, `Directives`, `Narrative`, `Profile`, `Entities`, `Constraints`). Zero markdown framing leakage; retry count $\le 2$.
- **Semantic Extraction Quality:** LLM-as-a-Judge evaluation scores $\ge 85\%$ Fact Accuracy, $\ge 90\%$ Disambiguation, and $0\%$ Hallucination/Misattribution across 300 curated turns.
- **Queue Insertion:** All extracted facts correctly enqueued into `personal_memory_queue` with `status = 'staged_pending'`.

#### 2. Eval 2 — Stage 1: Jaccard Exact Deduplication (`stage1_dedup.rs`)
- **Exact String/Word-Set Dedup:** Facts with Jaccard similarity $= 1.0$ against active facts in the same collection are transitioned to `status = 'superseded'`.
- **Non-Duplicates:** Unique facts correctly transition from `staged_pending` $\rightarrow$ `status = 'deduped'`.
- **Batch Processing:** Processes up to 128 pending items per batch with zero lost records or race conditions.

#### 3. Eval 2 — Stage 2: Dense Embedding & Soft Vector Dedup (`stage2_embed.rs`)
- **Vector Generation:** 384-dimensional INT8 ONNX MiniLM-L12 embeddings generated for all `deduped` items (~10ms/item).
- **Soft Vector Dedup:** Intra-collection candidates with cosine similarity $\ge 0.95$ trigger soft deduplication: candidate item set to `status = 'superseded'` and a `SUPERSEDES` edge written to `memory_relations`.
- **Status Transition:** Non-duplicate items transition to `status = 'embedded'`. Error retry limit (max 3) enforced before setting `status = 'failed'`.

#### 4. Eval 2 — Stage 3: Unified Edge & NLI State Evaluation (`stage3_eval.rs`)
- **Sub-Branch A (NLI DeBERTa-v3):**
  - `Identity`/`Directives` Contradiction ($\ge 0.85$): Old fact `status` updated to `'inactive'`, new fact remains `'active'`, `SUPERSEDES` edge written.
  - `Identity`/`Directives` Entailment ($\ge 0.85$): Both facts remain `'active'`, `SUPPORTS` edge written.
  - `Constraints` Contradiction ($\ge 0.85$): Writes `CONFLICTS` edge; **neither constraint is deactivated** (both remain `'active'`).
- **Sub-Branch B (Edge Classifier ModernBERT):** Cross-collection edges (`SHAPES`, `DEPENDS_ON`, `CONFLICTS_WITH`) created when confidence $\ge 0.80$, writing both forward and inverse edge labels atomically into `memory_relations`.
- **Status Transition:** Processed items transition from `embedded` $\rightarrow$ `status = 'evaluated'`.

#### 5. Eval 2 — Stage 4: Commit & Graph Prune (`stage4_commit.rs`)
- **Fact Commit:** Evaluated items committed to `memory_facts` with exact expected state (`active`, `inactive`, `superseded`).
- **Provenance Mandate:** Zero row deletions (`DELETE FROM memory_facts` is strictly 0).
- **Relation Consistency:** All forward and inverse graph edges correctly populated with valid foreign key references in `memory_relations`.
- **Metrics Logging:** Stage latencies, item counts, relation counts written to `memory_pipeline_metrics`.

#### 6. Eval 3: Pre-Retrieval Scope Classifier & Dynamic Token Waterfall (`eval_retrieval.rs`)
- **`query-sieve-rs` Scope Classification:** 100% classification accuracy on test query benchmark into `ChitChat`, `User`, `Domain`, `Temporal`.
- **Zero-Overhead ChitChat:** `ChitChat` scope completely bypasses RAG vector retrieval (0ms SQL latency).
- **Scope Pruning:** `Profile` pruned from `Domain` scope; `Entities` and `Directives` pruned from `User` scope.
- **Dynamic Waterfall & Hard Budget Cap:** Prompt context rendering never exceeds `max_personal_memory_share = 0.15` (15% context window cap). BFS graph expansion respects `max_hops = 2`.

---

**Phase 11 Goal:** Evaluate each stage of the Vox v7 Memory System independently using empirical datasets (`app/src-tauri/evals/datasets/`), enforce strict metric thresholds, log all evidence, and perform independent subagent reviews prior to full E2E system testing.

