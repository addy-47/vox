# Memory Subsystem — Review & Target Architecture Specification

**Spec Type:** Architecture Spec + Review Ledger (2-in-1)  
**Scope:** `app/src-tauri/src/services/memory/**/*`, `app/src-tauri/src/persistence/*`, `app/src-tauri/src/services/pipeline/modular/*` + `realtime/*` (call sites)  
**Version:** 2.0 — Corrected to live tree (2026-08-29, 21 files / 6343 LoC)  
**Supersedes:** `memory_formatting_context_assembly_spec.md` v1 (15-file draft with wrong counts/thresholds)  
**Status:** Review on top (Part I) is frozen fact; Part II is the build target. No ambiguity.

---

## Part I — Current Code Review (Adversarial, Frozen Fact)

### I.1 Live Tree Inventory — what actually exists (not what the draft described)

`find app/src-tauri/src/services/memory -type f -name "*.rs" | sort` on `2026-08-29`:

```
app/src-tauri/src/services/memory/mod.rs                                # 128 — constants, re-exports, heap-trim facade
app/src-tauri/src/services/memory/working_memory.rs                      # 738 — ConversationManager god object (see F2)
app/src-tauri/src/services/memory/ingestion.rs                            # 192 — misnamed: actually Compaction (LLM fact extraction)
app/src-tauri/src/services/memory/retrieval.rs                            # 245 — hybrid SQL+vector+BFS waterfall
app/src-tauri/src/services/memory/scope_router.rs                         # 108 — MemoryScope → collection routing
app/src-tauri/src/services/memory/formatter.rs                            # 184 — format_relative_timestamp + format_user_profile_context
app/src-tauri/src/services/memory/deduplication.rs                        #  81 — jaccard_similarity / is_exact_duplicate
app/src-tauri/src/services/memory/embedder.rs                             # 288 — MiniLM-L12 INT8 ONNX singleton
app/src-tauri/src/services/memory/tokenizer.rs                             #  36 — tiktoken cl100k_base estimate_tokens
app/src-tauri/src/services/memory/classifiers/mod.rs                       #  16 — re-exports
app/src-tauri/src/services/memory/classifiers/query_classifier.rs         # 118 — ModernBERT MemoryScope classifier
app/src-tauri/src/services/memory/classifiers/intra_edge_classifier.rs   # 406 — DeBERTa-v3 NLI engine
app/src-tauri/src/services/memory/classifiers/inter_edge_classifier.rs   # 254 — ModernBERT Edge classifier
app/src-tauri/src/services/memory/pipeline/mod.rs                          #  18 — re-exports runner/stages
app/src-tauri/src/services/memory/pipeline/runner.rs                      # 148 — run_pipeline_cycle / drain_pipeline_queue
app/src-tauri/src/services/memory/pipeline/stage1_dedup.rs                # 375 — Jaccard dedup + priority resolution
app/src-tauri/src/services/memory/pipeline/stage2_embed.rs                # 242 — MiniLM embed + soft vector dedup
app/src-tauri/src/services/memory/pipeline/stage3_eval.rs                 # 475 — NLI + ModernBERT concurrent eval
app/src-tauri/src/services/memory/pipeline/stage4_commit.rs               # 230 — atomic SQLite commit + queue prune
app/src-tauri/src/services/memory/pipeline/batch_result.rs                 #  48 — RelationEdge / DedupAuditLog / CandidateAuditLog
app/src-tauri/src/services/memory/pipeline/metrics.rs                      #  13 — PipelineStageMetrics
```

Total: **21 files, 6343 LoC** (wc -l). The draft v1 claimed ~15 files and described `harness/window/{buffer,accountant,prompt_builder}.rs`, `retrieval/search.rs`, `compaction/{prompt,runner,opportunistic}.rs`, `ml/*` — none of which exist. See I.3.

Ancillary live files this spec governs (out-of-pillar but in scope for the facade):
- `app/src-tauri/src/persistence/queries.rs` (380) — vector/SQL candidate fetch, graph neighbors
- `app/src-tauri/src/persistence/mutations.rs` (250) — enqueue_personal_facts, mark_job_failed, session_end_consolidation, record_stage_metrics
- `app/src-tauri/src/core/constants.rs:99-327` — `COMPACTION_SYSTEM_PROMPT`, `MemoryCollection`, `is_valid_inter_collection_pair`, `PM_RELATION_*`, queue status constants
- `app/src-tauri/src/services/pipeline/modular/context.rs:74` — **current manual orchestration** (`build_generation_request`)

---

### I.2 Functional & Logical Issues — what must be fixed (the reason this refactor exists)

#### F1 — No facade / manual orchestration in the pipeline domain (architectural violation)

**Where:** `app/src-tauri/src/services/pipeline/modular/context.rs:74` `build_generation_request()`  
**What it does:** Manually sequences `classify_scope(text):93` → `generate_embedding(text):95` → `retrieve_personal_context(conn, embedding, scope, …):97` → `update_dynamic_user_profile(profile):123` → `push_user_turn(text):124` → `build_context(provider_kind, is_devanagari, None, None):125` → `enqueue_personal_facts(personal_memory, …):141` (fire-and-forget `tauri::async_runtime::spawn`).  
**Why it is wrong:** `realtime/passive.rs:283` does not call this path at all (it only does `push_user_turn`); `realtime/ptt.rs` and `dictation` likewise diverge. A future `retrieve_memory` tool (LLM tool call in `realtime/`) would have to re-implement the same chain. The subsystem is not **domain-agnostic** — where the query came from (STT vs tool) changes which code runs.  
**Target fix (Part II §7):** Single async facade `harness::prepare_turn_context()` owns the entire chain; `services/pipeline/*` never imports `retrieval`, `embedder`, `scope_router`, `queries`, or `ingestion`. Verified by invariant V1.

#### F2 — `working_memory.rs` is a god object (738 LoC, 5 responsibilities fused)

**Where:** `app/src-tauri/src/services/memory/working_memory.rs:55` `struct ConversationManager`  
**Fused concerns:**
- Sliding window + KV-cache index (`messages: Vec<ChatMessage>`, `kv_synced_index: usize`, `push_user_turn:272`/`push_assistant_turn:297`/`pop_last_user_turn:319`/`build_narrative_context_chain:232`)
- Static identity preload (`identity_facts: Vec<String>`, `set_identity_facts:181`/`load_identity_into_system_prompt:199`)
- Dynamic profile injection + prompt assembly (`dynamic_user_profile: Option<String>`, `assemble_system_prompt:109`/`update_dynamic_user_profile:148`/`build_session_history_xml:352`/`consolidate_system_message:382`)
- Token accounting + threshold governance (`total_token_count`, `max_context_tokens`, `reserved_generation_tokens`, `soft_threshold`, `critical_threshold`, `context_utilization:333`/`needs_threshold_maintenance:347`)
- Compaction lifecycle (`perform_fifo_maintenance:490`/`perform_compaction_maintenance:555`/`apply_compaction_result:522`/`try_trigger_opportunistic:590`/`commit_opportunistic:615`/`cancel_opportunistic:666`)

**Why it is wrong:** The 4-pillar intent (Harness / Retrieval / Compaction / Ingestion) is collapsed into one file + one struct. Testing token budgeting requires constructing a full `ConversationManager`; swapping the compaction strategy requires editing the FIFO path. Spec drift is inevitable.

