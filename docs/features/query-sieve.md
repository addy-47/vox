# Query Sieve — 4-Class MemoryScope Classifier

**Pre-retrieval scope classification** that routes user queries into 4 cognitive categories before embedding generation and vector search. Eliminates wasted inference on chit-chat and identity queries, while ensuring domain-specific and temporal queries get full memory retrieval.

This is a custom submodule at `submodules/query-sieve-rs/` — a standalone Rust crate wrapping ModernBERT multilingual (ONNX Runtime INT8) with a Vox integration layer in `app/src-tauri/src/services/memory/query_classifier.rs`.

---

## Vox Integration

### Architecture

```
query-sieve-rs (submodules/query-sieve-rs/)
  └─ MemoryScopeClassifier + MemoryScope          ← crate public API
       │
       ▼
Vox wrapper: QueryScopeClassifier (query_classifier.rs)  ← error-safe singleton wrapper
       │
       ▼
Singleton: SCOPE_CLASSIFIER_INSTANCE (OnceLock)     ← loaded at most once
       │
       ▼
Re-exported API:                                    ← services/memory public surface
  classify_scope()
  ensure_scope_classifier_loaded()
  init_scope_classifier()
  is_scope_classifier_loaded()
       │
       ▼
Production callers:
  services/pipeline.rs  → scope-based retrieval gating
  ipc/pipeline.rs       → lazy-load on pipeline engage
```

### Pipeline Gating

Every user turn passes through the MemoryScope classifier *before* embedding generation and RAG retrieval. The scope determines which memory collections are searched and how:

```rust
let scope = crate::services::memory::classify_scope(&text);

let memory_block = match scope {
    MemoryScope::ChitChat => String::new(),       // Skip all memory retrieval
    MemoryScope::User => fetch_user_memory(),     // Identity + Profile only
    MemoryScope::Domain => fetch_domain_memory(), // Vector search + Directives
    MemoryScope::Temporal => fetch_temporal_memory(), // SQL narrative + Directives
};
```

**Consequence:** A `ChitChat` classification saves 100% of ONNX embedding inference (~30ms) and Turso vector DB search (~10–50ms) for that turn. A `User` classification restricts retrieval to identity/profile collections only, cutting retrieval cost roughly in half.

### Lazy Loading

On pipeline engage (`ipc/pipeline.rs`), the classifier is loaded lazily:

```rust
if let Err(e) = crate::services::memory::ensure_scope_classifier_loaded() {
    log::warn!("[QueryScopeClassifier] Lazy load on pipeline engage skipped/failed: {}", e);
}
```

If model files are absent from disk, the classifier degrades gracefully — `classify_scope()` returns `MemoryScope::Domain`, ensuring all turns route to full memory ingestion (safe default, no dropped data).

### Singleton Wrapper

```rust
pub struct QueryScopeClassifier {
    engine: MemoryScopeClassifier,
}

static SCOPE_CLASSIFIER_INSTANCE: OnceLock<QueryScopeClassifier> = OnceLock::new();
```

Wrapped with interior error handling — any `MemoryScopeClassifier::classify()` error is logged and defaults to `MemoryScope::Domain` rather than propagating.

### Model Path

```
~/.vox/models/classifier/modernbert_memory_scope/
├── model_quantized.onnx      (INT8 ONNX model, 143.67 MB)
└── tokenizer.json             (HuggingFace tokenizer)
```

---

## Crate: `query-sieve` (submodule)

| Property | Value |
|----------|-------|
| **Path** | `submodules/query-sieve-rs/` |
| **Language** | Rust (edition 2021) |
| **Version** | 0.1.0 |
| **Runtime** | ONNX Runtime (`ort` 2.0.0-rc.12) + HuggingFace `tokenizers` 0.22 |
| **License** | MIT |
| **Repository** | `https://github.com/addy-47/query-sieve-rs.git` |
| **Pinned Commit** | Per `Cargo.lock` |

### API Surface

