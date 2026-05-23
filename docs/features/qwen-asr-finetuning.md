# Qwen3-ASR LoRA Fine-Tuning for Vox

**Model:** Qwen3-ASR-0.6B  
**Goal:** Vox-optimized streaming ASR for Hindi/Hinglish desktop conversations  
**Dataset repo:** `vox-qwen-asr-hindi`  
**Script repo:** `vox-qwen-asr-hindi/scripts/`  
**Completed:** 2026-05-23

---

## Overview

This documents the end-to-end pipeline used to fine-tune, merge, export, quantize, and evaluate a Qwen3-ASR-0.6B model specialized for Vox's Hindi/Hinglish streaming use case. The process ran in 6 sequential phases.

---

## Phase 1 — Corpus Assembly

**Script:** `scripts/compile_corpus.py`, `scripts/pack_to_parquet.py`  
**Output:** `data_parquet/` (train/val/test splits in Parquet format)

### Corpus Composition

| Split | Domain | Samples | Approx Hours |
|---|---|---|---|
| train | Clean Hindi (IndicVoices-R, NPTEL subset) | ~5500 | ~15h |
| train | Hinglish conversational (custom curated) | ~1036 | ~3h |
| train | Desktop-noisy augmented (fan/keyboard/reverb) | ~2500 | ~7h |
| train | Negative (silence/non-speech) | ~800 | ~2h |
| val | Clean Hindi held-out | 100 | — |
| val | Noisy Hindi held-out | 100 | — |
| test/benchmark | Clean Hindi | 500 | ~2.3h |
| test/benchmark | Hinglish | 1036 | ~3.8h |
| test/benchmark | Noisy Hindi | 250 | ~1.6h |
| test/benchmark | Negatives (silence/noise) | 250 | ~1.0h |

### Noise Augmentation Parameters

Applied to all clean training samples with:
- **Fan noise:** SNR 15–25 dB (CAIMAN-ASR background noise library)
- **Augmentation probability:** 0.5 per sample
- **Speed perturbation:** ±5% at p=0.3
- **Room reverb:** Not applied in v1 (planned for v2)

### Parquet Schema

```
{
  "audio": bytes (WAV, 16kHz mono),
  "transcript": str,
  "language": str ("hi" | "en" | "hi-en"),
  "source": str,
  "duration_s": float,
  "noise_type": str | null
}
```

---

## Phase 2 — LoRA Fine-Tuning

**Script:** `scripts/train_lora.py`  
**Command:**
```bash
nohup /opt/vox/venv/bin/python3 /opt/vox/temp/train/train_lora.py \
  > /opt/vox/temp/logs/train_lora.log 2>&1 &
```

### Key Hyperparameters

| Parameter | Value |
|---|---|
| Base model | `Qwen3-ASR-0.6B` (HuggingFace safetensors) |
| LoRA rank | 32 |
| LoRA alpha | 64 |
| LoRA dropout | 0.05 |
| Target modules | `q_proj`, `k_proj`, `v_proj`, `o_proj`, `gate_proj`, `up_proj`, `down_proj` |
| Quantization | QLoRA 4-bit (bitsandbytes NF4) |
| Learning rate | 2e-4 |
| LR scheduler | Cosine decay |
| Warmup steps | 100 |
| Batch size | 4 per device |
| Gradient accumulation | 8 steps (effective batch = 32) |
| Max epochs | 5 |
| Max audio length | 30s |
| Evaluation interval | every 500 steps |
| Optimizer | AdamW (paged, for QLoRA) |
| Gradient checkpointing | Enabled |
| Mixed precision | bf16 |
| Seed | 42 |

### Hardware

- **GPU:** RTX 5070 Ti (16GB VRAM)
- **Training time:** ~6–8 hours for 5 epochs
- **Peak VRAM:** ~13.5GB (QLoRA 4-bit)
- **Framework:** HuggingFace Transformers + PEFT

### Epoch Metrics (on internal eval split)

| Epoch | Clean WER | Clean CER | Noisy WER | Noisy CER | Noisy CJK% | False Trigger% |
|---|---|---|---|---|---|---|
| Baseline (ep 0) | 25.5% | 12.3% | 65.3% | 59.9% | **11.0%** | 1.0% |
| Epoch 1 | 30.3% | 16.3% | 63.2% | 56.0% | 11.0% | 0.0% |
| Epoch 2 | 20.8% | 8.0% | 60.5% | 50.3% | **0.0%** | 0.0% |
| Epoch 3 | 22.1% | 8.2% | 59.3% | 49.8% | 0.0% | 0.0% |
| Epoch 4 | **20.1%** | **7.4%** | 59.3% | 50.2% | 0.0% | 0.0% |
| Epoch 5 | 20.3% | 7.4% | **58.6%** | **49.3%** | 0.0% | 0.0% |

**Best checkpoint used for merge:** Epoch 5 (best combined noisy WER + zero CJK).

---

## Phase 3 — Weight Merging

**Script:** `scripts/merge_lora.py`  
**Command:**
```bash
/opt/vox/venv/bin/python3 /opt/vox/temp/train/merge_lora.py \
  > /opt/vox/temp/logs/merge_lora.log 2>&1
```

### Config

| Parameter | Value |
|---|---|
| Base model path | `/opt/vox/Qwen3-ASR-0.6b` |
| LoRA adapter path | `/opt/vox/temp/train/checkpoints/best/` |
| Output path | `/opt/vox/temp/merged/qwen3-asr-0.6b-vox-v1` |
| Merge method | `peft.merge_adapter()` |