**Target fix:** Decompose into `harness/buffer.rs` (FIFO + `Role`/`ChatMessage`/`ConversationContext`), `harness/accountant.rs` (budget + thresholds + FIFO prune), `harness/prompt_builder.rs` (identity + `<user_profile>` + history assembly + relative timestamps), `harness/mod.rs` (`ConversationManager` thin facade + `prepare_turn_context`).

#### F3 — Misnamed file: `ingestion.rs` is Compaction

**Where:** `app/src-tauri/src/services/memory/ingestion.rs:1-192` (`run_compaction:123`, `build_compaction_request:19`, `execute_compaction_attempt:66`, `CompactionResult { context_summary, personal_memory, diff_to_enqueue:11 }`)  
**What it is:** LLM-driven summarization + fact extraction into 6 collections (`Identity`, `Directives`, `Narrative`, `Profile`, `Entities`, `Constraints`) via `COMPACTION_SYSTEM_PROMPT` (`core/constants.rs:101`). It never touches `personal_memory_queue` — that is `pipeline/runner.rs` + stages.  
**Why it is wrong:** `pipeline/` is the real ingestion (DB-backed batch). The name inversion makes onboarding and `grep` unreliable.  
**Target fix:** `ingestion.rs` → `compaction/runner.rs` (+ `compaction/prompt.rs` for the prompt/schema); `pipeline/` → `ingestion/` (see I.4 mapping).

#### F4 — Retrieval chain is half-wired (formatter exists but is unused on the hot path)

**Where:** `app/src-tauri/src/services/memory/retrieval.rs:128-129` formats seeds as `format!("- [{}] {}", collection, fact_text)` with **no** `format_relative_timestamp(created_at_ms)`; `app/src-tauri/src/services/memory/formatter.rs:5` `format_relative_timestamp` and `formatter.rs:56` `format_user_profile_context` are fully implemented and tested (`formatter.rs:93`) but never called from `retrieval.rs` or `working_memory.rs:109` `assemble_system_prompt`.  
**Why it is wrong:** Spec contract "every temporal fact carries `(2 hours ago)`" is defined and tested in isolation, not enforced end-to-end. `retrieval.rs:212` also returns `""` for `ChitChat` correctly, but identity freshness is tied only to boot-time `load_identity_into_system_prompt:199`, never refreshed per turn.  
**Target fix:** `retrieval/search.rs` returns structured `RetrievedProfile` (`Vec<ScoredFact>` with `created_at`); `harness/prompt_builder.rs` is the sole place that calls `format_relative_timestamp` + `format_user_profile_context` to produce the final `<user_profile>` XML (see §8).

#### F5 — Threshold & constant drift vs truth

**Where:** `app/src-tauri/src/services/memory/mod.rs:13-14` defines `CONTEXT_SOFT_THRESHOLD: f32 = 0.65`, `CONTEXT_CRITICAL_THRESHOLD: f32 = 0.85`. Draft v1 §5.1 said "Soft: 70%".  
**Why it is wrong:** `docs/features/memory-architecture.md:15` and live code disagree by 0.05; callers (`working_memory.rs:593` `try_trigger_opportunistic`, `working_memory.rs:347` `needs_threshold_maintenance`, `working_memory.rs:493` `perform_fifo_maintenance`) all compare against the `0.65`/`0.85` values from `mod.rs`. The spec must lock to `0.65`/`0.85`.  
**Target fix:** §9 locks to **Soft 65%**, **Critical 85%**, single source of truth `services/memory/mod.rs:13-14`.

#### F6 — Sync-async compaction bridge (fragile)

**Where:** `app/src-tauri/src/services/memory/ingestion.rs:66` `execute_compaction_attempt` does `Handle::try_current()` → `block_in_place` / ephemeral `new_current_thread` runtime to `await provider.generate()`, then pumps `VoxEvent::LlmToken` via `rx.recv_timeout(45s)`.  
**Why it is wrong:** Works today because `LlmProvider::generate` is `async` but the caller is sync (`working_memory.rs:571`). When `prepare_turn_context` becomes fully `async` on `vox-memory-worker`, this bridge becomes a deadlock / double-runtime hazard. Also `diff_to_enqueue == personal_memory` (`ingestion.rs:190` clone) — the name implies a delta, but no dedup delta is computed.  
**Target fix:** `compaction/runner.rs::run_compaction` becomes `async`, `await`s `provider.generate()` directly, returns `CompactionResult` without channel pumping shim.

#### F7 — Background compaction is a placeholder

**Where:** `app/src-tauri/src/services/pipeline/modular/context.rs:171` `trigger_background_compaction` does not call the LLM — it concatenates `messages[1..len-1]` with `"; "` as a pseudo-summary, then `commit_opportunistic`.  
**Why it is wrong:** The real opportunistic path is `working_memory.rs:590` `try_trigger_opportunistic` → `commit_opportunistic:615` (atomic `AtomicBool` cancellation + `snapshot_len` race check). The caller bypasses it, so soft-window work is either no-op or double-booked, and `NARRATIVE_CHAIN_SOFT_CAP_SHARE = 0.05` (`mod.rs:36`) is never exercised via this path.  
**Target fix:** `harness/mod.rs` owns opportunistic lifecycle; `trigger_background_compaction` is deleted; the facade calls `harness::try_trigger_opportunistic` → `compaction::run_compaction` on a cloned `messages` snapshot, then `commit_opportunistic` guarded by the same `AtomicBool`.

#### F8 — Enqueue is unvalidated / audit-divergent

**Where:** `app/src-tauri/src/persistence/mutations.rs:11` `enqueue_personal_facts` inserts every string from `parse_compaction_json()` as `staged_pending` (or `paused` if `pipeline_processing_enabled == false`) with no pre-insert dedup or schema validation.  
**Collateral:** `app/src-tauri/src/services/memory/pipeline/stage1_dedup.rs:115` deletes empty facts via `DELETE FROM personal_memory_queue WHERE id = ?` (not `superseded`), so audit `dedup_match_json` diverges for empty vs duplicate.  
**Target fix:** No pre-insert dedup (stages own that), but `formatter` ensures no empty string is enqueued; `stage1` should mark empties `superseded` with audit, or the spec must explicitly allow `DELETE` for empties with audit. Target spec chooses **keep `DELETE` for empties + mandatory `DedupAuditLog { action: "empty_fact_deleted" }`** as today, documented in §12.

#### NF1 — Module sprawl / ML primitives not flat

**Where:** `app/src-tauri/src/services/memory/embedder.rs:13` at root, `tokenizer.rs:1` at root, `classifiers/*` as subdir, `deduplication.rs:6` at root.  
**Why it is wrong:** Draft v1 §3 proposed `ml/` flat primitives — correct intent, not realized.  
**Target fix:** `ml/` flat: `embedder.rs`, `tokenizer.rs`, `nli.rs` (rename `intra_edge_classifier.rs`), `edge_classifier.rs` (rename `inter_edge_classifier.rs`), `scope_classifier.rs` (rename `query_classifier.rs`), `mod.rs` (lifecycle + `unload_*`). `deduplication.rs` is deleted per Q5 and inlined into `ingestion/stage1_dedup.rs`.

---

### I.3 What the draft v1 got wrong (quantitative ledger)

