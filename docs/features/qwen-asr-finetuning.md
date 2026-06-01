# Qwen3-ASR LoRA Fine-Tuning for Vox (V1 & V2)

**Model:** Qwen3-ASR-0.6B  
**Goal:** Vox-optimized streaming ASR for Hindi/Hinglish desktop conversations  
**Dataset repo:** `vox-qwen-asr-hindi`  
**Script repo:** `vox-qwen-asr-hindi/scripts/`  
**Completed V1:** 2026-05-23  
**Completed V2:** 2026-05-29  

---

## Overview

This documents the end-to-end pipeline used to fine-tune, merge, export, quantize, and evaluate a Qwen3-ASR-0.6B model specialized for Vox's Hindi/Hinglish streaming use case. The pipeline has completed two full optimization iterations (V1 and V2).

---

## Phase 1 — Corpus Assembly & Augmentation

**Scripts:** `scripts/compile_corpus.py`, `scripts/pack_to_parquet.py`, `scripts/unpack_from_parquet.py`

### Corpus Growth

| Split | V1 Samples | V1 Hours | V2 Samples | V2 Hours | Domain / Source |
|---|---|---|---|---|---|
| **train** | ~5,500 | ~15.0h | ~5,500 | ~15.0h | Clean Hindi (IndicVoices-R, NPTEL) |
| **train** | ~1,036 | ~3.0h | ~6,665 | ~15.7h | Hinglish conversational (Curated pseudo-labeled talks/podcasts) |
| **train** | ~2,500 | ~7.0h | ~8,847 | ~22.6h | Desktop-noisy augmented (Fan/Keyboard/AC/RIR Reverb) |
| **train** | ~800 | ~2.0h | ~2,500 | ~5.0h | Negatives (Silence, background sounds, TV chatter) |
| **Total Train** | **~9,836** | **~27.0h** | **23,512** | **~31.41h** | Full training set |
| **Val Set** | 200 | — | 2,331 | ~3.15h | Held-out validation split |
| **Test Set** | 2,036 | ~8.7h | 2,838 | ~3.70h | Static benchmarks |

### V2 Augmentation Upgrades
* **Noise Diversity:** Custom keyboard tapping, TV/radio backgrounds, and AC/fan hum mixed at **$5\text{dB} - 15\text{dB}$ SNR** using the CAIMAN ambient noise library.
* **Room Reverberation:** Applied physical Room Impulse Responses (RIR) using open impulse libraries to simulate desktop environments.
* **Negative Rejection:** Expanded negative segments to **2,500 samples** mapped to empty targets (`""`) to completely eliminate false triggers during silent or noisy pauses.

---

## Phase 2 — LoRA Fine-Tuning

**Script:** `scripts/train_lora.py`

### Hyperparameter Evolution

| Hyperparameter | V1 Configuration | V2 Configuration |
|---|---|---|
| **Base Model** | `Qwen3-ASR-0.6B` | `Qwen3-ASR-0.6B` |
| **LoRA Rank (r)** | 32 | 16 |
| **LoRA Alpha** | 64 | 32 |
| **LoRA Dropout** | 0.05 | 0.05 |
| **Target Modules** | `q_proj`, `k_proj`... | `q_proj`, `k_proj`, `v_proj`, `o_proj` |
| **Batch Size** | 4 (eff. 32 via 8 steps) | 8 (eff. 32 via 4 steps) |
| **Learning Rate** | 2e-4 | 1e-4 |
| **Epochs** | 5 | 4 |
| **Mixed Precision** | bf16 | bf16 |
| **Compute Stack** | PyTorch 2.x | PyTorch Nightly (+cu128) |

### V2 Training Progress

The V2 model was trained on the noise-augmented dataset using an RTX 5070 Ti:

* **Epoch 1 (Step 735):** Clean WER 23.35%, Noisy WER 61.09%, False Trigger 0.00%
* **Epoch 2 (Step 1470):** Clean WER **20.69%**, Noisy WER **58.21%**, False Trigger 0.00% (Selected Checkpoint)
* **Epoch 3 (Step 2205):** Clean WER 23.24%, Noisy WER 58.27%, False Trigger 0.00%
* **Epoch 4 (Step 2940):** Clean WER 22.54%, Noisy WER 59.90%, False Trigger 0.00%

---

## Phase 3 — Weight Merging

**Script:** `scripts/merge_lora.py`

* The selected PEFT adapter (`checkpoint-1470` for V2) is fused directly into the baseline `Qwen3-ASR-0.6B` PyTorch layers.
* Generates a single, lightweight PyTorch model saved under `model/pytorch/` with `GenerationConfig` validated (cleared `temperature` when `do_sample=False`).

---

## Phase 4 — ONNX Export & INT8 Quantization

**Script:** `scripts/export_onnx.py`

The merged PyTorch model is split and exported into three distinct ONNX graphs compatible with `sherpa-onnx`:

| Component | File | Size | Format & Quantization |
|---|---|---|---|
| **Frontend subsampler** | `conv_frontend.onnx` | 44 MB | FP32 |
| **Transformer Encoder** | `encoder.int8.onnx` | 182 MB | Dynamic INT8 (`MatMul`, `Gemm` Dynamic) |
| **Transformer Decoder** | `decoder.int8.onnx` | 756 MB | Dynamic INT8 (`MatMul`, `Gemm` Dynamic) |

**Model Directory:** `~/.vox/models/stt/qwen3-asr/` (including copied tokenizer assets under `tokenizer/`).

---

## Phase 5 — Offline Evaluation

**Script:** `scripts/eval_srota_conv.py`

A multi-threaded CPU/GPU benchmark harness that evaluates full-precision PyTorch vs dynamic INT8 quantized ONNX models across static datasets. Tracks WER/CER, hallucination rates, CJK token occurrences, empty transcribing, and real-time factor (RTF).

---

## Phase 6 — Streaming Simulation

**Script:** `scripts/streaming_sim.py`

Simulates production-like desktop ASR inputs under a moving window configuration (2.0s window, 0.8s chunk step). Reports time-to-first-token (TTFT) and partial flip rate (PFR) on the target ONNX INT8 engine. V2 achieved **263ms TTFT** and reduced word flips by **40% (from 803 to 485)** compared to baseline Qwen3-ASR.
