---
title: "Vox Model Inventory & Specifications"
audience: "Internal — ML/model-config contributors, backend engineers, agents"
last_updated: 2026-08-25
owners: "ml-research-engineer role"
related_docs:
  - "docs/backend.md — Engines, threading, lifecycle"
  - "docs/features/memory-architecture.md — Memory subsystem models"
  - "docs/features/query-sieve.md — Query sieve architecture"
  - "docs/features/transliteration-rnn.md — Transliteration architecture"
  - "submodules/distilbert-query-classifier, vox-models — Model source"
---

# Vox — Model Inventory & Specifications

> **Complete reference of all models used in Vox**, their parameters, quantization, inference engines, load paths, and category-specific algorithms. This is a model catalog, not an architecture document — see `docs/backend.md` for threading, lifecycle, and system architecture.

---

## 0. How to read this doc

- **Audience:** ML/model-config contributors, backend engineers, and agents.
- **Scope:** the complete model catalog — parameters, quantization, engines, load paths, and category algorithms.
- **Convention:** claims use `path/file` pointers; manifest paths are canonical (`~/.vox/models/...`).
- **Non-goals:** not an architecture doc (→ `docs/backend.md`); not a memory-subsystem deep dive (→ `docs/features/memory-architecture.md`).
- **SSOT:** model IDs and manifest entries are authoritative; every model must appear in `~/.vox/models/models_manifest.json`.

## 1. Model Inventory — Complete Reference

### 1.1 Realtime Pipeline Models

| Role | Model | ID | Params | Quant | Footprint | Engine | Path | Manifest |
| :--- | :--- | :--- | :---: | :---: | :---: | :--- | :--- | :---: |
| **VAD** (default) | Earshot VAD | `earshot` | Embedded | — | < 1 MB | Rust-native | Embedded (no file) | ❌ |
| **VAD** (legacy) | TenVAD | `ten_vad` | — | INT8 | ~15 MB | sherpa-onnx ONNX | `~/.vox/models/vad/ten_vad.onnx` | ✅ |
| **STT** (primary) | Nvidia Nemotron-3.5 | `nvidia_nemotron` | ~1B | INT8 | ~2.5 GB | parakeet-rs ONNX | `~/.vox/models/stt/nemotron-3.5/` | ✅ |
| **STT** (fallback) | Qwen3-ASR-0.6B | `qwen3_asr` | 0.6B | INT8 | ~800 MB | sherpa-onnx ONNX | `~/.vox/models/stt/qwen3-asr/` | ✅ |
| **STT** (cloud) | Google Chirp 3 | `chirp_3` | Cloud | — | 0 MB | HTTP | `stt.cloud` (`google`) | N/A |
| **LLM** (default) | Qwen3 0.8B | `qwen_3_5_0_8b` | 0.8B | Q4_K_M | ~600 MB | llama.cpp GGUF | `~/.vox/models/llm/qwen3/` | ✅ |
| **LLM** (alternative) | Llama 3.2 1B Instruct | `llama_3_2_reasoning_q4` / `q6` | 1B | Q4_K_M / Q6_K | ~750 MB / 1.0 GB | llama.cpp GGUF | `~/.vox/models/llm/llama/` | ✅ |
| **LLM** (alternative) | Gemma3 4B | `gemma3:4b` | 4B | Q4_K_M | ~2.5 GB | llama.cpp GGUF | `~/.vox/models/llm/gemma3/` | ✅ |
| **LLM** (server) | Ollama / LM Studio | `ollama` / `lm_studio` | — | — | 0 MB (local) | HTTP (reqwest) | `http://localhost:11434` | N/A |
| **LLM** (cloud) | OpenAI / Gemini / Anthropic / Nvidia | provider-config | — | — | 0 MB (local) | HTTP (reqwest) | Remote API (`integrate.api.nvidia.com/v1` default) | N/A |
| **TTS** (default) | Microsoft Edge TTS | `edge_tts` | Remote | — | **0 MB** | Pure Rust (`tokio-tungstenite`) | Remote WebSocket | N/A |
| **TTS** (local) | Supertonic 3 | `supertonic_tts` | 99M | INT8 | ~144 MB | sherpa-onnx ONNX | `~/.vox/models/tts/supertonic-3/` | ✅ |
| **TTS** (local clone) | Chatterbox Local | `chatterbox_tts` | 340M | Q4 GGML | ~1.1 GB | chatterbox-rs GGML | `~/.vox/models/tts/chatterbox/` | ✅ |
| **TTS** (remote) | Chatterbox Remote | `chatterbox_remote` | 340M | Q4 GGML | 0 MB (local) | reqwest HTTP | Remote GPU server | ✅ |