| Draft v1 claim (`memory_formatting_context_assembly_spec.md` v1 §3) | Live truth (cited above) | Correction in this spec |
|---|---|---|
| `harness/window/buffer.rs`, `window/accountant.rs`, `window/prompt_builder.rs` (3 files under `window/`) | `working_memory.rs` single 738-LoC god object — no `harness/` dir exists | Target is `harness/{buffer,accountant,prompt_builder,mod}.rs` (4 files), not `window/` (Part II §5) |
| `retrieval/{mod,search,scope,formatter}.rs` (4 files) | `retrieval.rs` (245) + `scope_router.rs` (108) + `formatter.rs` (184) — `search.rs` doesn't exist; SQL/vector/BFS all in `retrieval.rs`; `scope.rs` misnamed | `retrieval/{mod,search,scope}.rs` (3 files) + `formatter` moves to `harness/prompt_builder.rs` per Q4 |
| `compaction/{mod,prompt,runner,opportunistic}.rs` (4 files) | `ingestion.rs` (192) monolith — no `compaction/` dir; prompt is `COMPACTION_SYSTEM_PROMPT` in `core/constants.rs:101`; opportunistic lives in `working_memory.rs:589` | `compaction/{mod,prompt,runner}.rs` (3 files); `opportunistic.rs` deleted — logic lives in `harness/mod.rs` (Q1) |
| `ingestion/{mod,stage1-4,batch_result,metrics}.rs` (7 files) | `pipeline/{mod,runner,stage1-4,batch_result,metrics}.rs` (8 files incl. `runner.rs` + `mod.rs`) | `pipeline/` → `ingestion/` rename; keep all 8 files |
| `ml/{mod,embedder,tokenizer,nli,edge_classifier,scope_classifier}.rs` (6 files) | `embedder.rs` + `tokenizer.rs` at root + `classifiers/{query,intra,inter}_*.rs` + `deduplication.rs` (5 + 1 stray) — no `ml/` | `ml/` flat as proposed, plus `deduplication.rs` deletion |
| Total "proposed" ≈ 15 files | **21 files, 6343 LoC** actual | **Target 24 files** (see §6) — every live file accounted for |
| Soft threshold "70%" (v1 §5.1) | `mod.rs:14` `CONTEXT_SOFT_THRESHOLD = 0.65` | Locked to **65%** per Q6 (§9) |
| `format_relative_timestamp` in `retrieval/formatter.rs` | `formatter.rs:5` at `services/memory/` root, unused on hot path (F4) | Moved under `harness/prompt_builder.rs` ownership |

---

## Part II — Target Architecture Spec (Build Target, No Ambiguity)

### II.1 Inferred Spec Type

**Architecture spec.** Components, boundaries, data flow, ownership. No frameworks, no libraries named beyond the ONNX model kinds already in `mod.rs`. One concept: the Vox Memory subsystem's 4-pillar decomposition and its single entrypoint.

### II.2 Name & Concept

**The Vox Memory 4-Pillar Subsystem** — a Trabant for long-term personal memory that is (1) governed by an in-memory harness, (2) pulled per turn by a scope-pruned retrieval waterfall, (3) compressed by LLM compaction, and (4) ingested offline by a 4-stage DB-backed pipeline, behind a single domain-agnostic facade.

### II.3 Purpose

Give the main voice pipelines (`services/pipeline/modular/*`, `services/pipeline/realtime/*`, future `retrieve_memory` tool) a single call — "here is the user query, give me the LLM request" — without them knowing about embeddings, vector search, token budgets, or queue statuses. Fix F1-F8/NF1 so the subsystem is standard, intuitive, and testable, while preserving all current behavioral contracts (6 collections, priority ordering, thresholds, batch sizes).

### II.4 The 4-Pillar Diagram (authoritative)

```
                                   ┌──────────────────────────────┐
                                   │   Voice Pipeline Domain      │
                                   │   modular/passive.rs         │
                                   │   realtime/passive.rs        │
                                   │   realtime/ptt.rs  (future) │
                                   │   tools/retrieve_memory      │
                                   └──────────────┬───────────────┘
                                                  │
                                                  │ prepare_turn_context(query, turn_id, session_id)
                                                  ▼
┌────────────────────────────────────────────────────────────────────────────────────────────────────────┐
│ 🏛️ PILLAR 1: HARNESS (Context Governor & In-Memory Window)  — services/memory/harness/                │
│                                                                                                        │
│  ┌────────────────────────┐    ┌───────────────────────────┐    ┌───────────────────────────────────┐  │
│  │     buffer.rs          │    │   accountant.rs           │    │      prompt_builder.rs            │  │
│  │ • Role / ChatMessage   │    │ • estimate_tokens()       │    │ • assemble_system_prompt()        │  │
│  │ • ConversationContext  │    │ • context_utilization()   │    │ • build_session_history_xml()     │  │
│  │ • Sliding FIFO queue   │    │ • Soft  65% / Crit 85%   │    │ • format_relative_timestamp()     │  │
│  │ • KV-cache sync index  │    │ • perform_fifo_maint.     │    │ • format_user_profile_context()   │  │
│  │ • push/pop/duplicate?  │    │ • NARRATIVE_CHAIN 5%      │    │ • <persona> + [Identity] +        │  │
│  └────────────────────────┘    └─────────────┬─────────────┘    │   <user_profile> + [Summary]      │  │
│                                              │                  └───────────────────────────────────┘  │
│                                    harness/mod.rs — ConversationManager (thin) + prepare_turn_context │
└──────────────────────────────────────────────┼─────────────────────────────────────────────────────────┘
                         ┌──────────────────────┴──────────────────────┐
                         ▼                                             ▼
┌───────────────────────────────────────────────┐ ┌──────────────────────────────────────────────────────┐
│ 🔍 PILLAR 2: RETRIEVAL                        │ │ 📦 PILLAR 3: COMPACTION                              │
│ services/memory/retrieval/                    │ │ services/memory/compaction/                          │
│ 1. scope.rs  — route_scope(MemoryScope)       │ │ 1. Triggered at 65% (Soft, background) or           │
│ 2. ml/scope_classifier.rs — classify_scope()  │ │    85% (Critical, blocking + filler TTS)            │
│ 3. ml/embedder.rs — generate_embedding()      │ │ 2. prompt.rs — COMPACTION_SYSTEM_PROMPT +            │
│ 4. search.rs — hybrid Turso search:           │ │    build_compaction_request()                         │
│    • SQL Directives & Narrative seeds         │ │ 3. runner.rs — run_compaction() async LLM →          │
│    • Vector cosine + BFS graph expansion      │ │    CompactionResult { context_summary,                │
│ 5. Returns structured RetrievedProfile        │ │      personal_memory, diff_to_enqueue }              │
│    (prompt_builder renders XML)               │ │ 4. Diff enqueued to ingestion via mutations          │
└───────────────────────────────────────────────┘ └──────────────────────────┬───────────────────────────┘
                                                                              │ HashMap<String, Vec<String>>
                                                                              ▼
                                                   ┌──────────────────────────────────────────────────────┐
                                                   │ 💾 PILLAR 4: INGESTION (formerly pipeline/)        │
                                                   │ services/memory/ingestion/  — offline, DB-backed    │
                                                   │ • stage1_dedup.rs  — Jaccard 1.0 + priority (128)  │
                                                   │ • stage2_embed.rs  — MiniLM + soft dedup 0.95 (16) │
                                                   │ • stage3_eval.rs   — NLI 0.85 + Edge 0.80 (16)     │
                                                   │ • stage4_commit.rs — atomic facts/vectors/edges (32)│
                                                   │ • runner.rs / batch_result.rs / metrics.rs          │
                                                   └──────────────────────────────────────────────────────┘
                              ┌──────────────────────────────────────────────────────┐
                              │  ml/ — flat ONNX primitives (shared)                │
                              │  ml/embedder.rs  ml/tokenizer.rs  ml/nli.rs         │
                              │  ml/edge_classifier.rs  ml/scope_classifier.rs      │
                              │  ml/mod.rs — lifecycle + heap-trim                  │
                              └──────────────────────────────────────────────────────┘
                              ┌──────────────────────────────────────────────────────┐
                              │  persistence/ — storage trait boundary (own dir)    │
                              │  persistence/{db,queries,mutations,memory_worker}.rs │
                              │  accessed only via MemoryStore trait (see §14)      │
                              └──────────────────────────────────────────────────────┘
```

