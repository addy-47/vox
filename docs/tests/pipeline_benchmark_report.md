# Vox End-to-End Pipeline Benchmark Report

**Date:** 2026-09-05  
**Version:** Vox v0.1.0  
**Engine:** `pipeline_bench` (Release Mode, Single-Threaded Isolation)  
**Input Audio:** `clip_01_en_briefing.wav` (7.25s duration, 16kHz mono PCM)  
**Test Audio Transcript Ground Truth:**  
> "Good morning team, let's review the quarterly metrics. All pipelines are operating within nominal parameters."

---

## 1. Executive Summary & Baseline Metrics

The initial end-to-end benchmark was executed on the default production stack:
- **Speech-to-Text (STT):** `Nemotron-3.5` (Sherpa-ONNX streaming transducer)
- **Large Language Model (LLM):** `Qwen 2.5-3B-Instruct` (GGUF via `llama.cpp`)
- **Text-to-Speech (TTS):** `Kokoro v1.0` (Sherpa-ONNX offline engine, voice: `af_heart`)
- **Mode:** `modular_passive` (Audio -> VAD -> Nemotron STT -> Qwen LLM -> Kokoro TTS -> Playback)

### Baseline Execution Metrics (Run ID: `20260905_125146_7ddbc968`)

| Pipeline Stage / Milestone | Wall-Clock Timestamp | Phase Duration | Latency / Description |
| :--- | :--- | :--- | :--- |
| **Audio Ingestion Start** | `+0.00s` | - | 7.25s audio streamed in real-time 16ms frames |
| **VAD Speech Start** | `+0.26s` | - | Speech onset detected |
| **VAD Speech End** | `+2.34s` | - | Speech cessation detected |
| **STT Turn 0 Transcript** | `+2.98s` | **640 ms** | `"Hey Vox, good morning"` |
| **STT Final Query Transcript** | `+8.65s` | **6,305.0 ms** | `"Can you check my calendar and give me a quick briefing on today's scheduled meetings"` |
| **Speaker Audio Playback (TTFB)** | `+13.59s` | **11,242.6 ms** | **Perceived E2E Latency (Speech End → First Audio Byte)** |
| **Synthesized Audio Duration** | - | **3.68 s** | 88,213 samples @ 24 kHz (16-bit PCM integer mono WAV) |
| **Peak Process Memory (RSS)** | - | - | ~1,241 MB |

### Verified Output Artifacts (Manual Verification)
1. **STT Final Transcript:**
   > `"Can you check my calendar and give me a quick briefing on today's scheduled meetings"`
2. **LLM Assistant Response:**
   > `"Meeting with you at 10:00 AM on 1:1."`