### 1.2 Memory Subsystem Models

| Role | Model | ID | Params | Quant | Footprint | Engine | Path | Manifest |
| :--- | :--- | :--- | :---: | :---: | :---: | :--- | :--- | :---: |
| **Embedding** (primary) | MiniLM-L12 | `minilm-l12-v2` | ~22M | INT8 | ~118 MB | ONNX Runtime (`ort`) | `~/.vox/models/embedding/minilm-l12-v2/` | ❌ |
| **Embedding** (fallback) | BGE-M3 | `bge-m3` | ~568M | INT8 | ~544 MB | ONNX Runtime (`ort`) | `~/.vox/models/embedding/bge-m3/` | ❌ |
| **NLI** | nli-deberta-v3-base | `nli-deberta-v3-base` | ~86M | INT8 | ~233 MB | ONNX Runtime (`ort`) | `~/.vox/models/nli/nli-deberta-v3-base/` | ❌ |
| **Edge Classifier** | ModernBERT | `modernbert_edge_creation` | ~143M | INT8 | ~144 MB | ONNX Runtime (`ort`) | `~/.vox/models/classifier/modernbert_edge_creation/` | ❌ |
| **MemoryScope Classifier** | ModernBERT | `modernbert_memory_scope` | ~143M | INT8 | ~144 MB | ONNX Runtime (`ort`) | `~/.vox/models/classifier/modernbert_memory_scope/` | ❌ |

> Memory subsystem models are **absent from `manifests/models_manifest.json`** — they load from fixed paths and degrade gracefully when absent.

---

## 2. Hardware Tier Model Availability

Which models and memory features are available depends on the user's hardware tier. **Tier 1B+ enables full memory ingestion; Tier 1A is retrieval-only (FIFO context buffer).**

| Tier | Hardware | Pipeline | VAD | STT | LLM | TTS | Memory Models | Memory Features |
| :--- | :------- | :------- | :-: | :-: | :-: | :-: | :-----------: | :-------------: |
| **1A** | 8GB CPU-only, no GPU | Modular (Local) | ✅ | ✅ Nemotron | ✅ Llama 1B Q4 | ✅ Supertonic 3 | ❌ (none loaded) | FIFO context window only |
| **1B** | 8GB+ with GPU | Modular (Local) | ✅ | ✅ Nemotron | ✅ Llama/Gemma | ✅ Supertonic/Chatterbox | ✅ MiniLM + ModernBERT | Full ingestion + retrieval |
| **2A** | Remote LLM + Local Audio | Modular (Remote LLM) | ✅ | ✅ Nemotron | ✅ Cloud models | ✅ Supertonic/Chatterbox | ✅ MiniLM + ModernBERT | Full ingestion + retrieval |
| **2B** ⭐ | Cloud LLM + Local Audio | Modular (Cloud LLM) | ✅ | ✅ Nemotron | ✅ Cloud models (tool-calling native) | ✅ Supertonic/Chatterbox | ✅ MiniLM + ModernBERT | Full ingestion + retrieval |
| **3** | Any (Realtime S2S) | Realtime (WebSocket) | ✅ Router | ❌ Bypassed | ❌ Bypassed | ❌ Bypassed | ⚠️ Provider-managed | Provider-managed via tool calls |

**Key constraints:**
- **Tier 1A**: No background memory worker. No embedding, NLI, or edge classifier loaded. Working Memory is a pure FIFO conversation buffer — models are never paged in.
- **Tier 1B+**: All memory models are loaded on-demand by the background worker during idle sweeps (not resident in the live pipeline). The worker loads embedding → compute vectors → run NLI/edge classifier → persist → unload.
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
|--------|------------------------|---------------------------|
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
    session.reset_state()  // Only at utterance end
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

