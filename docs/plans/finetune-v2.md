# Vox ASR — Next Steps Plan (Post LoRA v1)

**Created:** 2026-05-23  
**Context:** Following successful completion of QLoRA fine-tuning, ONNX export, and evaluation of Qwen3-ASR-0.6B for Hindi/Hinglish Vox deployment.

---

## Current State

| Component | Status |
|---|---|
| LoRA fine-tuned 0.6B (PyTorch) | ✅ `models/pytorch-0.6b-finetuned` |
| ONNX INT8 fine-tuned 0.6B | ✅ `models/onnx-0.6b-finetuned` (live) |
| Offline benchmark harness | ✅ `scripts/run_offline_eval.py` |
| Streaming simulation harness | ✅ `scripts/streaming_sim.py` |
| CJK hallucinations (noisy) | ✅ 0.4% (down from 11%) |
| Streaming TTFT | ✅ 292ms, zero flips |
| **CPU RTF** | ❌ ~1.4 (needs to be < 0.8) |
| Noisy WER gap vs 1.7B | ❌ 46% vs 30% (16pp gap) |
| On-device benchmark | ❌ Not run on i5-1145G7 target |

---

## Priority 1 — RTF Fix (Blocking Deployment)

**Problem:** RTF 1.34–1.90 on server CPU. Not real-time. Unshippable.

### Option A: sherpa-onnx thread tuning (Low effort, test first)
```bash
# Test with different thread counts
python3 -c "
import sherpa_onnx, time
rec = sherpa_onnx.OfflineRecognizer.from_whisper(...)
# Benchmark with num_threads=2, 4, 6
"
```
- `num_threads=4` on an i5 quad-core is often optimal
- Avoid OMP oversubscription — don't set higher than physical core count

### Option B: INT4 decoder re-quantization
The decoder is 756MB at INT8. INT4 can halve this and improve CPU throughput ~30%.
```bash
python3 -c "
from onnxruntime.quantization import quantize_dynamic, QuantType
quantize_dynamic('decoder.int8.onnx', 'decoder.int4.onnx', weight_type=QuantType.QInt4)
"
```
Risk: Accuracy degradation on Hinglish. Must re-benchmark after.

### Option C: Encoder beam pruning / faster decoding
- Reduce beam size from 5 → 3 in sherpa-onnx config
- Use greedy decode instead of beam for streaming (TTFT drops, accuracy ~same)

### Verification
```bash
# Run streaming sim and measure RTF on target hardware
python3 scripts/streaming_sim.py --model_dir models/onnx-0.6b-finetuned --window 2.0 --step 0.8
# Target: avg_rtf < 0.8
```

---

## Priority 2 — Noise Augmentation v2 (High Impact)

**Problem:** Noisy Hindi WER is 46.1%. This is the worst-performing domain and the one Vox faces most (desktop mics, fan noise, AC).

### What to add to training corpus

| Noise type | Source | Target SNR | Volume |
|---|---|---|---|
| Fan/AC hum | CAIMAN-ASR noise lib | 5–15 dB | +2000 clips |
| Keyboard typing | Record or FreeSound | 15–25 dB | +500 clips |
| Room reverb (IR) | OpenAIR library | — (convolve) | all clean clips |
| TV/radio in background | YouTube clips | 10–20 dB | +1000 clips |
| Mic clipping/saturation | Simulate with scipy | — | +500 clips |

### Script to update
`scripts/compile_corpus.py` — add noise augmentation pipeline section.

### Expected outcome
- Noisy WER: 46% → 35% (estimated)
- CJK: Should stay at 0% if similar LoRA config

### Training config changes for v2
```python
# Increase noisy sample weight in training loss
sample_weights = {
    "clean_hi": 1.0,
    "hinglish": 1.2,
    "noisy_hi": 2.0,   # upweight noisy domain
    "negatives": 1.5,
}
# Consider reverb + multi-condition training
```

---

## Priority 3 — Streaming Transcript Stitching

**Problem:** Streaming WER is +10.9% vs offline. Chunk boundaries cut words.

### Approach
Overlap-stitch: keep last N tokens from previous window and find best alignment with first M tokens of next window.

