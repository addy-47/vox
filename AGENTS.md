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

2. **NEVER RUN BENCHMARKS OR EVALUATION SCRIPTS IN DEBUG MODE**:
   - Debug builds (`dev` profile without `--release`) omit SIMD vectorization, ONNX graph optimizations, and LTO, producing invalid latency metrics (up to 7x slower).
   - Always execute evaluation scripts and benchmarks using `--release` mode (e.g. `cargo run --release --example <eval_name>`).

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
- **Benchmark Execution:** `--release` mode only (`cargo run --release --example <eval_name>`).

---

### 5.1 Progress Summary (Completed Architecture & Code Enhancements)

1. **Intra-Batch Candidate Self-Fetch Fix (`queries.rs`)**: Scoped candidate queries to `status IN ('embedded', 'evaluated')`, eliminating circular `A <-> B` loops within in-flight batch items.
2. **Chronological NLI Alignment (`stage3_eval.rs`)**: Passed DB historical fact as `Premise` and incoming item as `Hypothesis` for accurate DeBERTa-v3 state transition evaluation.
3. **Multi-Pair ONNX Tensor Batching (`nli.rs`)**: Implemented 2D ONNX tensor encoding (`raw_predict_batch`), reducing Stage 3 ONNX inference CPU latency by 5x.
4. **5-Collection Priority Cross-Collection Dedup (`stage1_dedup.rs` & `stage2_embed.rs`)**: Implemented Priority resolution (`Identity` > `Constraints` > `Directives` > `Profile` > `Entities`) on exact/soft vector collisions.
5. **Heuristic Stripping & ModernBERT Probability Logging (`stage3_eval.rs`)**: Removed brittle keyword filters; unified NLI threshold (`contradiction >= 0.85`, margin `>= 0.20`); logged ModernBERT relation probabilities as `edge_score`.

---

### 5.2 QA Evaluation Purpose, Non-Negotiable Rules & Pitfalls to Avoid

> 🛑 **MANDATORY EVALUATION INVARIANTS:**
> 1. **No Shallow Summaries / No Narrative Abstractions**: Reports must contain exact failure counts, percentages, un-truncated text pairs, logits, similarity scores, and rejection tags. NEVER accept 2 handpicked examples as a complete analysis.
> 2. **No Threshold Guessing**: Do NOT tweak constants in `constants.rs` based on superficial observations. Every threshold recommendation must be backed by a full confusion matrix.
> 3. **Source of Truth for Fine-Tuning**: The primary deliverable is an un-truncated, evidence-backed diagnostic artifact (`memory_architecture_failure_analysis.md`) that `@ml-research-engineer.md` can directly consume to curate fine-tuning datasets (`vox-nli-state-transitions` and `vox-modernbert-graph-edges`).

---

### 5.3 Deterministic Error Taxonomy & Classifier Confusion Matrix

Every evaluation run must track and report exact item counts across these failure modes:

| Classifier Stage | Metric / Cell | Failure Type & Definition |
| :--- | :--- | :--- |
| **Vector Search Floor** | **Sub-Floor FN** | Valid state update/relation pruned before Stage 3 because $0.25 \le \text{cos\_sim} < \text{threshold}$. |
| **Intra-Collection NLI** | **False Negative (FN)** | Incoming fact updated/superseded DB fact, but NLI scored `Neutral` or $< 0.85$ `Contradiction`. |
| **Intra-Collection NLI** | **False Positive (FP)** | Distinct incoming fact incorrectly deleted an existing fact (`Contradiction` $\ge 0.85$). |
| **Intra-Collection NLI** | **True Positive / TN** | Correctly superseded facts (TP) and correctly retained distinct facts (TN). |
| **Inter-Collection Edge** | **False Negative (FN)** | Valid cross-collection relation present, but ModernBERT score $< 0.80$ (`below_edge_classifier_confidence`). |
| **Inter-Collection Edge** | **False Positive (FP)** | Unrelated entities/directives linked with confidence $\ge 0.80$. |

---

### 5.4 GPU Server & 3-Tier Ollama Judge Cascade Architecture

To avoid API rate limits and achieve fast, scalable evaluation across large datasets, evaluation uses the remote GPU server (`hypr4@100.86.62.14` RTX 5070 Ti, creds in `temp/server.txt`):
- **Temperature & Window Config**: Evaluation judge calls use `temperature = 0.0` for deterministic scoring and respect the **8192 token context window cap**.

```mermaid
flowchart TD
    Eval[eval_pipeline.rs Run] --> Batches[Atomic Batches of 16 Items]
    Batches --> Llama[Ollama llama3.1:8b Atomic Batch Judge]
    Llama --> AtomicReports[Per-Batch Audit Reports (Batch 01..NN)]
    AtomicReports --> Gemma[Ollama gemma4:e4b Sub-Master Synthesizer]
    Gemma --> SubMasterReports[Sub-Master Dataset Group Reports]
    SubMasterReports --> Subagent[Persistent QA Subagent send_message]
    Subagent --> MasterReport[Report C Master Synthesis & ML Diagnostic Spec]
```

1. **Atomic Batch Judge (`llama3.1:8b`)**: Evaluates individual 16-item batches from `eval_pipeline.rs`, auditing raw NLI logits, sub-floor candidates, and rejection reasons.
2. **Sub-Master Batch Synthesizer (`gemma4:e4b`)**: Aggregates 3-4 atomic batch reports into a sub-master report per dataset session.
3. **Persistent QA Subagent (`.agents/rules/qa-engineer.md`)**: A single QA Subagent is invoked **ONCE** via `invoke_subagent` and reused across all phases via `send_message` to execute deep semantic audits and compile the final master synthesis.

---

### 5.5 Mandatory Per-Eval Independent Subagent Audit & HITL Gate

> 🛑 **MANDATORY PER-EVAL GATING INVARIANT:**
> 1. **Single QA Subagent Reuse**: Spawn ONE QA Subagent at Phase 2 using `.agents/rules/qa-engineer.md`. Reuse its conversation ID via `send_message` for ALL subsequent checkpoints. DO NOT spawn duplicate subagents per turn.
> 2. **2 Checkpoints Per Phase**:
>    - **Checkpoint A (Post-Compaction)**: QA Subagent audits extracted facts for completeness, schema disambiguation, and context retention.
>    - **Checkpoint B (Post-Pipeline)**: QA Subagent audits Stage 1/2 dedup merges, Stage 3 NLI false positives/negatives, ModernBERT edge calibration, sub-floor near-misses, and Ollama judge reports.
> 3. **Deep Semantic Audit**: The subagent must act as a mini QA Lead, inspecting raw database rows (`audit_json`, `dedup_match_json`) and verifying output matching our goals rather than just checking exit codes.
> 4. **HITL User Approval Gate**: After the QA Subagent completes its report for a phase, execution MUST STOP. Present the audit findings to the user and wait for explicit HITL user approval before starting the next evaluation phase.

**Phase 11 Goal:** Produce a 100% evidence-backed, un-truncated diagnostic report and dataset curation specification for `@ml-research-engineer.md` using the multi-dataset GPU judge pipeline.




