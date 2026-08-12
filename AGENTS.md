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

### 5.4 Consolidated System Notes & Critical Pitfalls

- **Memory UI Page & Observability (`Memory.tsx` / `MemoryGraph.tsx` / `MemoryPipelineDrawer.tsx`)**:
  - **WebGL GPU Hardware Acceleration**: Graph geometry rendering is powered by Three.js WebGL GPU instanced buffer geometry (`Three.js` v0.184.0 via `react-force-graph-3d` in 2D WebGL camera mode), offloading node and edge rasterization to GPU fragment shaders for locked 60 FPS performance at 10,000 nodes scale.
  - **Subpanel 6 Initial WebGL Loader**: Glowing geometric polyhedron icon, ambient radial pulse ring, tracking-widest title `Building memory graph...`, metric pill (`1,299 nodes · 470 edges`), and status text `Optimizing spatial topology layout`.
  - **Subpanel 1 Landmark Badge Hover Tooltip**: Expanded collection header (`ENTITIES 104`), collection description (`Semantic Graph Collection...`), 2x2 metric stat grid (`Active Facts: 192`, `Total Relations: 387`, `Avg. Connections: 2.01`, `Last Updated: 2m ago`), and connected relation breakdown. Sphere/dot icon removed.
  - **Subpanel 2 Pipeline Drawer (`MemoryPipelineDrawer.tsx`)**: Slide-in panel (`w-[420px] top-[40px] h-[calc(100vh-40px)]`) aligned below titlebar without clipping or dark backdrop dimming. Pauses background graph topology polling while open. Features `Auto-Ingestion Daemon` status card (`Uptime`, `Total Processed`, `Avg/Fact`, `Success Rate`), 5-stage vertical timeline, Stage 5 expandable failed items with selective retries, and bottom CTA consolidation button.
  - **Subpanel 3 Detailed Node Tooltip (`MemoryNodeTooltip.tsx`)**: Status tag (`Active Fact`), metadata grid (`Fact ID`, `Created`, `Updated`, `Confidence`, `Source`), quote box, relation summary count pills (`SUPPORTS`, `DEPENDS_ON`, `CONFLICTS_WITH`), and direct connected relations list.
  - **Right Action Dock**: Vertically centered top-right floating dock (`top-1/2 -translate-y-1/2 right-6`) with 18px icons in `w-10 h-10` button targets matching edge navigation size.
  - **Strict 2D WebGL Graph View**: OrbitControls rotation is disabled (`controls.enableRotate = false`) to enforce 2D pan and zoom navigation strictly.
  - **Decoupled Three.js Mesh Instantiation**: Node selection highlights run via GPU uniform material color updates without destroying or re-instantiating 1,200 Three.js WebGL Meshes, eliminating 4-5s click lag.
  - **Isolated Search Component**: Search bar state is isolated into `<SearchBar>`, preventing typing keystrokes from re-rendering the parent page or triggering graph VDOM passes.
  - **Dynamic Cluster Badge Tracking**: Attached `change` event listener to OrbitControls so landmark badges glide smoothly over cluster centroids frame-by-frame during camera drag/zoom.
  - **Cardless Subpanel 6 Initial Loader**: Full-screen centered glass surface with glowing 3D polyhedron wireframe and concentric ambient rings (`bg-radial`), without card box wrapper.
  - **Vox Semantic Theme CSS Variables**: Zero double-border ghost cards, zero hardcoded dark boxes. Uses Vox semantic CSS tokens (`bg-[rgb(var(--card))]`, `text-[rgb(var(--foreground))]`, `border-[rgba(var(--border),0.15)]`) ensuring 100% WCAG contrast across Light Mode and Dark Mode.

- **Linux Window Invariant (`window_customizer.rs`)**:
  - WebKitGTK trackpad pinch-to-zoom is disabled in `PinchZoomDisablePlugin` by destroying `wk-view-zoom-gesture` handlers on GTK widget realization and forcing `zoom-level` back to `1.0`.

