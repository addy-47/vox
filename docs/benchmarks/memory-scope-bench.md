# MemoryScope INT8 ONNX Local Rust Benchmark Report

- **Date**: Local Execution
- **Target Hardware**: Local Desktop CPU
- **Model Path**: `~/.vox/models/classifier/memory_scope/model_quantized.onnx`
- **Runtime Engine**: Native Rust `ort` 2.0 (ONNX Runtime CPU Engine)
- **Evaluation Dataset**: `sandbox/datasets/memory_scope_eval_test.json` (`500` samples)

---

## 🎯 Core Gate Metrics

| Metric | Target SLA | Measured Local Value | Gate Verdict |
|---|---|---|---|
| **Raw Holdout Accuracy** | >= 88.0% | **94.60%** | **PASSED** |
| **Calibrated Holdout Accuracy** | >= 88.0% | **91.60%** | **PASSED** |
| **Non-Default Label Precision** | >= 98.0% | **98.08%** | **PASSED** |
| **Uncertainty Fallback Rate** | <= 15.0% | **6.00%** | **PASSED** |
| **Rust CPU Latency (P50 / Median)** | 10--30 ms | **25.36 ms** | **PASSED** |
| **Rust CPU Latency (P95)** | < 40.0 ms | **46.70 ms** | **PASSED** |
| **Rust CPU Latency (P99)** | < 50.0 ms | **65.70 ms** | **PASSED** |

---

## 📊 Per-Class Precision Summary (tau* = 0.81)

| Scope Class | Precision |
|---|---|
| **ChitChat** | **100.00%** |
| **User** | **94.23%** |
| **Domain** (Primary Default) | **78.82%** |
| **Temporal** | **100.00%** |
