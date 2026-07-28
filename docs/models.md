# Vox — Model Inventory & Specifications

> **Complete reference of all models used in Vox**, their parameters, quantization, inference engines, load paths, and category-specific algorithms. This is a model catalog, not an architecture document — see `docs/backend.md` for threading, lifecycle, and system architecture.

---

## 1. Model Inventory — Complete Reference

### 1.1 Realtime Pipeline Models

| Role | Model | ID | Params | Quant | Footprint | Engine | Path | Manifest |
| :--- | :--- | :--- | :---: | :---: | :---: | :--- | :--- | :---: |
| **VAD** (default) | Earshot VAD | `earshot` | Embedded | — | < 1 MB | Rust-native | Embedded (no file) | ❌ |
| **VAD** (legacy) | TenVAD | `ten_vad` | — | INT8 | ~15 MB | sherpa-onnx ONNX | `~/.vox/models/vad/ten_vad.onnx` | ✅ |
| **STT** (primary) | Nvidia Nemotron-3.5 | `nvidia_nemotron` | ~1B | INT8 | ~2.5 GB | parakeet-rs ONNX | `~/.vox/models/stt/nemotron-3.5/` | ✅ |
| **STT** (fallback) | Qwen3-ASR-0.6B | `qwen3_asr` | 0.6B | INT8 | ~800 MB | sherpa-onnx ONNX | `~/.vox/models/stt/qwen3-asr/` | ✅ |
| **LLM** (default) | Llama 3.2 1B Instruct | `llama_3_2_reasoning_q4` | 1B | Q4_K_M | ~750 MB | llama.cpp GGUF | `~/.vox/models/llm/llama/` | ✅ |
| **LLM** (high quality) | Llama 3.2 1B Instruct | `llama_3_2_reasoning` | 1B | Q6_K | ~1.0 GB | llama.cpp GGUF | `~/.vox/models/llm/llama/` | ✅ |
| **LLM** (alternative) | Gemma 4 E2B-it | `gemma_4_reasoning` | ~2B | Q4_K_M | ~1.4 GB | llama.cpp GGUF | `~/.vox/models/llm/gemma4/` | ✅ |
| **LLM** (uncensored) | Gemma 4 Uncensored | `gemma_4_uncensored` | ~2B | Q2_K_P | ~2.9 GB | llama.cpp GGUF | `~/.vox/models/llm/gemma4/` | ✅ |
| **LLM** (cloud) | OpenAI / Gemini / Anthropic | provider-config | — | — | 0 MB (local) | HTTP (reqwest) | Remote API | N/A |
| **TTS** (default) | Supertonic 3 | `supertonic_tts` | 99M | INT8 | ~144 MB | sherpa-onnx ONNX | `~/.vox/models/tts/supertonic-3/` | ✅ |
| **TTS** (local clone) | Chatterbox Local | `chatterbox_tts` | 340M | Q4 GGML | ~1.1 GB | chatterbox-rs GGML | `~/.vox/models/tts/chatterbox/` | ✅ |
| **TTS** (remote) | Chatterbox Remote | `chatterbox_remote` | 340M | Q4 GGML | 0 MB (local) | reqwest HTTP | Remote GPU server | ✅ |

### 1.2 Memory Subsystem Models

| Role | Model | ID | Params | Quant | Footprint | Engine | Path | Manifest |
| :--- | :--- | :--- | :---: | :---: | :---: | :--- | :--- | :---: |
| **Embedding** (primary) | MiniLM-L12 | `minilm-l12-v2` | ~22M | INT8 | ~118 MB | ONNX Runtime (`ort`) | `~/.vox/models/embedding/minilm-l12-v2/` | ❌ |
| **Embedding** (fallback) | BGE-M3 | `bge-m3` | ~568M | INT8 | ~544 MB | ONNX Runtime (`ort`) | `~/.vox/models/embedding/bge-m3/` | ❌ |
| **NLI** | DeBERTa-v3-xsmall | `deberta-v3-xsmall` | ~44M | INT8 | ~233 MB | ONNX Runtime (`ort`) | `~/.vox/models/nli/deberta-v3-xsmall/` | ❌ |
| **NLI** (candidate) | nli-deberta-v3-base | `nli-deberta-v3-base` | ~86M | INT8 | ~233 MB | ONNX Runtime (`ort`) | `~/.vox/models/nli/nli-deberta-v3-base/` | ❌ |
| **Edge Classifier** | LFM2.5-230M | `edge-classifier` | 230M | Q8_0 GGUF | ~235 MB | llama.cpp GGUF | `~/.vox/models/llm/LFM2.5-230M-Q8_0.gguf` | ❌ |
| **Query Gate** | DistilBERT query-sieve | `distilbert-query-classifier` | ~67M | INT8 | — | ONNX Runtime (`ort`) | `~/.vox/models/classifier/distilbert-query-classifier/` | ❌ |