**Tag Stripping (Accumulated-Buffer + Delta Emission):** Emotion tags `<laugh>`, `<breath>`, `<sigh>` and think blocks `</think>...</think>` are stripped from the token stream before text reaches TTS. Uses accumulated-buffer stripping to avoid partial-tag leakage, with delta emission to maintain per-token display cadence.

### 3.4 TTS

| Aspect | Edge TTS (default) | Supertonic 3 (local) | Chatterbox (local clone) | Chatterbox Remote |
|--------|--------------------|----------------------|--------------------------|-------------------|
| **Architecture** | Microsoft Bing ReadAloud | Flow-matching transformer | Transformer (GGML) | Same as Chatterbox |
| **Engine** | Pure Rust WebSocket (`tokio-tungstenite`) | sherpa-onnx ONNX | chatterbox-rs GGML | reqwest HTTP |
| **Params** | Remote API | 99M | 340M Q4 | 340M (server-side) |
| **Output** | Native 24kHz f32 MP3-decoded PCM | 44.1kHz → 24kHz (downsampled) | Native 24kHz | 24kHz |
| **Voice** | ~300+ Edge Neural Voices (default: `en-US-AriaNeural`) | 10 built-in (5M/5F) | 5s reference → cloned | Cloned via remote |
| **Languages** | 100+ global languages & dialects | 31 | Multilingual | Multilingual |
| **Quality Steps** | N/A (Streaming MP3) | 2–12 (configurable) | Fixed | Fixed |
| **Speed** | 0.7×–2.0× (prosody rate) | Configurable | Fixed | Fixed |
| **RTF / Latency** | **0.30× RTF** (~3.3× real-time) | 1.76× RTF | Variable | Variable |

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

### 3.6 NLI: nli-deberta-v3-base

| Aspect | Detail |
|--------|--------|
| **Model** | `nli-deberta-v3-base` |
| **Params** | ~86M |
| **Quantization** | INT8 |
| **Footprint** | ~233 MB |
| **Engine** | ONNX Runtime (`ort`) |
| **Labels** | Contradiction, Entailment, Neutral |
| **Threshold** | `NLI_CONTRADICTION_THRESHOLD = 0.85`, `NLI_ENTAILMENT_THRESHOLD = 0.85` |
| **CPU Latency** | ~65ms / pair |
| **Graph Opt** | Level 3 |
| **Intra Threads** | 1 |
| **Tokenizer** | Truncation clamped to 512 tokens |

**Dynamic Startup Calibration (`calibrate()`):** At boot, runs dummy premise/hypothesis pairs to dynamically map ONNX logit output indices to the correct label order (prevents index drift across ONNX export versions).

**State Transitions (v7):**
- Identity/Directives CONTRADICTION → `SUPERSEDES` (old record inactive, new replaces)
- Identity/Directives ENTAILMENT → `SUPPORTS` (both records active)
- Constraints CONTRADICTION → `CONFLICTS` (both records active)

### 3.7 Edge Classifier: ModernBERT

| Aspect | Detail |
|--------|--------|
| **Model** | ModernBERT |
| **Format** | INT8 ONNX |
| **Footprint** | ~144 MB |
| **Engine** | ONNX Runtime (`ort`) |
| **Labels** | 4 cognitive edges: `REQUIRES`, `RESTRICTS`, `ENABLES`, `RELATES_TO` + `NONE` |
| **Context** | Receives candidate fact pair + active session Narrative summary (per Core Invariable Rule 1) |
| **Pre-Filter** | Candidate pairs filtered by `edge_candidate_search_cutoff = 0.55` before inference |
| **Connection Matrix** | Only invoked for domain pairs specified in the Cognitive Connection Policy Matrix (7 pairs: Identity→Profile, Directives→Constraints, Directives→Entities, Entities→Constraints, Entities→Profile, Entities→Entities, Profile→Profile) |
| **CPU Latency** | ~28ms / pair |
| **Test Accuracy** | 87.50% |
| **Macro F1** | 0.8722 |
| **Positive Edge Precision** | 86.67% (at τ* = 0.80) |
| **FP Rate** | 7.69% |