### II.5 Directory Layout & File Responsibilities (Target, Post-Refactor)

```
app/src-tauri/src/services/memory/
├── mod.rs                        # Public re-exports, all threshold/batch constants, MemoryError
│                                 # (CONTEXT_SOFT_THRESHOLD 0.65, CONTEXT_CRITICAL_THRESHOLD 0.85,
│                                 #  NARRATIVE_CHAIN_SOFT_CAP_SHARE 0.05, JACCARD 1.0, etc.)
│                                 # Does NOT own ONNX lifecycle (that moves to ml/mod.rs)
│
├── harness/                      # 🏛️ PILLAR 1: HARNESS (The Governor) — Q1 agreed split
│   ├── mod.rs                    # ConversationManager (thin) + prepare_turn_context() facade
│   │                             # + opportunistic Atomics (try_trigger/commit/cancel/on_speech_start)
│   ├── buffer.rs                 # Role, ChatMessage, ConversationContext, sliding FIFO, kv_synced_index
│   │                             # push_user_turn / push_assistant_turn / pop_last_user_turn / is_duplicate
│   ├── accountant.rs             # estimate_tokens (via ml/tokenizer), context_utilization(),
│   │                             # needs_threshold_maintenance(), perform_fifo_maintenance(),
│   │                             # build_narrative_context_chain() (soft cap 5%)
│   └── prompt_builder.rs         # assemble_system_prompt() + build_session_history_xml()
│                                 # + consolidate_system_message() + format_relative_timestamp()
│                                 # + format_user_profile_context()  — SOLE <user_profile> producer (Q4)
│
├── retrieval/                    # 🔍 PILLAR 2: RETRIEVAL (Search, no formatting)
│   ├── mod.rs                    # retrieve_turn_profile() facade → RetrievedProfile
│   ├── search.rs                 # collect_sql_sections + collect_vector_graph_sections + BFS
│   │                             # hybrid Turso queries (no XML)
│   └── scope.rs                  # route_scope(MemoryScope) — 4-variant pruning matrix (renamed from scope_router.rs)
│
├── compaction/                   # 📦 PILLAR 3: COMPACTION — Q2 agreed (replaces misnamed ingestion.rs)
│   ├── mod.rs                    # re-exports run_compaction + CompactionResult
│   ├── prompt.rs                 # COMPACTION_SYSTEM_PROMPT (moved from core/constants.rs:101) + build_compaction_request
│   └── runner.rs                 # run_compaction() async (LLM generate + parse_compaction_json, 2 attempts)
│                                 # + execute_compaction_attempt (now async, no Handle::try_current shim)
│
├── ingestion/                    # 💾 PILLAR 4: INGESTION (renamed from pipeline/) — offline background
│   ├── mod.rs                    # re-exports runner + stages + batch_result + metrics
│   ├── runner.rs                 # run_pipeline_cycle / drain_pipeline_queue / recover_stuck_pipeline_jobs
│   ├── stage1_dedup.rs           # Jaccard 1.0 + 5-collection priority, batch ceiling 128, EMPTY→DELETE+AUDIT
│   │                             # (jaccard_similarity inlined here per Q5; deduplication.rs deleted)
│   ├── stage2_embed.rs           # MiniLM 384 + cross-collection soft dedup cos≥0.95, batch 16
│   ├── stage3_eval.rs            # NLI (DeBERTa 0.85) + ModernBERT edge (0.80) concurrent via spawn_blocking+join, batch 16
│   ├── stage4_commit.rs          # atomic INSERT memory_facts/vectors/relations + status flip + DELETE queue, batch 32
│   ├── batch_result.rs           # RelationEdge, DedupAuditLog, CandidateAuditLog, BatchEvaluationResult
│   └── metrics.rs                # PipelineStageMetrics
│
└── ml/                           # Shared flat ONNX ML primitives — Q5 agreed
    ├── mod.rs                    # re-exports + lifecycle: ensure_*_loaded / is_*_loaded / unload_* + heap trim
    ├── embedder.rs               # TextEmbedder (MiniLM-L12 384, fallback bge-m3 1024) + generate_embedding
    ├── tokenizer.rs              # estimate_tokens via tiktoken cl100k_base (moved from root)
    ├── nli.rs                    # NliEngine (DeBERTa-v3-base) — rename of intra_edge_classifier.rs
    ├── edge_classifier.rs        # EdgeClassifierEngine (ModernBERT) — rename of inter_edge_classifier.rs
    └── scope_classifier.rs       # QueryScopeClassifier (ModernBERT) — rename of query_classifier.rs
```

**Counts:** 21 live files → **24 target files** (net +3 from splitting `working_memory.rs` 1→4, minus 1 `deduplication.rs` deletion, plus `compaction/prompt.rs` extraction). Every live file is accounted for in §6.

### II.6 Current → Target File Mapping (Exhaustive — every live file accounted for)