3. **Synthesized TTS Audio:**
   - **Path:** `app/src-tauri/benches/results/pipeline_bench/20260905_125146_7ddbc968/wav/modular_passive_qwen_kokoro.wav`
   - **Artifact Copy:** [modular_passive_qwen_kokoro.wav](file:///home/addy/.gemini/antigravity/brain/83b40d6b-650d-47be-9961-ae8f001822d0/modular_passive_qwen_kokoro.wav)
   - **Format:** Microsoft PCM, 16-bit mono, 24,000 Hz (3.68 seconds, 88,213 samples, 172 KB)

---

## 2. Complete Combinatorial Test Matrix

All combinations below use **Nemotron STT** across the entire matrix as specified.

### Domain 1: Modular Passive (`--mode modular_passive`)
*Full voice turn triggered autonomously by VAD speech-end detection.*

| Pair ID | STT Engine | LLM Engine | TTS Engine | Status | CLI Command |
| :--- | :--- | :--- | :--- | :--- | :--- |
| `MP-01` | Nemotron | Qwen 2.5 (Local GGUF) | Kokoro (Local ONNX) | **Tested (Baseline)** | `cargo test --bench pipeline_bench --release -- --mode modular_passive --stt nemotron --llm qwen --tts kokoro` |
| `MP-02` | Nemotron | Qwen 2.5 (Local GGUF) | Supertonic (Local ONNX) | Ready | `cargo test --bench pipeline_bench --release -- --mode modular_passive --stt nemotron --llm qwen --tts supertonic` |
| `MP-03` | Nemotron | Qwen 2.5 (Local GGUF) | EdgeTTS (Cloud / Remote) | Ready | `cargo test --bench pipeline_bench --release -- --mode modular_passive --stt nemotron --llm qwen --tts edge` |
| `MP-04` | Nemotron | DeepSeek-R1 (Local GGUF) | Kokoro (Local ONNX) | Ready | `cargo test --bench pipeline_bench --release -- --mode modular_passive --stt nemotron --llm deepseek --tts kokoro` |
| `MP-05` | Nemotron | DeepSeek-R1 (Local GGUF) | Supertonic (Local ONNX) | Ready | `cargo test --bench pipeline_bench --release -- --mode modular_passive --stt nemotron --llm deepseek --tts supertonic` |
| `MP-06` | Nemotron | DeepSeek-R1 (Local GGUF) | EdgeTTS (Cloud / Remote) | Ready | `cargo test --bench pipeline_bench --release -- --mode modular_passive --stt nemotron --llm deepseek --tts edge` |
| `MP-07` | Nemotron | Claude 3.5 Sonnet (Anthropic Cloud) | Kokoro (Local ONNX) | Ready (API Key Req) | `cargo test --bench pipeline_bench --release -- --mode modular_passive --stt nemotron --llm claude --tts kokoro` |
| `MP-08` | Nemotron | Claude 3.5 Sonnet (Anthropic Cloud) | Supertonic (Local ONNX) | Ready (API Key Req) | `cargo test --bench pipeline_bench --release -- --mode modular_passive --stt nemotron --llm claude --tts supertonic` |
| `MP-09` | Nemotron | Claude 3.5 Sonnet (Anthropic Cloud) | EdgeTTS (Cloud / Remote) | Ready (API Key Req) | `cargo test --bench pipeline_bench --release -- --mode modular_passive --stt nemotron --llm claude --tts edge` |
| `MP-10` | Nemotron | GPT-4o / Mini (OpenAI Cloud) | Kokoro (Local ONNX) | Ready (API Key Req) | `cargo test --bench pipeline_bench --release -- --mode modular_passive --stt nemotron --llm openai --tts kokoro` |
| `MP-11` | Nemotron | GPT-4o / Mini (OpenAI Cloud) | Supertonic (Local ONNX) | Ready (API Key Req) | `cargo test --bench pipeline_bench --release -- --mode modular_passive --stt nemotron --llm openai --tts supertonic` |
| `MP-12` | Nemotron | GPT-4o / Mini (OpenAI Cloud) | EdgeTTS (Cloud / Remote) | Ready (API Key Req) | `cargo test --bench pipeline_bench --release -- --mode modular_passive --stt nemotron --llm openai --tts edge` |

---

### Domain 2: Modular Push-To-Talk (`--mode modular_ptt`)
*User explicitly controls turn boundary via PTT button start/stop events.*

| Pair ID | STT Engine | LLM Engine | TTS Engine | Status | CLI Command |
| :--- | :--- | :--- | :--- | :--- | :--- |
| `MPT-01` | Nemotron | Qwen 2.5 (Local GGUF) | Kokoro (Local ONNX) | Ready | `cargo test --bench pipeline_bench --release -- --mode modular_ptt --stt nemotron --llm qwen --tts kokoro` |
| `MPT-02` | Nemotron | Qwen 2.5 (Local GGUF) | Supertonic (Local ONNX) | Ready | `cargo test --bench pipeline_bench --release -- --mode modular_ptt --stt nemotron --llm qwen --tts supertonic` |
| `MPT-03` | Nemotron | Qwen 2.5 (Local GGUF) | EdgeTTS (Cloud / Remote) | Ready | `cargo test --bench pipeline_bench --release -- --mode modular_ptt --stt nemotron --llm qwen --tts edge` |
| `MPT-04` | Nemotron | DeepSeek-R1 (Local GGUF) | Kokoro (Local ONNX) | Ready | `cargo test --bench pipeline_bench --release -- --mode modular_ptt --stt nemotron --llm deepseek --tts kokoro` |
| `MPT-05` | Nemotron | DeepSeek-R1 (Local GGUF) | Supertonic (Local ONNX) | Ready | `cargo test --bench pipeline_bench --release -- --mode modular_ptt --stt nemotron --llm deepseek --tts supertonic` |
| `MPT-06` | Nemotron | DeepSeek-R1 (Local GGUF) | EdgeTTS (Cloud / Remote) | Ready | `cargo test --bench pipeline_bench --release -- --mode modular_ptt --stt nemotron --llm deepseek --tts edge` |
| `MPT-07` | Nemotron | Claude 3.5 Sonnet (Cloud) | Kokoro (Local ONNX) | Ready (API Key Req) | `cargo test --bench pipeline_bench --release -- --mode modular_ptt --stt nemotron --llm claude --tts kokoro` |
| `MPT-08` | Nemotron | Claude 3.5 Sonnet (Cloud) | Supertonic (Local ONNX) | Ready (API Key Req) | `cargo test --bench pipeline_bench --release -- --mode modular_ptt --stt nemotron --llm claude --tts supertonic` |
| `MPT-09` | Nemotron | Claude 3.5 Sonnet (Cloud) | EdgeTTS (Cloud / Remote) | Ready (API Key Req) | `cargo test --bench pipeline_bench --release -- --mode modular_ptt --stt nemotron --llm claude --tts edge` |
| `MPT-10` | Nemotron | GPT-4o / Mini (Cloud) | Kokoro (Local ONNX) | Ready (API Key Req) | `cargo test --bench pipeline_bench --release -- --mode modular_ptt --stt nemotron --llm openai --tts kokoro` |
| `MPT-11` | Nemotron | GPT-4o / Mini (Cloud) | Supertonic (Local ONNX) | Ready (API Key Req) | `cargo test --bench pipeline_bench --release -- --mode modular_ptt --stt nemotron --llm openai --tts supertonic` |
| `MPT-12` | Nemotron | GPT-4o / Mini (Cloud) | EdgeTTS (Cloud / Remote) | Ready (API Key Req) | `cargo test --bench pipeline_bench --release -- --mode modular_ptt --stt nemotron --llm openai --tts edge` |

---

### Domain 3: Realtime Passive (`--mode realtime_passive`)
*Bidirectional native WebSocket streaming (Gemini Live / OpenAI Realtime).*

| Pair ID | Transport / Provider | Native Speech Recognition | Voice Output Mode | Status | CLI Command |
| :--- | :--- | :--- | :--- | :--- | :--- |
| `RP-01` | Gemini Live (Bidi WebSocket) | Native Multi-Modal Audio | Gemini Native Audio Stream | Ready (API Key Req) | `cargo test --bench pipeline_bench --release -- --mode realtime_passive --llm gemini_live` |
| `RP-02` | OpenAI Realtime (WebSocket) | Native Realtime Audio | OpenAI Native Audio Stream | Ready (API Key Req) | `cargo test --bench pipeline_bench --release -- --mode realtime_passive --llm openai_realtime` |

---

### Domain 4: Realtime Push-To-Talk (`--mode realtime_ptt`)
*Push-to-talk gating on bidirectional native audio streaming channels.*

| Pair ID | Transport / Provider | Gate Mechanism | Voice Output Mode | Status | CLI Command |
| :--- | :--- | :--- | :--- | :--- | :--- |
| `RPT-01` | Gemini Live (Bidi WebSocket) | Button Press -> Mic Open -> Release -> Commit | Gemini Native Audio Stream | Ready (API Key Req) | `cargo test --bench pipeline_bench --release -- --mode realtime_ptt --llm gemini_live` |
| `RPT-02` | OpenAI Realtime (WebSocket) | Button Press -> Mic Open -> Release -> Commit | OpenAI Native Audio Stream | Ready (API Key Req) | `cargo test --bench pipeline_bench --release -- --mode realtime_ptt --llm openai_realtime` |