### 3.8 MemoryScope Classifier: ModernBERT

| Aspect | Detail |
|--------|--------|
| **Model** | ModernBERT multilingual |
| **Format** | INT8 ONNX |
| **Footprint** | ~144 MB |
| **Engine** | ONNX Runtime (`ort`) |
| **Path** | `~/.vox/models/classifier/modernbert_memory_scope/model_quantized.onnx` |
| **Purpose** | Pre-retrieval scope classification — routes queries to the correct memory collection before embedding + vector search |
| **Classes** | 4: `ChitChat` (0), `User` (1), `Domain` (2), `Temporal` (3) |
| **Threshold** | τ* = 0.81 — predictions below this default to `Domain` |
| **CPU Latency** | P50: 25.36 ms / query (target: 10–30 ms) |
| **Test Accuracy** | 96.60% |
| **Calibrated Accuracy** | 91.60% |
| **Non-Default Precision** | 98.08% (at τ* = 0.81) |
| **Fallback Rate** | 6.00% |
| **Languages** | English (36.8%), Devanagari Hindi (41.3%), Code-switched Hinglish (22.0%) |
| **Max Tokens** | 32 |

**Fallback:** If the model is absent or classification errors, defaults to `MemoryScope::Domain` — full vector-search retrieval (safe default, no dropped data).

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

### Load/Unload by Configuration

What sits in RAM is decided by **settings**, not by model family. The two master switches are `dictation.enabled` and *how the engine gets launched* — `engage()` (main window), passive auto-launch at boot, or the PTT hotkey. Read this matrix top-to-bottom as the lifecycle of a running app.

Legend: ✅ resident · ❌ not loaded · ↺ lazy/on-demand (loaded on first warm-up or first turn, offloaded again on auto-sleep, re-warmed on next activity) · — n/a (cloud / not applicable).

¹ *Idle-sweep ONNX* = Embedding (MiniLM/BGE-M3) + NLI (DeBERTa-v3) + Edge Classifier (ModernBERT). Loaded **only** during the 30s-debounced idle consolidation sweep, evicted the instant the pipeline goes active or on `stop_engine` — never concurrently resident with a live turn.

| # | Configuration / state | VAD | STT | LLM | TTS | Scope Clf² | Idle-sweep ONNX¹ | Tray HUD | What moves it to the next state |
|:--|:---|:--:|:--:|:--:|:--:|:--:|:--:|:--:|---|
| 1 | **dictation disabled**, app boot, nothing engaged | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | user `engage()` → #4; or enable dictation → #2 / #3a |
| 2 | **dictation enabled · PTT**, app boot | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ (lazy) | first hotkey press → #3b |
| 3a | **dictation enabled · Passive**, app boot | ✅ | ✅ | ↺ | ↺ | ❌ | ❌ | ✅ if `output_mode=Tray` | first warm-up (any speech) → #3b |
| 3b | Passive/PTT **after first use** (engine warm) | ✅ | ✅ | ↺ | ↺ | ❌ | ❌ | ✅ if Tray | auto-sleep → #5; `engage()` main → #4 |
| 4 | **MainWindow engaged** (`engage()`) | ✅ | ✅ | ✅ | ✅ | ✅ | ❌ | n/a | auto-sleep → #5; idle sweep → #6 |
| 5 | **Auto-sleep** reached (idle > `auto_sleep_timeout`) | ✅ | ✅ | ❌ | ❌ | ✅ | ❌ | per owner | new activity re-warms LLM+TTS → #3b/#4 |
| 6 | **Idle memory sweep** (mem pipeline on + queue + 30s idle) | ✅ | ✅ | ❌ | ❌ | ✅ | ✅ transient | — | any activity evicts sweep ONNX → #5 |
| 7 | **Realtime S2S** session active | ✅ | ❌ | — | — | ❌ | ❌ | — | cloud WebSocket — no local model |
| 8 | **stop_engine** / app quit (dictation off or disengage) | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | hidden | everything evicted + `trim_heap()` |

² *Scope Clf* = MemoryScope Classifier (ModernBERT). Loaded only by `engage()` (`ensure_scope_classifier_loaded`), i.e. the **main-window** conversational session — not by dictation PTT/passive, which need only transcription.

