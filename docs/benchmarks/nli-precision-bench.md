# Gate 2 DeBERTa-v3 NLI Domain Precision Audit Report

**Status**: **FAILED** (Overall Accuracy: **78.67%** vs Target: **$\ge 95.0\%$**)  
**Target Component**: Ingestion Pipeline Step 4A NLI State Resolution Engine  
**Evaluated Model**: `deberta-v3-xsmall` ONNX (`83.2 MB`, local CPU inference)  
**Execution Harness**: `benches/nli_bench.rs` (Subcommand `batch-nli-score`)  
**Dataset**: 450 High-Context Synthetic Fact Pairs (`sandbox/datasets/gate2_nli_400_pairs.json`)  
**Raw Results File**: `sandbox/results/gate2_nli_raw_scores.json`  

---

## 1. Executive Summary & Benchmark Outcome

Gate 2 evaluated the classification accuracy and false-positive supersession rate of `deberta-v3-xsmall` ONNX across 450 domain-specific fact pairs spanning `Identity`, `Directives`, and `Constraints`.

### Summary Findings
- **Overall Accuracy**: **`78.67%`** (354 / 450 correct classifications) — **MISSED TARGET ($\ge 95\%$)**
- **Average Latency**: **36.74 ms / pair** on local CPU
- **Domain Performance**:
  - **`Directives`**: **`98.67%`** (148 / 150) — **PASSED** (Exceptional performance on active task status changes)
  - **`Identity`**: **`76.00%`** (76 / 100) — **FAILED**
  - **`Constraints`**: **`65.00%`** (130 / 200) — **FAILED**

---

## 2. Empirical Performance Breakdown by Domain

| Domain | Pair Count ($n$) | Correct Matches | Accuracy (%) | Primary Failure Mode | Status |
| :--- | :---: | :---: | :---: | :--- | :---: |
| **`Directives`** | 150 | 148 | **98.67%** | Minor edge case on multi-step tasks | **PASSED** |
| **`Identity`** | 100 | 76 | **76.00%** | `ENTAILMENT` misclassified as `NEUTRAL` (24%) | **FAILED** |
| **`Constraints`** | 200 | 130 | **65.00%** | `ENTAILMENT` misclassified as `NEUTRAL` (29.5%) | **FAILED** |

---

## 3. Root Cause & Error Pattern Analysis

### 3.1 Systematic Failure Mode: Strict Formal Logic vs. Domain Subsumption
Standard NLI models trained on MNLI (`deberta-v3-xsmall`) adhere strictly to formal linguistic entailment. In formal logic:
- Premise: *"User lives in New York City."*
- Hypothesis: *"User resides in Manhattan."*
- **Model Output**: **`NEUTRAL`** ($P = 0.934$) — *Because Manhattan is a specific borough, living in NYC does not strictly entail living in Manhattan.*

Similarly for system constraints:
- Premise: *"Do not execute any command that modifies system files without explicit user confirmation."*
- Hypothesis: *"Agent requires user's go-ahead before altering /etc/hosts."*
- **Model Output**: **`NEUTRAL`** ($P = 0.988$) — *Because MNLI lacks embedded domain ontology mapping `/etc/hosts` as a system file.*

### 3.2 Impact on Memory Pipeline
If domain-subsumption entailments are misclassified as `NEUTRAL`:
1. Specific constraint refinements (e.g. altering `/etc/hosts`) remain stored as independent active facts alongside the general constraint.
2. The memory system accumulates redundant, overlapping constraints instead of executing clean state supersession.

---

## 4. Exploration of Remediation Options (ML Engineering Evaluation)

Because `deberta-v3-xsmall` failed Gate 2 on `Constraints` and `Identity`, we must evaluate alternative architecture paths:

### Option 1: Route `Constraints` & `Identity` to Step 4B (LLM Edge Classifier)
- **Rationale**: `LFM2.5-230M` GGUF possesses internal world knowledge (e.g., recognizing that `/etc/hosts` is a system file or Manhattan is in NYC).
- **Pipeline Adjustment**:
  - Keep `deberta-v3-xsmall` for `Directives` (where it achieved **98.67% accuracy** at 36ms).
  - Move `Constraints` and `Identity` state resolution to Step 4B (`LFM2.5-230M` LLM classifier).
- **Tradeoff**: Increases Step 4 latency for `Constraints` from 36ms to ~80-100ms per pair.

### Option 2: Benchmark `bge-reranker-v2-m3` or `mDeBERTa-v3-base` ONNX
- **Rationale**: Larger cross-encoders or multi-lingual DeBERTa models may have broader world-knowledge representations.
- **Tradeoff**: Higher disk footprint (~280MB vs 83MB) and higher CPU latency (~80ms vs 36ms).

### Option 3: Remote Fine-Tuning on `hypr4@100.86.62.14`
- **Rationale**: Fine-tune `deberta-v3-small` ONNX on a 1,000-pair memory-subsumption dataset to teach the model explicit domain inclusion logic.
- **Tradeoff**: Requires training pipeline execution on the remote GPU server.

---

## 5. Summary Table of Next Steps
| Option | Component Affected | Expected Accuracy | Expected Latency | Recommended Action |
| :--- | :--- | :---: | :---: | :--- |
| **Option 1 (LLM Routing)** | Step 4 Pipeline Logic | **> 90%** | ~80ms | **Primary Candidate** |
| **Option 2 (Larger ONNX)** | NLI Engine Weights | **80-88%** | ~80ms | Benchmark if Option 1 latency is too high |
| **Option 3 (Fine-Tuning)** | NLI Engine Weights | **> 95%** | ~36ms | Contingency if local CPU constraints dominate |
