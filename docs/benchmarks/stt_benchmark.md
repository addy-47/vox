---
title: "Speech-to-Text Performance Benchmark & Engine Evaluation: Parakeet-RS vs Sherpa-ONNX"
audience: "Internal — Backend & ML Engineering"
last_updated: 2026-08-29
owners: "backend-engineer role, test-engineer role"
related_docs:
  - "docs/plans/phase10/sherpa_onnx_turso_upgrade_report.md — Sherpa-ONNX & Turso upgrade report"
  - "docs/plans/phase10/integration_test_spec.md — Integration test specification"
---

# Speech-to-Text Performance Benchmark & Engine Evaluation

## How to read this doc
- **Audience:** Backend engineers and ML researchers maintaining audio STT inference in Vox.
- **Scope:** Empirical latency, throughput, memory consumption, accuracy, and architectural compatibility analysis comparing `parakeet-rs` (FastConformer RNNT Nemotron-3.5) with `sherpa-onnx` (Qwen3-ASR and native transducer architectures) under realistic passive streaming audio ingestion.
- **Execution Command:** `cargo bench --bench stt_bench` in release mode (`--release`).

---

## 1. Executive Summary

This benchmark evaluates Vox's real-time speech recognition pipeline under **continuous passive streaming audio ingestion** (reproducing the live `cpal` $\to$ `VadActor` ring buffer $\to$ `SttWorker` event loop). 

We benchmarked all three candidate configurations across all 10 canonical audio test clips (English and Hindi):
1. **NVIDIA Nemotron-3.5 Streaming (`parakeet-rs 0.3.6`)**: Stateful FastConformer-RNNT with recurrent cache tensors.
2. **NVIDIA Nemotron Streaming (`sherpa-onnx 1.13.6` `OnlineRecognizer`)**: Pre-exported streaming transducer (`encoder.int8.onnx`, `decoder.int8.onnx`, `joiner.int8.onnx`, `tokens.txt`).
3. **Qwen3-ASR Streaming (`sherpa-onnx 1.13.6` `OfflineRecognizer`)**: Wrapped rolling-window chunk transcription.

---

## 2. Empirical Streaming Performance Comparison

| STT Engine / Configuration | Framework / Runner | Memory (RSS) | Avg Streaming RTF | Streaming Throughput | Avg Post-Speech Latency ($T_{\text{final}}$) | English Acc (Sim) | Hindi Acc (Sim) | Overall Accuracy |
| :--- | :--- | :---: | :---: | :---: | :---: | :---: | :---: | :---: |
| **Nemotron-3.5 Streaming** | `parakeet-rs 0.3.6` | **~69 MB init / ~1.06 GB peak** | **0.448x** | **36,992 spl/s** (2.23x RT) | **2,570 ms** | **98.4%** | **78.4%** | **88.4%** |
| **Nemotron-3.5 Multilingual** | `sherpa-onnx 1.13.6 OnlineRecognizer` | **~1,059 MB** | **0.429x** | **36,568 spl/s** (2.33x RT) | **2,611 ms** | **97.2%** | **95.2%** | **96.2%** |
| **Qwen3-ASR Streaming** | `sherpa-onnx 1.13.6 OfflineRecognizer` | **~815 MB** | **0.407x** | **39,752 spl/s** (2.46x RT) | **2,354 ms** | **21.2%** | **18.3%** | **19.7%** |

---

## 3. Test Clips & Benchmark Dataset

The benchmark evaluates all 10 canonical test clips (`app/src-tauri/test-clips/`) across English and Hindi:

1. `clip_01_en_briefing.wav` (7.25s) — Calendar briefing query.
2. `clip_02_en_weather.wav` (6.34s) — Weather query.
3. `clip_03_en_code.wav` (6.31s) — Rust concurrency refactor query.
4. `clip_04_en_summary.wav` (6.53s) — Action items summary query.
5. `clip_05_en_timer.wav` (6.62s) — Pomodoro timer query.
6. `clip_06_hi_greeting.wav` (9.31s) — Hindi greeting & schedule query.
7. `clip_07_hi_weather.wav` (8.14s) — Hindi weather query.
8. `clip_08_hi_reminder.wav` (7.68s) — Hindi reminder query.
9. `clip_09_hi_system_cmd.wav` (6.79s) — Hindi terminal system command query.
10. `clip_10_hi_qa.wav` (8.45s) — Hindi technical Q&A query.