**The three things that actually change your RAM footprint:**

- **`dictation.enabled = false`** → boots with **zero models and zero webviews**. Nothing loads until the user engages the main window (#1 → #4).
- **`dictation.enabled = true` + `interaction_mode = Ptt`** → still **zero RAM at boot** (#2). The audio/STT engine lazy-launches on the *first* hotkey press and then stays warm (VAD resident, STT lazy) so subsequent presses are instant; LLM/TTS only spin up per turn and cool on auto-sleep.
- **`dictation.enabled = true` + `interaction_mode = Passive`** → engine **auto-launches at boot**, so VAD is resident immediately (#3a). STT warms up on first speech.
- **`interaction.pipeline_mode = Realtime`** → launches audio capture and VAD routing without loading STT or LLM/TTS weights (0 MB local models).

**Transliteration (ONNX)** is orthogonal to the above: it loads on the first Devanagari string seen in *any* state (`transliterate` → `init_transliteration_engine`) and stays resident until `stop_engine`.

**Reference — load entry points** (for tracing in code):
- VAD: `services/audio/engine.rs` (`start_audio_engine`) — eager at engine launch.
- STT: `services/stt/providers/embedded.rs` (`ensure_loaded`) — lazy on first `transcribe`/`transcribe_chunk` turn.
- LLM / TTS: `services/llm/actor.rs` & `services/tts/actor.rs` (`warm_up_*`) — lazy on `VoxEvent::WarmUp` / first turn; `cool_down_*` on auto-sleep.
- Scope Clf: `services/intent/` (`ensure_scope_classifier_loaded`).
- Embedding / NLI / Edge: `services/memory/**` + `persistence/memory_worker.rs` — idle sweep only.
- Transliteration: `services/translit.rs`.
- Realtime S2S: `services/pipeline/realtime/` (`passive.rs`, `ptt.rs`, `session.rs`) — cloud WebSocket, no local weights.
- Full teardown: `services/audio/engine.rs` (`stop_audio_engine` → `unload_all_onnx_models` + `trim_heap`).

### Auto-Sleep Cooldown

Auto-sleep is driven by the pipeline router (`services/pipeline/router.rs`) using `interaction.auto_sleep_timeout` (default 300s). On sustained inactivity it sets `is_sleeping` and runs a **tiered offload**:

- `cool_down_llm()` (`services/llm/actor.rs`) — drops the local GGUF model / closes the cloud provider, frees ~0.75–1.4 GB.
- `cool_down_tts()` (`services/tts/actor.rs`) — drops the TTS engine, frees ~0.14–1.1 GB.

VAD and STT are **not** cooled on auto-sleep — they stay resident so the mic keeps listening for the next wake word / push-to-talk. Any new activity flips `is_sleeping` back to false and re-warms LLM + TTS lazily via `warm_up_*`. Only `stop_engine()` (main-window close with dictation disabled, disengage, or app quit) tears down VAD/STT and evicts **every** ONNX model via `unload_all_onnx_models()` followed by a cross-platform `trim_heap()`.

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
├── MemoryScope Classifier: < 0.01 GB  (ModernBERT, always resident)
└── Safety Margin:        ~1.00 GB  (Headroom for OS + UI + other processes)

Memory models loaded transiently during idle sweeps (not in pipeline budget):
├── Embedding (MiniLM):   ~0.12 GB  (Loaded, embed queue, unloaded)
├── NLI (nli-deberta-v3-base): ~0.23 GB  (Loaded, classify pairs, unloaded)
└── Edge Classifier:      ~0.14 GB  (Loaded, classify edges, unloaded)
```

The three idle-sweep memory models (Embedding, NLI, Edge Classifier) are **not concurrently resident** with an active pipeline turn — the background worker loads them during prolonged idle (30s debounce), processes the queue, and drops them before the pipeline resumes. The **MemoryScope Classifier** and **Transliteration** engine are exceptions: they are intentionally kept resident once loaded (until `stop_engine`). Tier 1A machines never load the idle-sweep memory models.

---

**Last Updated:** 2026-08-25