# Query Sieve — Generic vs. Semantic Query Classifier

**Filters chit-chat from durable knowledge.** A lightweight binary classifier that categorizes user utterances as **GENERIC** (no durable knowledge — greetings, commands, chit-chat) or **SEMANTIC** (contains facts, preferences, relationships worth storing).

This is a custom submodule at `submodules/query-sieve-rs/` — a standalone Rust crate wrapping DistilBERT multilingual (ONNX Runtime INT8) with a Vox integration layer in `app/src-tauri/src/services/memory/classifier.rs`.

---

## Vox Integration

### Architecture

```
query-sieve-rs (submodules/query-sieve-rs/)
  └─ GenericSemanticClassifier + Classification      ← crate public API
       │
       ▼
Vox wrapper: QueryClassifier (classifier.rs)          ← error-safe singleton wrapper
       │
       ▼
Singleton: CLASSIFIER_INSTANCE (OnceLock)             ← loaded at most once
       │
       ▼
Re-exported API:                                      ← services/memory public surface
  classify_query()
  ensure_classifier_loaded()
  init_classifier()
  is_classifier_loaded()
       │
       ▼
Production callers:
  services/pipeline.rs  → gating RAG retrieval per turn
  ipc/pipeline.rs        → lazy-load on pipeline engage
```

### Pipeline Gating

The primary integration point is in `services/pipeline.rs` (~line 541). Every user turn passes through Query Sieve *before* embedding generation and RAG retrieval:

```rust
let classification = crate::services::memory::classify_query(&text);

let personal_memory_block = if classification.is_generic() {
    // Skip embedding + DB RAG entirely
    String::new()
} else {
    // Proceed with embedding generation, vector search, graph retrieval...
};
```

**Consequence:** A GENERIC classification saves 100% of ONNX embedding inference (~30ms) and Turso vector DB search (~10–50ms) for that turn. Over a session with 50% chit-chat ratio, this cuts memory subsystem latency roughly in half.

### Lazy Loading

On pipeline engage (`ipc/pipeline.rs` ~line 99), the classifier is loaded lazily:

```rust
if let Err(e) = crate::services::memory::ensure_classifier_loaded() {
    log::warn!("[QueryClassifier] Lazy load on pipeline engage skipped/failed: {}", e);
}
```

If model files are absent from disk, the classifier degrades gracefully — `classify_query()` returns `Classification::Semantic`, ensuring all turns route to full memory ingestion (safe default, no dropped data).

### Singleton Wrapper

```rust
pub struct QueryClassifier {
    engine: GenericSemanticClassifier,
}

static CLASSIFIER_INSTANCE: OnceLock<QueryClassifier> = OnceLock::new();
```

Wrapped with interior error handling — any `GenericSemanticClassifier::classify()` error is logged and defaults to `Classification::Semantic` rather than propagating.

### Model Path

```
~/.vox/models/classifier/distilbert-query-classifier/
├── model_quantized.onnx      (INT8 ONNX model)
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
| **Pinned Commit** | `ace3c6d` (per `Cargo.lock`) |

### API Surface

```rust
// Core types
use query_sieve::{
    Classification,              // Enum: Generic | Semantic
    GenericSemanticClassifier,   // ONNX classifier engine
    ClassifierConfig,            // Configuration struct
    ClassifierError,             // Error enum
};

// Construction
GenericSemanticClassifier::load(model_path, tokenizer_path)
GenericSemanticClassifier::load_with_config(config)

// Inference
classifier.classify("my name is John")     // -> Result<Classification>
classifier.classify_raw("I love spicy")     // -> Result<(Classification, Vec<f32>)>
```

### Configuration (`ClassifierConfig`)

| Field | Default | Description |
|-------|---------|-------------|
| `max_length` | 64 | Max sequence length (tokens) |
| `max_input_chars` | 512 | Max input length (chars) |
| `intra_op_threads` | 0 | ONNX Runtime intra-op threads (0 = default) |
| `max_words_for_classification` | 10 | Bypass threshold — sentences over this skip the model |

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
| **Architecture** | DistilBERT multilingual (cased) |
| **Parameters** | ~67M |
| **Quantization** | INT8 (static, per-tensor) |
| **Framework** | ONNX Runtime |
| **Max length** | 64 tokens |
| **Classes** | 2: GENERIC (index 0), SEMANTIC (index 1) |
| **HuggingFace Model** | [addyo07/distilbert-query-classifier](https://huggingface.co/addyo07/distilbert-query-classifier) |
| **Training Dataset** | [addyo07/query-classification-dataset](https://huggingface.co/datasets/addyo07/query-classification-dataset) |

### Performance

| Metric | Value |
|--------|-------|
| **Test accuracy** | **98.39%** |
| Precision | 0.9844 |
| Recall | 0.9834 |
| F1 score | 0.9839 |

### Latency (CPU, AMD Ryzen)

| Mode | P50 | P99 | Target |
|------|-----|-----|--------|
| Multi-thread | 8.39 ms | 11.81 ms | <50ms ✓ |
| Single-thread | 14.81 ms | 16.87 ms | <50ms ✓ |

Benchmarked with `intra_op_threads=1` for single-thread mode. Well under the <50ms CPU target.

### Latency Bypass Heuristic

Sentences longer than **10 words** bypass the model entirely and return `Classification::Semantic` with placeholder logits `[-10.0, 10.0]`. Rationale:

1. Long sentences almost always contain durable information.
2. Long sequences are slower to tokenize and infer — skipping them saves latency against the <50ms CPU target.

The threshold is configurable via `ClassifierConfig::max_words_for_classification` (set to 0 to disable).

---

## Training

- **Base model**: DistilBERT multilingual cased
- **Dataset**: 12,044 synthetic examples
  - 3,003 English generic
  - 3,017 English semantic
  - 3,019 Hindi generic
  - 3,005 Hindi semantic
- **Generator**: Llama 3.1 8B (synthetic data generation)
- **Hardware**: RTX 5070 Ti
- **Hyperparameters**: 5 epochs, batch size 32
- **Output**: INT8 quantized ONNX via static quantization

---

## Repository Structure (`query-sieve-rs`)

```
submodules/query-sieve-rs/
├── src/
│   ├── lib.rs          ← Module declarations, re-exports
│   ├── classifier.rs   ← GenericSemanticClassifier + ClassifierConfig + Classification
│   └── error.rs        ← ClassifierError enum
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
cd app/src-tauri && cargo test -- services::memory::classifier
```

Includes English/Hindi generic and semantic queries, long-sentence bypass behavior, error handling for empty/overlong input, and load failures.
