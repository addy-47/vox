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

**Code Fixes & Audit Expansion Applied (Current Working Tree):**
1. **Intra-Batch Candidate Self-Fetch & Circular Loop Fix (`queries.rs`)**: Updated `fetch_intra_collection_candidates` and `fetch_inter_collection_candidates` SQL status clauses from `IN ('embedded', 'processing_eval', 'evaluated')` to `IN ('embedded', 'evaluated')`. In-flight items in `processing_eval` within the same active batch can no longer self-fetch as candidates, eliminating bidirectional `A <-> B` circular `SUPERSEDES` loops.
2. **Chronological NLI Premise/Hypothesis Alignment (`stage3_eval.rs`)**: Configured NLI candidate pair evaluation so `cand_fact` (established historical DB state) is passed as **Premise** and `item.fact` (new incoming state) is passed as **Hypothesis**, ensuring DeBERTa-v3 evaluates state transitions in chronological order.
3. **Multi-Pair ONNX Tensor Batching (`nli.rs` & `stage3_eval.rs`)**: Implemented `raw_predict_batch` and `classify_batch` in `nli.rs` to encode and predict all candidate pairs for an item in a single 2D ONNX tensor pass (`[batch_size, max_seq_len]`), reducing Stage 3 CPU ONNX execution latency by up to 5x.
4. **Expanded Audit Logging & 3-Report Architecture**:
   - `personal_memory_queue`: Added `dedup_match_json` (stage, action, matched_fact_id, matched_fact, score) and `audit_json` (nli_scores, edge_score, rejection_reason, candidate_source).
   - `memory_pipeline_metrics`: Redesigned lean schema (removed SQL-derivable columns `items_processed`, `items_superseded`, `relations_created`), added `batch_seq`.
   - `stage1_dedup.rs`: Refactored prefetch map to `HashMap<collection, Vec<(id, fact_text)>>` to surface matched fact IDs at zero SQL cost.
   - `stage2_embed.rs`: Writes `dedup_match_json` on soft vector dedup hits.
   - `stage3_eval.rs`: Enriched `CandidateAuditLog` with explicit rejection reasons (`below_nli_confidence`, `topic_overlap_failed`, `nli_neutral`, `below_edge_classifier_confidence`) and candidate sources (`memory_facts` vs `queue_in_flight`).
   - `eval_pipeline.rs`: Built 3-report architecture (`stage1_stage2_dedup_report.md`, per-batch `stage3_batch_{01..NN}_report.md`, with Report C reserved for QA Subagent synthesis). Sub-floor candidate pass (`0.25 <= sim < threshold`) tags near-miss pairs.

**Last Evaluation Run Observations:**
- **Eval 1 (Compaction)**: Overall score **85/100** (Accuracy: 92%, Recall: 90%, Schema Disambiguation: 95%, Redundancy: 8%). 0% hallucinations. Occasional context drops and sliding window redundancy across turns.
- **Eval 2 (4-Stage Pipeline)**: Overall score **8.5/10** (Stage 1: 9/10, Stage 2: 8.5/10, Stage 3 NLI: 8.5/10, ModernBERT: 9/10).
- **Core Observation**: We cannot tune global thresholds based on superficial observations of 1-2 edge cases. We must collect granular, un-truncated per-batch logging data and evaluate facts, candidate pairs, model confidence scores, and rejection reasons via scope-reduced, per-batch/per-stage LLM judge reports.

---

### Phase 11 QA Evaluation Focus (Semantic Quality & System Behavior)

Deterministic checks (e.g. table creation, schema existence, non-null values, non-crashing execution) belong in automated unit tests. **QA Evaluation focuses strictly on semantic validity, information coverage, false positive/negative detection, edge correctness, and latency dynamics.**

#### 1. Eval 1 — LLM Compaction & Fact Extraction (`eval_compaction.rs`)
- **Information Coverage:** Do extracted facts capture all critical user information across conversation turns, or was vital context silently dropped?
- **Redundancy & Over-Extraction:** Is there semantic redundancy or over-extraction across or within collections?
- **Collection Disambiguation:** Are extracted facts correctly assigned to their true semantic domain (`Identity` vs `Profile`, `Entities` vs `Constraints`), or miscategorized?
- **Failure Analysis:** If compaction fails or requires retries, why did it fail (prompt length overflow, schema confusion, LLM degradation)?

#### 2. Eval 2 — 4-Stage Ingestion Pipeline & State Resolution (`eval_pipeline.rs`)
- **Deduplication Semantic Validity (Stage 1 & Stage 2 Audit Report)**: Audit all facts merged in Stage 1 (Jaccard) and Stage 2 (Soft Vector). Evaluate whether merged facts were genuinely identical, or if soft-dedup caused false positives that destroyed distinct facts.
- **Per-Batch Stage 3 Evaluation (Batch Size 16 Judge Reports)**: Stage 3 naturally executes in atomic batches of 16 items (`STAGE3_BATCH_SIZE = 16`). For EACH Stage 3 batch of 16 items, execute a dedicated LLM judge pass using the 16 facts + full candidate audit logs (including model logits, sub-floor candidates `0.25 <= sim < 0.40`, topic overlap flags, and rejection reasons).
- **Report C Master Synthesis**: Executed by the **QA Subagent** (`agy-subagent`), synthesizing individual deduplication and per-batch Stage 3 reports into an empirical scorecard and recommendation matrix.
- **Formed Relations & Candidate Retrieval Audit**:
  - Evaluate formed `SUPERSEDES`, `CONFLICTS`, and `SUPPORTS` edges for semantic correctness vs false positives.
  - Audit candidate retrieval precision: Did any candidate fall in the sub-floor range (`0.25 <= sim < 0.40`) that should have formed a relation but was missed?
  - Threshold & Heuristic Rejection Audit: Did a relation fail because model confidence was just below threshold or topic overlap failed?
- **Grouped Stage Latency & Throughput**: Measure per-stage grouped latencies per batch. Track end-to-end processing time for a single fact vs. a full batch of 128 facts. Verify metric population in `memory_pipeline_metrics`.

#### 3. Eval 3 — Pre-Retrieval Scope Classifier & Dynamic Token Waterfall (`eval_retrieval.rs`) [CRITICAL]
- **Scope Classification Validity:** Did `query-sieve-rs` classify test queries into the correct `MemoryScope` (`ChitChat`, `User`, `Domain`, `Temporal`)?
- **Retrieval Precision & Noise Ratio:** For each query, what facts were retrieved? Were they genuinely relevant? How many irrelevant facts were retrieved, and why?
- **Adversarial & Vague Query Dataset:** Benchmark queries must be purposefully engineered by analyzing stored facts and creating vague, ambiguous, or negative queries designed to stress-test vector search and BFS graph expansion.
- **Context Budget Enforcement:** Verify dynamic waterfall rendering respects the 15% budget cap under an **8192 token context window**.

---

**Phase 11 Goal:** Evaluate each stage of the Vox v7 Memory System independently against semantic quality, coverage, false positive rates, and latency dynamics using empirical datasets (`app/src-tauri/evals/datasets/`), logging all evidence, and executing independent subagent reviews via `agy-subagent`.