> Memory subsystem models are **absent from `manifests/models_manifest.json`** — they load from fixed paths and degrade gracefully when absent.

---

## 2. Hardware Tier Model Availability

Which models and memory features are available depends on the user's hardware tier. **Tier 1B+ enables full memory ingestion; Tier 1A is retrieval-only (FIFO context buffer).**

| Tier | Hardware | Pipeline | VAD | STT | LLM | TTS | Memory Models | Memory Features |
| :--- | :------- | :------- | :-: | :-: | :-: | :-: | :-----------: | :-------------: |
| **1A** | 8GB CPU-only, no GPU | Modular (Local) | ✅ | ✅ Nemotron | ✅ Llama 1B Q4 | ✅ Supertonic 3 | ❌ (none loaded) | FIFO context window only |
| **1B** | 8GB+ with GPU | Modular (Local) | ✅ | ✅ Nemotron | ✅ Llama/Gemma | ✅ Supertonic/Chatterbox | ✅ MiniLM + DeBERTa + LFM2.5 | Full ingestion + retrieval |
| **2A** | Remote LLM + Local Audio | Modular (Remote LLM) | ✅ | ✅ Nemotron | ✅ Cloud models | ✅ Supertonic/Chatterbox | ✅ MiniLM + DeBERTa + LFM2.5 | Full ingestion + retrieval |
| **2B** ⭐ | Cloud LLM + Local Audio | Modular (Cloud LLM) | ✅ | ✅ Nemotron | ✅ Cloud models (tool-calling native) | ✅ Supertonic/Chatterbox | ✅ MiniLM + DeBERTa + LFM2.5 | Full ingestion + retrieval |
| **3** | Any (Realtime S2S) | Realtime (WebSocket) | ✅ Router | ❌ Bypassed | ❌ Bypassed | ❌ Bypassed | ⚠️ Provider-managed | Provider-managed via tool calls |

**Key constraints:**
- **Tier 1A**: No background memory worker. No embedding, NLI, or edge classifier loaded. Working Memory is a pure FIFO conversation buffer — models are never paged in.
- **Tier 1B+**: All memory models are loaded on-demand by the background worker during idle sweeps (not resident in the live pipeline). The worker loads embedding → compute vectors → run NLI/LLM → persist → unload.
- **Tier 2B (recommended default)**: Cloud LLM handles all reasoning and tool calling; local models handle VAD/STT/TTS + memory. Best balance of capability and resource usage.
- **Tier 3**: Provider owns the full voice loop. Local memory models may be used to inject episodic/semantic context into the WebSocket session setup.

---

## 3. Model-Specific Optimizations & Algorithms

### 3.1 VAD

| Aspect | Earshot (default) | TenVAD (legacy) |
|--------|-------------------|-----------------|
| **Algorithm** | Energy-based with embedded NN weights | Silero-style ONNX DNN |
| **Latency** | ~1ms / 256-sample frame | ~15ms / frame |
| **Threshold** | 0.5 (hot-reloadable via `update_threshold()`) | 0.45 (requires detector reinit) |
| **Config** | None needed | `min_silence_duration: 0.5s`, `min_speech_duration: 0.25s` |
| **Dispatch** | `VadBackend::Earshot` — enum dispatch, no vtable | `VadBackend::Ten` — enum dispatch |
| **Threading** | Zero allocation, runs on VAD OS thread | 1 thread (sherpa-onnx) |

### 3.2 STT

