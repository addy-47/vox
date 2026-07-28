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

2. **TRUE END-TO-END PER-PAIR LATENCY MANDATE**:
   - The per-pair latency timer MUST start **BEFORE** user-turn tokenization and `ctx.decode(&mut batch)` prefill, and stop **AFTER** logit extraction.
   - Timers measuring only array indexing or logit lookup are strictly forbidden.

3. **FROZEN SYSTEM PROMPT KV CACHE**:
   - System prompt tokens MUST be prefilled into `llama_context` **EXACTLY ONCE** at startup.
   - Per-pair inference MUST clear only user-turn positions (`sys_len..N`), leaving system prompt KV cache frozen.

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

## 5. Current Phase — Vox v7 Architecture Implementation & Gate Validation

### What is being validated (v7 Specification)
1. **6 Domain-Agnostic Cognitive Taxonomy**: `Identity`, `Directives`, `Constraints`, `Profile`, `Entities`, `Narrative`.
2. **4-Stage Pipeline with Unified Evaluation**:
   - **Stage 1**: String & Jaccard Dedup (`staged_pending` $\rightarrow$ `deduped`).
   - **Stage 2**: Dense Vector Embedding (`deduped` $\rightarrow$ `embedded`).
   - **Stage 3**: Unified Edge & State Evaluation (Intra-Domain NLI + Inter-Domain Edge Classifier) (`embedded` $\rightarrow$ `evaluated`).
   - **Stage 4**: Terminal Commit & Prune (`evaluated` $\rightarrow$ `completed`/deleted).
3. **Precision RAG Retrieval & Hybrid Budgeting**:
   - `Identity`: Deterministic SQL fetch of ALL active facts (`WHERE status = 'active'`).
   - `Directives`: Top-level parent seed on **Turn 1 of new session ONLY**; child graph node on Turns 2+.
   - `Constraints`: Integrated into **Semantic Vector Search + Graph Traversal** (`RESTRICTS` / `CONFLICTS`).
   - `Profile` / `Entities`: Semantic Vector Search (ANN) + Graph Traversal.

### Domain Taxonomy & Pipeline Setup
| Domain | Purpose | Evaluation Pipeline | Retrieval Policy |
|---|---|---|---|
| **`Identity`** | Core User Identity | Step 1 Dedup $\rightarrow$ Step 2 Soft Dedup $\rightarrow$ Step 3 Unified Eval | Deterministic SQL (`WHERE status = 'active'`). All active facts. |
| **`Directives`** | Agent Operational State / Active Tasks | Step 1 Dedup $\rightarrow$ Step 2 Soft Dedup $\rightarrow$ Step 3 Unified Eval | **Turn 1 Parent Seed ONLY**; Child Graph Node on Turns 2+. |
| **`Constraints`** | Hard Boundaries / Rules | Step 1 Dedup $\rightarrow$ Step 2 Soft Dedup $\rightarrow$ Step 3 Unified Eval | **Semantic Vector Search + Graph Traversal** (`RESTRICTS` / `CONFLICTS`). |
| **`Profile`** | User Persona / Tastes | Step 1 Dedup $\rightarrow$ Step 2 Soft Dedup $\rightarrow$ Step 3 Unified Eval | Semantic Vector Search (ANN) + Graph Traversal. |
| **`Entities`** | External Knowledge Graph | Step 1 Dedup $\rightarrow$ Step 2 Soft Dedup $\rightarrow$ Step 3 Unified Eval | Semantic Vector Search (ANN) + Graph Traversal. |
| **`Narrative`** | Session History Summary | Compaction turn summary | Backward Prepending Context Chain (5% Cap). |

### Pre-Implementation Gate Matrix Status
| Gate | Target / Component | Status / Outcome |
|---|---|---|
| **Gate 1** | MiniLM-L12 Soft Vector Dedup Calibration | **PASSED** (Threshold = 0.95, 0.0% false inactivations across 500 pairs, 29.7ms/pair. See `docs/benchmarks/dedup-bench.md`) |
| **Gate 2** | DeBERTa-v3 NLI Domain Precision Audit | **PASSED** (`nli-deberta-v3-base` selected, 85.11% overall, Directives = 99.33%, Constraints = 75.50%, 64.8ms/pair. Multi-model PyTorch candidate evaluation complete. See `docs/benchmarks/nli-precision-bench.md`) |
| **Gate 3** | Cognitive Edge Classifier Calibration | **IN PROGRESS (Pending ONNX Sequence Classifier Fine-Tuning)** — Edge ontology finalized to 4 operational labels (`SHAPES`, `DEPENDS_ON`, `CONFLICTS_WITH`, `NONE`). 1-pass `ModernBERT-base` INT8 ONNX sequence classifier selected as winning architecture (~35ms CPU latency, <120MB RAM). Pending 500-pair MVD dataset creation and PyTorch fine-tuning. See `docs/benchmarks/edge-classifier-bench.md`. |

### Model Paths & Primary Specs
- Embedding: `~/.vox/models/embedding/minilm-l12-v2` (384d INT8 ONNX)
- NLI Engine: `~/.vox/models/nli/nli-deberta-v3-base/model_quantized.onnx` (233MB INT8 ONNX)
- Edge Classifier Engine: `~/.vox/models/classifier/modernbert-base/model_quantized.onnx` (1-pass INT8 ONNX sequence classifier)
- Architecture Spec: `docs/plans/memory-spec-v7.md`
- Benchmark Harness: `app/src-tauri/examples/edge_classifier_probe.rs` & `app/src-tauri/examples/modernbert_zeroshot_probe.rs`