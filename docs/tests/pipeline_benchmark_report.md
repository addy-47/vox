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

### Baseline Execution Metrics (Run ID: `20260905_120532_6e778a48`)

| Pipeline Stage / Milestone | Wall-Clock Timestamp | Phase Duration | Latency / Throughput |
| :--- | :--- | :--- | :--- |
| **Audio Ingestion Start** | `+0.00s` | - | 7.25s audio streamed in 20ms chunks |
| **VAD Speech Start** | `+0.12s` | - | Speech onset detected |
| **VAD Speech End** | `+3.63s` | - | Speech cessation detected |
| **STT Final Transcript** | `+7.34s` | **3,714.2 ms** | Post-speech STT latency |
| **LLM First Token** | `+7.65s` | **308.5 ms** | Time-To-First-Token (TTFT) |
| **TTS First Audio Chunk** | `+9.02s` | **1,372.4 ms** | First synthesized buffer ready |
| **Speaker Audio Playback** | `+9.08s` | **5,448.9 ms** | **Perceived Latency (Speech End → Audio Out)** |
| **Pipeline Idle Complete** | `+10.45s` | **3.11s** | Total LLM/TTS generation time |

### STT & LLM Transcription / Output Quality
- **STT Output:** `"good morning team lets review the quarterly metric so pipelines are operating within nominal parameters"`
- **LLM Prompt Context:** `"Answer in 1-2 concise sentences."`
- **LLM Response:** `"The quarterly metrics review is underway, and all pipelines are currently operating normally within expected parameters."`
- **Synthesized TTS Audio:** `4.86s` (116,608 samples @ 24 kHz)
- **Synthesized Audio Path:** `app/src-tauri/benches/results/pipeline_bench/20260905_120532_6e778a48/wav/modular_passive_qwen_kokoro.wav`

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

