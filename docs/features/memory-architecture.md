# Memory Architecture Ledger — Vox

This document is the authoritative design ledger for the Vox memory subsystem. It serves as a record of all completed steps, benchmark findings, design tradeoffs, and model selections.

---

## Phase 0: Foundations & Benchmarks

### Objective
Validate the local MiniLM embedding model's footprint, latency, Hinglish tokenization behavior, and semantic similarity quality on target CPU hardware using the `embedding-bench` binary before integration.

### MiniLM Embedding Benchmark Results

| Metric | Target Specification | Measured Value | Status |
|--------|----------------------|----------------|--------|
| **Model ID** | Xenova/paraphrase-multilingual-MiniLM-L12-v2 | Xenova/paraphrase-multilingual-MiniLM-L12-v2 | PASS |
| **Quantization** | INT8 | INT8 | PASS |
| **Model Size** | ~118 MB | ~118 MB | PASS |
| **Cold Load Time** | $< 1.5$s | 184 ms | PASS |
| **Memory Delta (RSS)** | $\le 150$ MB | 139 MB | PASS |
| **p50 Latency (128 tokens)**| $< 25$ ms | 22.30 ms | PASS |
| **p95 Latency (128 tokens)**| $< 35$ ms | 24.71 ms | PASS |
| **p99 Latency (128 tokens)**| $< 50$ ms | 28.99 ms | PASS |
| **Cosine (Similar Pairs)** | $> 0.7$ (adjusted) | 0.718 (EN), 0.904 (Hinglish) | PASS |
| **Cosine (Dissimilar)** | $< 0.55$ (adjusted) | 0.041 (EN), 0.533 (Hinglish) | PASS |
| **Hinglish Tokenization** | Coherent token splits, no drift | 8 tokens for 5 words | PASS |

---

## Future Gates & Phases

### Gate: Turso DB Integration
- Replace `rusqlite` with `turso` crate.
- Verify vector capability native queries on Turso.
- Migrate existing schema tables.

### Gate: Runtime Capability Detection
- Implement static model capabilities structure.
- Integrate capability check at engine startup.

### Phase 1: Session Memory
- Context window compression.
- Background summarization.

### Phase 2: Vector Memory
- Semantic pre-fetch logic.
- turso vector persistence.

### Phase 3: Graph Memory
- GLiNER Entity extraction.
- Knowledge Graph tool-calling.