| Aspect | Nemotron-3.5 (primary) | Qwen3-ASR-0.6B (fallback) |
|--------|----------------------|---------------------------|
| **Architecture** | FastConformer-RNNT | Conformer-Transducer |
| **Engine** | `parakeet-rs` (Rust-native ONNX) | `sherpa-onnx` (C++ ONNX) |
| **Files** | `encoder.onnx`, `decoder_joint.onnx`, `config.json`, `tokenizer.model` | `conv_frontend.onnx`, `encoder.int8.onnx`, `decoder.int8.onnx`, `tokenizer` |
| **RTF** | 0.02–0.35× (avg 0.18×) | 0.38–4.63× |
| **Streaming** | 8960-sample windows (~560ms @ 16kHz), stateful across chunks | 15s rolling overlap window |
| **State Reset** | `reset_state()` called only at end of utterance | Full window flush |
| **Chunked Strategy** | Context is preserved across all chunks — produces coherent Devanagari Hindi from multilingual speech | Per-window decoding |
| **Throttling** | Partials capped at 1 per 800ms (`STT_THROTTLE_MS`) | Same |

**Chunked Transcription Algorithm (Nemotron):**
```rust
fn transcribe(audio: &[f32]) -> String {
    let window_size = 8960;
    for chunk in audio.chunks(window_size) {
        session.run(ORTFeed { name: "audio_signal", tensor: chunk });
    }
    session.reset_state();  // Only at utterance end
    decode_output(session)
}
```

### 3.3 LLM

| Aspect | EmbeddedProvider (Local) | OpenAiCompatProvider (Cloud) |
|--------|------------------------|-----------------------------|
| **Engine** | `llama.cpp` via `llama-cpp-4` crate | `reqwest` blocking HTTP with streaming |
| **Context** | Configurable 1024–8192 tokens | Provider-managed |
| **Threading** | `n_threads` (default: N-2 cores) | N/A (network I/O) |
| **Routing** | N/A | `provider_name` param → auto URL mapping |
| **Model Family Detection** | Auto-detects Gemma, Qwen, Llama3, Nemotron formats | N/A (server handles formatting) |

**Prompt Format (Llama 3.2 Instruct):**
```text
<|begin_of_text|><|start_header_id|>system<|end_header_id|>
{system_prompt}<|eot_id|><|start_header_id|>user<|end_header_id|>
{user_text}<|eot_id|><|start_header_id|>assistant<|end_header_id|>
```

**Tag Stripping (Accumulated-Buffer + Delta Emission):** Emotion tags `<laugh>`, `<breath>`, `<sigh>` and think blocks `<think>...</think>` are stripped from the token stream before text reaches TTS. Uses accumulated-buffer stripping to avoid partial-tag leakage, with delta emission to maintain per-token display cadence.

### 3.4 TTS

| Aspect | Supertonic 3 (default) | Chatterbox (local clone) | Chatterbox Remote |
|--------|----------------------|------------------------|-------------------|
| **Architecture** | Flow-matching transformer | Transformer (GGML) | Same as Chatterbox |
| **Engine** | sherpa-onnx ONNX | chatterbox-rs GGML | reqwest HTTP |
| **Params** | 99M | 340M Q4 | 340M (server-side) |
| **Output** | 44.1kHz → 24kHz (downsampled) | Native 24kHz | 24kHz |
| **Voice** | 10 built-in (5M/5F) | 5s reference → cloned | Cloned via remote |
| **Languages** | 31 | Multilingual | Multilingual |
| **Quality Steps** | 2–12 (configurable) | Fixed | Fixed |
| **Speed** | 0.7×–2.0× (configurable) | Fixed | Fixed |

**Anti-Aliasing Low-Pass Filter (Supertonic 3):** The vocoder outputs 44.1kHz, downsampled to 24kHz for TTS output. A 2nd-order Butterworth LPF (cutoff: 11000Hz) is applied before downsampling to prevent aliasing artifacts near Nyquist (22.05kHz). Implemented as a biquad filter applied sample-by-sample in the resampling loop.

### 3.5 Embedding: MiniLM-L12 / BGE-M3