```rust
// Core types
use query_sieve::{
    MemoryScope,              // Enum: ChitChat | User | Domain | Temporal
    MemoryScopeClassifier,   // ONNX classifier engine
    ClassifierConfig,         // Configuration struct
    ClassifierError,          // Error enum
};

// Construction
MemoryScopeClassifier::load(model_path, tokenizer_path)
MemoryScopeClassifier::load_with_config(config)

// Inference
classifier.classify("my name is John")     // -> MemoryScope
classifier.classify_raw("I love spicy")     // -> Result<(MemoryScope, f32, Vec<f32>)>
```

### Configuration (`ClassifierConfig`)

| Field | Default | Description |
|-------|---------|-------------|
| `max_token_length` | 32 | Max token length for ModernBERT input |
| `tau_star` | 0.81 | Confidence threshold — below this defaults to `Domain` |
| `max_input_chars` | 512 | Max input length (chars) |
| `intra_op_threads` | 0 | ONNX Runtime intra-op threads (0 = default) |

### Error Types (`ClassifierError`)

- `LoadFailed(String)` — ONNX model or tokenizer failed to load
- `TokenizationFailed(String)` — HuggingFace tokenizer error
- `InferenceFailed(String)` — ONNX Runtime inference error
- `EmptyInput` — Input was empty after trimming
- `InputTooLong { length, max }` — Exceeded `max_input_chars`

---

## Model

| Attribute | Value |
|-----------|-------|
| **Architecture** | ModernBERT multilingual |
| **Parameters** | ~143M (INT8 quantized) |
| **Quantization** | INT8 (static, per-tensor) |
| **Framework** | ONNX Runtime |
| **Max tokens** | 32 |
| **Classes** | 4: `ChitChat` (0), `User` (1), `Domain` (2), `Temporal` (3) |
| **Threshold** | τ* = 0.81 — predictions below this default to `Domain` |
| **HuggingFace Model** | `addyo07/modernbert-memory-scope` |
| **Training Dataset** | `addyo07/query-classification-dataset` (`v2/memory_scope_golden_v1.json`) |

### Performance

| Metric | Value |
|--------|-------|
| **Test accuracy** | **96.60%** |
| **Calibrated accuracy** | **91.60%** |
| **Non-Default Precision** | **98.08%** (at τ* = 0.81) |
| **Fallback Rate** | **6.00%** |

### Latency (CPU, AMD Ryzen)

| Mode | P50 | P99 | Target |
|------|-----|-----|--------|
| Single-thread | 25.36 ms | < 50 ms | 10–30 ms ✓ |

Benchmarked with `intra_op_threads=1`. Meets the sub-30ms P50 CPU target for real-time classification.

### Fallback Behavior

When confidence is below τ* = 0.81 or the model is absent/error, the classifier defaults to `MemoryScope::Domain`. This is the safest fallback — Domain scope triggers full vector-search retrieval, ensuring no durable knowledge is missed.

---

## Training

- **Base model**: ModernBERT multilingual
- **Dataset**: 22,006 items (balanced across 4 classes)
  - ChitChat: casual greetings, small talk, identity questions
  - User: personal facts, preferences, profile updates
  - Domain: code, math, domain-specific tasks
  - Temporal: time-sensitive queries, schedules, dates, reminders
- **Quantization**: INT8 static quantization
- **Hardware**: CPU training pipeline

---

## Repository Structure (`query-sieve-rs`)

```
submodules/query-sieve-rs/
├── src/
│   ├── lib.rs          ← Module declarations, re-exports
│   ├── config.rs       ← ClassifierConfig, DEFAULT_MEMORY_SCOPE_MAX_TOKEN_LENGTH
│   ├── error.rs        ← ClassifierError enum
│   ├── generic.rs      ← Legacy 2-class GenericSemanticClassifier (unused in v7)
│   └── memory_scope.rs ← MemoryScope enum + MemoryScopeClassifier (ModernBERT)
├── tests/
│   └── integration_test.rs
├── models/             ← Local model weight symlinks (gitignored)
├── Cargo.toml
└── README.md           ← Original crate README
```

---

## Tests

```bash
# Crate unit tests
cd submodules/query-sieve-rs && cargo test

# Vox integration wrapper tests
cd app/src-tauri && cargo test -- services::memory::query_classifier
```

Includes 4-class classification accuracy, confidence threshold fallback, error handling for empty/overlong input, and load failures.