# Memory Subsystem, Context Harness & Knowledge Architecture Specification

---

## 1. Executive Summary & Purpose

This specification defines the 4-pillar architecture of the Vox Memory subsystem, the unified **Context Harness** governor, the canonical XML prompt template, the relative timestamping contract, and the decoupled storage ingestion pipeline.

### Core Objectives:
1. **The 4-Pillar Architecture:** Deconstruct monolithic god-objects (`working_memory.rs`, misnamed `ingestion.rs`) into four distinct, single-responsibility pillars:
   - **🏛️ Pillar 1: Harness** (In-memory conversation window, token budget governance, and pipeline facade)
   - **🔍 Pillar 2: Retrieval** (Query classification, hybrid search, and relative timestamp XML formatting)
   - **📦 Pillar 3: Compaction** (Conversation history summarization and raw fact extraction)
   - **💾 Pillar 4: Ingestion** (Offline 4-stage batch pipeline: Dedup, Embed, Eval, SQLite Commit)
2. **Single Pipeline Facade:** The main voice pipeline domain files (`modular/passive.rs`, `realtime/passive.rs`, etc.) do not coordinate retrieval, embeddings, or DB queries. They call a single facade function: `prepare_turn_context()`.
3. **Canonical `<user_profile>` & Relative Timestamps:** Dynamic memory facts are formatted with humanized relative timestamps (`format_relative_timestamp`) and assembled into an unambiguous XML block. Static identity facts remain in the base system prompt.
4. **Token Budget & Compaction Thresholds:** Deterministic threshold governance (Soft: 70% $	o$ opportunistic background compaction; Critical: 85% $	o$ immediate compaction with filler speech fallback).
5. **Flat ML Primitives:** All local ONNX model runners live in a flat `ml/` module.

---

## 2. The 4 Canonical Pillars Architecture

```
                                  ┌──────────────────────────────┐
                                  │   Voice Pipeline Domain      │
                                  │   (e.g., modular/passive.rs) │
                                  └──────────────┬───────────────┘
                                                 │
                                                 │ 1. `prepare_turn_context(query, turn_id)`
                                                 ▼
┌────────────────────────────────────────────────────────────────────────────────────────────────────────┐
│ 🏛️ PILLAR 1: HARNESS (Context Governor & In-Memory Window)                                             │
│                                                                                                        │
│  ┌────────────────────────┐    ┌───────────────────────────┐    ┌───────────────────────────────────┐  │
│  │     window/buffer.rs   │    │   window/accountant.rs    │    │      window/prompt_builder.rs     │  │
│  │ • Sliding ChatMessage  │    │ • Token utilization       │    │ • Merges Persona + Identity +     │  │
│  │   FIFO queue           │    │ • Triggers Soft / Critical│    │   <user_profile> + Summary        │  │
│  │ • KV-cache sync index  │    │   compaction thresholds   │    │ • Generates GenerationRequest     │  │
│  └────────────────────────┘    └─────────────┬─────────────┘    └───────────────────────────────────┘  │
└──────────────────────────────────────────────┼─────────────────────────────────────────────────────────┘
                        ┌──────────────────────┴──────────────────────┐
                        ▼                                             ▼
┌───────────────────────────────────────────────┐ ┌──────────────────────────────────────────────────────┐
│ 🔍 PILLAR 2: RETRIEVAL                        │ │ 📦 PILLAR 3: COMPACTION                              │
│ (The Dynamic Context Pull)                    │ │ (Conversation History Compressor)                    │
│                                               │ │                                                      │
│ 1. Scope Classifier (ModernBERT)              │ │ 1. Triggered at 70% (Soft) or 85% (Critical)         │
│ 2. Dense Embedder (MiniLM)                    │ │ 2. Takes raw multi-turn messages from window         │
│ 3. Hybrid Search (Turso SQLite)               │ │ 3. LLM summarizes history -> Narrative Summary      │
│    • SQL Directives & Narrative               │ │ 4. LLM extracts candidate facts -> Personal Queue   │
│    • Vector Cosine + BFS Graph Expansion      │ │ 5. Critical returns filler audio to TTS              │
│ 4. Formatter: `format_relative_timestamp`     │ └──────────────────────────┬───────────────────────────┘
│ 5. Returns `<user_profile>` XML               │                            │
└───────────────────────────────────────────────┘                            │ Raw Candidate Facts
                                                                             ▼
                                                  ┌──────────────────────────────────────────────────────┐
                                                  │ 💾 PILLAR 4: INGESTION (Knowledge Storage Pipeline)  │
                                                  │ (Offline Background Worker)                          │
                                                  │                                                      │
                                                  │ • Stage 1: Exact & Jaccard Deduplication             │
                                                  │ • Stage 2: Dense Embedding (MiniLM)                  │
                                                  │ • Stage 3: NLI Contradiction & ModernBERT Edge Eval  │
                                                  │ • Stage 4: Atomic Commit to SQLite (facts, vectors)  │
                                                  └──────────────────────────────────────────────────────┘
```