```python
def stitch_chunks(prev_text: str, next_text: str, overlap_words: int = 3) -> str:
    prev_words = prev_text.split()
    next_words = next_text.split()
    # Find longest suffix of prev that is prefix of next
    for n in range(min(overlap_words, len(prev_words)), 0, -1):
        if prev_words[-n:] == next_words[:n]:
            return " ".join(prev_words + next_words[n:])
    return prev_words + next_words  # no overlap found, concatenate
```

Target: WER drift +10.9% → < +3%.

Add to `scripts/streaming_sim.py` as `--stitch` flag.

---

## Priority 4 — False Trigger Reduction

**Problem:** 4.4% of silence/noise samples produce a transcript (false trigger).

### Root cause
Training negatives were ~800 clips. Not enough diversity. Model is over-confident on certain noise patterns.

### Fix
Add to negatives corpus:
- Background music clips (50–100 clips)
- TV dialogue in English (not Hindi — should be rejected)
- Pure silence (already have, add more)
- Outdoor ambient sounds

Also add confidence thresholding in sherpa-onnx runtime:
```python
# Reject output if log-prob below threshold
if result.log_prob < -2.5:
    return ""
```

---

## Priority 5 — On-Device Benchmark

**Why this matters:** All current RTF numbers are from a 32-core server. The deployment target is an i5-1145G7 (4 cores, no GPU).

### Steps
1. Copy `models/onnx-0.6b-finetuned/` to target machine
2. Install sherpa-onnx CPU-only build
3. Run `scripts/streaming_sim.py` with `--num_threads 4`
4. Record RTF, TTFT, and WER on benchmark set

If RTF > 1.0 on target after Priority 1 fixes → consider:
- Shipping the 0.6B as a server-side model (not on-device)
- OR pursuing a smaller distilled model (Qwen3-ASR-0.5B if released, or WhisperTiny fine-tuned)

---

## Optional — CJK Token Filtering (Defense in Depth)

Even though fine-tuning eliminated CJK from the model's output distribution, add a post-processing filter as a safety net:

```python
import unicodedata

CJK_RANGES = [(0x4E00, 0x9FFF), (0x3400, 0x4DBF), (0xF900, 0xFAFF)]

def strip_cjk(text: str) -> str:
    return "".join(
        c for c in text
        if not any(lo <= ord(c) <= hi for lo, hi in CJK_RANGES)
    )
```

Add this to `vox-app` ASR post-processing pipeline.

---

## Parking Lot (Low Priority / Future)

- **1.7B LoRA fine-tune** — would give best accuracy but needs more VRAM (may need A100/H100 or CPU offload)
- **Streaming CTC head** — replace autoregressive decoder with CTC for 10× faster streaming
- **Distillation** — use 1.7B as teacher to distill into a smaller student model
- **Hindi-specific tokenizer** — Qwen's BPE tokenizer was trained on Chinese-dominant data; a Hindi-first tokenizer could reduce WER significantly
- **WFST language model rescoring** — for proper nouns and domain vocabulary

---

## File Locations

| Item | Path |
|---|---|
| Fine-tuned ONNX (live) | `/opt/vox/models/onnx-0.6b-finetuned/` |
| Fine-tuned PyTorch | `/opt/vox/models/pytorch-0.6b-finetuned/` |
| Original ONNX (backup) | `/opt/vox/models/onnx-0.6b-original/` |
| Training script | `/opt/vox/vox-qwen-asr-hindi/scripts/train_lora.py` |
| Export script | `/opt/vox/vox-qwen-asr-hindi/scripts/export_onnx.py` |
| Eval harness | `/opt/vox/vox-qwen-asr-hindi/scripts/run_offline_eval.py` |
| Streaming sim | `/opt/vox/vox-qwen-asr-hindi/scripts/streaming_sim.py` |
| Benchmark results | `/opt/vox/vox-qwen-asr-hindi/benchmark/qwen_asr_lora_results.md` |
| Process docs | `/opt/vox/vox-app/features/qwen-asr-finetuning.md` |
| Dataset repo | `/opt/vox/vox-qwen-asr-hindi/` |
