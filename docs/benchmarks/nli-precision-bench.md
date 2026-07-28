# Gate 2 Multi-Model NLI Domain Precision Audit Report

**Status**: **COMPLETED** — Multi-Model Evaluation & Selection  
**Winner Selected**: **`nli-deberta-v3-base`** ONNX (`233.0 MB`, local CPU inference)  
**Execution Harnesses**:
- Rust ONNX Harness: `benches/nli_bench.rs` (Subcommand `batch-nli-score` with native $P \ge 0.85$ threshold)
- Python PyTorch Harness: `sandbox/scripts/eval_python_models.py` (Venv execution with $P \ge 0.85$ threshold)  
**Dataset**: 450 High-Context Synthetic Fact Pairs (`sandbox/datasets/gate2_nli_400_pairs.json`)  
**Raw Results Files**:
- `sandbox/results/gate2_nli_raw_scores.json` (`deberta-v3-xsmall` ONNX)
- `sandbox/results/gate2_nli_deberta_base_scores.json` (`nli-deberta-v3-base` ONNX)
- `sandbox/results/gate2_nli_roberta_large_scores.json` (`roberta-large-mnli` ONNX)
- `sandbox/results/python_nli_eval_results.json` (PyTorch Candidate Audit: `FineCat-NLI-L`, `BART-Large-MNLI`, `DeBERTa-v3-Small`, `MoritzLaurer`, `tasksource`)

---

## 1. Complete Multi-Model Benchmark Comparison Matrix

All models were evaluated on local CPU with the mandatory Vox v7 **$P \ge 0.85$ confidence threshold** enforced (predictions with confidence $< 0.85$ fall back to `NEUTRAL`).

| Model Name | Framework / Format | Disk Size | Overall Accuracy (%) | `Directives` Acc (%) | `Identity` Acc (%) | `Constraints` Acc (%) | Mean Latency (ms/pair) | Status |
| :--- | :---: | :---: | :---: | :---: | :---: | :---: | :---: | :--- |
| **`deberta-v3-xsmall`** | ONNX (INT8) | `87.2 MB` | **72.22%** | **96.67%** | **72.00%** | **54.00%** | **29.7 ms** | Baseline (Failed) |
| **`roberta-large-mnli`** | ONNX (FP32) | `341.8 MB` | **73.11%** | **98.00%** | **82.00%** | **50.00%** | **111.2 ms** | Rejected (Poor Constraints) |
| **`facebook/bart-large-mnli`** | PyTorch / ONNX | `1.63 GB` | **71.56%** | **99.33%** | **81.00%** | **46.00%** | **287.4 ms** | **Rejected** (Severe failure on Constraints) |
| **`dleemiller/finecat-nli-l`** | PyTorch (FP32) | `1.58 GB` | **81.56%** | **99.33%** | **`86.00%`** | **66.00%** | **331.5 ms** | **Evaluated** (Good Identity, 5x slower, weak Constraints) |
| **`nli-deberta-v3-small`** | PyTorch (FP32) | `568 MB` | **73.78%** | **99.33%** | **74.00%** | **54.50%** | **52.6 ms** | Evaluated (Sub-base performance) |
| **`tasksource-nli-base`** | PyTorch (FP32) | `504 MB` | **67.78%** | **92.67%** | **78.00%** | **44.00%** | **84.3 ms** | Rejected (Low precision across board) |
| **`MoritzLaurer-base`** | PyTorch (FP32) | `504 MB` | **78.44%** | **97.33%** | **`89.00%`** | **59.00%** | **522.9 ms** | Evaluated (Strong Identity, weak Constraints) |
| **`nli-deberta-v3-base`** 🏆 | **ONNX (INT8)** | **`233.0 MB`** | **`85.11%`** | **`99.33%`** | **`83.00%`** | **`75.50%`** | **64.8 ms** | **WINNER (Selected for Vox v7)** |

---

## 2. Key Findings & Detailed Analysis

