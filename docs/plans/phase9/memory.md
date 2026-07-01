# Memory Subsystem — End-to-End Overview

> **STATUS**: Pre-Phase-0. Nothing built yet. This document defines the end goal and
> the current phase of work. Updated 2026-07-01.

---

## Why Memory Matters for Vox

From `docs/vision.md`:
- "Remember relevant information"
- "Context aware"
- "Memory driven"

Vox is not a chatbot — it's a voice-native AI operator. Memory is what transforms it
from a stateless question-answerer into a personal assistant that knows the user,
their projects, preferences, and history.

---

## Three Memory Phases (Build Order)

Each phase is built, tested end-to-end, and working before the next begins.

```
Phase 0 ── Foundation & Benchmarks (CURRENT)
  ↓
Phase 1 ── Session Memory (context compression)
  ↓
Phase 2 ── Vector Memory (semantic search)
  ↓
Phase 3 ── Graph Memory (entity-relation knowledge graph)
```

### Phase 1: Session Memory

**What it does**: Keeps the LLM's context window from overflowing during long
conversations. When the conversation grows too large, older turns are automatically
summarized and replaced with a compact structured summary.

**Approach**: Hermes-agent inspired dual-threshold compressor.
- At ~60% context window usage, trigger automatic background summarization using the active LLM
- System prompt + first exchange are always preserved verbatim
- Summaries use a structured template: goal, decisions, progress, key facts
- Runs as a background task, never blocks the voice pipeline

### Phase 2: Vector Memory

**What it does**: Remembers facts across sessions. "User prefers Rust over Python."
"User is building a voice assistant called Vox." These are embedded and stored, then
retrieved on future turns via vector similarity.

**Approach**: Single-model, single-worker, automatic pre-fetch.
- One local MiniLM ONNX model for embeddings (~118MB, ~15ms)
- Turso Database vector search for storage and retrieval
- Automatic: every turn, embed the user's query and pre-fetch top-3 relevant memories
- Write on disengage: when the user stops the pipeline, extract facts from the session

### Phase 3: Graph Memory

**What it does**: Answers relational questions. "What database package did my friend
recommend when I was setting up the audio worker last week?" Requires understanding
entities and their relationships.

**Approach**: Dual-track (vector + graph), tool-gated retrieval.
- GLiNER ONNX model for entity extraction (~240MB, ~40ms)
- Knowledge graph built from extracted entities and relations
- LLM-driven graph traversal via tool-calling when simple vector search isn't enough
- Falls back to vector-only if the runtime doesn't support tool-calling

---

## Architecture Diagram (End State)

```
                         ┌─────────────────────────────────────────┐
                         │              PIPELINE                    │
                         │                                          │
User audio → VAD → STT ──┤  ┌─ Session Memory (context compress)   ├──→ LLM → TTS
                         │  ├─ Vector Memory (semantic pre-fetch)   │
                         │  └─ Graph Memory (tool-gated retrieval)  │
                         └──────────────────┬──────────────────────┘
                                            │
                         ┌──────────────────▼──────────────────────┐
                         │         Memory Worker (OS Thread)        │
                         │  - MiniLM ONNX (embeddings)              │
                         │  - GLiNER ONNX (entity extraction)       │
                         │  - Context compression                   │
                         └──────────────────┬──────────────────────┘
                                            │
                         ┌──────────────────▼──────────────────────┐
                         │     Turso Database (single .db file)     │
                         │  - memory_entries (vector table)         │
                         │  - graph_nodes + graph_edges             │
                         │  - conversation_summaries                │
                         └─────────────────────────────────────────┘
```

---

## Runtime Capability Detection Gate

All memory features are gated by two runtime checks, evaluated at provider selection
time:

| Gate | Checks | Enables |
|------|--------|---------|
| **Gate 1 — Agent Capability** | Tool calling, structured output, reasoning quality, context window >= 16K, streaming | Phase 3 tool-gated graph queries |
| **Gate 2 — Memory Capability** | Embedding model available, Turso DB initialized | Phase 2 vector memory |

If Gate 1 fails, the system runs in Baseline mode (Phase 1 session memory only).
If Gate 2 fails, the system falls back to session memory only.

**Evaluation mechanism**: Static capability map per provider/model, verified by a
single `health_check()` call. No continuous eval or benchmark prompts.

---

# Phase 0: Foundation & Benchmarks (CURRENT)

## Objective

Before writing a single line of memory subsystem code, we must validate:

1. **Model availability & quality**: Do MiniLM and GLiNER actually work as claimed?
   What are their real latencies and memory footprints on our target hardware?
2. **Turso Database integration**: Can we replace rusqlite with the `turso` crate?
   Does vector search work? What's the API like?
3. **Runtime capability detection**: Build a simple capability tier evaluator that
   gates features based on LLM provider capabilities.
4. **Benchmark infrastructure**: Create binaries and tests that we can run again
   later to catch regressions.

## File Manifest