---

## 3. Directory Layout & File Responsibilities

```
app/src-tauri/src/services/memory/
├── mod.rs                        # Public facade API, threshold constants, ONNX memory cleanup
│
├── harness/                      # 🏛️ PILLAR 1: HARNESS (The Governor)
│   ├── mod.rs                    # Public entrypoint: `prepare_turn_context(user_query, turn_id)`
│   ├── buffer.rs                 # In-memory sliding ChatMessage buffer & KV index
│   ├── accountant.rs             # Token budgeting & threshold evaluation (Nominal / Soft / Critical)
│   └── prompt_builder.rs         # Prompt template assembler (<persona>, [Identity], Summary)
│
├── retrieval/                    # 🔍 PILLAR 2: RETRIEVAL (Search & Temporal Formatting)
│   ├── mod.rs                    # `retrieve_turn_profile(query, budget) -> Option<String>`
│   ├── search.rs                 # Hybrid SQL directives + Vector cosine seeds + BFS graph expansion
│   ├── scope.rs                  # Scope classifier routing (Personal / ChitChat / Command)
│   └── formatter.rs              # XML formatter with `format_relative_timestamp`
│
├── compaction/                   # 📦 PILLAR 3: COMPACTION (Compressor & Fact Extractor)
│   ├── mod.rs                    # `compact_session_history(messages)`
│   ├── prompt.rs                 # Compaction prompt builder
│   ├── runner.rs                 # LLM summarization & fact extraction executor (replaces old ingestion.rs)
│   └── opportunistic.rs          # Background cancellation atomics & state reconciliation
│
├── ingestion/                    # 💾 PILLAR 4: INGESTION (Knowledge Storage - formerly "pipeline/")
│   ├── mod.rs                    # Batch pipeline runner & queue manager
│   ├── stage1_dedup.rs           # Exact string & Jaccard deduplication
│   ├── stage2_embed.rs           # Batch dense embedding generation
│   ├── stage3_eval.rs            # DeBERTa NLI contradiction & ModernBERT edge evaluation
│   ├── stage4_commit.rs          # Atomic Turso SQLite commit (facts, vectors, edges)
│   ├── batch_result.rs           # Batch processing result structs
│   └── metrics.rs                # Ingestion performance telemetry
│
└── ml/                           # Shared Flat Local ONNX ML Primitives
    ├── mod.rs                    # Model lifecycle & VRAM/RAM cleanup
    ├── embedder.rs               # MiniLM 384-dim ONNX embedder
    ├── tokenizer.rs              # Token estimator
    ├── nli.rs                    # DeBERTa v3 NLI contradiction engine
    ├── edge_classifier.rs        # ModernBERT Graph Edge classifier
    └── scope_classifier.rs       # ModernBERT Query Scope classifier
```

---

## 4. Prompt Template Architecture & Relative Timestamps

### 4.1 The Complete Message Array Sent to LLM

```
┌────────────────────────────────────────────────────────────────────────────────────────┐
│                          COMPLETE LLM PROMPT ARCHITECTURE                              │
├────────────────────────────────────────────────────────────────────────────────────────┤
│ MESSAGE 0: ROLE "system"                                                               │
│ ├── <persona>                                                                          │
│ │   You're Vox. Quick, sharp, and you get things done. Spoken response rules...        │
│ │   </persona>                                                                         │
│ ├── <internal_rules>                                                                   │
│ │   - Everything is spoken aloud. Short sentences. No markdown formatting.             │
│ │   </internal_rules>                                                                  │
│ ├── [Identity] (Static User Persona — Evergreen)                                       │
│ │   - User's name is Addy.                                                             │
│ │   - User lives in Seattle.                                                           │
│ └── <user_profile> (Dynamic Retrieved Facts for Active Turn)                           │
│     [Directives & Constraints]                                                         │
│     - Prefer Rust examples and concise explanations.                                   │
│     [Active Tasks & Goals]                                                             │
│     - (2 hours ago) Working on Phase 10 architecture refactor.                         │
│     [User Context & Knowledge]                                                         │
│     - (Yesterday) Configured remote GPU server for Ollama LLM.                         │
│     </user_profile>                                                                    │
├────────────────────────────────────────────────────────────────────────────────────────┤
│ MESSAGE 1: ROLE "system" (Optional — Present Only After Compaction Occurs)             │
│ └── [Compacted History Summary]                                                        │
│     Earlier in this session, the user discussed audio routing and PTT boundaries...    │
├────────────────────────────────────────────────────────────────────────────────────────┤
│ MESSAGES 2..N: RECENT CONVERSATION TURNS (Sliding FIFO Window)                         │
│ ├── user: "What model are we using for STT?"                                           │
│ ├── assistant: "We're using Sherpa-ONNX with the Nemotron transducer."                 │
│ └── user: "<Current User Utterance Text>"                                              │
└────────────────────────────────────────────────────────────────────────────────────────┘
```

