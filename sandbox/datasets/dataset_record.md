# Vox Master Dataset Ledger & Record

_Last Updated: July 29, 2026_

---

## 1. Master Dataset Summary

| Dataset File | Domain / Gate | Size (Pairs / Turns) | Primary Model / Purpose | Status |
| :--- | :--- | :---: | :--- | :---: |
| [`gate3_v7_ontology_6000p.json`](file:///home/addy/projects/apps/vox/sandbox/datasets/gate3_v7_ontology_6000p.json) | **Vox v7 Gate 3** | **6,000 Pairs** | `ModernBERT-base` 1-Pass Edge Classifier Fine-Tuning Golden Dataset | **GOLDEN MASTER** |
| [`v7-gate1_dedup_500_pairs.json`](file:///home/addy/projects/apps/vox/sandbox/datasets/v7-gate1_dedup_500_pairs.json) | **Vox v7 Gate 1** | **500 Pairs** | `MiniLM-L12` Soft Vector Dedup Threshold Calibration | **ACTIVE** |
| [`gate2_nli_400_pairs.json`](file:///home/addy/projects/apps/vox/sandbox/datasets/gate2_nli_400_pairs.json) | **Vox v7 Gate 2** | **400 Pairs** | `DeBERTa-v3-base` Intra-Domain NLI Precision Audit | **ACTIVE** |
| [`gate3_v7_ontology_5000p.json`](file:///home/addy/projects/apps/vox/sandbox/datasets/gate3_v7_ontology_5000p.json) | **Vox v7 Gate 3** | **5,000 Pairs** | Initial Unbalanced Generation Milestone | **SUPERSEDED** |
| [`gate3_v7_ontology_56p.json`](file:///home/addy/projects/apps/vox/sandbox/datasets/gate3_v7_ontology_56p.json) | **Vox v7 Gate 3** | **56 Pairs** | Prototype Validation & Micro-Batch Schema Calibration | **ARCHIVED** |
| [`vox_embedding_baseline_v1.json`](file:///home/addy/projects/apps/vox/sandbox/datasets/vox_embedding_baseline_v1.json) | **Embedding Baseline** | **1,000 Pairs** | Multi-model Embedding Retrieval Precision Baseline | **ACTIVE** |
| [`dataset_session1.json`–`5.json`](file:///home/addy/projects/apps/vox/sandbox/datasets/) | **Vox v6 Dialogue** | **4,993 Turns** | Synthetic Multi-Turn Dialogue Memory Retrieval Benchmark | **ACTIVE** |

---

## 2. Vox v7 Cognitive Memory Pipeline Datasets

### 2.1 Gate 3 Edge Classifier Golden Dataset (`gate3_v7_ontology_6000p.json`)
- **Location**: `sandbox/datasets/gate3_v7_ontology_6000p.json`
- **Volume**: **6,000 Verified Ground-Truth Pairs**
- **Creation Date**: July 29, 2026
- **Purpose**: Fine-tuning the **1-pass `ModernBERT-base` INT8 ONNX Sequence Classifier** (~35ms CPU target) for inter-domain cognitive edge classification across 4 operational labels (`SHAPES`, `DEPENDS_ON`, `CONFLICTS_WITH`, `NONE`).
- **Generation Method**: Multi-layer deterministic loop over 12 micro-batches using `llama3.1:8b` zero-temperature inline consensus (`temperature: 0.0`) and independent GPU LLM-as-a-Judge audits.
- **Section 7.1 Policy Matrix Compliance**: **100.00% PASS (0 Violations)**.

#### Distribution Breakdown

##### Domain Pair Matrix (Section 7.1 Authorized Connections)
- `Entities -> Constraints`: **975 pairs (16.25%)**
- `Entities -> Entities`: **903 pairs (15.05%)**
- `Directives -> Entities`: **899 pairs (14.98%)**
- `Profile -> Profile`: **850 pairs (14.17%)**
- `Directives -> Constraints`: **848 pairs (14.13%)**
- `Identity -> Profile`: **769 pairs (12.82%)**
- `Entities -> Profile`: **756 pairs (12.60%)**

##### Operational Edge Labels (Balanced Hard Negatives)
- `SHAPES`: **2,118 pairs (35.30%)**
- `NONE` (Hard Negatives): **1,293 pairs (21.55%)**
- `DEPENDS_ON`: **1,309 pairs (21.82%)**
- `CONFLICTS_WITH`: **1,280 pairs (21.33%)**

---

### 2.2 Gate 1 Soft Vector Dedup Calibration Dataset (`v7-gate1_dedup_500_pairs.json`)
- **Location**: `sandbox/datasets/v7-gate1_dedup_500_pairs.json`
- **Volume**: **500 Pairs**
- **Purpose**: Calibrated MiniLM-L12 cosine similarity threshold for soft deduplication. Established threshold $= 0.95$ with 0.0% false inactivations across 500 benchmark pairs (29.7ms/pair).

---

### 2.3 Gate 2 DeBERTa-v3 NLI Benchmark Dataset (`gate2_nli_400_pairs.json`)
- **Location**: `sandbox/datasets/gate2_nli_400_pairs.json`
- **Volume**: **400 Pairs**
- **Purpose**: Evaluated PyTorch & ONNX NLI candidates (`DeBERTa-v3-base`, `RoBERTa-large`, `BGE-Reranker`). Established `nli-deberta-v3-base` INT8 ONNX as winning NLI engine (85.11% overall accuracy, Directives = 99.33%).

---

## 3. Vox v6 Synthetic Dialogue Benchmarks

Synthetic multi-turn dialogue corpora used for full end-to-end memory recall evaluations:

- `dataset_session1.json` (1,000 turns)
- `dataset_session2.json` (999 turns)
- `dataset_session3.json` (995 turns)
- `dataset_session4.json` (999 turns)
- `dataset_session5.json` (1,000 turns)

**Total Dialogue Benchmark Volume**: 4,993 conversation turns covering technical deep dives (Rust memory manager, SIMD perception software, Neovim buffer optimization) and personal contextual facts (Mexico City trip logistics, tree nut allergy, Doughvid sourdough starter, fingerstyle guitar).

---

## 4. Data Governance & Maintenance Guidelines

1. **Golden Master Integrity**: `gate3_v7_ontology_6000p.json` is immutable and serves as the single source of truth for all Gate 3 ModernBERT fine-tuning experiments.
2. **Ephemeral Artifact Cleanup**: All temporary batch generation files (`candidate_batch_*.json`) and intermediate audit logs (`audit_report_batch_*.json`) must be purged immediately after micro-batch commitment to prevent workspace bloat.
3. **Ledger Synchronization**: Any new benchmark dataset created in `sandbox/datasets/` must be registered in this ledger prior to experiment execution.
