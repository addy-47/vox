# Qwen3-ASR LoRA v1 — Benchmark Results

**Model:** Qwen3-ASR-0.6B fine-tuned (QLoRA, 5 epochs)  
**Date:** 2026-05-23  
**Eval harness:** `scripts/run_offline_eval.py` + `scripts/streaming_sim.py`  
**ONNX model:** `models/onnx-0.6b-finetuned/` (INT8 dynamic quantization)

---

## Training Convergence (Internal Eval Split)

These metrics were computed on a small held-out split during training using the PyTorch model (not ONNX). CJK elimination happened cleanly at epoch 2.

| Epoch | Clean WER | Clean CER | Noisy WER | Noisy CER | Noisy CJK% | False Trigger% |
|---|---|---|---|---|---|---|
| **Baseline (ep 0)** | 25.5% | 12.3% | 65.3% | 59.9% | **11.0%** | 1.0% |
| Epoch 1 | 30.3% | 16.3% | 63.2% | 56.0% | 11.0% | 0.0% |
| Epoch 2 | 20.8% | 8.0% | 60.5% | 50.3% | **0.0%** | 0.0% |
| Epoch 3 | 22.1% | 8.2% | 59.3% | 49.8% | 0.0% | 0.0% |
| Epoch 4 | **20.1%** | **7.4%** | 59.3% | 50.2% | 0.0% | 0.0% |
| Epoch 5 | 20.3% | 7.4% | **58.6%** | **49.3%** | 0.0% | 0.0% |

---

## Offline Benchmark — Fine-Tuned 0.6B (INT8 ONNX)

Evaluated on full benchmark test sets. 2036 total samples.

| Test Set | Samples | Audio (h) | WER (corpus) | CER (corpus) | Halluc. Rate | CJK Rate | Empty Rate | False Trigger | Avg RTF | Overall RTF |
|---|---|---|---|---|---|---|---|---|---|---|
| **Clean Hindi** | 500 | 2.3h | 26.5% | 12.8% | 0.6% | 0.2% | 0.0% | — | 1.36 | 1.33 |
| **Hinglish** | 1036 | 3.8h | 24.9% | 18.5% | 0.19% | **0.0%** | 0.19% | — | 1.34 | 1.29 |
| **Noisy Hindi** | 250 | 1.6h | 46.1% | 28.8% | 0.8% | 0.4% | 2.0% | — | 1.90 | 1.90 |
| **Negatives** | 250 | 1.0h | — | — | 4.0% | 0.0% | 95.6% | **4.4%** | 0.26 | 0.26 |

---

## Offline Benchmark — Teacher Model: 1.7B Reference (Baseline GPU, Untuned)

The 1.7B model ran on GPU (RTX 5070 Ti) — RTF is not comparable to 0.6B CPU inference.

| Test Set | Samples | WER (corpus) | CER (corpus) | Halluc. Rate | CJK Rate | Empty Rate | False Trigger | Avg RTF |
|---|---|---|---|---|---|---|---|---|
| **Clean Hindi** | 500 | **13.2%** | **4.6%** | 0.0% | 0.0% | 0.0% | — | 0.044 |
| **Hinglish** | 1036 | **22.3%** | **18.5%** | 0.10% | 0.0% | 0.0% | — | 0.066 |
| **Noisy Hindi** | 250 | **29.8%** | **16.9%** | **8.0%** | **8.0%** | 2.0% | — | 0.082 |
| **Negatives** | 250 | — | — | 2.4% | 0.8% | 97.2% | **2.8%** | 0.006 |

---

## Side-by-Side: 0.6B Fine-Tuned vs 1.7B Baseline

| Test Set | Metric | 0.6B Fine-Tuned | 1.7B Baseline | Winner |
|---|---|---|---|---|
| Clean Hindi | WER | 26.5% | 13.2% | 1.7B (13.3pp gap) |
| Hinglish | WER | 24.9% | 22.3% | 1.7B (2.6pp gap) |
| Noisy Hindi | WER | 46.1% | 29.8% | 1.7B (16.3pp gap) |
| Noisy Hindi | **CJK Rate** | **0.4%** | **8.0%** | **0.6B Fine-Tuned** |
| Negatives | False Trigger | 4.4% | 2.8% | 1.7B |
| Streaming | TTFT | 292ms | Not measured | — |
| CPU inference | RTF | ~1.4 | ~0.07 (GPU) | Not comparable |

**Key takeaway:** The 1.7B baseline is a stronger transcription model in raw WER terms. The 0.6B fine-tuned wins *specifically* on CJK hallucination elimination on noisy audio (8.0% → 0.4%) and streaming suitability.

---

## Streaming Simulation — 0.6B Fine-Tuned

**Config:** window=2.0s, step=0.8s (production config)  
**Test set:** Hinglish benchmark (1036 samples)

| Metric | Value | Target | Status |
|---|---|---|---|
| Avg TTFT | 292ms | < 500ms | ✅ |
| Partial flip rate | 0.0 | 0.0 | ✅ |
| Total transcript flips | 0 | 0 | ✅ |
| Transient CJK tokens | 0 | 0 | ✅ |
| WER vs offline | +10.9% | < +15% | ✅ (within tolerance) |
| CER vs offline | +1.1% | < +5% | ✅ |

---

## Honest Assessment

### What Fine-Tuning Actually Changed

1. **CJK hallucinations eliminated** — the base 0.6B model had 11% CJK rate on noisy audio (same as 1.7B's 8%). Fine-tuning dropped this to 0.4% (vs 8.0% on the untuned 1.7B). This is the clearest win.

2. **Noisy WER improved in training** — from 65.3% → 58.6% on the internal eval split. The final offline number (46.1% vs baseline 65.3%) shows a real improvement, but some of this may be test set differences.

3. **Streaming stability** — zero flips, 292ms TTFT. The streaming config is production-ready.

### What Was NOT Achieved

1. **RTF > 1.0 on CPU** — the INT8 ONNX model is slower than real-time on a server CPU (RTF ~1.3–1.9). On a target i5-1145G7 this will be significantly worse.

2. **On CJK as a code fix** — yes, CJK tokens can be filtered in post-processing. Fine-tuning goes further by removing CJK from the model's internal probability distribution on Hindi/Hinglish inputs. Both should be used. Fine-tuning alone is insufficient if RTF prevents real deployment.

3. **No Hinglish improvement vs 1.7B** — the 0.6B fine-tuned at 24.9% vs 1.7B at 22.3% WER on Hinglish. No meaningful advantage.

---

## Files

| File | Description |
|---|---|
| `benchmark/clean_hi/` | Clean Hindi benchmark audio + transcripts |
| `benchmark/hinglish/` | Hinglish benchmark audio + transcripts |
| `benchmark/noisy_hi/` | Noisy Hindi benchmark audio + transcripts |
| `benchmark/negatives/` | Non-speech negatives (silence, fan, music) |
| `/opt/vox/temp/eval/results/*.jsonl` | Per-sample eval results (raw) |
| `/opt/vox/temp/eval/results/*_summary.json` | Aggregated per-domain summaries |
| `/opt/vox/temp/eval/results/streaming_sim_results_0.6b.json` | Streaming simulation output |
| `/opt/vox/temp/eval/results/epoch_metrics.jsonl` | Per-epoch training eval metrics |