```
app/src-tauri/src/
├── memory/                          # NEW: memory module (phase 0 skeleton)
│   ├── mod.rs                       # Module declarations, error types
│   ├── bench.rs                     # Benchmark helpers for memory models
│   └── capability.rs                # Runtime capability detection
├── bin/
│   ├── memory-bench.rs              # NEW: MiniLM + GLiNER benchmark binary
│   └── turso-test.rs                # NEW: Turso integration test binary
└── tests/
    └── memory_integration.rs        # NEW: Integration tests
```

## Workstream 1: Model Benchmarks

### Binary: `memory-bench.rs`

A standalone binary (following the `vox-bench.rs` pattern) that:

**Inputs:**
- Path to MiniLM INT8 ONNX model (downloaded from HuggingFace)
- Path to GLiNER ONNX model (downloaded from HuggingFace)
- Path to a text file with sample sentences (English + Hindi + Hinglish)
- Flag: `--model` = `"minilm" | "gliner" | "both"`

**What it measures (MiniLM):**

| Metric | Collection Method |
|--------|------------------|
| Load time | `Instant::now()` before/after `Session::commit_from_file()` |
| Per-inference latency (p50/p95/p99) | 100 iterations of `session.run()`, record durations |
| Memory footprint (RSS delta) | `BenchReporter::get_memory_snapshot()` before/after load |
| Embedding quality | Cosine similarity on known-pairs: similar > 0.8, dissimilar < 0.3 |
| Tokenization sanity | Verify Hindi text is tokenized without excessive subword splits |
| Max sequence length handling | Test with 128-token input (model's limit) |

**What it measures (GLiNER):**

| Metric | Collection Method |
|--------|------------------|
| Load time | `Instant::now()` before/after `Session::commit_from_file()` |
| Per-inference latency | 100 iterations, record durations |
| Memory footprint (RSS delta) | Same as MiniLM |
| Entity extraction quality | Run on test sentences with known entities, verify output |
| Hindi/Hinglish accuracy | Test on mixed-language text |
| Zero-shot entity types | Test with custom entity labels ("language", "framework", "person") |

**Output:**
```json
{
  "minilm": {
    "load_time_ms": 850,
    "latency_p50_ms": 12.3,
    "latency_p95_ms": 15.1,
    "latency_p99_ms": 18.7,
    "rss_delta_mb": 118,
    "cosine_similar_scores": [0.82, 0.91, ...],
    "cosine_dissimilar_scores": [0.12, 0.21, ...],
    "hinglish_tokenization_ok": true
  },
  "gliner": {
    "load_time_ms": 1200,
    "latency_p50_ms": 35.2,
    "latency_p95_ms": 42.0,
    "latency_p99_ms": 51.3,
    "rss_delta_mb": 240,
    "entity_accuracy_pct": 94.5
  }
}
```

### Model Download Paths

Models will be stored under `~/.vox/models/memory/` following the existing pattern:

```
~/.vox/models/memory/
├── minilm/
│   ├── model_int8.onnx         # MiniLM INT8 quantized (118MB)
│   ├── tokenizer.json          # Multilingual tokenizer (CRITICAL: must be the
│   │                           # exact one from paraphrase-multilingual-MiniLM-L12-v2,
│   │                           # NOT a generic English tokenizer — see BUG note)
│   └── config.json
└── gliner/
    ├── model_quantized.onnx    # GLiNER INT8 quantized (~200MB)
    ├── config.json
    └── tokenizer.json
```

> **BUG**: Hinglish tokenization drift. Standard tokenizers break Hinglish words into
> massive subword tokens, destroying embedding quality. The MiniLM model MUST use
> the exact `tokenizer.json` from `paraphrase-multilingual-MiniLM-L12-v2`, which
> treats Romanized South Asian phonetic text as coherent vocabulary groups.

### ONNX Session Configuration

Both models use the same `ort` crate used by the existing transliteration engine:

```rust
// Consistent with existing translit.rs pattern
let session = Session::builder()?
    .with_intra_threads(2)?     // Dedicated 2 threads, don't starve STT
    .with_inter_threads(1)?
    .commit_from_file(model_path)?;
```

### Model Sources

| Model | HuggingFace Source | ONNX Variant |
|-------|-------------------|--------------|
| **MiniLM** | `sentence-transformers/paraphrase-multilingual-MiniLM-L12-v2` | `Xenova/paraphrase-multilingual-MiniLM-L12-v2` — pre-converted ONNX, `model_int8.onnx` (118MB) |
| **GLiNER** | `urchade/gliner_multi-v2.1` | `onnx-community/gliner_multi-v2.1` — pre-converted ONNX, INT8 quantized variant |

---

## Workstream 2: Turso Database Integration

### Binary: `turso-test.rs`

A standalone binary that validates the `turso` crate works for our use case.

**Dependencies to add to `Cargo.toml`:**
```toml
turso = { git = "https://github.com/tursodatabase/turso" }
```

> **Note**: The `turso` crate is the Rust rewrite (formerly Limbo), not the
> `libsql-client-rs` crate. We are replacing rusqlite entirely, not adding alongside it.

**What it tests:**

| Test | Description |
|------|-------------|
| Create/open database | Create `~/.vox/vox.db`, verify file exists |
| WAL mode | Enable WAL via PRAGMA, verify with `PRAGMA journal_mode` |
| Basic CRUD | Create table, insert, select, update, delete |
| Vector column | Create table with vector column, insert vector, query with cosine distance |
| Concurrent read/write | Reader thread reads while writer thread inserts (simulates IPC + persistence pattern) |
| Memory overhead | RSS delta before/after opening database |
| Migration pattern | Run CREATE TABLE IF NOT EXISTS (must work like existing schema.rs) |
| Error handling | Corrupt file → graceful error, not panic |

**Output:** Pass/fail per test, with latency numbers for each operation.

### Migration Path from rusqlite

Current state:
- `persistence/worker.rs` uses `rusqlite::Connection` on a dedicated OS thread
- `ipc/history.rs` uses `VoxDb::open_readonly()` for concurrent reads
- `schema.rs` runs CREATE TABLE via `conn.execute_batch()`

Future state:
- Single `turso` database file
- Async by default — persistence worker becomes a tokio task instead of OS thread
- Vector search native (no extension needed)
- All existing tables (sessions, turns, voices) migrated

**This migration happens in a separate phase. Phase 0 only validates that turso
works for our needs — we don't migrate existing tables yet.**

---

## Workstream 3: Runtime Capability Detection

### File: `memory/capability.rs`

A simple static capability evaluator.

```rust
#[derive(Debug, Clone, PartialEq)]
pub enum CapabilityTier {
    Baseline,  // Session memory only (context compression)
    Advanced,  // All memory features enabled
}

#[derive(Debug, Clone)]
pub struct RuntimeCapabilities {
    pub tier: CapabilityTier,
    pub has_tool_calling: bool,
    pub has_structured_output: bool,
    pub context_window: u32,
    pub supports_streaming: bool,
    pub memory_gate_passed: bool,
}

pub fn evaluate_capabilities(
    provider_kind: ProviderKind,
    model_name: &str,
    health_check_result: &HealthCheckResult,
) -> RuntimeCapabilities;
```

**Capability map** (static data, one match statement):
- OpenAiCompat providers with frontier models (gpt-4o, gemini-2.5-pro, claude-4, etc.)
  → Advanced tier
- Embedded local LLMs (1B-3B params) → Baseline tier (small context, no reliable tool calling)
- Unknown/unrecognized models → Baseline tier (safe default)

**Health check integration**: The existing `health_check()` on `LlmProvider` already
returns `true/false`. We extend it to return capability-relevant info:
- Model name
- Context window size (where available from API)
- Whether streaming is supported

### Integration Points

The capability evaluator is called:
1. When the user changes the LLM provider in Settings
2. At app startup when the engine warms up

The result is stored in `AppState` and consulted by:
- The pipeline (to decide whether to inject memory)
- The frontend (to show/hide memory-related UI)

---

## Workstream 4: Memory Module Skeleton

### File: `memory/mod.rs`

```rust
pub mod bench;
pub mod capability;

/// Memory-specific error types
#[derive(Debug)]
pub enum MemoryError {
    ModelNotFound(String),
    ModelLoadFailed(String),
    InferenceFailed(String),
    DatabaseError(String),
}

/// Memory model types
pub enum MemoryModelKind {
    MiniLM,
    GLiNER,
}
```

---

## Success Criteria for Phase 0

Phase 0 is complete when:

1. [ ] `cargo run --bin memory-bench -- --model both` produces valid JSON output
       with all latency/memory/quality metrics for both models
2. [ ] MiniLM embedding quality validated: similar pairs > 0.8 cosine, dissimilar < 0.3
3. [ ] Hinglish tokenization verified: Hindi text produces coherent token sequences
4. [ ] `cargo run --bin turso-test` passes all tests (create, CRUD, vector, concurrent)
5. [ ] `Turso` database file can be opened, queried, and closed without errors
6. [ ] `RuntimeCapabilities` can be computed from a provider selection and stored in AppState
7. [ ] All benchmark JSON output is saved to `~/.vox/benchmarks/memory/` with timestamps
8. [ ] No regressions in existing benches: `vox-bench`, `tts-bench` still pass

---

## Future Phases (Brief)

### Phase 1: Session Memory
- Context window monitoring (track token usage per turn)
- Structured summary template design
- Hermes-style dual-threshold compression (60% trigger, 20% target ratio)
- Background summarization worker
- Test: 50-turn conversation stays within 80% of context window

### Phase 2: Vector Memory
- MiniLM embedding worker on dedicated OS thread
- Register `PersistenceEvent::MemoryEntry` for async writes
- Pre-fetch top-3 memories before each LLM generation
- Memory UI: browser/editor in Settings page
- Test: "What language do I prefer?" after restart returns correct answer

### Phase 3: Graph Memory
- GLiNER entity extraction pipeline
- Turso graph node/edge tables
- Tool-gated graph traversal (`query_knowledge_graph` tool)
- Hybrid read path (vector pre-fetch + graph tool)
- Test: "What was the name of that package my friend recommended?" returns correct result