| # | Live file (with LoC + primary export) | Target file(s) | Action | Notes |
|---|----------------------------------------|----------------|--------|-------|
| 1 | `services/memory/mod.rs:1` (128, constants + `unload_*`) | `services/memory/mod.rs` + `services/memory/ml/mod.rs` | **Split** | Constants stay in root `mod.rs`; `unload_memory_pipeline_onnx_models` / `unload_all_onnx_models` / `trim_heap` move to `ml/mod.rs` |
| 2 | `services/memory/working_memory.rs:1` (738, `ConversationManager`) | `harness/buffer.rs` + `harness/accountant.rs` + `harness/prompt_builder.rs` + `harness/mod.rs` | **Split 1→4** | Q1 agreed: buffer (FIFO/Role/ChatMessage/Context), accountant (tokens/thresholds/FIFO), prompt_builder (assembly + history XML + `format_relative_timestamp`/`format_user_profile_context` from `formatter.rs`), mod (thin `ConversationManager` + `prepare_turn_context` + opportunistic Atomics) |
| 3 | `services/memory/ingestion.rs:1` (192, `run_compaction`) | `compaction/runner.rs` + `compaction/prompt.rs` + `compaction/mod.rs` | **Move + split + rename** | Misnamed file becomes `compaction/` (Q2). `COMPACTION_SYSTEM_PROMPT` migrates from `core/constants.rs:101` → `compaction/prompt.rs`; `execute_compaction_attempt` becomes `async` |
| 4 | `services/memory/retrieval.rs:1` (245, `retrieve_personal_context`) | `retrieval/search.rs` + `retrieval/mod.rs` | **Move + rename** | `collect_sql_sections` + `collect_vector_graph_sections` + `retrieve_personal_context` → `search.rs`; `mod.rs` becomes thin facade returning `RetrievedProfile` (not `String` XML) |
| 5 | `services/memory/scope_router.rs:1` (108, `route_scope`) | `retrieval/scope.rs` | **Move + rename** | Pure rename (`scope_router` → `scope`); matrix unchanged (see §10) |
| 6 | `services/memory/formatter.rs:1` (184, `format_relative_timestamp`) | `harness/prompt_builder.rs` | **Move (absorbed)** | Q4 agreed: sole `<user_profile>` producer is prompt_builder; `format_relative_timestamp` + `format_user_profile_context` migrate there, `formatter.rs` deleted |
| 7 | `services/memory/deduplication.rs:1` (81, `jaccard_similarity`) | `ingestion/stage1_dedup.rs` (inlined) | **Delete + inline** | Q5 agreed: `jaccard_similarity` inlined at top of `stage1_dedup.rs`, file deleted |
| 8 | `services/memory/embedder.rs:1` (288, `TextEmbedder`) | `ml/embedder.rs` | **Move** | No logic change; `EMBEDDING_DIM 384`, `PRIMARY_MODEL_DIR "minilm-l12-v2"` stay |
| 9 | `services/memory/tokenizer.rs:1` (36, `estimate_tokens`) | `ml/tokenizer.rs` | **Move** | No logic change; `tiktoken cl100k_base` + Devanagari 3× heuristic preserved |
| 10 | `services/memory/classifiers/mod.rs:1` (16, re-exports) | `ml/mod.rs` | **Move + merge** | Absorbed into `ml/mod.rs` alongside embedder/tokenizer/nli/edge/scope re-exports |
| 11 | `services/memory/classifiers/query_classifier.rs:1` (118) | `ml/scope_classifier.rs` | **Move + rename** | `QueryScopeClassifier` → `ScopeClassifier` file name only; `classify_scope` behavior unchanged (`tau*=0.81` → `Domain` fallback) |
| 12 | `services/memory/classifiers/intra_edge_classifier.rs:1` (406, `NliEngine`) | `ml/nli.rs` | **Move + rename** | `NliEngine` + `classify_batch` + calibration unchanged; thresholds `NLI_CONTRADICTION/ENTAILMENT_THRESHOLD 0.85` stay in root `mod.rs` |
| 13 | `services/memory/classifiers/inter_edge_classifier.rs:1` (254, `EdgeClassifierEngine`) | `ml/edge_classifier.rs` | **Move + rename** | `EdgeClassifierEngine` + `classify_edge` unchanged; `EDGE_CLASSIFIER_THRESHOLD 0.80` stays |
| 14 | `services/memory/pipeline/mod.rs:1` (18) | `ingestion/mod.rs` | **Move + rename** | `pipeline/` → `ingestion/` (Q2) |
| 15 | `services/memory/pipeline/runner.rs:1` (148) | `ingestion/runner.rs` | **Move** | `run_pipeline_cycle` / `drain_pipeline_queue` / `recover_stuck_pipeline_jobs` unchanged |
| 16 | `services/memory/pipeline/stage1_dedup.rs:1` (375) | `ingestion/stage1_dedup.rs` | **Move** | + inlined `jaccard_similarity`; `STAGE1_BATCH_CEILING 128`, `JACCARD_EXACT_MATCH_THRESHOLD 1.0` |
| 17 | `services/memory/pipeline/stage2_embed.rs:1` (242) | `ingestion/stage2_embed.rs` | **Move** | `STAGE2_BATCH_SIZE 16`, `SOFT_VECTOR_DEDUP_THRESHOLD 0.95`, `EMBEDDING_DIM 384` |
| 18 | `services/memory/pipeline/stage3_eval.rs:1` (475) | `ingestion/stage3_eval.rs` | **Move** | `STAGE3_BATCH_SIZE 16`, `SAME_COLLECTION_CANDIDATE_SEARCH 0.60`, `INTER_COLLECTION_CANDIDATE_SEARCH 0.40`, `SUBFLOOR_CANDIDATE_FLOOR 0.25`; sub-branches `NLI` + `ModernBERT` via `spawn_blocking`+`join` |
| 19 | `services/memory/pipeline/stage4_commit.rs:1` (230) | `ingestion/stage4_commit.rs` | **Move** | `STAGE4_BATCH_SIZE 32`, `BEGIN/COMMIT` atomic |
| 20 | `services/memory/pipeline/batch_result.rs:1` (48) | `ingestion/batch_result.rs` | **Move** | `RelationEdge`, `DedupAuditLog`, `CandidateAuditLog`, `BatchEvaluationResult` unchanged |
| 21 | `services/memory/pipeline/metrics.rs:1` (13) | `ingestion/metrics.rs` | **Move** | `PipelineStageMetrics` unchanged |

**Deleted files:** `services/memory/deduplication.rs` (inlined), `services/memory/formatter.rs` (absorbed into `harness/prompt_builder.rs`), `services/memory/classifiers/` dir (flattened into `ml/`).  
**Net:** 21 → 24 files (4 harness + 3 retrieval + 3 compaction + 8 ingestion + 6 ml + root mod). All `app/src-tauri/src/persistence/*` files stay in `persistence/` and are accessed via the trait in §14, not moved.

### II.7 Public Facade Contract (the only way the pipeline talks to memory)

The entire memory subsystem is consumed via one async function. This is the sole entrypoint; everything else is `pub(crate)`.

```rust
// services/memory/harness/mod.rs
pub async fn prepare_turn_context(
    harness: &Arc<Mutex<ConversationManager>>,
    conn: Option<&Connection>,       // Some(&Connection) if memory enabled, else None
    query: &str,                     // post-STT, post-transliteration, trimmed
    turn_id: u32,
    session_id: &str,                // caller-owned identity, e.g. conv_id.to_string()
    memory: &MemorySettings,         // &MemorySettings, not &VoxSettings (domain-agnostic)
    context_window: usize,           // LlmSettings::context_window as usize
    provider_kind: ProviderKind,     // caller-derived from LlmSettings::active
) -> Result<(GenerationRequest, Option<String>), MemoryError>
```

**What it does (must happen in this order, must be observable in tests):**
1. If `!memory.context_retrieval_enabled` or `conn.is_none()` or `query.trim().is_empty()` → skip to step 4 with `retrieved_profile = None`.
2. `let scope = ml::scope_classifier::classify_scope(query)` — `tau* = 0.81` below-threshold → `Domain`; missing model → `Domain` (today `classifiers/query_classifier.rs:100`).
3. If `scope == ChitChat` → `retrieved_profile = None`; else `let embedding = ml::embedder::generate_embedding(query)` — `None` or error → `retrieved_profile = None`; else `retrieval::search::retrieve_turn_profile(conn.unwrap(), &embedding, scope, memory, context_window)` → structured `RetrievedProfile`. This is the scope-pruned waterfall (see §10).
4. Acquire `harness.lock()` for a short critical section: `prompt_builder::update_dynamic_user_profile(harness, retrieved_profile)` → `buffer::push_user_turn(harness, query)` → `(conv_ctx, filler, diff_to_enqueue) = accountant::build_context(harness, provider_kind, is_devanagari(query))`. The `is_devanagari` check uses `services/translit::is_devanagari(query)` (caller may precompute and pass `bool` to avoid double detection).
5. If `diff_to_enqueue` non-empty and `conn.is_some()` and `memory.pipeline_processing_enabled` → `persistence::mutations::enqueue_personal_facts(conn.unwrap(), diff_to_enqueue, session_id, true).await` — fire-and-forget is allowed but the spec recommends `await` on `vox-memory-worker`; the call must not block TTS `filler` dispatch.
6. Assemble `GenerationRequest { input: ConversationInput { messages: conv_ctx.messages }, options: GenerationOptions { temperature, max_output_tokens, .. }, output: Text, purpose: Conversation }` where `messages` already contains `base_system_prompt + [Identity] + <user_profile> + [Summary]` via `prompt_builder::assemble_system_prompt`.
7. Return `(request, filler)` where `filler` is `Some(String)` only when the **critical** path (≥85%) triggered and produced a transition phrase (see §9); soft-path filler is always `None`.