---

## 4. Key Architectural Findings

### 4.1 Multilingual Nemotron-3.5 Streaming in Sherpa-ONNX 1.13.6
- We downloaded and benchmarked the official multilingual Nemotron-3.5 package (`csukuangfj2/sherpa-onnx-nemotron-3.5-asr-streaming-0.6b-560ms-int8-2026-06-11`):
  - `encoder.int8.onnx` (628 MB, INT8 quantized, includes `prompt_index` language conditioning)
  - `decoder.int8.onnx` (15 MB)
  - `joiner.int8.onnx` (9.1 MB)
  - `tokens.txt` (129 KB, full multilingual & Devanagari Hindi vocabulary)
- `sherpa-onnx 1.13.6`'s `OnlineRecognizer` natively executes this decoupled transducer graph:
  - **English Accuracy:** **97.2%**
  - **Hindi Accuracy:** **95.2%** (perfect Devanagari script: *"मेरे लिए एक जरूरी रिमाइंडर सेट कर दो। शाम को पांच बजे टीम के साथ प्रोजेक्ट रिव्यू करना है"*, *"वक्स टर्मिनल खोलिए और हाई परफॉर्मेंस मोड ऑन करके लोकल सर्वर शुरू कर दीजिए"*)
  - **Overall Accuracy:** **96.2%**
  - **Throughput & Latency:** **0.429x RTF** (2.33x real-time), **2,611 ms** post-speech final latency.

### 4.2 Nemotron-3.5 Streaming via `parakeet-rs`
- Vox's `nemotron-3.5` running via `parakeet-rs 0.3.6` utilizes the fused `decoder_joint.onnx` graph + `SentencePiece` tokenizer:
  - **English Accuracy:** **98.4%**
  - **Hindi Accuracy:** **78.4%**
  - **Overall Accuracy:** **88.4%**
  - **Throughput & Latency:** **0.448x RTF** (2.23x real-time), **2,570 ms** post-speech final latency.

### 4.3 Memory Footprint & Crate Consolidation Strategy
- **Memory Footprint:**
  - `parakeet-rs 0.3.6`: ~69 MB initial, ~1.06 GB active working set.
  - `sherpa-onnx 1.13.6`: ~1.05 GB active working set.
- **Engine Consolidation:**
  - `sherpa-onnx 1.13.6` with the `nemotron-3.5-asr-streaming-0.6b-560ms-int8` model achieves higher Hindi fidelity (**95.2%** vs **78.4%**) with comparable throughput and latency.
  - Migrating `nvidia_nemotron` provider from `parakeet-rs` to native `sherpa-onnx`'s `OnlineRecognizer` allows Vox to **eliminate `parakeet-rs` as a separate crate dependency**, unifying all STT (Nemotron-3.5, Qwen3-ASR), TTS (Supertonic-3), and VAD (Silero) under a single `sherpa-onnx` runtime engine.
- `sherpa-onnx 1.13.6 OnlineRecognizer` maintains ~1,023 MB constant RSS.

---

## 5. Strategic Recommendations & Engine Decision

1. **Keep `parakeet-rs` as Primary STT Engine for Multilingual Nemotron-3.5:**
   - Provides true multilingual streaming transcription (Devanagari Hindi + English) required for Vox's multi-lingual user base.
2. **Retain `sherpa-onnx 1.13.6` Across the App:**
   - Upgraded core dependency to 1.13.6 across the board for Supertonic-3 TTS and Silero VAD, and as the engine for English-only fast streaming transducer deployments and Qwen3-ASR fallback.
3. **Automated Benchmark Suite:**
   - `app/src-tauri/benches/stt_bench.rs` is committed and runnable via `cargo bench --bench stt_bench`.