| Aspect | MiniLM-L12 (primary) | BGE-M3 (fallback) |
|--------|---------------------|-------------------|
| **Model** | `paraphrase-multilingual-MiniLM-L12-v2` | `BAAI/bge-m3` (Xenova) |
| **Dimensions** | 384 | 1024 |
| **Footprint** | ~118 MB | ~544 MB |
| **CPU Latency** | ~10ms | ~86ms |
| **Normalization** | L2 unit-normalized (`normalize_l2()` in `embedder.rs`) | L2 unit-normalized |
| **Similarity** | Cosine similarity | Cosine similarity |
| **Cutoff Floor** | `semantic_similarity_cutoff = 0.40` (calibrated to MiniLM geometry; noise baseline 0.04–0.23, margin 0.34) | N/A (fallback) |

### 3.6 NLI: DeBERTa-v3

| Aspect | DeBERTa-v3-xsmall (default) | nli-deberta-v3-base (candidate) |
|--------|---------------------------|-------------------------------|
| **Model** | `deberta-v3-xsmall-nli` | `nli-deberta-v3-base` |
| **Params** | ~44M | ~86M |
| **Labels** | Contradiction, Entailment, Neutral | Same |
| **Threshold** | `NLI_CONTRADICTION_THRESHOLD = 0.85`, `NLI_ENTAILMENT_THRESHOLD = 0.85` | Same |
| **CPU Latency** | ~35ms / pair | ~65ms / pair |
| **Graph Opt** | Level 3 | Level 3 |
| **Intra Threads** | 1 | 1 |
| **Tokenizer** | Truncation clamped to 512 tokens | Same |

**Dynamic Startup Calibration (`calibrate()`):** At boot, runs dummy premise/hypothesis pairs to dynamically map ONNX logit output indices to the correct label order (prevents index drift across ONNX export versions).

### 3.7 Edge Classifier: LFM2.5-230M

| Aspect | Detail |
|--------|--------|
| **Model** | LFM2.5-230M |
| **Format** | GGUF Q8_0 |
| **Footprint** | ~235 MB |
| **Engine** | llama.cpp (shared `global_llama_backend()` singleton) |
| **Labels** | 4 cognitive edges: `REQUIRES`, `RESTRICTS`, `ENABLES`, `RELATES_TO` + `NONE` |
| **Context** | Receives candidate fact pair + active session Narrative summary (per Core Invariable Rule 1) |
| **Pre-Filter** | Candidate pairs filtered by `edge_candidate_search_cutoff = 0.55` before LLM invocation |
| **Connection Matrix** | Only invoked for domain pairs specified in the Cognitive Connection Policy Matrix (7 pairs: Identity→Profile, Directives→Constraints, Directives→Entities, Entities→Constraints, Entities→Profile, Entities→Entities, Profile→Profile) |

### 3.8 Query Classifier: DistilBERT query-sieve

| Aspect | Detail |
|--------|--------|
| **Model** | DistilBERT fine-tuned for voice query classification |
| **Path** | `~/.vox/models/classifier/distilbert-query-classifier/model_quantized.onnx` |
| **Purpose** | Short-circuits generic chatter ("hello", "thanks", "okay") — saves 100% of ONNX embedding and DB search overhead on non-substantive turns |
| **Latency** | < 1.5ms CPU inference |
| **Fallback** | If model is absent, defaults to `SEMANTIC` gate (all queries pass through to retrieval) |

---

## 4. Cloud Provider Configurations

### 4.1 LLM Providers

| Provider | `provider_name` | Base URL | Key Auth |
| :--- | :--- | :--- | :--- |
| **OpenAI** | `"openai"` | `https://api.openai.com/v1` | Bearer token |
| **Gemini** | `"gemini"` | `https://generativelanguage.googleapis.com/v1beta/openai` | Bearer token |
| **Anthropic** | `"anthropic"` | `https://api.anthropic.com/v1` | `x-api-key` + Bearer |

All cloud LLMs use the same `OpenAiCompatProvider` struct — no provider-specific code needed. Anthropic additionally injects `anthropic-version: 2023-06-01` header.

### 4.2 Realtime S2S Providers