- **Backend ONNX Model Memory Footprint & Eviction Lifecycle**:
  - **Zero Idle ONNX Model RAM Baseline**: 0 ONNX models are loaded on application boot.
  - **Singleton Eviction Pattern**: Replaced static `OnceLock<T>` with thread-safe `parking_lot::RwLock<Option<T>>` for all 5 ONNX singletons (`TransliterationEngine`, `QueryScopeClassifier`, `TextEmbedder`, `NliEngine`, `EdgeClassifierEngine`). Calling `*SINGLETON.write() = None` drops the ORT Session and tokenizer, immediately returning process memory back to the OS.
  - **Audio Engine Gating**: `launch_engine()` on startup is strictly gated on `tray_enabled == true`. Opening the Main App window (`show_main_window`) DOES NOT trigger `launch_engine()` in passive mode. Engine STT/VAD models are lazy-loaded on-demand when the user clicks engage (`engage()`).
  - **Memory Pipeline ONNX Lifecycle**: The 3 memory pipeline ONNX models (Embedder, NLI Engine, Edge Classifier) are lazy-loaded only when `memory.pipeline_processing_enabled == true` AND `personal_memory_queue` has pending items (`status IN ('staged_pending', 'deduped', 'embedded')`). When the queue is empty during 30s idle sweeps, model loading is skipped entirely.
  - **Barge-In Eviction**: On voice engagement (`PipelineActive` event) or session disengage / batch completion, `unload_memory_pipeline_onnx_models()` / `unload_all_onnx_models()` evicts all pipeline ONNX sessions from RAM so voice processing runs with full CPU/RAM headroom.

- **Frontend & Event Safety Standards**:
  - **Listeners**: Push unlisteners immediately to cleanup array (`TrayApp.tsx`). Direct `listen` promise chains without cleanup are banned.
  - **Routing**: `TitleBar.tsx` must use `useNavigate()`; manual `window.history.pushState` dispatching is banned.
  - **State Mutations**: Render-phase state mutations in settings/models components are banned; use `useEffect`.

- **Backend Memory Subsystem Architecture (`app/src-tauri/src/ipc/memory/`)**:
  - **Modular Structure**: `ipc/memory/{mod.rs, graph.rs, ingestion.rs, mutations.rs, conflicts.rs}`.
  - **Cache Token (`graph_version`)**: Atomic `Arc<AtomicU64>` in `MemoryAppState` (`state.rs`), incremented on mutations, conflict resolutions, and background worker commits.
  - **Topology Endpoint**: `get_memory_graph_topology` fetches node topology via a single SQL query (`EXISTS(SELECT 1 FROM memory_relations WHERE to_id = f.id AND relation = 'SUPERSEDES') as is_superseded`), avoiding $O(N)$ N+1 subqueries. Full text is lazy-loaded per node via `get_memory_fact_detail`.
  - **Fact Mutations & Transactions**: Multi-statement operations (`edit_fact_content`, `soft_delete_fact`, `resolve_memory_conflict`) MUST be wrapped in explicit SQLite transactions (`BEGIN TRANSACTION;` ... `COMMIT;` with `ROLLBACK` on error).
  - **Vector Embedding Synchronization**: `edit_fact_content` updates raw text and executes SQLite `UPSERT` on `memory_facts_vectors(fact_id)` (`ON CONFLICT(fact_id) DO UPDATE SET embedding = excluded.embedding`) to prevent vector drift.
  - **Status Invariant on Deletes & Conflict Resolution**: `soft_delete_fact` and `resolve_memory_conflict` MUST execute `UPDATE memory_facts SET status = 'superseded' WHERE id = ?` on target/loser nodes. *Pitfall: Omitting this leaves superseded facts as `status = 'active'`, causing vector retrieval (`WHERE status = 'active'`) to inject deleted/loser facts into LLM context windows.*
  - **Queue Retries**: `retry_failed_queue` (all failed items) and `retry_failed_queue_items` (selective item IDs) reset both `attempts = 0` AND `retry_count = 0`. Worker auto-retries idle items when `retry_count < 3`.
  - **Database Indexes**: Schema defines `idx_mfv_fact_id` on `memory_facts_vectors(fact_id)` and `idx_pmq_session` on `personal_memory_queue(session_id)` to prevent $O(N)$ full table scans.
  - **Relation Constants**: Conflict relation string is `PM_RELATION_CONFLICTS` (`"CONFLICTS"`). Using `"CONFLICTS_WITH"` in SQL returns 0 rows.