**What it does NOT do:** Open DB via `paths::db_path()`, read `VoxSettings`, touch `services/tts`, or emit `VoxEvent`. All I/O beyond the passed `&Connection` is forbidden.

**Domain-agnostic guarantee:** `modular/passive.rs:283` `build_generation_request`, `realtime/passive.rs:283` `on_transcript_final`, `realtime/ptt.rs`, `dictation`, and any future `tools/retrieve_memory` all call this function with their own `query` and `session_id`. Where the query came from must not affect which collections are retrieved — only `query` text and `memory` settings do.

### II.8 Prompt Template & Relative Timestamp Contract

#### The complete message array sent to the LLM (after `prepare_turn_context`)

```
MESSAGE 0 — role: system
  <persona>  — from PersonaSettings (SYSTEM_PROMPT_MODULAR or REALTIME)
  <internal_rules> — spoken, no markdown, one idea per sentence
  [Identity] — static evergreen facts, preloaded via load_identity_into_system_prompt
  <user_profile> — dynamic per-turn facts, produced SOLELY by harness/prompt_builder.rs
    [Directives & Constraints]
    - (2 hours ago) Prefer Rust examples and concise explanations.   ← relative timestamp prefix
    [Active Tasks & Goals]
    [User Context & Knowledge]
    - (Yesterday) Configured remote GPU server for Ollama LLM.
  </user_profile>
MESSAGE 1 — role: system — OPTIONAL, present only after compaction
  [Summary of prior context: <narrative_chain> ... ]  — capped at NARRATIVE_CHAIN_SOFT_CAP_SHARE 5% of context_window
MESSAGES 2..N — sliding FIFO window (most recent turns, oldest pruned first)
  user: "<Current User Utterance Text>"   ← the query just pushed
  assistant: "<prior response>"
  ...
```

**Relative timestamping (`harness/prompt_builder.rs::format_relative_timestamp`):**

```
pub fn format_relative_timestamp(created_at_ms: i64) -> String
  diff = now_ms - created_at_ms
  diff < 0              → "Just now"
  diff < 60_000         → "Just now"
  diff < 3_600_000      → "{m} minute(s) ago"
  diff < 86_400_000     → "{h} hour(s) ago"
  diff == 86_400_000    → "Yesterday"
  diff < 7*86_400_000   → "{d} days ago"
  diff < 30*86_400_000  → "{w} week(s) ago"
  else                  → "{d} days ago"
```

Every temporal fact rendered inside `<user_profile>` carries this prefix in parentheses immediately before the fact text. Facts without a valid `created_at` render without a prefix (must not render `"(0 days ago)"`).

**No double XML wrapping:** `<user_profile>` tags are produced exclusively by `harness/prompt_builder.rs::format_user_profile_context`. `retrieval/search.rs` never emits `<user_profile>`; `working_memory.rs:122`'s old strip-and-rewrap (`trimmed[14..len-15]`) is deleted.

### II.9 Token Governance & Compaction State Machine

```
util = total_token_count / (context_window - RESERVED_GENERATION_TOKENS)   // RESERVED 512
```

- **Nominal: util < 0.65 (65%)** — normal turn, zero compaction overhead.
- **Soft: 0.65 ≤ util < 0.85** — opportunistic background compaction on `vox-memory-worker`:
  - `harness::try_trigger_opportunistic()` clones `messages` + `Arc<AtomicBool>` cancel flag if `messages.len() > 3` and no opportunistic is active (`working_memory.rs:592` today).
  - `compaction::runner::run_compaction(messages[1..len-1], settings).await` runs off the hot path.
  - On next `prepare_turn_context` or `on_speech_start`, `cancel_opportunistic()` sets `AtomicBool(true)`; `commit_opportunistic(snapshot_len, summary)` verifies `snapshot_len == current.len()` and `!cancelled` before `messages = [system_prompt, summary_msg, last_user_turn]` and `total_token_count` recompute. Zero latency added to the active turn.
- **Critical: util ≥ 0.85 (85%)** — immediate maintenance inside `prepare_turn_context`:
  - Pick filler: `TRANSITION_MESSAGES_EN` or `TRANSITION_MESSAGES_HI` by `is_devanagari(query)` (today `working_memory.rs:439`).
  - If `provider_kind == Embedded && context_window ≤ 4096` or no LLM provider → `accountant::perform_fifo_maintenance()` (drop oldest `(User,Assistant)` pairs until `util ≤ 0.65`).
  - Else `compaction::runner::run_compaction(history_slice, settings).await` → `apply_compaction_result` → `messages = [system_prompt, last_user_turn]` + `session_compaction_contexts.push(context_summary)` + `latest_compaction_facts = personal_memory - {Context,Narrative}`.
  - On any `run_compaction` error → fall back to FIFO. Return `Some(filler)` so the pipeline can `tts_tx.send(Generate { turn_id, text: filler })` non-blockingly.

Constants (single source `services/memory/mod.rs`):

| Constant | Value | Source |
|---|---|---|
| `CONTEXT_SOFT_THRESHOLD` | `0.65` | `mod.rs:14` (Q6 locked) |
| `CONTEXT_CRITICAL_THRESHOLD` | `0.85` | `mod.rs:13` |
| `RESERVED_GENERATION_TOKENS` | `512` | `mod.rs:12` |
| `NARRATIVE_CHAIN_SOFT_CAP_SHARE` | `0.05` | `mod.rs:36` |
| `CONTEXT_MAX_CONTEXT_TOKENS` | caller-supplied `context_window` | `LlmSettings::context_window` |

### II.10 Retrieval Waterfall (Scope-Pruned, Budgeted, Graph-Expanded)

`retrieval/search.rs::retrieve_turn_profile(conn, query_embedding, scope, memory, context_window) -> RetrievedProfile`

1. **Scope pruning** — `retrieval/scope.rs::route_scope(scope)` (today `scope_router.rs:11`):

| Scope | `sql_collections` | `vector_collections` |
|---|---|---|
| `ChitChat` | _(empty)_ | _(empty)_ |
| `User` | _(empty)_ | `Profile`, `Constraints` |
| `Domain` | _(empty)_ | `Entities`, `Directives`, `Constraints` |
| `Temporal` | `Directives`, `Narrative` | `Constraints` |

Invariants: `Identity` never in `sql_collections` (preloaded at boot via `load_identity_into_system_prompt`); `Narrative` only in `Temporal` SQL; `Directives` in both `Domain` vector and `Temporal` SQL (different access patterns).

2. **Budget:** `total_budget = (context_window as f32 * memory.max_context_share) as usize` where `max_context_share` default `0.15` (today `core/settings.rs:739` `MemorySettings::max_context_share`). `remaining_budget` decremented per section.

3. **SQL branch — seeds (no vectors):** `queries::fetch_narrative_history(conn, 3)` + `queries::fetch_latest_directives(conn, 5)` (today `retrieval.rs:35/61`), ordered `created_at DESC`, added until `remaining_budget` exhausted. Rendered later with relative timestamps by `harness/prompt_builder.rs`.

