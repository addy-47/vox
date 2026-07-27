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
2. **Step 2 Soft Vector Deduplication**: `MiniLM-L12` 384d INT8 ONNX (`soft_vector_dedup_threshold = 0.95`).
3. **Step 4A NLI State Resolution**: `deberta-v3-xsmall` ONNX for `Identity`, `Directives`, `Constraints` ($\ge 0.85$ threshold).
4. **Step 4B Cognitive Edge Classification**: `LFM2.5-230M` GGUF for `Profile`, `Entities` according to Connection Policy Matrix.
5. **Operational State Temporal Fetch**: `Directives` bypass vector RAG and fetch active state temporally (`ORDER BY created_at DESC`).

### Domain Taxonomy & Pipeline Setup
| Domain | Purpose | Evaluation Pipeline | Retrieval Policy |
|---|---|---|---|
| **`Identity`** | Core User Identity | Step 1 Dedup $\rightarrow$ Step 2 Soft Dedup $\rightarrow$ Step 4A NLI | Deterministic SQL (`WHERE status = 'active'`) |
| **`Directives`** | Agent Operational State / Active Tasks | Step 1 Dedup $\rightarrow$ Step 2 Soft Dedup $\rightarrow$ Step 4A NLI | Temporal Active Fetch (`ORDER BY created_at DESC`) |
| **`Constraints`** | Hard Boundaries / Rules | Step 1 Dedup $\rightarrow$ Step 2 Soft Dedup $\rightarrow$ Step 4A NLI | Dynamic Hybrid Core Budget (8% Cap) |
| **`Profile`** | User Persona / Tastes | Step 1 Dedup $\rightarrow$ Step 2 Soft Dedup $\rightarrow$ Step 4B LLM Edge Classifier | Semantic Vector Search (ANN) + Graph Traversal |
| **`Entities`** | External Knowledge Graph | Step 1 Dedup $\rightarrow$ Step 2 Soft Dedup $\rightarrow$ Step 4B LLM Edge Classifier | Semantic Vector Search (ANN) + Graph Traversal |
| **`Narrative`** | Session History Summary | Compaction turn summary | Backward Prepending Context Chain (5% Cap) |

### Pre-Implementation Gate Matrix Status
| Gate | Target / Component | Status / Outcome |
|---|---|---|
| **Gate 1** | MiniLM-L12 Soft Vector Dedup Calibration | **PASSED** (Threshold = 0.95, 0.0% false inactivations across 500 pairs, 29.7ms/pair. See `docs/benchmarks/dedup-bench.md`) |
| **Gate 2** | DeBERTa-v3 NLI Domain Precision Audit | **FAILED** (78.67% overall. Directives = 98.67%, Identity = 76.00%, Constraints = 65.00%. See `docs/benchmarks/nli-precision-bench.md`) |
| **Gate 3** | LFM2.5-230M Edge Classifier Capabilities Probe | Pending Evaluation |

### Model Paths & Primary Specs
- Embedding: `~/.vox/models/embedding/minilm-l12-v2` (384d INT8 ONNX)
- NLI Engine: `~/.vox/models/nli/deberta-v3-xsmall/model_quantized.onnx`
- Edge Classifier LLM: `~/.vox/models/llm/LFM2.5-230M-Q8_0.gguf`
- Architecture Spec: `docs/plans/memory-spec-v7.md`
- Benchmark Harness: `app/src-tauri/benches/dedup_bench.rs`

### Proactively update the `AGENTS.md` current phase section to reflect current status.