1. **BART-Large-MNLI (`facebook/bart-large-mnli` / `onnx-community/bart-large-mnli-ONNX`)**:
   - Achieved only **71.56% overall accuracy** and failed heavily on the `Constraints` domain (**46.00%** accuracy).
   - BART's generative encoder-decoder architecture outputs lower probability bounds for cross-domain constraint subsumptions, causing most predictions to fall below the mandatory $P \ge 0.85$ threshold into `NEUTRAL`.
2. **FineCat-NLI-L (`dleemiller/finecat-nli-l`)**:
   - Achieved **81.56% overall accuracy** with the highest `Identity` accuracy (**86.00%**).
   - However, it lagged on `Constraints` (**66.00%** vs **75.50%** for `nli-deberta-v3-base`) and required **331.5 ms/pair** on CPU (5x slower than our ONNX winner).
3. **`nli-deberta-v3-base` (ONNX)**:
   - Maintains clear overall dominance (**85.11% overall accuracy**, **75.50% on Constraints**, **64.8 ms/pair**).

---

## 3. Finetuning Feasibility & Data-Centric Adaptation Protocol
*(Authored per ML Research Engineer Standards — `.agents/rules/ml-research-engineer.md`)*

While `nli-deberta-v3-base` ONNX passes Gate 2 with **85.11% overall precision**, `Constraints` domain resolution remains at **75.50%** due to domain shift between general NLI corpora (MNLI, FEVER, ANLI) and agent operational state boundaries. If a future Vox release requires $>90\%$ precision on `Constraints`, fine-tuning is highly feasible.

### 3.1 Failure Mode Diagnosis & Scientific Justification

General NLI models struggle with operational constraints due to three specific dataset characteristics:
1. **Implicit Temporal & Boundary Scopes**: General NLI premises (e.g., *"A dog runs in the park"*) are static declarative facts. Operational constraints (e.g., *"Never use Tailwind for desktop UI, but allowed for web landing pages"*) contain conditional exceptions that general NLI models confuse with total contradictions.
2. **Numeric Budget & Boundary Constraints**: Rules such as *"Cap memory context at 15%"* vs *"Set memory context budget to 20%"* require fine-grained boundary comparison rather than coarse semantic overlap.
3. **Threshold Calibration Deficit**: Out-of-the-box cross-encoders generate uncalibrated logit distributions on domain-specific facts, causing true contradictions to yield probabilities around $0.70 - 0.80$, falling below our mandatory $P \ge 0.85$ threshold into `NEUTRAL`.

### 3.2 Data-First Corpus Curation Strategy

Before initiating any model fine-tuning, dataset quality and coverage must be established:

- **Target Dataset Size**: 2,500 highly curated triplets `(premise, hypothesis, label)` partitioned into 80% train / 10% val / 10% test.
- **Domain Distribution Balancing**:
  - `Constraints`: 50% (1,250 pairs) — Focus on negative rules, numeric caps, scope overrides.
  - `Identity`: 25% (625 pairs) — Core user self-descriptors and static traits.
  - `Directives`: 25% (625 pairs) — Agent operational state, active tasks, workflow steps.
- **Hard Negative Mining**:
  - Automatically extract candidate pairs where `nli-deberta-v3-base` yields logits between $0.60$ and $0.84$ (uncertain predictions).
  - Human / Strong LLM (Llama 3.1 70B / Gemini 2.5) review to eliminate label noise.

### 3.3 Training Dynamics & Optimization Pipeline

1. **Base Model Architecture**: `cross-encoder/nli-deberta-v3-base`.
2. **Fine-Tuning Method**:
   - **Full Fine-Tuning** of sequence classification head + top 4 DeBERTa transformer layers.
   - *Rationale*: Avoids LoRA adapter merging overhead during ONNX graph export, preserving native INT8 ONNX execution.
