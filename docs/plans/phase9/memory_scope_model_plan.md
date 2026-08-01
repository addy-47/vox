# Phase 9: MemoryScope 4-Class Multilingual Classifier Fine-Tuning & Gate Validation Plan

**Status:** Approved Architectural Plan  
**Version:** 3.0 (mDeBERTa-v3-base Multilingual Specification & 9-Phase Gate Pipeline)  
**Governing Specifications:** `docs/plans/memory-spec-v7.md` & `docs/plans/memory-orchestration-spec.md`  
**Target Paths:** `/opt/vox/` (Remote Server), `submodules/query-sieve-rs`, `submodules/vox-models/classifier/`, `sandbox/scripts/`  

---

## 1. Executive Summary & Context

Pre-retrieval scope classification (`query-sieve-rs`) is the gatekeeper for Vox's RAG memory subsystem. To eliminate unnecessary vector search and graph expansion overheads in real-time voice interactions, every turn query must be categorized into one of four `MemoryScope` categories:

```rust
pub enum MemoryScope {
    ChitChat,   // Casual greetings, banter, filler (0 RAG lookup)
    User,       // Persona, preferences, personal constraints (Profile + Constraints)
    Domain,     // Technical Q&A, codebases, tools, projects (Entities + Directives) [PRIMARY DEFAULT]
    Temporal,   // Session recency, context recaps, continuity (Narrative + Directives)
}
```

### Key Architectural Mandates
1. **Selected Model Architecture**: `microsoft/mdeberta-v3-base` (Selected via research, 278M parameters, superior cross-lingual representation across English, Devanagari Hindi, and code-switched Hinglish).
2. **Confidence Threshold & Precision Mandate ($\tau^*$)**:
   - Non-default predictions (`ChitChat`, `User`, `Temporal`) must achieve **$\ge 98.0\%$ Precision** with near-zero false positives.
   - Any prediction with confidence score $< \tau^*$ or low margin falls back safely to **`Domain`** (the primary default).
3. **Pure ONNX Execution**: Zero rules, regex, or word-count fast-paths in production code.

---

## 2. Dataset Strategy & Tri-Lingual Corpus Composition

### 2.1 Base Dataset Audit (`/opt/vox/query-classification-dataset/`)
The base dataset on the remote GPU server (`hypr4@100.86.62.14`) contains **12,044 raw samples**:
- `en_generic.jsonl` (3,003 items) $\rightarrow$ Directly mapped to `ChitChat` (0 LLM calls required)
- `hi_generic.jsonl` (3,019 items) $\rightarrow$ Directly mapped to `ChitChat` (0 LLM calls required)
- `en_semantic.jsonl` (3,017 items) $\rightarrow$ Relabeled via parallel LLM into `User`, `Domain`, `Temporal`
- `hi_semantic.jsonl` (3,005 items) $\rightarrow$ Relabeled via parallel LLM into `User`, `Domain`, `Temporal`

### 2.2 Relabeling & Augmentation Pipeline
- **Parallel LLM Pipeline**: Run Ollama (`llama3.1:8b` / `gemma4:e4b` on port 11434) and LMS (`llama-3.1-8b-instruct` on port 1234) in parallel on `hypr4@100.86.62.14`.
- **Hinglish Code-Switched Augmentation**: Synthesize Hinglish queries for all 4 categories.
- **ASR/STT Noise Corruption**: Apply phonetic spelling variations, punctuation stripping, and acoustic filler insertions (`um`, `uh`, `yaar`, `मतलब`).
- **Final Golden Dataset Target**: ~18,000 samples balanced across English (40%), Devanagari Hindi (30%), and Hinglish (30%).

---

## 3. The 9-Phase Granular Pipeline

Every phase below includes explicit gate criteria and requires **Manual Human-in-the-Loop (HITL) User Approval** before moving to the next phase:

| Phase | Description | Key Deliverables / Gate Criteria |
|---|---|---|
| **Phase 1** | Remote Server Setup & Model Download | Setup `/opt/vox` on `hypr4@100.86.62.14`, create Python venv, install PyTorch/ONNX deps, download `microsoft/mdeberta-v3-base`, verify GPU & LLM servers. |
| **Phase 2** | Dataset Audit & Fast-Path Mapping | Map 6,022 generic items directly to `ChitChat`. Isolate 6,022 semantic items for LLM classification. |
| **Phase 3** | Parallel LLM Relabeling | Run Ollama + LMS parallel workers to classify semantic queries into `User`, `Domain`, `Temporal`. Complete verification audit pass. |
| **Phase 4** | Hinglish Augmentation & STT Corruption | Synthesize Hinglish code-switched entries and apply ASR noise. Build ~18,000 sample tri-lingual Golden Dataset. |
| **Phase 5** | Baseline Model Evaluation | Run zero-shot evaluation of pretrained `mDeBERTa-v3-base` on 4-class Golden Dataset test split. Record baseline metrics. |
| **Phase 6** | Model Fine-Tuning & GPU Optimization | Fine-tune `mDeBERTa-v3-base` on GPU using weighted cross-entropy loss, group-stratified splitting, and cosine annealing. |
| **Phase 7** | INT8 ONNX Quantization & Export Pipeline | Export PyTorch model to ONNX FP32 and apply dynamic INT8 quantization. Verify tensor graph and file size. |
| **Phase 8** | Confidence Threshold Calibration ($\tau^*$) | Conduct threshold sweep ($\tau = 0.50 \dots 0.95$). Calibrate $\tau^*$ to guarantee **$\ge 98\%$ Non-Default Precision** with `Domain` fallback. |
| **Phase 9** | Pure ONNX Rust Integration & SLA Validation | Integrate INT8 ONNX model into `submodules/query-sieve-rs`. Run Rust benchmark harness on single-core CPU to verify latency and RAM SLAs. |

---

## 4. Hardware & Server Context
- **Remote GPU Server**: `hypr4@100.86.62.14`
- **Workspace Directory**: `/opt/vox`
- **Active LLM Engines**:
  - Ollama (`http://localhost:11434`): `llama3.1:8b`, `gemma4:e4b`
  - LMS (`http://localhost:1234/v1`): `llama-3.1-8b-instruct`