4. **Vector branch — seeds + BFS:** `queries::fetch_inter_collection_candidates(conn, &target_collections, query_embedding, memory.semantic_similarity_cutoff, None)` with `semantic_similarity_cutoff` default `0.40` (`mod.rs:20` `INTER_COLLECTION_CANDIDATE_SEARCH` is the stage-3 value; retrieval uses the `MemorySettings` value). No `K` cap; `target_collections` from `vector_collections`. Each seed reserves `parent_quota = max(30, remaining_budget / seed_count)` tokens.

5. **BFS expansion:** `max_hops = memory.max_hops.min(2)` (default `2`). Frontier = seed IDs. Per hop: `queries::fetch_graph_neighbors(conn, &frontier)` (bidirectional `from_id IN … OR to_id IN …`), collect unvisited child IDs, `queries::fetch_facts_by_ids(conn, &child_ids)`, render as `"  ↳ --[relation]--> [collection] fact"` until `remaining_budget < 20` or frontier empty. This is the user-visible graph; citations must be omitted (no `fact_id` leaks).

6. **Return:** Structured `RetrievedProfile { sql_sections: Vec<MemoryFact>, vector_seeds: Vec<ScoredFact>, graph_children: Vec<GraphEdge> }` — NOT a `String`. The harness formats it.

### II.11 Compaction

`compaction/runner.rs::run_compaction(history_messages: &[ChatMessage], settings: &LlmSettings) -> Result<CompactionResult, MemoryError>`

- Input: `history_messages = messages[1..]` (exclude system prompt), require `len > 0` (`ingestion.rs:128`).
- Prompt: `compaction/prompt.rs::build_compaction_request` wraps `history_text` (`"role: content\n\n"`) in `<conversation_history>` + `<task>` + `COMPACTION_SYSTEM_PROMPT` (`core/constants.rs:101` moved), producing a `GenerationRequest { purpose: MemoryCompaction, input: [System(COMPACTION_SYSTEM_PROMPT), User(task)], options: { temperature: settings.compaction_temperature, … } }`.
- Execution: `provider.generate(request, COMPACTION_SENTINEL_TURN_ID 999_999, &cancel_flag, &tx).await` (now `async`, no `Handle::try_current` shim). Pump `VoxEvent::LlmToken` → `summary_content: String` with `45s` timeout.
- Parsing: `utils::json::parse_compaction_json(&summary_content)` → `HashMap<String, Vec<String>>` with 6 keys (`Identity`, `Directives`, `Narrative` is `Vec` with 0-1 element, etc.). Retry up to **2 attempts** on parse failure (`ingestion.rs:143`).
- Output: `CompactionResult { context_summary: final_summary, personal_memory, diff_to_enqueue: personal_memory.clone() }` where `final_summary = personal_memory["Narrative"].first().or(personal_memory["Context"].first()).or(summary_content)`. Empty `final_summary` → `Err`. `diff_to_enqueue == personal_memory` (no delta — ingestion stages own dedup).

### II.12 Ingestion Pipeline (Offline 4-Stage Batch, formerly `pipeline/`)

DB: `personal_memory_queue` (ephemeral staging) → `memory_facts` + `memory_facts_vectors` + `memory_relations` (permanent). All stages claim atomically via `UPDATE … WHERE status = ?` + `claimed_at`.

| Stage | Input → Output statuses | Batch | Core logic & thresholds | File |
|---|---|---|---|---|
| **1 Dedup** | `staged_pending` → `deduped` / `superseded` (or `DELETE` for empty) | `STAGE1_BATCH_CEILING 128` | Jaccard `1.0` (`mod.rs:17` `JACCARD_EXACT_MATCH_THRESHOLD`) across 5 factual collections (`Identity:6 > Constraints:5 > Directives:4 > Profile:3 > Entities:2` priority). Empty → `DELETE` + `DedupAuditLog{action:"empty_fact_deleted"}`; duplicate & `incoming_prio ≤ existing_prio` → incoming `superseded`; else existing `memory_facts` row → `superseded` and incoming `deduped`. In-flight `personal_memory_queue` rows in `deduped/embedded/evaluated/processing_*` also considered. | `ingestion/stage1_dedup.rs` |
| **2 Embed** | `deduped` → `embedded` / `superseded` / `failed` | `STAGE2_BATCH_SIZE 16` | `ml/embedder::generate_embedding` (MiniLM-L12 384, `mod.rs:38`); `Narrative` short-circuits to `embedded` with no vector; cross-collection `fetch_cross_collection_candidates` cos ≥ `SOFT_VECTOR_DEDUP_THRESHOLD 0.95` (`mod.rs:18`); priority resolution writes `SUPERSEDES` edge (`match_id → item_X`) on drop | `ingestion/stage2_embed.rs` |
| **3 Eval** | `embedded` → `evaluated` / `superseded` | `STAGE3_BATCH_SIZE 16` | Two sub-branches **concurrently** (`spawn_blocking` + `join!`): **A NLI** — intra-collection `fetch_intra_collection_candidates` cos ≥ `SAME_COLLECTION_CANDIDATE_SEARCH 0.60` (`mod.rs:19`) for `Identity/Directives/Constraints` only; **B Edge** — inter-collection `fetch_inter_collection_candidates` cos ≥ `INTER_COLLECTION_CANDIDATE_SEARCH 0.40` (`mod.rs:20`) for `has_inter_collection_relationship(col1,col2)` pairs (7 sanctioned, `core/constants.rs:271`). Thresholds: NLI `CONTRADICTION/ENTAILMENT_THRESHOLD 0.85`, contradiction also requires `contradiction - neutral ≥ 0.20`; Edge `EDGE_CLASSIFIER_THRESHOLD 0.80`. Writes forward+inverse edges into `relations_json`; `is_superseded` if any relation is `SUPERSEDES` where `to_id == item_X`. | `ingestion/stage3_eval.rs` |
| **4 Commit** | `evaluated` / `superseded` → _(deleted from queue)_ | `STAGE4_BATCH_SIZE 32` | Single `BEGIN/COMMIT` tx: `INSERT memory_facts (id=mem_{ts}_{uuid}, status active/superseded)`, `INSERT memory_facts_vectors` if vector present, `INSERT OR IGNORE memory_relations` per `relations_json`, `UPDATE memory_facts SET status='inactive' WHERE id=to_id` for any `SUPERSEDES` relation, `DELETE FROM personal_memory_queue WHERE id IN (…)`; `ROLLBACK` on error | `ingestion/stage4_commit.rs` |

Queue lifecycle: `staged_pending → processing_dedup → deduped → processing_embed → embedded → processing_eval → evaluated → processing_commit → (deleted)`; `superseded` and `failed` (retry ≥3) are terminal; `paused` is used when `pipeline_processing_enabled == false`. `persistence/mutations.rs:53` `mark_job_failed` is `CASE WHEN retry_count+1 ≥ 3 THEN 'failed' ELSE 'staged_pending' END`.

Runner: `ingestion/runner.rs::run_pipeline_cycle(conn, cancel_flag)` runs stages 1→4 sequentially, respecting `cancel_flag` between stages; `drain_pipeline_queue` loops until `cycle_processed == 0`; `recover_stuck_pipeline_jobs` resets `status LIKE 'processing_%'` → `staged_pending` on boot.

### II.13 ML Primitives (flat `ml/`)

