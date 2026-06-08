# Real-Time TTS Pipeline Upgrades for Resource-Constrained Vox Environments

## 1. Architectural Posture & System Constraints

- **Environment:** Real-time, event-driven, CPU-only, 8-16GB RAM, local-first
- **UI pattern:** Ephemeral overlay triggered by VAD; reactive, not request-response
- **Pipeline mandate:** Streaming TTS overlapped with LLM generation; no synchronous blocking chains
- **Latency target:** Sub-300ms TTFA; <500ms absolute ceiling
- **Current stack:**
  - Kokoro (82M params, English, ONNX, ~24s for 60s speech via deterministic phoneme alignment)
  - Piper (Hindi, low latency but older acoustic model, struggles with Hinglish code-switching)
  - Dual-engine wastes ~2-3GB RAM, adds IPC complexity
- **Target:** Unified TTS surpassing both, native English+Hindi, Rust/C++ bindings (Sherpa-ONNX)
- **Memory budget:** 1.5-2.5GB (8GB tier), 4-5GB (16GB tier)

## 3. The 8GB Baseline Replacement: Supertonic 3

**99M params | OpenRAIL-M | ONNX Runtime | 31 languages**

### Architecture & Latency
- Flow-matching backbone (non-autoregressive) — dynamic `total_steps` parameter tunes latency/fidelity tradeoff
- 4-core CPU (no GPU):
  - Fast mode (steps=2): 8s for 850 chars — slurred/synthetic
  - Quality mode (steps=5): 16s — clear, surpasses Kokoro (9s faster than Kokoro's 25s)
- RTF ~167x real-time; TTFA <300ms with chunked LLM output

### Multilingual & Expressiveness
- 31 languages including English+Hindi natively in unified latent space
- `lang="na"` agnostic mode — seamless code-switching without model swap
- Expression Tags: `<laugh>`, `<breath>`, `<sigh>` — inject natural prosody via text

### Rust Integration
- Official Rust SDK (`supertone-inc/supertonic/rust/`) via `ort` crate
- ~400MB weights, <2GB total footprint
- Outputs 44.1kHz 16-bit WAV directly

| Trait | Assessment | Impact |
|---|---|---|
| Parameters | ~99M open-weight | Fits 8GB alongside LLM |
| Runtime | ONNX CPU | Native Rust/Tauri, no Python |
| Speed | Up to 167x RTF | <300ms TTFA |
| Languages | 31 (EN+HI) | Eliminates dual-model pipeline |
| Licensing | OpenRAIL-M | Commercial OK, need legal review |

## 4. The 16GB Premium Upgrade: OmniVoice

**Diffusion LM | Sherpa-ONNX | 600+ languages | Zero-shot voice cloning**

### Architecture
- Diffusion LM (iterative denoising) — RTF as low as 0.025 (40x real-time)
- Exported to 1.2GB FP16 ONNX graph via Sherpa-ONNX
- C++/Rust bindings — no Python/PyTorch dependency
- ~4GB headroom on 16GB machine (OS 2800 + Tauri 300 + LLM 8000 + OmniVoice 1500-2000)

### Voice Design & Emotional Control
- Dynamic control via textual attributes: gender, age, pitch, dialect, vocal effort (whisper)
- Fine-grained pronunciation (pinyin/phonemes), non-verbal symbols `[laughter]`
- Zero-shot voice cloning — speaker embedding extracted from text prompt, cached per turn

| Trait | Assessment | Impact |
|---|---|---|
| Parameters | 1.2GB ONNX FP16 | 16GB tier only |
| Runtime | Sherpa-ONNX | Native C++/Rust, CPU-optimized |
| Speed | 0.025 RTF | Ultra-low latency continuous streaming |
| Languages | 600+ | Flawless Hindi/Hinglish |
| Capabilities | Voice cloning + design | Dynamic personality, no model swap |

## 5. Comprehensive Analysis of Alternative Models

### 5.1. Qwen3-TTS & FastNeuTTS
- **Qwen3-TTS** (0.6B/1.7B): Dual-stream Transformer (28L Talker + 5L CodePredictor). Pure Rust impl (`qwen3_tts_rs`) with libtorch/MLX backends. SSE streaming support. **Fatal flaw:** No native Hindi text normalization — 10 supported languages (European + East Asian). Community "hacks" via zero-shot cloning degrade prosody, introduce latency spikes. 1.7B too heavy for 8GB; CPU contention on 16GB.
- **NeuTTS/FastNeuTTS** (0.5B GGUF): Pure Rust CPU inference (up to 221 tok/s on mid-range), espeak-ng phonemization, NeuCodec decoder. **Rejected:** No Hindi, too slow on CPU (full pipeline timeout after 10min, zero audio output), llama.cpp singleton conflict with main LLM.

### 5.2. Ultra-Lightweight Tier
- **KittenTTS** (15-80M params, 25-80MB int8): Pure Rust (`kitten_tts_rs`), ~10MB binary, 100ms startup, RTF 0.47, SIMD-optimized ONNX. **Strictly monolingual English** (8 built-in voices) — cannot replace Piper.
- **MOSS-TTS-Nano** (0.1B params): CPU-friendly, 48kHz 2-channel. Skewed Chinese/English; Hindi undocumented/unoptimized.

### 5.3. Heavyweight Violations (Disqualified)
- **Higgs Audio v3** (4B params): ~8GB INT8 weight transfer per forward pass on CPU — mathematically exceeds latency budget regardless of software optimization.
- **Miso TTS** (8B params): 110ms on cloud GPU; architecturally invalid for edge CPU.
- **Fish TTS / F5-TTS**: Dual-AR / Diffusion Transformer. High CPU utilization starves local LLM, breaking real-time loop.

### 5.4. Nvidia Magpie-TTS, MeloTTS, StyleTTS2
- **Magpie-TTS** (357M): 9 languages incl. Hindi, multi-codebook + CFG. Deeply embedded in NeMo/PyTorch — no mature ONNX/Rust export. Python dependency violates architecture mandate.
- **MeloTTS** (8.34M encoder): Excellent Sherpa-ONNX integration via C++ API. **No Hindi support** (EN/ES/FR/ZH/JA/KO only).
- **StyleTTS2**: ONNX export exists. Hindi synthesis degrades severely — phonemizer/prosody models fail on retroflex consonants; retraining degrades PLbert quality.

## 6. Systems Architecture: Integration Pathways

### 6.1. Supertonic 3 — ONNX Runtime (ort crate)

1. **Static graph caching:** Load ONNX graphs once at startup via `OnceCell`/`lazy_static` — never per utterance.
2. **I/O binding & pre-allocation:** Pre-allocate contiguous buffers for input text tensors + output audio waveforms; avoid expensive data copying.
3. **Thread affinity:** Restrict ONNX to 2-3 threads via `intra_op_num_threads`. Pin with `core_affinity` crate to prevent LLM starvation.
4. **Custom text normalization (TN) layer:** Expand digits/abbreviations per language context (e.g., "3" → "तीन" in Hindi), inject Expression Tags before ONNX inference. Replaces Supertonic's delegated language handling.

### 6.2. OmniVoice — Sherpa-ONNX FFI

1. **Stateful initialization:** Persistent thread-safe Sherpa offline TTS generator in Tauri app state; 1.2GB model never paged to disk.
2. **Diffusion step configuration:** Expose denoising steps to config; reduce under load to maintain real-time performance.
3. **Zero-shot conditioning caching:** Cache speaker embedding per conversational turn; avoid recomputing per chunk.

## 7. Streaming Topologies & Latency Eradication

**Principle:** Overlap LLM generation with TTS synthesis — never wait for a complete paragraph.

### Clause-Boundary Chunking
Flow-matching/diffusion models need contextually complete inputs. Accumulate LLM tokens, dispatch at boundaries:
- `. , ! ? ;`
- `।` (Hindi Purna Viram)
- Coordinating conjunctions (`and`, `but`, `और`)
- Max character/token limit (fallback for run-on sentences)

### Concurrent Audio Ring Buffer
- Lock-free concurrent ring buffer (Rust)
- Playback starts immediately on first chunk (high RTF means synthesis >> playback time)
- Chunk 2 (LLM + TTS) produced while chunk 1 plays
- Pre-buffer 200ms to absorb LLM generation spikes
- Higher OS scheduling priority for LLM thread vs TTS thread to prevent buffer underflow

### Ephemeral UI Reactive Layer
- Backend emits async Tauri events (`SynthesisStarted`, `PlaybackPlaying`, `PlaybackCompleted`)
- Frontend: lightweight CSS opacity transitions, no heavy React reconciliation
- Audio visualizer: AudioWorklet via Web Audio API (60fps, zero IPC overhead, reads PCM stream from Rust)

## 8. Memory Mapping & Resource Allocation

### 8GB Tier (Supertonic 3)
| Component | Memory |
|---|---|
| OS & background tasks | ~2500 MB |
| Tauri app (WebView + Rust) | ~300 MB |
| Local LLM (3B-8B Q4_K_M) | ~3500-4500 MB |
| Supertonic 3 (ONNX graph + alloc) | ~200-400 MB |
| **Headroom** | **~892-1892 MB** |

Must manually drop intermediate tensors/vectors immediately after consumption.

### 16GB Tier (OmniVoice)
| Component | Memory |
|---|---|
| OS & background tasks | ~2800 MB |
| Tauri app | ~300 MB |
| Local LLM (8B Q8_0 / 14B Q4_K_M) | ~8000 MB |
| OmniVoice (FP16 graph + overhead) | ~1500-2000 MB |
| **Headroom** | **~5284 MB** |

## 9. Final Feedback & Architectural Directives

| Label | Issue | Suggested Fix | Confidence |
|---|---|---|---|
| 🐛 **BUG** | LLM vs TTS CPU starvation — flow-matching/diffusion TTS running concurrently with LLM without thread isolation causes generation stalls | `core_affinity` crate: restrict TTS to 2-3 pinned threads; higher OS priority for LLM | 100% |
| 🐛 **BUG** | Monolingual Hindi failure — forcing English-first models (KittenTTS, NeuTTS, StyleTTS2) through English phonemizers produces catastrophic audio hallucinations | Categorically reject EN-only models. Use native multilingual latent spaces only. | 100% |
| ⚖️ **TRADEOFF** | Supertonic 3 fast mode (steps=2, 8s, slurred) vs quality mode (steps=5, 16s, natural) | Default steps=5; dynamically downgrade to steps=3 under thermal/lag pressure | 95% |
| 💡 **IMPROVEMENT** | Deprecate Kokoro + Piper — dual-engine wastes memory, complicates IPC routing | Replace both with Supertonic 3 (unified 99M ONNX, 31 languages, <2GB) | 95% |
| 💡 **IMPROVEMENT** | Implement OmniVoice via Sherpa-ONNX (16GB tier) — Voice Design attributes driven by LLM context for emotional reactivity | Use 1.2GB FP16 export; cache speaker embeddings per turn | 90% |

## 10. Appendix: NeuTTS Integration Autopsy

### What Was Attempted
- `neutts` Rust crate (v0.1.1, neuphonic/neutts) integrated as path dependency
- 0.5B GGUF backbone (Q4: 194MB, Q8: 242MB) generates speech token IDs via llama.cpp
- Decoded to 24kHz audio via NeuCodec (FSQ + Vocos + ISTFT, ndarray CPU backend, 840MB weights)
- Vox contributions: phonemize fixes (espeak-ng data path auto-discovery, punctuation preservation), backbone optimization (deterministic seed, cleaned sampler chain)

### Rejection Reason 1: No Hindi Support
- espeak-ng phonemizer + backbone model lack Hindi phoneme mappings
- Gemini deep research: "Categorically reject KittenTTS, NeuTTS, and StyleTTS2 for the Vox pipeline — the system must utilize models with native multilingual latent spaces."
- Single fatal flaw for bilingual (English+Hindi) Vox use case

### Rejection Reason 2: CPU Inference Too Slow
- Full vox-bench pipeline (7.9s WAV → STT → LLM → TTS): first 11-char chunk ("I'm just a") dispatched to NeuTTS never completed — timeout after 10 minutes, zero audio output
- Standalone tts-bench produced 44-byte WAV (header only, zero audio samples)
- Bottleneck: ndarray CPU codec decoder

### Rejection Reason 3: Llama Backend Singleton Conflict
- NeuTTS calls `LlamaBackend::init()` in its backbone loader
- When loaded after main LLM (already initialized llama.cpp), second `init()` fails
- Fix: `load_with_backend()` path sharing process-wide backend — verified working but performance issues remained

### Conclusion
Rejected on three grounds: no Hindi, too slow on CPU, crate complexity outweighs benefit. Recommended Supertonic 3 (ONNX, 99M params, 31 languages, Rust `ort` integration) addresses all three failure points. The `neutral_llm_backend()` refactoring to `llm/mod.rs` was kept as a pure improvement (reduces code duplication in backend initialization).

#### Works cited

1. Kokoro 82M vs Supertonic 3: A Real CPU TTS Benchmark - Neo, accessed on June 8, 2026, https://heyneo.com/blog/kokoro-tts-vs-supertonic-3-tts
2. Hosting a Text to Speech model can be challenging. So I benchmarked 2 recently released TTS models - Kokoro vs Supertonic! : r/selfhosted - Reddit, accessed on June 8, 2026, https://www.reddit.com/r/selfhosted/comments/1tgo3qr/hosting_a_text_to_speech_model_can_be_challenging/
3. Supertone's Supertonic is just a 66M param, on-device text-to-speech engine that runs via ONNX for cross-platform inference. : r/LocalLLM - Reddit, accessed on June 8, 2026, https://www.reddit.com/r/LocalLLM/comments/1tho03e/supertones_supertonic_is_just_a_66m_param/
4. supertone-inc/supertonic: Lightning-Fast, On-Device, Multilingual TTS — running natively via ONNX. - GitHub, accessed on June 8, 2026, https://github.com/supertone-inc/supertonic
5. Supertonic TTS: The Lightning-Fast On-Device Text-to-Speech Revolution in 2025 | Efficient Coder, accessed on June 8, 2026, https://www.xugj520.cn/en/archives/supertonic-tts-on-device-revolution.html
6. Expressive tags do not work with Rust runner · Issue #148 - GitHub, accessed on June 8, 2026, https://github.com/supertone-inc/supertonic/issues/148
7. Supertone - Hugging Face, accessed on June 8, 2026, https://huggingface.co/Supertone/supertonic
8. Supertonic TTS: Lightning Fast On-Device Text-to-Speech System, accessed on June 8, 2026, https://supertonic-tts.com/
9. Supertone/supertonic-3 - Hugging Face, accessed on June 8, 2026, https://huggingface.co/Supertone/supertonic-3
10. Supertonic download | SourceForge.net, accessed on June 8, 2026, https://sourceforge.net/projects/supertonic.mirror/
11. Open-source on-device TTS model : r/rust - Reddit, accessed on June 8, 2026, https://www.reddit.com/r/rust/comments/1p4ohus/opensource_ondevice_tts_model/
12. LICENSE · Supertone/supertonic-3 at main - Hugging Face, accessed on June 8, 2026, https://huggingface.co/Supertone/supertonic-3/blob/main/LICENSE
13. supertonic/LICENSE at main - GitHub, accessed on June 8, 2026, https://github.com/supertone-inc/supertonic/blob/main/LICENSE
14. Voice Cloning with OmniVoice TTS in ComfyUI - over 600 languages!, accessed on June 8, 2026, https://www.youtube.com/watch?v=zwQOe8rSqBM
15. k2-fsa/OmniVoice - Hugging Face, accessed on June 8, 2026, https://huggingface.co/k2-fsa/OmniVoice
16. k2-fsa/OmniVoice: High-Quality Voice Cloning TTS for 600+ Languages - GitHub, accessed on June 8, 2026, https://github.com/k2-fsa/OmniVoice
17. [FEATURE] Add new version of OmniVoice TTS · Issue #3651 · k2-fsa/sherpa-onnx - GitHub, accessed on June 8, 2026, https://github.com/k2-fsa/sherpa-onnx/issues/3651
18. [FEATURE] Add OmniVoice TTS (0.8B zero-shot model from k2-fsa) #3486 - GitHub, accessed on June 8, 2026, https://github.com/k2-fsa/sherpa-onnx/issues/3486
19. sherpa-onnx/rust-api-examples/README.md at master - GitHub, accessed on June 8, 2026, https://github.com/k2-fsa/sherpa-onnx/blob/master/rust-api-examples/README.md
20. GitHub - k2-fsa/sherpa-onnx: Speech-to-text, text-to-speech, speaker diarization, speech enhancement, source separation, and VAD using next-gen Kaldi with onnxruntime without Internet connection. Support embedded systems, Android, iOS, HarmonyOS, Raspberry Pi, RISC-V, RK NPU, Axera NPU, Ascend NPU, x86_64 servers, websocket server/client, support 12 programming languages, accessed on June 8, 2026, https://github.com/k2-fsa/sherpa-onnx
21. I created rust bindings to sherpa-onnx - local speech AI models - Reddit, accessed on June 8, 2026, https://www.reddit.com/r/rust/comments/1dx2bh4/i_created_rust_bindings_to_sherpaonnx_local/
22. High-Quality Long-Form TTS with Qwen3 Open-Weight Models - Medium, accessed on June 8, 2026, https://medium.com/data-science-collective/high-quality-long-form-tts-with-qwen3-open-weight-models-cdd6e3d00df0
23. second-state/qwen3_tts_rs: A Rust implementation of the ... - GitHub, accessed on June 8, 2026, https://github.com/second-state/qwen3_tts_rs
24. andimarafioti/faster-qwen3-tts - GitHub, accessed on June 8, 2026, https://github.com/andimarafioti/faster-qwen3-tts
25. Qwen3-TTS Update! 49 Timbres + 10 Languages + 9 Dialects, accessed on June 8, 2026, https://qwen.ai/blog?id=qwen3-tts-1128
26. GPU-Powered Voice AI: Multilingual Speech, Captions & Voice Cloning: Qwen 3 TTS, accessed on June 8, 2026, https://amit-shukla.medium.com/gpu-powered-voice-ai-multilingual-speech-captions-voice-cloning-qwen-3-tts-dfd15a22fd8f
27. Best open source Hinglish(Hindi+English) TTS : r/LocalLLaMA - Reddit, accessed on June 8, 2026, https://www.reddit.com/r/LocalLLaMA/comments/1qz7k5i/best_open_source_hinglishhindienglish_tts/
28. Fine-tuning Qwen3-TTS-12Hz-1.7B-Base for Bengali: Foreign/Hindi Accent Bias and Lack of Naturalness · Issue #323 - GitHub, accessed on June 8, 2026, https://github.com/QwenLM/Qwen3-TTS/issues/323
29. neuphonic/neutts-air - Hugging Face, accessed on June 8, 2026, https://huggingface.co/neuphonic/neutts-air
30. NeuTTS - On-device TTS model by Neuphonic - GitHub, accessed on June 8, 2026, https://github.com/neuphonic/neutts
31. neutts - Rust - Docs.rs, accessed on June 8, 2026, https://docs.rs/neutts
32. GitHub - KittenML/KittenTTS: State-of-the-art TTS model under 25MB, accessed on June 8, 2026, https://github.com/KittenML/KittenTTS
33. GitHub - second-state/kitten_tts_rs: Rust implementation of ..., accessed on June 8, 2026, https://github.com/second-state/kitten_tts_rs
34. Kitten TTS 15M, accessed on June 8, 2026, https://mikeesto.com/posts/kitten-tts-15m/
35. Show HN: Three new models by KittenML. <25 MB Open-source TTS. Highly Expressive, accessed on June 8, 2026, https://news.ycombinator.com/item?id=47082802
36. OpenMOSS/MOSS-TTS-Nano - GitHub, accessed on June 8, 2026, https://github.com/OpenMOSS/MOSS-TTS-Nano
37. MOSS-TTS-Nano: a 0.1B open-source multilingual TTS model that runs on 4-core CPU and supports realtime speech generation - Reddit, accessed on June 8, 2026, https://www.reddit.com/r/LocalLLaMA/comments/1sjdfp6/mossttsnano_a_01b_opensource_multilingual_tts/
38. Higgs Audio v3 TTS 4B. Built for voice chat. Support 100 languages and inline control. : r/LocalLLaMA - Reddit, accessed on June 8, 2026, https://www.reddit.com/r/LocalLLaMA/comments/1tx2mot/higgs_audio_v3_tts_4b_built_for_voice_chat/
39. bosonai/higgs-audio-v3-tts-4b - Hugging Face, accessed on June 8, 2026, https://huggingface.co/bosonai/higgs-audio-v3-tts-4b
40. Higgs Audio v3 TTS: Beyond Reading, Toward Real Speech for Voice AI - Boson AI, accessed on June 8, 2026, https://www.boson.ai/blog/higgs-audio-v3-tts
41. README.md · MisoLabs/MisoTTS at main - Hugging Face, accessed on June 8, 2026, https://huggingface.co/MisoLabs/MisoTTS/blob/main/README.md
42. Miso Labs Releases MisoTTS: An 8B Emotive Text-to-Speech Model with Open Weights, accessed on June 8, 2026, https://www.marktechpost.com/2026/06/04/miso-labs-releases-misotts-an-8b-emotive-text-to-speech-model-with-open-weights/
43. f5-tts - PyPI, accessed on June 8, 2026, https://pypi.org/project/f5-tts/
44. Text-To-Speech - vLLM-Omni, accessed on June 8, 2026, https://docs.vllm.ai/projects/vllm-omni/en/latest/user_guide/examples/offline_inference/text_to_speech/?q=
45. DakeQQ/F5-TTS-ONNX - GitHub, accessed on June 8, 2026, https://github.com/DakeQQ/F5-TTS-ONNX
46. nvidia/magpie_tts_multilingual_357m - Hugging Face, accessed on June 8, 2026, https://huggingface.co/nvidia/magpie_tts_multilingual_357m
47. magpie-tts-multilingual Model by NVIDIA, accessed on June 8, 2026, https://build.nvidia.com/nvidia/magpie-tts-multilingual/modelcard
48. Magpie-TTS — NVIDIA NeMo Framework User Guide, accessed on June 8, 2026, https://docs.nvidia.com/nemo-framework/user-guide/latest/nemotoolkit/tts/magpietts.html
49. qualcomm/MeloTTS-ZH · Hugging Face, accessed on June 8, 2026, https://huggingface.co/qualcomm/MeloTTS-ZH
50. MiaoMint/MeloTTS-ONNX - Hugging Face, accessed on June 8, 2026, https://huggingface.co/MiaoMint/MeloTTS-ONNX
51. Speech Synthesis (TTS) | Open LLM Vtuber, accessed on June 8, 2026, https://docs.llmvtuber.com/en/docs/user-guide/backend/tts/
52. MeloTTS/docs/training.md at main - GitHub, accessed on June 8, 2026, https://github.com/myshell-ai/MeloTTS/blob/main/docs/training.md
53. hexgrad/styletts2 - Hugging Face, accessed on June 8, 2026, https://huggingface.co/hexgrad/styletts2
54. Trained StyleTTS2 for Hindi but didn't get good results · Issue #286 - GitHub, accessed on June 8, 2026, https://github.com/yl4579/StyleTTS2/issues/286
55. How I Built an Open-Source Hinglish TTS — Harshal Singh, accessed on June 8, 2026, https://harrrshall.github.io/hinglish-tts/
56. DevGitPit/supertonic-android: Initial setup for a F-droid release. - GitHub, accessed on June 8, 2026, https://github.com/DevGitPit/supertonic-android