| Provider | Input SR | Output SR | Free Tier | Key Advantage | Status |
|----------|:--------:|:---------:|:---------:|:-------------|:------:|
| **Gemini Live** | 16 kHz | 24 kHz | 10–15 RPM | Native 16 kHz input, cheapest | ✅ |
| **Deepgram Voice Agent** | 16 kHz | Configurable | $200 credits | Flat $0.075/min | ✅ |
| **OpenAI Realtime** | 24 kHz | 24 kHz | None | ~232ms P50 latency | ⏳ |
| **ElevenLabs ConvAI** | 16 kHz | 44.1 kHz | 15 min/mo | Best voice quality, 74 languages | ⏳ |

---

## 5. Model Lifecycle & Management

### Load/Unload by Category

| Category | Load Trigger | Unload Trigger | Resident During Pipeline? |
| :--- | :--- | :--- | :---: |
| **VAD** (Earshot) | Embedded at compile time | Never | ✅ Always resident |
| **VAD** (TenVAD) | Boot (if selected) | On backend switch | ✅ Always resident |
| **STT** | Pipeline warm-up (`engage`) | Pipeline cooldown (`auto-sleep`) | ✅ While engaged |
| **LLM** (local) | Pipeline warm-up (`engage`) | Pipeline cooldown (`auto-sleep`) | ✅ While engaged |
| **LLM** (cloud) | Per-request HTTP | N/A (connection closed) | ❌ Stateless |
| **TTS** | Pipeline warm-up (`engage`) | Pipeline cooldown (`auto-sleep`) | ✅ While engaged |
| **Embedding** | On-demand (memory worker idle sweep) | After sweep completes | ❌ Loaded per sweep |
| **NLI** | On-demand (memory worker idle sweep) | After sweep completes | ❌ Loaded per sweep |
| **Edge Classifier** | On-demand (memory worker idle sweep) | After sweep completes | ❌ Loaded per sweep |
| **Query Classifier** | Boot (eager init in `ensure_classifier_loaded()`) | Never (kept warm) | ✅ Resident (tiny) |

### Auto-Sleep Cooldown

```
if last_interaction.elapsed() > auto_sleep_timeout (default: 5 min):
    cool_down_llm()  → drop LlamaModel + LlamaContext, save ~0.75–1.4 GB
    cool_down_tts()  → drop TTS engine, save ~0.14–1.1 GB
```

### Hot-Reload Rules

| Change | Policy | Effect |
|--------|--------|--------|
| VAD threshold / noise gate | `WorkerCommand` | Hot-update on VAD thread (no restart) |
| VAD backend | `Restart` | Full pipeline restart |
| ASR model / provider | `Restart` | Full pipeline restart |
| LLM model / ctx_size / threads | `Restart` | Full pipeline restart |
| LLM provider | `Restart` | Full pipeline restart |
| TTS provider / voice | `Restart` | Full pipeline restart |
| TTS quality steps / speed | `WorkerCommand` | Hot-update on TTS thread |

---

## 6. Memory Budget Allocation

```text
Total Active Pipeline Budget: ~3.5 GB (on 8GB baseline — design target)
├── VAD:                  < 0.01 GB  (Earshot VAD)
├── STT:                  ~2.50 GB  (Nemotron-3.5 INT8)
├── LLM:                  ~0.75 GB  (Llama 3.2 1B Q4_K_M)
├── TTS:                  ~0.14 GB  (Supertonic 3)  or  ~1.1 GB (Chatterbox)
├── Audio Buffers:        ~0.10 GB  (Pre-allocated ring buffers)
├── Query Classifier:     < 0.01 GB  (DistilBERT, always resident)
└── Safety Margin:        ~1.00 GB  (Headroom for OS + UI + other processes)

Memory models loaded transiently during idle sweeps (not in pipeline budget):
├── Embedding (MiniLM):   ~0.12 GB  (Loaded, embed queue, unloaded)
├── NLI (DeBERTa):        ~0.23 GB  (Loaded, classify pairs, unloaded)
└── Edge Classifier:      ~0.24 GB  (Loaded, classify edges, unloaded)
```

Memory models are **not concurrently resident** with pipeline models. The background worker loads them during prolonged idle (30s debounce), processes the queue, and drops them before the pipeline resumes. Tier 1A machines never load memory models.

---

**Last Updated:** 2026-07-27