All ONNX singletons use `parking_lot::RwLock<Option<T>>` + `unload_*()` → `*lock = None` → `trim_heap` (malloc_trim / EmptyWorkingSet). Zero idle RAM via 30s `vox-memory-worker` idle check that skips load when `personal_memory_queue` has 0 pending rows (see `docs/features/memory-architecture.md:16`).

| Primitive | File | Model | Dim | Threshold |
|---|---|---|---|---|
| Scope classifier | `ml/scope_classifier.rs` | ModernBERT `modernbert_memory_scope/model_quantized.onnx` (`mod.rs:49` `MEMORY_SCOPE_MODEL_DIR`) | — | `tau*=0.81` → `Domain` fallback |
| Embedder | `ml/embedder.rs` | MiniLM-L12 `minilm-l12-v2/model_int8.onnx` (`mod.rs:38`), fallback `bge-m3` 1024 | 384 (primary) | `SOFT 0.95`, `SAME 0.60`, `INTER 0.40` |
| NLI | `ml/nli.rs` | DeBERTa `nli-deberta-v3-base/model_quantized.onnx` (`mod.rs:43`) | — | `contradiction/entailment 0.85`, margin `0.20` |
| Edge classifier | `ml/edge_classifier.rs` | ModernBERT `classifier/modernbert_edge_creation/model_quantized.onnx` (`mod.rs:46`) | — | `0.80` |
| Tokenizer | `ml/tokenizer.rs` | `tiktoken cl100k_base` | — | — |

### II.14 Persistence Trait Boundary (own dir, Q7)

`persistence/{db,queries,mutations,memory_worker}.rs` stays in `persistence/` and is not moved. `services/memory/*` depends on it only via a trait-like `Connection` surface (`queries::fetch_*`, `mutations::{enqueue_personal_facts, mark_job_failed, record_stage_metrics, write_*_audit}`, `db::VoxDb::open`). No `services/memory` file imports `crate::utils::paths::db_path` — that stays in the pipeline domain (the caller). This keeps the memory crate unit-testable with an in-memory Turso `Connection` and satisfies "persistence has its own dir."

### II.15 Must Be True (numbered, verifiable regardless of language)

1. `prepare_turn_context` is the only public async entrypoint of `services/memory`; all other `pub` items in `services/memory` are `pub(crate)`.
2. For any `query` and any `MemoryScope`, the set of collections searched is exactly the 4-row matrix in §10 (row = scope, column = SQL/vector).
3. `ChitChat` scope produces no SQL and no vector search.
4. `Identity` is never in `sql_collections` for any scope.
5. `total_budget = floor(context_window * max_context_share)`; no stage exceeds this budget.
6. `context_utilization = total_token_count / (context_window - 512)`; Soft is `<0.65 → [0.65,0.85) → ≥0.85` with no gaps.
7. Every fact rendered inside `<user_profile>` that has a `created_at_ms` is prefixed with `"(<relative>)"` produced by `format_relative_timestamp` (§8).
8. `<user_profile>` is produced in exactly one place: `harness/prompt_builder.rs`.
9. `diff_to_enqueue` is a clone of `personal_memory` (no silent filtering).
10. Stage 1 batch ceiling is 128, Stage 2 batch is 16, Stage 3 batch is 16, Stage 4 batch is 32.
11. Stage 1 empty fact → `DELETE` + `DedupAuditLog { action: "empty_fact_deleted" }`.
12. Stage 2 `Narrative` collection never gets a vector; it goes `deduped → embedded` with `vector = NULL`.
13. Stage 3 always runs NLI and Edge sub-branches concurrently (`spawn_blocking` + `join!`) and merges into one `UPDATE … relations_json`.
14. Stage 4 is a single `BEGIN/COMMIT` transaction; any `SUPERSEDES` relation flips the target `memory_facts` row to `inactive` before the queue row is deleted.
15. `COMPACTION_SYSTEM_PROMPT` lives in `compaction/prompt.rs` (not `core/constants.rs`).
16. `process_pipeline_cycle` recovers `processing_%` → `staged_pending` on every cycle.
17. No file in `services/pipeline/*` imports `services/memory/retrieval`, `services/memory/ml`, `persistence::queries`, or `services/memory/ingestion`.

### II.16 Must Not Happen

- No `services/pipeline/*` file opens the DB, classifies scope, embeds, or queries vectors directly.
- No double `<user_profile>` wrapping (inner strip `trimmed[14..len-15]` is deleted).
- No `Identity` in any `WHERE collection = 'Identity'` vector search.
- No `K` cap on Stage 3 candidate selection (pure `cos ≥ threshold`).
- No `JACCARD` or `COSINE_HARD_MATCH` threshold change without updating both `mod.rs` and this spec.

### II.17 Out of Scope

- The Turso schema DDL (`memory_facts`, `memory_facts_vectors`, `memory_relations`, `personal_memory_queue`, `memory_pipeline_metrics`) — governed by `persistence/db.rs` and `docs/features/memory-architecture.md:12`.
- The 3D Cognitive Memory Graph frontend (`Memory.tsx`) and IPC (`ipc/memory/*`) — governed by `docs/features/memory-architecture.md:15`.
- Chatterbox/STT/VAD engines — governed by their own specs.
- The exact `parse_compaction_json` error messages — governed by `utils/json.rs`.

### II.18 Verification Invariants (how to tell the refactor is done)

- `cargo check --all-targets` and `cargo nextest run --release --test-threads=1` green (ingestion stages are CPU-bound; `--release` required).
- `rg "use crate::services::memory::(retrieval|embedder|scope_router|ingestion|pipeline)" app/src-tauri/src/services/pipeline` — 0 hits (only `harness::prepare_turn_context` is allowed).
- `rg "format_relative_timestamp" app/src-tauri/src/services/memory` — hits only in `harness/prompt_builder.rs` and `ml/` is 0.
- `rg "deduplication" app/src-tauri/src/services/memory` — 0 hits (inlined).
- `rg "CONTEXT_SOFT_THRESHOLD" app/src-tauri/src/services/memory/mod.rs` — `0.65`.
- `rg "COMPACTION_SYSTEM_PROMPT" app/src-tauri/src/core/constants.rs` — 0 hits (moved to `compaction/prompt.rs`).
- File count: `find app/src-tauri/src/services/memory -name "*.rs" | wc -l` == 24 and matches §5 tree.

### II.19 Open Questions (resolved, frozen)

- Q1 split: agreed — `harness/buffer.rs` + `accountant.rs` + `prompt_builder.rs` + `mod.rs` thin facade.
- Q2 renames: agreed — `ingestion.rs` → `compaction/`, `pipeline/` → `ingestion/`.
- Q3 facade: agreed — `prepare_turn_context(harness, conn: Option<&Connection>, query, turn_id, session_id, memory, context_window, provider_kind)` as in §7, `async`, owns the full waterfall, persistence via trait.
- Q4 formatter: agreed — `harness/prompt_builder.rs` is sole `<user_profile>` producer.
- Q5 deduplication: agreed — `deduplication.rs` deleted, `jaccard_similarity` inlined into `ingestion/stage1_dedup.rs`.
- Q6 soft threshold: agreed — `0.65` (critical `0.85` unchanged).
- Q7 persistence boundary: agreed — `persistence/` stays own dir, accessed via trait.

---

*This spec is the SSOT for the memory refactor. Update it first when the live tree changes; re-verify Part I's inventory (`find … | sort`) and thresholds (`mod.rs:12-45`) on every bump.*