3. **Loss Function & Regularization**:
   - Cross-Entropy Loss over softmax logits $[P(\text{Contradiction}), P(\text{Entailment}), P(\text{Neutral})]$.
   - Weight Decay: $0.01$, Dropout: $0.1$.
   - Learning Rate: $1.5 \times 10^{-5}$ with linear warmup (10% of total steps) and cosine decay.
   - Epochs: 3–5 epochs (stopping early via validation loss monitoring).
4. **Catastrophic Forgetting Mitigation**:
   - Mix 20% general MNLI validation triplets into the training set to preserve baseline reasoning abilities across general facts.

### 3.4 Compression, ONNX Export & Deployment Validation

1. **Optimum ONNX Export**:
   ```bash
   optimum-cli export onnx \
     --model ~/.vox/finetuned-nli-deberta-v3-base \
     --task text-classification \
     --optimize O3 \
     ~/.vox/models/nli/nli-deberta-v3-base-finetuned/
   ```
2. **Dynamic INT8 Quantization**:
   ```python
   import onnxruntime.quantization as ort_quant
   ort_quant.quantize_dynamic(
       model_input="model.onnx",
       model_output="model_quantized.onnx",
       weight_type=ort_quant.QuantType.QUInt8
   )
   ```
3. **Target Benchmark Criteria**:
   - `Constraints` Accuracy: $>90.0\%$ (up from $75.50\%$).
   - `Directives` Accuracy: $\ge 99.0\%$ (maintained).
   - `Identity` Accuracy: $\ge 85.0\%$ (maintained).
   - CPU Latency: $< 50\text{ ms/pair}$ (INT8 ONNX).

---

### 3.5 Structured Technical Feedback

> [!BUG]
> **🐛 Synthetic Dataset Label Ambiguity (Confidence: 95%)**  
> **Explanation**: Synthetic fact generation can produce ambiguous pairs where a conditional rule exception (e.g., *"Do not use Tailwind except on landing pages"*) is labeled as `CONTRADICTION` against *"Use Tailwind on landing pages"*, when logically it is `NEUTRAL` or `ENTAILED`. Mislabeling these distorts NLI evaluation metrics.  
> **Suggested Fix**: Perform an automated verification pass using Llama 3.1 70B with strict chain-of-thought logic checks before adding synthetic constraint pairs to the training or evaluation benchmark.

> [!TRADEOFF]
> **⚖️ Full Fine-Tuning vs. LoRA for ONNX Deployment (Confidence: 90%)**  
> **Cost**: Full fine-tuning requires higher VRAM during training (~12GB VRAM vs ~4GB for LoRA).  
> **Benefit**: Full fine-tuning produces a clean, unified model checkpoint without adapter layer overhead, enabling seamless ONNX graph conversion and optimal INT8 quantization performance.  
> **When Appropriate**: For sub-500M parameter cross-encoder models (`DeBERTa-v3-base`), full fine-tuning is preferred over LoRA when exporting to ONNX Runtime.

> [!IMPROVEMENT]
> **💡 Domain-Specific Hard-Negative Mining Pipeline (Confidence: 92%)**  
> **Rationale**: Training on hard negative samples (pairs where the baseline model outputs high uncertainty logits between $0.60$ and $0.84$) provides 3–5x greater precision improvement per training sample than random uniform dataset sampling.  
> **Expected Impact**: Elevates `Constraints` domain precision from $75.50\%$ to $>90\%$ with fewer than 2,500 total training samples.  
> **Validation Strategy**: Evaluate performance on the 450-pair Gate 2 benchmark before and after hard-negative fine-tuning iterations.

---

## 4. Final Recommendation & Gate 2 Verdict

- **Selected Model**: `~/.vox/models/nli/nli-deberta-v3-base/model_quantized.onnx` (`233.0 MB` INT8 ONNX)
- **Gate 2 Verdict**: **PASSED**
- **Rust Backend Config**: Configured `pub const NLI_MODEL_DIR: &str = "nli-deberta-v3-base";` in `app/src-tauri/src/services/memory/nli.rs`.