### 4.2 Relative Timestamping Contract (`formatter.rs`)
Every retrieved temporal memory fact is formatted with its relative time offset immediately before prompt assembly:
```rust
pub fn format_relative_timestamp(created_at_ms: i64) -> String {
    let now_ms = current_epoch_ms();
    let diff_ms = now_ms - created_at_ms;

    if diff_ms < 60_000 { "Just now".to_string() }
    else if diff_ms < 3_600_000 { format!("{} minute{} ago", diff_ms / 60_000, if diff_ms / 60_000 == 1 { "" } else { "s" }) }
    else if diff_ms < 86_400_000 { format!("{} hour{} ago", diff_ms / 3_600_000, if diff_ms / 3_600_000 == 1 { "" } else { "s" }) }
    else if diff_ms < 2 * 86_400_000 { "Yesterday".to_string() }
    else if diff_ms < 7 * 86_400_000 { format!("{} days ago", diff_ms / 86_400_000) }
    else if diff_ms < 30 * 86_400_000 { format!("{} week{} ago", diff_ms / (7 * 86_400_000), if diff_ms / (7 * 86_400_000) == 1 { "" } else { "s" }) }
    else { format!("{} days ago", diff_ms / 86_400_000) }
}
```

---

## 5. Token Governance & Compaction State Machine

### 5.1 Threshold Calculation
$$	ext{Utilization} = rac{	ext{Total Tokens in Window}}{	ext{Context Window Size}}$$

- **Nominal ($< 70\%$):** Normal turn execution. Zero compaction overhead.
- **Soft Threshold ($70\% \le 	ext{Util} < 85\%$):**
  - Triggers **Opportunistic Compaction** asynchronously on `vox-memory-worker`.
  - Zero latency added to active turn.
  - If a new user turn arrives while running, the background task is cancelled cleanly via atomic flag.
- **Critical Threshold ($\ge 85\%$):**
  - Triggers **Immediate Compaction** (or fast FIFO prune).
  - Emits a non-blocking filler speech phrase to TTS (e.g., *"Just a second while I organize our context..."*).
  - Rebuilds context window with compacted summary before executing the user query.

---

## 6. Public Facade Contract (`harness/mod.rs`)

The entire memory subsystem is consumed via a clean, single asynchronous call:

```rust
pub async fn prepare_turn_context(
    harness: &Arc<Mutex<MemoryHarness>>,
    user_query: &str,
    turn_id: u32,
    settings: &VoxSettings,
) -> Result<(GenerationRequest, Option<String>), MemoryError> {
    // 1. Dynamic Retrieval (Scope -> MiniLM -> Turso DB -> Relative Timestamps -> <user_profile>)
    // 2. Token Budget & Threshold Evaluation (Soft / Critical check)
    // 3. Sliding Window Update (Append user turn)
    // 4. System Prompt Assembly (Persona + Identity + <user_profile> + Summary)
    // 5. Returns ready GenerationRequest + optional filler speech phrase
}
```

---

## 7. Verification Invariants

1. **Zero Memory Leakage in Pipeline:** `services/pipeline/` domain files must not import `retrieval`, `embedder`, `scope_router`, `VoxDb`, or `ingestion`. They interact solely via `prepare_turn_context()`.
2. **Deterministic Relative Timestamps:** All temporal memory facts rendered in `<user_profile>` must contain formatted relative timestamp prefixes (e.g. `(2 hours ago)`).
3. **No Double XML Wrapping:** `<user_profile>` tags are generated exclusively by `formatter.rs` and never stripped or re-wrapped downstream.
4. **Clean 4-Pillar Separation:** No circular dependencies between `harness`, `retrieval`, `compaction`, and `ingestion`.