**Known issue fixed:** `GenerationConfig` validation error — `temperature` must be unset when `do_sample=False`. Fixed by programmatically clearing the field before save.

**Output size:** ~1.8GB (full fp32 safetensors, same as base model)

---

## Phase 4 — ONNX Export & INT8 Quantization

**Script:** `scripts/export_onnx.py` (based on wasser-onnx framework)  
**Command:**
```bash
/opt/vox/venv/bin/python3 /opt/vox/temp/train/export_wasser.py \
  > /opt/vox/temp/logs/export_wasser.log 2>&1
```

### Architecture Split

The Qwen3-ASR model is exported as three separate ONNX graphs, matching how `sherpa-onnx` loads them:

| Component | File | Size | Quantization |
|---|---|---|---|
| Convolutional frontend (mel+subsampling) | `conv_frontend.onnx` | 44 MB | FP32 |
| Transformer encoder | `encoder.int8.onnx` | 182 MB | INT8 dynamic |
| LM decoder (attention + embedding) | `decoder.int8.onnx` | 756 MB | INT8 dynamic |

### Known Issues Fixed During Export

1. **NumPy 2.x incompatibility** — `onnxruntime-tools` requires NumPy < 2.0. Downgraded to `numpy==1.26.4`.
2. **Missing registration import** — `export_wasser.py` required `from wasser.register import ...` before model load.
3. **TorchScript TracerWarning** — `conv_chunksize` conditional produces a tracing warning (not an error); the exported graph is correct for the default chunk size.

### Quantization Parameters

- **Method:** `onnxruntime.quantization.quantize_dynamic`
- **Weight type:** `QUInt8`
- **Op types quantized:** `MatMul`, `Gemm`
- **Per-channel:** Yes (encoder/decoder)

### Deployment

```
/opt/vox/models/onnx-0.6b-finetuned/
  conv_frontend.onnx
  encoder.int8.onnx
  decoder.int8.onnx
  tokenizer/
  test_wavs/
```

Previous (original, pre-finetune) ONNX weights backed up to:
```
/opt/vox/models/onnx-0.6b-original/
```

---

## Phase 5 — Offline Evaluation

**Script:** `scripts/run_offline_eval.py`  
**Parallelism:** `ProcessPoolExecutor(max_workers=8)` — 8 parallel eval workers  
**Runtime:** ~3 hours total (clean + hinglish + noisy + negatives × 2 models)

### Metrics Computed

- `corpus_wer` — aggregate WER across all samples
- `avg_wer` — mean per-sample WER
- `corpus_cer` / `avg_cer` — same for character error rate
- `hallucination_rate` — fraction of samples with any non-empty output on a noise-only input OR unexpected language
- `cjk_rate` — fraction of output tokens in CJK unicode range (U+4E00–U+9FFF)
- `empty_rate` — fraction of samples producing empty transcript
- `false_trigger_rate` — fraction of negative (silence) samples that produce non-empty output
- `avg_rtf` / `overall_rtf` — real-time factor (elapsed / audio duration)

### Usage

```bash
python3 scripts/run_offline_eval.py \
  --model_dir /opt/vox/models/onnx-0.6b-finetuned \
  --test_dir /opt/vox/vox-qwen-asr-hindi \
  --output_dir /opt/vox/temp/eval/results \
  --workers 8
```

---

## Phase 6 — Streaming Simulation

**Script:** `scripts/streaming_sim.py`  
**Config evaluated:** window=2.0s, step=0.8s (production config)

### Metrics

- `avg_ttft_sec` — time to first token (first chunk decode latency)
- `avg_pfr` — partial flip rate (fraction of chunks where final transcript differs from partial)
- `total_flips` — absolute count of transcript reversals
- `total_transient_cjk` — CJK tokens appearing in any partial chunk output
- `avg_wer_vs_offline` — ratio of streaming WER to offline WER (1.0 = identical)
- `avg_cer_vs_offline` — same for CER

### Usage

```bash
python3 scripts/streaming_sim.py \
  --model_dir /opt/vox/models/onnx-0.6b-finetuned \
  --test_dir /opt/vox/vox-qwen-asr-hindi/benchmark \
  --window 2.0 \
  --step 0.8 \
  --output /opt/vox/temp/eval/results/streaming_sim_results_0.6b.json
```

---

## Environment

```
Python: 3.11
PyTorch: 2.x (CUDA 12.x)
transformers: 4.46+
peft: 0.13+
bitsandbytes: 0.43+
onnxruntime: 1.18+
numpy: 1.26.4  ← must be <2.0 for onnxruntime-tools
sherpa-onnx: 1.10+
```

```bash
# Activate environment
source /opt/vox/venv/bin/activate
```

---

## Model Directory Structure

```
/opt/vox/models/
  onnx-0.6b-original/      ← pre-finetune ONNX (backup)
  onnx-0.6b-finetuned/     ← live fine-tuned ONNX (INT8, deployed)
  pytorch-0.6b-original/   ← Qwen3-ASR-0.6B HuggingFace weights
  pytorch-0.6b-finetuned/  ← merged LoRA weights (vox-v1)
  pytorch-1.7b-original/   ← Qwen3-ASR-1.7B HuggingFace weights (reference)
```

All entries are symlinks — no data is duplicated.
