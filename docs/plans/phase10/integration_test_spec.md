# Integration Test Specification — Phase 10 Subsystem Real-Model Verification

---

## 1. Overview & Testing Philosophy

This document defines the **Single Source of Truth (SSOT)** for Integration Tests (IT) in Vox.
Following the testing pyramid (**UT → IT → E2E → Benches**), Integration Tests:
1. **Test Standalone Subsystems in Isolation**: Verify one engine at a time against real model weights and real audio fixtures.
2. **Execute Stage-by-Stage**: Isolate components before assembling full domain loops.
3. **Assert Semantic & Acoustic Correctness**: Prove that the real model outputs match expected ground-truth text/audio criteria.
4. **Resilience**: Skip gracefully (`eprintln!` or test ignore) if optional multi-GB weights are absent, but assert strict correctness when present.

---

## 2. Integration Test Matrix

| # | Test Target | Test File | Component Under Test | Fixtures & Models Used | Core Assertions & Metrics |
|---|-------------|-----------|----------------------|------------------------|---------------------------|
| **IT-1** | Acoustic STT Engine | `tests/stt_test.rs` | `services::stt::nemotron_onnx::SttEngine` (Parakeet FastConformer-RNNT) | • Model: `~/.vox/models/stt/nvidia_nemotron`<br>• Audio: `clip_01_en_briefing.wav`, `clip_06_hi_greeting.wav` | • 4 distinct execution passes (EN Batch, EN Stride, HI Batch, HI Stride).<br>• Normalized Word Error Rate (WER) / Similarity $\ge 85\%$.<br>• Batch vs Stride streaming output equivalence. |
| **IT-2** | Neural TTS Synthesis | `tests/tts_test.rs` | *[TBD in next grill-me]* | *[TBD]* | *[TBD]* |
| **IT-3** | Local LLM Generation | `tests/llm_test.rs` | *[TBD in next grill-me]* | *[TBD]* | *[TBD]* |
| **IT-4** | Memory DB & Graph | `tests/memory_test.rs` | *[TBD in next grill-me]* | *[TBD]* | *[TBD]* |
| **IT-5** | Hindi Transliteration | `tests/translit_test.rs` | *[TBD in next grill-me]* | *[TBD]* | *[TBD]* |

---

## 3. Candidate 1 Specification: Real Acoustic STT (`tests/stt_test.rs`)

### 3.1 Scope & Prerequisites
- **Target Engine:** NVIDIA Nemotron-3.5 FastConformer-RNNT ONNX model (`parakeet-rs`).
- **Model Path:** Resolved via `vox_lib::utils::paths::model_dir("stt").join("nvidia_nemotron")` or `~/.vox/models/stt/nvidia_nemotron`.
- **Audio Fixtures:**
  1. **English:** `app/src-tauri/test-clips/clip_01_en_briefing.wav`
     - *Spoken Text:* `"Hey Vox, good morning! Can you check my calendar and give me a quick briefing on today's scheduled meetings?"`
  2. **Hindi:** `app/src-tauri/test-clips/clip_06_hi_greeting.wav`
     - *Spoken Text:* `"हे वॉक्स, नमस्ते! क्या आप मेरा आज का शेड्यूल देखकर बता सकते हैं कि मेरी अगली मीटिंग कब है?"`

---

### 3.2 Execution Modes (4 Inferences in Total)

The test executes exactly **4 transcription passes**:

1. **Pass 1 — English Full Batch (`clip_01` Full Buffer):**
   - Decodes `clip_01_en_briefing.wav` into a continuous 16kHz mono `&[f32]` slice.
   - Calls `engine.transcribe(&audio_samples)`.
   - Asserts normalized Levenshtein similarity $\ge 85\%$ against English ground truth.

2. **Pass 2 — English Incremental Stride Streaming (`clip_01` 8960-Sample Strides):**
   - Simulates real-time microphone buffer flow by slicing `clip_01_en_briefing.wav` into discrete `STRIDE_SAMPLES = 8960` frames ($560\text{ms}$ chunks).
   - Ingests strides sequentially through model chunk interface.
   - Asserts:
     - Output text is non-empty.
     - Stride output matches Pass 1 (Batch) output with $\ge 95\%$ similarity.

3. **Pass 3 — Hindi Full Batch (`clip_06` Full Buffer):**
   - Decodes `clip_06_hi_greeting.wav` into 16kHz mono `&[f32]` slice.
   - Calls `engine.transcribe(&audio_samples)`.
   - Asserts Devanagari character presence and similarity $\ge 80\%$ against Hindi ground truth.

4. **Pass 4 — Hindi Incremental Stride Streaming (`clip_06` 8960-Sample Strides):**
   - Slices `clip_06_hi_greeting.wav` into 8960-sample strides.
   - Ingests strides sequentially.
   - Asserts stride streaming matches Pass 3 (Batch) output.

---

### 3.3 Evaluation Metrics & Normalization Rules

- **Text Normalization:**
  - Strip punctuation: `. , ! ? । " ' -`
  - Normalize multiple whitespaces to single space.
  - Lowercase Latin characters.
- **Similarity Assertion:**
  $$\text{Similarity}(A, B) = 1.0 - \frac{\text{Levenshtein}(A, B)}{\max(|A|, |B|)}$$
  - Must be $\ge 0.85$ for English and $\ge 0.80$ for Hindi.
- **Deterministic & Leak-Free:** Model states (`model.reset()`) must be called between passes to guarantee zero cross-turn acoustic context contamination.
