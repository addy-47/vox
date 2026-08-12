# AGENTS.md — Vox Workspace Rules

---

## 0. MANDATORY RULE: Automatic Documentation & AGENTS.md Sync Hook

> 🛑 **MANDATORY POST-TASK DOCUMENTATION HOOK (NON-NEGOTIABLE):**
> Every time code, architecture, candidate thresholds, system prompts, or LLM judge models are modified, or a task/phase is completed:
> 1. You **MUST** automatically update `AGENTS.md` to reflect the exact current implementation, model configuration, and threshold matrix. 
> 2. You **MUST** automatically update the relevant feature documentation (e.g. `docs/features/memory-architecture.md` and feature ledgers).
> 3. This is a **mandatory post-task completion hook** — do NOT wait for the user to explicitly remind you to sync documentation.

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
| ML Research Engineer | `.agents/rules/ml-research-engineer.md` | ML model research, evaluation, and fine-tuning dataset curation |
| Test Engineer | `.agents/rules/test-engineer.md` | Test case design, benchmark validation, and performance analysis |

---

## 5. Current Phase — Phase 11: Memory Pipeline Stage-by-Stage Evaluation & QA

**Global Evaluation Configuration:**
- **Context Window Cap:** Set to **8192 tokens** across all stage evaluation runs (`eval_compaction.rs`, `eval_pipeline.rs`, `eval_retrieval.rs`).
- **Benchmark Execution:** `--release` mode only (`cargo run --release --example <eval_name>`).

---

### 5.1 Calibrated Memory Pipeline Threshold Matrix

| Pipeline Stage & Purpose | Constant Name | Calibrated Code Setting | Rationale & Domain Logic |
| :--- | :--- | :--- | :--- |
| **Stage 1 Exact Text Dedup** | Jaccard Sub-word | `0.85` | Verbatim and sub-word exact duplicate filter. |
| **Stage 2 Paraphrase Vector Merge** | `SOFT_VECTOR_DEDUP_THRESHOLD` | `0.95` | Unlimited candidate search (`None` limit) + collection priority resolution (`Identity` > `Constraints` > `Directives` > `Profile` > `Entities`). `Narrative` collection facts bypass Stage 2 vector generation (`vector = NULL`). |
| **Stage 3 Intra-Collection NLI Floor** | `SAME_COLLECTION_CANDIDATE_SEARCH` | **`0.60`** | Intra-collection facts (same topic/state) require high similarity for DeBERTa-v3 state replacement evaluation. |
| **Stage 3 Inter-Collection Edge Floor** | `INTER_COLLECTION_CANDIDATE_SEARCH` | **`0.40`** | Inter-collection cross-domain facts naturally have lower cosine similarity but form valid directed graph edges (`restricted_by`, `DEPENDS_ON`, `SHAPES`). |
| **Sub-Floor Candidate Audit Floor** | `SUBFLOOR_CANDIDATE_FLOOR` | **`0.25`** | Audit range: `[0.25, 0.60)` (`subfloor-intra`) and `[0.25, 0.40)` (`subfloor-inter`). |
| **Compaction Engine Parameters** | `chunk_size` / `temperature` | **`50 turns` / `0.5`** | 50 conversation turns per extraction chunk; 0.5 temperature for low-variance declarative fact compaction. |

---

### 5.2 LLM Judge Model Hierarchy (BANNED: `llama3.1:8b`)

To eliminate hallucinated reports and fictitious item comparisons, evaluation models are strictly tiered:

1. **Master Synthesis Judge (`Report C` & Compaction Master)**:
   - Model: **`meta/llama-3.1-70b-instruct`** via Nvidia API (`https://integrate.api.nvidia.com/v1/chat/completions`).
   - Fallback: `gemma4:e4b` on local GPU server if Nvidia API key is unavailable.
2. **Compaction Extractions & Sub-Batch Audit Reports**:
   - Model: **`gemma4:e4b`** on local GPU server (`http://100.86.62.14:11434`).
   - Fallback: `google/gemma-2-27b-it` via Nvidia API.
3. **BANNED MODEL**:
   - **`llama3.1:8b` is strictly forbidden** from judge evaluation due to hallucinated facts and fictitious report outputs.

---

### 5.3 Mandatory Per-Eval Independent Subagent Audit & HITL Gate

> 🛑 **MANDATORY PER-EVAL GATING INVARIANT:**
> 1. **Single QA Subagent Reuse**: Spawn ONE QA Subagent at Phase 2 using `.agents/rules/qa-engineer.md`. Reuse its conversation ID via `send_message` for ALL subsequent checkpoints. DO NOT spawn duplicate subagents per turn.
> 2. **2 Checkpoints Per Phase**:
>    - **Checkpoint A (Post-Compaction)**: QA Subagent audits extracted facts for completeness, schema disambiguation, and context retention.
>    - **Checkpoint B (Post-Pipeline)**: QA Subagent audits Stage 1/2 dedup merges, Stage 3 NLI false positives/negatives, ModernBERT edge calibration, sub-floor near-misses, and report accuracy.
> 3. **Deep Semantic Audit**: The subagent must act as a mini QA Lead, inspecting raw database rows (`audit_json`, `dedup_match_json`) and verifying output matching our goals rather than just checking exit codes.
> 4. **HITL User Approval Gate**: After the QA Subagent completes its report for a phase, execution MUST STOP. Present the audit findings to the user and wait for explicit HITL user approval before starting the next evaluation phase.

### 5.4 Consolidated System Invariants & Critical Pitfalls

- **Memory UI & WebGL Invariants (`Memory.tsx` / `MemoryGraph.tsx`)**:
  - **Strict 2D WebGL View**: `OrbitControls.enableRotate = false` is enforced.
  - **GPU Uniform Highlights**: Node selection updates GPU uniform material colors without destroying/re-instantiating 1,200 Three.js WebGL Meshes, avoiding 4-5s click lag.
  - **Isolated Search Input**: Search state is isolated inside `<SearchBar>` to prevent keystroke re-renders of graph VDOM.

- **Linux Window Invariant (`window_customizer.rs`)**:
  - WebKitGTK trackpad pinch-to-zoom is disabled in `PinchZoomDisablePlugin` by destroying `wk-view-zoom-gesture` handlers and setting `zoom-level = 1.0`.

- **Backend ONNX Lifecycle & Eviction Rules**:
  - **Zero Idle RAM**: 0 ONNX models loaded on boot. All 5 ONNX singletons use thread-safe `parking_lot::RwLock<Option<T>>` for instant process memory eviction (`*SINGLETON.write() = None`).
  - **Audio Engine Gating**: `launch_engine()` on startup is strictly gated on `tray_enabled == true`. Opening main window DOES NOT launch engine in passive mode. Models lazy-load on `engage()`.
  - **Memory Pipeline ONNX Lifecycle**: Memory ONNX models load ONLY when `memory.pipeline_processing_enabled == true` AND `personal_memory_queue` has pending items. Skip when queue is empty.
  - **Barge-In Eviction**: On voice engagement (`PipelineActive`), disengage, or batch completion, `unload_all_onnx_models()` evicts pipeline ONNX sessions from RAM.

- **Frontend & Event Safety Standards**:
  - **Listeners**: Push unlisteners immediately to cleanup array (`TrayApp.tsx`). Direct `listen` promise chains without cleanup are banned.
  - **Routing**: `TitleBar.tsx` must use `useNavigate()`; `window.history.pushState` is banned.
  - **State Mutations**: Render-phase state mutations are banned; use `useEffect`.
  - **Settings Card Sync**: `InteractionCard` and `ModelsCard` sync bidirectionally using `sync_pipeline_tab` and `sync_interaction_category` custom events.
  - **Manifest-Driven UI**: `LlmConfigDesk.tsx` and workspace cards read titles, descriptions, and flags (`is_cloud`, `is_remote`, `is_built_in`) directly from `modelCatalog` without hardcoded fallback strings.
  - **Test Clip Invariant (`homeData.ts`)**: `TEST_CLIPS` must match the 5 canonical WAV IDs in `app/src-tauri/test-clips/` (`short_en`, `short_hi`, `hinglish`, `command`, `expressive`).

- **Backend Memory Subsystem Invariants (`app/src-tauri/src/ipc/memory/`)**:
  - **Cache Token**: `graph_version` (`Arc<AtomicU64>`) incremented on all mutations.
  - **Transactions**: Multi-statement operations MUST be wrapped in explicit SQLite transactions (`BEGIN TRANSACTION;` ... `COMMIT;`).
  - **Vector Sync**: `edit_fact_content` executes SQLite `UPSERT` on `memory_facts_vectors(fact_id)`.
  - **Superseded Status Invariant**: `soft_delete_fact` and `resolve_memory_conflict` MUST execute `UPDATE memory_facts SET status = 'superseded' WHERE id = ?`. *Pitfall: Omitting this injects deleted/loser facts into LLM context windows.*
  - **Relation Constant**: Conflict relation string is `"CONFLICTS"`. Using `"CONFLICTS_WITH"` in SQL returns 0 rows.


