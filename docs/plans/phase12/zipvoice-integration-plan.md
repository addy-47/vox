---
title: "ZipVoice Integration Plan — Vox Conversational TTS"
audience: "Internal — backend engineer + ML research engineer implementing the second local TTS provider"
last_updated: 2026-09-27
owners: "backend-engineer role (implementation) · ml-research-engineer role (voice pack + quality gate)"
related_docs:
  - "docs/specs/ipc-spec.md — §2.6/§2.7 must be amended before code (AGENTS.md §4.3)"
  - "docs/specs/events-spec.md — §4 model-eviction contract and §8 error matrix name TTS providers"
  - "docs/specs/harness-spec.md — provider-agnostic TtsActor contract; no change required"
  - "docs/plans/phase12/kokoro-finetuning-plan.md — fallback path if this fails the blind gate"
  - ".agents/rules/backend-style-guide.md — MUST be read before any write batch"
---

# ZipVoice Integration Plan — Vox Conversational TTS

> **Status:** ready for implementation · **Date:** 2026-09-27
> **Supersedes:** the 2026-09-27 draft of this file. That draft's technical claims were
> re-derived from source; **nine of its parameter/asset claims were wrong** (§2). Its
> architecture (zero-shot cloner, no `sid`, mandatory reference clip + transcript) was right.
> **Governed by:** `/home/addy/projects/apps/vox/AGENTS.md` (§4.3 spec-first is a hard gate)

---

## 0. How to read this doc

- **Scope:** wiring ZipVoice as a sixth local TTS provider, its model assets, its
  8–10-clip reference voice pack, and its user-facing settings.
- **Citation convention:** every claim is either (a) a `file:line` in this repo, (b) a
  `file:line` in the sherpa-onnx 1.13.6 crate on this machine, (c) a `file:line` in
  `k2-fsa/sherpa-onnx@master` C++, (d) a verbatim quote, or (e) explicitly marked
  **UNVERIFIED**. There are no invented numbers. Where the prior draft asserted a
  benchmark number I could not reproduce from source, it is marked UNVERIFIED and
  turned into a measurement task.
- **Non-goals:** ZipVoice2, fine-tuning, TensorRT, GPU execution, the Tauri/React
  voice-picker redesign.
- **SSOT for behaviour:** `docs/specs/ipc-spec.md` (commands) and
  `docs/specs/events-spec.md` (lifecycle + errors). This plan is subordinate to both and
  Batch 0 exists to bring them into agreement before any code lands.

---

## 1. Target end state

Vox gains a **`zipvoice`** local TTS provider that zero-shot clones a chosen reference
voice from a shipped 8–10-clip pack, sits alongside Kokoro/Chatterbox in the existing
settings desk, downloads through the existing manifest subsystem, and is measurable
side-by-side with Kokoro on RTF, time-to-first-audio, and seam audibility.

Concretely, when done:

1. `manifests/models_manifest.json` carries a `tts/zipvoice` group; the app downloads and
   SHA-verifies it through the existing path with **no new dependency**.
2. `settings.tts.zipvoice { voice_id, min_char_in_sentence, guidance_scale }` selects a
   voice by slug. `settings.tts.voice_index` and every Kokoro/Supertonic consumer are
   untouched.
3. `ZipvoiceEngine` implements `TtsProvider` and streams to `PlaybackEngine` with an
   inter-chunk equal-power cross-fade.
4. The voice pack is reproducible from `sandbox/voices/` by a committed script, with a
   per-clip ASR round-trip gate at WER = 0.
5. A blind listening gate decides ship / no-ship, with RTF as a hard constraint.

---

## 2. Validation record — what I re-derived and what changed

I re-derived every technical claim from source rather than from documentation. Nine of
the prior draft's claims did not survive.

### 2.1 Confirmed correct in the prior draft

| Claim | Evidence |
|---|---|
| ZipVoice is a zero-shot cloner; no `sid`, no `voices.bin` | `offline-tts-zipvoice-impl.h` `Generate()` never reads `config.sid`; the C API struct carries it but the impl ignores it |
| Output is not streamed — callback fires per chunk *after* that chunk is fully synthesised | `offline-tts-zipvoice-impl.h` `Generate()`: `GenerateChunk(...)` → `result.samples.insert(...)` → `callback(cur.samples.data(), ...)` |
| Chunks are hard-concatenated, no cross-fade | same loop; `result.samples.insert(result.samples.end(), cur.samples.begin(), cur.samples.end())` |
| `silence_scale` is applied *after* the callback loop | `if (config.silence_scale != 1) { result = result.ScaleSilence(config.silence_scale); }` sits below the `for` loop |
| `min_char_in_sentence` default **30**, `max_char_in_sentence` default **200** | `offline-tts-zipvoice-impl.h` `Generate()`: `config.GetExtraInt("max_char_in_sentence", 200)` / `...("min_char_in_sentence", 30)` |
| There is **no CLI flag** for either; `extra` is the only route | same; the `Register()` in `offline-tts-zipvoice-model-config.cc` registers no `--min-char-in-sentence` |
| Reference audio **and** reference text are mandatory, hard-fail otherwise | `Generate()` returns `{}` on empty `reference_audio`, empty `reference_text`, or `reference_sample_rate <= 0` |
| Upstream uses `cross_fade_concat(fade_duration=0.1)` and sherpa does not | `zipvoice/bin/infer_zipvoice_onnx.py` `generate_sentence()` |
| Exact transcript matters because token count sets the speaking-rate denominator | paper §II-F Eq. 11: `T_synthesis = T_prompt · (|y_synthesis| / |y_prompt|)` |
| `vocos_24khz.onnx` underscore, separate release, 54.2 MB | `releases/tag/vocoder-models` asset list |
| Distill int8 tarball is 109.2 MB | `releases/tag/tts-models` asset list |
| Only a zh+en Emilia variant is released in sherpa-onnx | same; assets are `…-zh-en-emilia.{tar.bz2}` ×4 (int8, fp32, base, distill) |
| Distill at 4 NFE vs 8 NFE is roughly a wash on WER/UTMOS | paper Table I: LS-PC 0.657/1.51/4.05 vs 0.647/1.54/4.11; SeedTTS-en 0.679/1.64/3.91 vs 0.670/1.62/3.91 |
| ZipVoice2 is unreleased | no ZipVoice2 checkpoint, model card, or sherpa-onnx asset found; the only ZipVoice assets are the four above |

### 2.2 Corrected — the prior draft was wrong

| # | Prior claim | What the source says | Impact |
|---|---|---|---|
| C1 | `OfflineTtsZipvoiceModelConfig` defaults are `feat_scale 0.1 / t_shift 0.5 / target_rms 0.1 / guidance_scale 1.0` | Those are the **C++** defaults. The **Rust** `Default` impl is **all zeros**: `~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/sherpa-onnx-1.13.6/src/tts.rs:248-263` | **Silent-failure trap.** `..Default::default()` → `feat_scale <= 0` → `Validate()` fails → `OfflineTts::create` returns `None` → factory surfaces "Failed to create ZipVoice engine". Must set all four explicitly. The official Rust example (`rust-api-examples/examples/zipvoice_tts.rs`) does exactly that. |
| C2 | `target_rms = 0` disables normalisation | Rejected twice: `offline-tts-zipvoice-model-config.cc` `Validate()` (`--zipvoice-target-rms must be positive`) **and** `offline-tts-zipvoice-impl.h` `Generate()` (`target_rms must be > 0`) → returns empty audio | "set to 0 to disable" is upstream **Python** argparse help text (`infer_zipvoice_onnx.py`), not sherpa semantics. **Never set it to 0.** |
| C3 | `num_steps` falls back to 4 | Falls back to 4 only when `config.num_steps <= 0`. Rust `GenerationConfig::default().num_steps = 5` (`tts.rs:434`) | `..Default::default()` gives **5 NFE**, not 4. Set it explicitly or you silently pay 25% more decoder time. |
| C4 | `extra` values must be **strings** | Official example uses `serde_json::json!(10)` — a number. `GetExtraInt` reads a `std::string` and parses via `ToIntOrDefault` (`offline-tts.cc:130-147`) | Both work. Use `json!` numbers, matching the official example. |
| C5 | Only `guidance_scale` deviates from upstream Distill defaults | Upstream `model_defaults` in `infer_zipvoice_onnx.py`: `zipvoice_distill → {num_step: 8, guidance_scale: 3.0}`. sherpa-onnx ships **4 and 1.0** | **Two** variables deviate, not one. The prior draft's single-variable A/B was mis-specified. §5.3 fixes this. |
| C6 | Reference re-encode is "the dominant per-call fixed cost" | `ComputePromptFeatures(...)` is called **once per `Generate()`**, outside the chunk loop, and is a 2-3 s STFT + a mel matrix | Overstated. It is a bounded ~280-frame STFT, not dominant. The dominant cost is NFE × full-sequence decoder passes. Do not spend effort caching mel; **do** cache the decoded `Vec<f32>` to skip disk I/O. |
| C7 | Distill NFE ablation supports "do not go below 4" for the shipped model | Paper Table I (the Emilia-100K Distill) has **only 4 and 8 NFE points**. The NFE 1/2 collapse (WER 92.17 / 15.00) and the 2-NFE WER 2.33 are **Table VI, the LibriTTS 555 h model** — a different checkpoint | The "≥4 NFE" conclusion is still sound but the evidence cited was from the wrong model. Stated correctly in §5.2. |
| C8 | `min_char_in_sentence` is "the biggest first-audio lever" and warrants a new user setting | **Wrong for Vox's architecture.** Vox already chunks: `services/harness/stages/streaming/chunker.rs` `ClauseChunker` emits clauses at 5/8/14 → 10/15/22 → 16/24/32 words, and each clause is a separate `TtsCommand::Generate` (`harness/stages/streaming/router.rs:240,301`) | `min_char_in_sentence` is near-inert for the first clause. **Do not expose it. Do not A/B it.** See §4.3 — this deleted a whole user-facing setting. |
| C9 | Reference clips: recommend AMI/LibriTTS/VCTK, and separately "record 9–10 humans" | `k2-fsa/TTS_eval_datasets` ships `librispeech_pc_testset` — 448 reference clips, **39 distinct speakers**, each with an exact `ref_text`, 16 kHz, 229 MB | The prior draft missed the one asset that is *ready today with exact transcripts and real distinct speakers*. §6. |

### 2.3 New findings the prior draft did not have

| # | Finding | Evidence | Impact |
|---|---|---|---|
| N1 | ⚠️ **The weights have no declared licence *and* the primary training corpus is non-commercial.** HF `k2-fsa/ZipVoice` `cardData` has **no `license` field**. Its 100 K-hour training corpus is **Emilia**, which is **CC-BY-NC and access-gated** — the gated terms on `amphion/Emilia-Dataset` state *"The researcher shall use the Emilia dataset under the CC-BY-NC license"*. Only Emilia-*YODAS* is CC-BY. The paper (paper §IV-A) names no licence at all. | HF API on `k2-fsa/ZipVoice`; HF API on `amphion/Emilia-Dataset` `extra_gated_prompt`; paper §IV-A | Materially worse than "get clarification later". The GitHub **code** is Apache-2.0 (`k2-fsa/ZipVoice` license field = `apache-2.0`); the **weights** are not covered by it. Per your decision, this ships without a UI warning — recorded in §10. |
| N2 | ⚠️ **Vox's model downloader cannot extract `.tar.bz2`.** `setup/model_manager.rs` `do_extract` handles `"zip"` (:342), `"tar.gz" \| "tgz"` (:369), and errors otherwise (:386). Cargo has `tar` and `zip` but **no `bzip2`** (`Cargo.toml:94,96`). | `app/src-tauri/src/setup/model_manager.rs:341-389`; `Cargo.toml:94,96` | Resolved by decision: extract the tarball locally and publish **individual files**, exactly as the `kokoro` group already does (5 separate `files[]` entries, one of them a `.tar.gz` for `espeak-ng-data`). No new dependency. |
| N3 | 🔴 **`caps_for_id` and `FALLBACK_CAPS` both have permissive catch-alls.** `settings.rs:681` `_ =>` returns Catalog caps; `settingsService.ts:51` `DEFAULT_CAPS` mirrors it. | `app/src-tauri/src/core/settings.rs:661-688`; `app/src/services/settingsService.ts:43-51` | A forgotten `"zipvoice"` arm **compiles, deploys, and silently mis-gates the whole TTS settings panel**. Must be an explicit arm plus a test assertion. |
| N4 | 🔴 **`TtsVoiceManager.tsx:147` is a two-way branch that will misroute ZipVoice.** `const customConfigKey = isRemoteGroup ? "chatterbox_remote" : "chatterbox";` — a *local* clone provider gets `"chatterbox"`, writing to the wrong settings sub-struct. | `app/src/shared/components/settings/models/TtsVoiceManager.tsx:147` | The single most likely silent frontend bug in this change. §7 Batch 4 fixes it. |
| N5 | `pre_bake_speaker_tensors` is **hardcoded to Chatterbox** (`chatterbox_rs::Engine::save_voice` → `speaker_emb.npy`) and `resolve_reference_audio` returns a `voice_dir` whenever `speaker_emb.npy` exists. ZipVoice needs a raw WAV **plus a transcript**. | `app/src-tauri/src/services/tts/voice.rs:234-266`; `app/src-tauri/src/services/tts/factory.rs:16-37` | Motivates the `voice_dir` sidecar contract in §6.4. |
| N6 | The paper is **silent** on streaming, incremental synthesis, and latency — zero matches for `streaming`/`incremental`/`latency` in the body. The prior draft's issue-quote citations (#100, #126, #188) are **UNVERIFIED** and I did not reproduce them. | paper v3 body; prior draft §1.1 | The non-streaming conclusion still stands — it rests on the sherpa C++ I read, not on the paper or the issues. I am not restating the quotes. |
| N7 | `MatchaTtsLexicon` is constructed with `skip_replacement = true` for ZipVoice, so no word/jieba replacement runs. | `offline-tts-zipvoice-impl.h` `InitFrontend()`; `matcha-tts-lexicon.h:22-24` | Good: nothing rewrites our reference transcripts behind our back. |
| N8 | sherpa-onnx **PR #3804** ("add configurable `espeak_voice` to ZipVoice model config for OOV") is **not merged** into 1.13.6. OOV words fall back to a default espeak voice. | PR #3804 file list; 1.13.6 `tts.rs:235-246` has no `espeak_voice` | A proper noun absent from `lexicon.txt` in a reference transcript will be espeak'd with the wrong voice. §6.3 adds a lexicon-coverage check. |

### 2.4 Verified model facts (with sources, since the paper does not state them)

| Fact | Value | Source |
|---|---|---|
| Parameters | "around 123M" | paper §IV-B |
| Text encoder | 4 Zipformer layers, dim 192, ff 512, 4 heads, CNN kernel 9 | `k2-fsa/ZipVoice/zipvoice_distill/model.json` |
| Flow-matching decoder | 5 stacks, layers `[2,2,4,4,4]`, downsample `[1,2,4,2,1]`, dim 512, ff 1536, 4 heads, CNN kernel `[31,15,7,15,31]` | same |
| Feature dim | 100 mel bins | same (`feat_dim`) |
| Sample rate | 24 000 Hz | same (`feature.sampling_rate`); corroborated by `offline-tts-zipvoice-model.cc:232-240` metadata defaults |
| `n_fft` / `hop` / `window` | 1024 / 256 / 1024 | `offline-tts-zipvoice-model.cc:234-238` (sherpa metadata defaults) |
| Vocoder | Vocos, **not** counted in the 123M | paper §IV-B: "All models use the same Vocos vocoder" |
| Lexicon requirement | Chinese pinyin `lexicon.txt` (1.7 MB) required by `MatchaTtsLexicon` | prior draft asset table; `Validate()` does **not** require it, but the frontend does |

> The paper contains **no** statement of sample rate, mel bins, or STFT geometry. Do not
> cite it for those; cite the model card JSON and the sherpa metadata reader.

---

## 3. Resolved decisions

| # | Question | Decision |
|---|---|---|
| D1 | Weight licensing posture | **Ship as a normal provider, no UI warning.** Risk recorded in §10 and N1. |
| D2 | Voice source | **User downloads clips into `sandbox/voices/`.** Plan defines that directory's contract and the normalisation script in §6.3. |
| D3 | Voice identity in settings | **Mirror Chatterbox:** new `TtsZipvoiceConfig { voice_id, min_char_in_sentence, guidance_scale }` on `TtsSettings`. `voice_index` untouched. |
| D4 | Scope | **Full end-to-end** in this document: specs → provider → manifest → voices → frontend → tests → bench. |
| D5 | Weight hosting | **I extract + mirror into `submodules/vox-models/tts/zipvoice/` and list individual files in the manifest; you upload to `addyo07/vox-models`.** No `bzip2` dependency. |
| D6 | Spec governance | **Amend `docs/specs/ipc-spec.md` and `docs/specs/events-spec.md` before code** (Batch 0). |

---

## 4. Architecture: how ZipVoice is wired into sherpa-onnx

### 4.1 The call path

```
ZipvoiceEngine::synthesize_chunk(text, ctx)
  └─ sherpa_onnx::OfflineTts::generate_with_config(text, &GenerationConfig, Some(cb))
       └─ C API SherpaOnnxOfflineTtsGenerateWithConfig
            └─ OfflineTtsZipvoiceImpl::Generate(text, gen_config, callback)
                 ├─ guard: reference_sample_rate > 0, reference_audio non-empty,
                 │          reference_text non-empty
                 ├─ guard: speed > 0, num_steps > 0, feat_scale > 0,
                 │          t_shift >= 0, target_rms > 0, guidance_scale > 0
                 ├─ prompt_token_ids = frontend_->ConvertTextToTokenIds(reference_text)   [ONCE]
                 ├─ prompt_features  = ComputePromptFeatures(ref_audio, ref_sr,
                 │                                 feat_scale, target_rms)              [ONCE]
                 ├─ sentences = SplitByPunctuation(text)          // '.', '!', '?', CJK
                 ├─ sentences = MergeShortSentences(sentences, min_char_in_sentence)  // default 30
                 ├─ chunks    = SplitLongSentence(s, max_char_in_sentence) per s     // default 200
                 └─ for each chunk:                                // SEQUENTIAL, no overlap
                      cur = GenerateChunk(chunk, prompt_tokens, prompt_features,
                                          speed, num_steps, feat_scale, t_shift, guidance_scale)
                        ├─ tokens = frontend_->ConvertTextToTokenIds(chunk)
                        ├─ mel = model_->Run(tokens, prompt_tokens, prompt_features,
                        │                    speed, num_steps, t_shift, guidance_scale)
                        │      // encoder once, then num_steps full-tensor decoder passes
                        ├─ out = vocoder_->Run(mel / feat_scale)   // Vocos, per chunk
                        └─ result.samples.insert(cur)
                        callback(cur.samples, (i+1)/total)         // AFTER full chunk
                 result = result.ScaleSilence(silence_scale)       // AFTER the loop
```

Three consequences the provider must respect:

1. **The reference STFT + tokenisation happen once per `Generate()`, not per chunk.** Do
   not build an elaborate prompt-feature cache; cache only the decoded `Vec<f32>` so we
   skip disk I/O and WAV parsing.
2. **The callback is the only streaming surface, and it is post-chunk.** Cross-fade
   therefore has to live in *our* provider across successive callback invocations (§7 Batch 2).
3. **`silence_scale` must be `1.0`.** Any other value mutates `result` after we have
   already forwarded every sample to playback, so callback audio and the returned
   `GeneratedAudio` would differ — a difference any test comparing the two will surface
   as a confusing failure.

### 4.2 `IsSentenceBoundary` and why Vox's own chunker dominates latency

sherpa's `IsSentenceBoundary` (`text-utils.cc:1273-1276`) is **only**
`. ! ? 。 ！ ？` — **commas do not split sentences.**

Vox, however, already chunks the LLM stream before the provider is ever called:

- `services/harness/stages/streaming/chunker.rs` `ClauseChunker::find_split_point` (`:61`)
  splits on `\n`, `?`/`!` (gated at 2 words for chunk 0), `, ; : — –` (gated at
  `w_min` words), and `.` (with decimal and abbreviation guards), with an emergency
  word cap at `w_max`.
- Word thresholds by chunk index (`:52-58`): `(5, 8, 14)`, `(10, 15, 22)`, `(16, 24, 32)`.
- Each emitted clause becomes its own `TtsCommand::Generate`
  (`harness/stages/streaming/router.rs:240` and `:301`).

So the first synthesis call already carries roughly 2–8 words. Running
`MergeShortSentences(30)` over a single 25-character clause is a no-op: the buffer is
flushed at the end regardless. **Lowering `min_char_in_sentence` from 30 to 10 changes
nothing for the first clause** — it only affects the final multi-sentence remainder that
`ClauseChunker::flush()` emits, and any clause that happens to contain an internal
sentence boundary.

**Therefore: leave `min_char_in_sentence` at 30. Do not expose it. Do not A/B it.** The
real first-audio levers, in order, are (a) `ClauseChunker`'s thresholds, (b) `num_steps`,
(c) how fast the LLM emits its first two words. §7 Batch 5 measures the first-audio
number against the *existing* chunker rather than tuning a knob that does nothing.

### 4.3 Seam count is worse than the prior draft assumed

The prior draft treated seams as a per-chunk concern inside one call. In Vox, a turn
produces roughly 3–8 `Generate()` calls, and each may itself produce multiple internal
chunks. Every boundary is a hard sample-level splice. With `min_char_in_sentence = 30` a
typical 5-clause turn yields ~5 seams.

`kokoro.rs:262` `trim_and_fade_samples` fades **in and out of every chunk independently**,
which is a click fix but produces an amplitude dip to zero at every seam — audible as
pumping across ~5 seams per turn. For ZipVoice the correct treatment is an
**equal-power cross-fade between consecutive chunks**, not independent fades. §7 Batch 2
implements that and leaves `trim_and_fade_samples` for Kokoro alone.

### 4.4 Parameter surface (authoritative)

`OfflineTtsZipvoiceModelConfig` — set **all ten fields explicitly** (C1):

| Field | C++ default | Rust `Default` | **Ship** | Validation rule |
|---|---|---|---|---|
| `tokens` | — | `None` | `tokens.txt` | non-empty + file exists |
| `encoder` | — | `None` | `encoder.int8.onnx` | non-empty + file exists |
| `decoder` | — | `None` | `decoder.int8.onnx` | non-empty + file exists |
| `vocoder` | — | `None` | `vocos_24khz.onnx` | non-empty + file exists |
| `data_dir` | — | `None` | `espeak-ng-data/` | if non-empty, must contain `phontab`, `phonindex`, `phondata`, `intonations` |
| `lexicon` | — | `None` | `lexicon.txt` | not validated, but `MatchaTtsLexicon` needs it |
| `feat_scale` | `0.1` | **`0.0`** | `0.1` | `> 0` else **empty audio** |
| `t_shift` | `0.5` | **`0.0`** | `0.5` | `>= 0` |
| `target_rms` | `0.1` | **`0.0`** | `0.1` | `> 0` else **empty audio** (C2) |
| `guidance_scale` | `1.0` | **`0.0`** | A/B → see §5.3 | `> 0` else **empty audio** |

`GenerationConfig` — per call:

| Field | Ship | Notes |
|---|---|---|
| `num_steps` | `4` **explicit** | maps to `TtsSettings.quality_steps`; must not inherit `5` (C3) |
| `speed` | from `TtsSettings.speed` | set the struct field; do **not** also set `extra["speed"]`, which silently overrides it |
| `silence_scale` | `1.0` | §4.1.3 |
| `sid` | `0` | ignored by ZipVoice; do not expose |
| `reference_audio` | cached `Vec<f32>` | mandatory, mono |
| `reference_sample_rate` | `24000` | mandatory, `> 0`; other rates are resampled internally |
| `reference_text` | sidecar transcript | mandatory, verbatim |
| `extra["min_char_in_sentence"]` | **omit** | §4.2 |
| `extra["guidance_scale"]` | only while sweeping | §5.3 |

### 4.5 Quality and latency — the evidence, correctly attributed

| Question | Answer | Source |
|---|---|---|
| Is 4 NFE enough for Distill? | Yes. Paper Table I (Emilia-100K Distill): 4 NFE scores **better** on LS-PC WER (1.51 vs 1.54) and UTMOS (4.05 vs 4.11), worse on SIM-o (0.657 vs 0.647), and CMOS +0.05 vs +0.16. | paper Table I |
| Do not go below 4 NFE — on what evidence? | The 1/2-NFE data is **Table VI, the LibriTTS 555 h model**, not the shipped Emilia Distill. Base model: NFE 2 → WER 15.00, NFE 1 → WER 92.17. Our distillation: NFE 2 → WER 2.33, NFE 1 → WER 18.81 (worse than ReFlow's 17.40). The paper attributes base-model collapse to "the absence of explicit token-level duration in the text condition". So: **4 is a sound floor, justified by a related checkpoint, not by the shipped one.** | paper Table VI + §V-E |
| Published CPU speed | Distill 4 NFE: **RTF 1.2202**; 8 NFE: 2.4177; base 16 NFE: 9.5529; F5-TTS 32 NFE: 37.284. Hardware: "a single thread of an Intel Xeon Platinum 8457C", 3 s prompt → 10 s output. Distill-4NFE is 32.6× faster than F5-TTS on that thread. | paper Table II + §V-A |
| Prior draft's "RTF 0.2065 vs Kokoro 0.2135" | **UNVERIFIED.** I cannot reproduce it; no script, no corpus, no raw samples are committed. Treat as a hypothesis. §7 Batch 5 measures it. | — |
| Prompt length guidance | README §3.1: "less than 3 seconds for single-speaker speech generation… A very long prompt will slow down the inference and degenerate the speech quality." Warns above 10 s, again above 20 s. **The paper itself says nothing about prompt length vs quality** — cite the README, not the paper. | ZipVoice README §3.1; `infer_zipvoice_onnx.py` `generate_sentence()` |
| Speaker similarity metric | SIM-o = cosine similarity between ECAPA-TDNN (**WavLM-based**) embeddings of the prompt and the synthesis. Not vs ground truth. | paper §IV-E |
| int8 quality | README §3.2: "the quantized model will result in a certain degree of speech quality degradation." fp32 Distill is 477.8 MB vs int8's 109.2 MB. | README §3.2; release asset list |
| Guidance scale | Distill passes ω as a **model input** via Fourier embedding, so there is **no** doubled forward pass. The paper recommends **no numeric value**. Upstream's script default is 3.0 for Distill; sherpa's is 1.0. | paper §II-E, §V-A; `infer_zipvoice_onnx.py` `model_defaults` |

---

## 5. Corrected reference-clip specification

### 5.1 Hard requirements

| Property | Requirement | Why |
|---|---|---|
| Duration | **2.0–3.0 s** | README §3.1; longer is slower *and* worse |
| Speakers | exactly one per clip | ZipVoice-Dialog's own paper reports 2-speaker flow-matching training produced "consistently unintelligible speech" |
| Internal pauses | **none** > ~0.5 s | A pause breaks the `|y_prompt|` token-duration estimate; upstream strips them via `remove_silence(only_edge=False, trail_sil=200)` |
| Edges | tight | upstream `remove_silence` trims beyond the threshold; we pre-trim so sherpa sees clean edges |
| Music / BGM | none | Emilia's pipeline runs source separation before training — ZipVoice never saw BGM |
| Sample rate | 24 kHz mono | `model.json` `feature.sampling_rate`; 16 kHz is resampled internally and is acceptable |
| Level | peak-normalise to **−1 dBFSFS**, then RMS ≈ **0.1** | See §5.2 |

### 5.2 Level handling — corrected

`ComputePromptFeatures` does:

```cpp
if (prompt_rms < target_rms && prompt_rms > 0.0f) { scale = target_rms / prompt_rms; /* boost prompt */ }
```

**One-sided boost, on a local copy of the prompt waveform, before mel extraction.** There
is **no** matching attenuation on the output. Upstream Python *does* apply the inverse
(`if prompt_rms < target_rms: wav = wav * prompt_rms / target_rms` in
`generate_sentence()`) — sherpa-onnx does not. So in sherpa:

- A **quiet** prompt is boosted into the mel condition, and the output comes out loud.
- A **loud** prompt is left alone, and the output is not pulled down.

Consequence: **normalise every clip ourselves** so `target_rms` never has anything to do.
Peak-normalise to −1 dBFSFS, then verify RMS lands near 0.1. This makes the one-sided
boost a no-op and makes level consistent across all 8–10 voices. The prior draft's
stated mechanism ("the inverse gain is applied to the output") is upstream-only and is
**not** what sherpa does.

### 5.3 Two-variable guidance/steps sweep (fixes C5)

Upstream Distill ships `num_step 8 + guidance 3.0`. sherpa-onnx ships `4 + 1.0`. Two
variables, never validated together in this runtime. The prior draft's
single-variable A/B could not have found the right pair. Correct order, **one variable
per run**:

1. `num_steps = 4`, `guidance_scale = 1.0` (sherpa default — baseline)
2. `num_steps = 4`, `guidance_scale = 3.0` (upstream Distill pairing)
3. `num_steps = 8`, `guidance_scale = 3.0` (upstream Distill exactly — the latency ceiling)
4. `num_steps = 6`, `guidance_scale = 3.0` (only if 2 wins and 3 is over budget)

Gate on blind listening, not on a metric. `guidance_scale` becomes a user setting **only
if** step 2 wins step 1 by a clear margin.

---

## 6. The reference voice pack — 8–10 clips (the open gap)

### 6.1 What is available (verified)

| Source | Licence | Conversational | Distinct speakers | Exact transcript | Verdict |
|---|---|---|---|---|---|
| **`k2-fsa/TTS_eval_datasets` → `librispeech_pc_testset`** | repo packaging **Apache-2.0**; underlying audio LibriSpeech **CC BY 4.0** (openslr SLR12), LibriVox source PD | 🔴 read audiobook | ✅ **39** | ✅ **exact**, `ref_text` field | **Ready today.** Best balance of licence + identity + transcript exactness. |
| Same repo → `seedtts_testset/en` | Apache-2.0 packaging; source clips are **Common Voice (CC0-1.0)** | 🔴 read | ✅ | ✅ | Strong alternative if CC0 beats CC BY 4.0 for you. 1.0 GB tarball. |
| **AMI** (`ihm` config) | **CC BY 4.0** (official licence page) | 🟢 **true spontaneous** | ✅ persistent `speaker_id` (`MEE068`…) | ✅ human | Only CC-BY corpus that is genuinely conversational. 16 kHz, ~150 spk, **mostly non-native English**. Higher effort: needs a crop + ASR gate. |
| **VCTK 0.92** | **ODC-By v1.0** (per the release `license_text.txt`) | 🔴 newspaper / Rainbow Passage | ✅ 109 native English, 48 kHz studio | ✅ | Best timbre diversity and studio quality; least natural material. Note the CC-BY-4.0 tag on HF uploads is uploader error. |
| **Emilia / Emilia-YODAS** | Emilia **CC-BY-NC + gated**; Emilia-YODAS CC-BY | 🟡 in-the-wild | ⚠️ video-scoped | ⚠️ ASR | Skip: gated, and NC. |
| **OpenDialog** | **CC-BY-NC-4.0** (HF `cardData.license`) | 🟢 best dialogue | — | ⚠️ | Disqualified on NC. |
| Expresso · The People's Speech · Piper voices | CC-BY-NC / no speaker field / per-voice non-commercial traps | — | ❌ | — | Disqualified, see prior draft §4 table. |
| ElevenLabs output | Prohibited by their ToS §9(j)(k)(l) for this use | — | — | — | **Excluded.** Prior draft's analysis holds. |

**Recommendation: start from `librispeech_pc_testset`.** It is the F5-TTS paper's
standard zero-shot English eval set, ZipVoice's own authors repackaged it, every clip
carries its exact transcript, and the speakers are real and distinct (39 of them). The
cost is narration register. If listening rejects the register, **AMI `ihm` is the only
CC-BY conversational source**, and it is a larger project.

**Cross-check worth doing:** LibriSpeech read speech is close to the LibriTTS half of
ZipVoice's own training mixture, so LibriSpeech clips are mildly in-distribution — a
genuine advantage for fidelity, and a mild caution against treating blind-listening wins
on this corpus as evidence about conversational voices.

### 6.2 `sandbox/voices/` — the contract you will satisfy

You drop raw candidate clips here. The plan's pipeline consumes exactly this layout:

```
sandbox/voices/
  <slug>.wav          # any sample rate, mono or stereo, any bit depth
  <slug>.txt          # EXACT verbatim transcript of <slug>.wav, one line, UTF-8
  voices.json         # optional metadata; see below
```

`voices.json` (optional but recommended — drives slug, label, gender, attribution):

```json
[
  {
    "slug": "ava_warm",
    "label": "Ava",
    "gender": "female",
    "wav": "ava_warm.wav",
    "text": "the exact transcript, verbatim",
    "source": "librispeech_pc_testset",
    "source_id": "4992-41806-0009",
    "license": "CC BY 4.0 (LibriSpeech / openslr.org/12)",
    "attribution": "LibriSpeech test-clean, speaker 4992"
  }
]
```

If `voices.json` is absent the script pairs `<slug>.wav` with `<slug>.txt` and derives the
slug from the filename.

**Minimum viable submission: 10 pairs.** The script will reject non-conforming clips and
report why, so a partial drop is fine — it will tell you what is missing.

### 6.3 Normalisation script — `sandbox/scripts/build_zipvoice_voicepack.py`

Non-production code, so it lives in `sandbox/` per AGENTS.md §2, with results in
`sandbox/results/`. One clip in → one validated asset out. Steps per clip:

1. Decode to mono f32 (stdlib `wave` + `audioop`-free manual handling, or `soundfile`
   if available; declare the dependency in the script header).
2. **Reject** if duration < 1.5 s or > 5.0 s (log the actual duration; do not silently
   pad or truncate).
3. Trim leading/trailing silence above −45 dBFS, then trim to ≤ 3.0 s, preferring to
   cut at a whitespace boundary inside the last 300 ms.
4. **Reject** if any internal silence run exceeds 0.5 s (`split_on_silence`-equivalent:
   RMS below threshold over a sliding window).
5. Peak-normalise to −1 dBFSFS; report RMS.
6. Resample to 24 000 Hz (linear or better; log the method).
7. **ASR round-trip gate:** transcribe the processed clip, compute WER against the stored
   transcript. **Reject unless WER = 0.** This is the correctness law of §4.4/§5 — a
   wrong transcript silently re-tunes the global speaking rate of that voice on every
   call. Use a local model (Vox already ships Whisper/Nemotron STT) or a hosted ASR you
   approve; log which.
8. **Lexicon coverage check** (N8): tokenise the transcript and flag any token not
   resolvable via `lexicon.txt` + espeak fallback, i.e. likely to be espeak'd with the
   wrong voice. Report, do not reject.
9. Write `~/.vox/models/tts/zipvoice/voices/<slug>/clip.wav` (16-bit PCM 24 kHz mono)
   and `reference.txt` (verbatim transcript, no trailing newline changes).
10. Emit `voices.json` next to them plus a `sandbox/results/zipvoice_voicepack_report.json`
    with per-clip duration, peak, RMS, WER, verdict, and rejection reason.

### 6.4 Shipped layout and the `voice_dir` sidecar contract

```
~/.vox/models/tts/zipvoice/
  encoder.int8.onnx  decoder.int8.onnx  vocos_24khz.onnx
  tokens.txt  lexicon.txt  espeak-ng-data/
  voices/
    voices.json
    <slug>/clip.wav          # 2.0-3.0 s, 24 kHz mono, peak -1 dBFSFS, RMS ~0.1
    <slug>/reference.txt     # EXACT transcript, verbatim, never regenerated
```

The `<slug>/` directory layout is deliberate: it reuses the existing `voices.voice_dir`
column with **no schema change** (`persistence/schema.rs:121-130` already has `voice_dir
TEXT`), and it deliberately does **not** contain `speaker_emb.npy` — which is exactly
how `resolve_reference_audio` (`factory.rs:16-37`) distinguishes a Chatterbox pre-baked
dir from a plain WAV path, and how `chatterbox.rs:55` / `chatterbox_remote.rs:49` detect
packaged voices via a `zipvoice_voice_` id prefix. **Follow that existing convention.**

### 6.5 Seeding — mirror `seed_packaged_voices`

`persistence/voices.rs:131-159` already seeds Chatterbox's packaged voices with ids
prefixed `chatterbox_voice_` (`:162`) and a display-name map. Add a
`seed_zipvoice_voices` alongside it, called from `persistence/schema.rs:260` next to the
existing call. It inserts one row per `<slug>/` with `source_kind = "zipvoice_pack"`,
`voice_dir = <…>/voices/<slug>`, and id `zipvoice_voice_<slug>`. It must be idempotent
(re-seeding must not duplicate rows) exactly as the Chatterbox seeder is.

### 6.6 Deliberate deferral: user-supplied reference clips

A user-recorded or user-imported ZipVoice reference needs a **transcript**, and the
`voices` table has nowhere to put one. Options all cost something: a schema migration
(which `db-spec.md` does not currently cover for `voices` at all — it is a pre-existing
spec gap), or a new `add_voice_from_file` transcript argument.

**This is out of scope for this plan.** Consequence: `caps_for_id("zipvoice")` returns
`clone: false`, so the existing clone button in `VoiceCarousel.tsx` stays hidden and the
user cannot create a broken (transcript-less) ZipVoice voice. The capability ships with
the pre-baked pack only. Recorded as the named follow-up in §10.

---

## 7. Implementation batches

Every batch is independently verifiable. Blast radius was traced against the live tree;
compile-breaking vs silent-breaking is marked because the silent ones are the dangerous
ones.

> **Build verification for all Rust batches:** `cargo clippy --all-targets` only
> (AGENTS.md §3.0). Test commands require your explicit approval.

### Batch 0 — Spec amendments (governance gate, no code)

**Depends on:** nothing. **Build:** n/a (markdown).

Per AGENTS.md §4.3, amend before code.

| File | Anchor | Change |
|---|---|---|
| `docs/specs/ipc-spec.md` | `### 2.6` header (`:247`) | Path drift: says `ipc/settings.rs`; code is `ipc/settings/{catalog,core,mutation}.rs` |
| `docs/specs/ipc-spec.md` | `#### get_model_catalog() & get_provider_caps()` (`:254-256`) | Currently does not name the provider set or the caps shape. Add: the canonical provider id set, the `ProviderCaps` shape (`voices`/`speed`/`quality_steps`/`clone`), the `TtsProviderConfig` `kind` tag set, and the rule that **an unknown id must not fall through to Catalog caps** (N3) |
| `docs/specs/ipc-spec.md` | `#### get_settings() & update_setting()` (`:250-252`) | Add the `tts.zipvoice` sub-struct key to the documented `tts` key list |
| `docs/specs/ipc-spec.md` | `#### list_voices(), add_voice_from_file(), …` (`:271-273`) | Currently "Custom voice profile CRUD for **Sherpa-ONNX / Kokoro** TTS" — stale. Correct to name the actual provider set and record that ZipVoice reads packaged `voice_dir` sidecars and does **not** support user-supplied clips |
| `docs/specs/events-spec.md` | `## 4` bullet "**Always Offload LLM & TTS**" (`:146`) | Names "Chatterbox/Sherpa". Add ZipVoice to the local-TTS offload set and record its ~164 MB footprint |
| `docs/specs/events-spec.md` | `## 8` matrix row 4 (`:258`) | "**Kokoro ONNX** or Edge TTS drops clause or fails decoding". Add ZipVoice, and record the new failure mode: **empty `GenerationAudio` when any of the four floats is out of range** (C1/C2) — which presents as silence, not an error |

**Done when:** both specs name ZipVoice, the caps fall-through rule is stated, and the
"silent empty audio" failure mode is in the error matrix.

### Batch 1 — Settings, constants, provider registration (backend, compile-breaking as a unit)

**Depends on:** Batch 0. **Build:** red until complete — this batch changes a shared
enum consumed across the crate. Expected.

| File | Symbol | Change |
|---|---|---|
| `services/tts/mod.rs` | new const block after `:66` | `ZIPVOICE_MODEL_DIR = "tts/zipvoice"`, `MODEL_FILE_TTS_ZIPVOICE_{ENCODER,DECODER,VOCODER,TOKENS,LEXICON}`, `MODEL_DIRNAME_TTS_ZIPVOICE_ESPEAK = "espeak-ng-data"`, `MAX_QUALITY_STEPS_ZIPVOICE`, `ZIPVOICE_SILENCE_SCALE = 1.0` (with the §4.1.3 rationale as the doc comment), `MIN_ZIPVOICE_GUIDANCE_SCALE = 1.0` / `MAX_… = 3.0` |
| `services/tts/mod.rs` | `:7-11` re-export | add `zipvoice::ZipvoiceEngine` |
| `services/tts/providers/mod.rs` | `:1-5` | `pub mod zipvoice;` |
| `services/tts/providers/mod.rs` | `TtsProviderKind` (`:24-30`) | `+ Zipvoice` |
| `core/settings.rs` | `TtsActiveProvider` (`:557-566`) | `+ Zipvoice` |
| `core/settings.rs` | new `TtsZipvoiceConfig` | `{ voice_id: Option<String>, guidance_scale: f32 }` with `#[serde(default)]`. `num_steps` reuses the shared `TtsSettings.quality_steps`; `min_char_in_sentence` is **not** a setting (§4.2) |
| `core/settings.rs` | `TtsSettings` (`:692-704`) + `Default` (`:706-721`) | `+ pub zipvoice: TtsZipvoiceConfig` and its default |
| `core/settings.rs` | `TtsProviderConfig` (`:618-642`) | `+ Zipvoice { voice_id: Option<String>, guidance_scale: f32 }` |
| `core/settings.rs` | `to_provider_config()` (`:723-747`) | `+ TtsActiveProvider::Zipvoice` arm |
| `core/settings.rs` | `caps_for_id()` (`:661-688`) | **explicit** `"zipvoice"` arm → `{ voices: Custom, speed: true, quality_steps: true, clone: false }`. Must not fall through (N3) |
| `core/settings.rs` | `test_caps_for_id_matrix` (`:1153-1173`) | add the `"zipvoice"` assertion |
| `core/defaults.rs` | TTS block (`:43-46`) | `DEFAULT_TTS_ZIPVOICE_GUIDANCE_SCALE` (value decided by Batch 5; seed with `1.0` and revise) |
| `ipc/settings/mutation.rs` | `"provider"` arm (`:345`) | `+ TtsProviderConfig::Zipvoice` arm |
| `ipc/settings/mutation.rs` | per-provider key arms (`:329-344`) | `+ "zipvoice" => TtsZipvoiceConfig` |
| `services/health.rs` | `check_tts_health` (`:170-223`) | `+ Zipvoice` arm → check all six asset paths + the `voices/` dir exist |
| `services/tts/factory.rs` | `create_tts_provider` (`:51-106`) | `+ Zipvoice` arm |

**Done when:** `cargo clippy --all-targets` is clean and `test_caps_for_id_matrix` passes.

> Note: `DEFAULT_TTS_VOICE_INDEX = 10` (`core/defaults.rs:43`) is a bare int meaningful
> only to Kokoro/Supertonic. It is **left untouched** — ZipVoice does not use it, and
> D3 keeps `voice_index` out of this change.

### Batch 2 — `zipvoice.rs` provider (backend, green throughout)

**Depends on:** Batch 1. **Build:** green throughout.

New file `app/src-tauri/src/services/tts/providers/zipvoice.rs`. Follow
`backend-style-guide.md` §2.1 file grammar and §3 constant placement; constants local
to this file go at the top, and anything shared goes in `services/tts/mod.rs`.

| Item | Requirement |
|---|---|
| `ZipvoiceEngine` | `tts: Mutex<OfflineTts>`, `speed: AtomicF32`, `reference: RwLock<Option<Arc<ZipvoiceReference>>>`, `guidance_scale: AtomicF32`, `quality_steps: AtomicU32`. Mirrors `kokoro.rs:35-53` for the `AtomicF32` newtype |
| `ZipvoiceReference` | `{ samples: Vec<f32>, sample_rate: u32, text: String, slug: String }` — loaded once per voice, shared via `Arc`, never re-decoded per call (§4.1.1) |
| `new(model_path, voices_dir, speed, guidance_scale, quality_steps, num_threads)` | Sets **all ten** `OfflineTtsZipvoiceModelConfig` fields explicitly (C1). Returns an error naming the missing file rather than letting `create` return `None` |
| `set_reference(&self, ref: ZipvoiceReference)` | Replaces the cached reference; the existing `TtsProvider::set_voice(&self, i32)` stays a no-op because the trait's `i32` cannot carry a slug. Log a `debug!` noting the index is ignored for ZipVoice — the real switch happens at factory time |
| `synthesize_chunk` | Cancel check → `is_devanagari` guard (emits the same `VoxEvent::Error` shape as `kokoro.rs:145-159`, since ZipVoice is zh+en only) → empty-text early return → `GenerationConfig` → `generate_with_config` with a callback |
| **Callback** | Accumulates into a per-turn buffer and applies an **equal-power cross-fade** against the tail of the previously emitted chunk (§4.3). This is the one new DSP routine; make it a free `fn` with a single `///` doc comment. Do **not** call `trim_and_fade_samples` — it is `kokoro.rs`-shaped and re-adds a 150 ms gap per chunk |
| Peak guard | Track max abs across the turn; if > 1.0, log a warning with the value. Do not silently normalise — a changing gain between turns is worse than occasional clipping |
| RTF telemetry | Mirror `kokoro.rs:232-255` exactly, including the `streamed_total == 0` fallback path |
| Failure paths | `generate_with_config` → `None` **or** zero-length samples **or** a suspiciously short result are all logged at `error!` with the reason. This is the C1/C2 empty-audio mode; silence must be diagnosable from the log, never silent |

`clippy` gate: no `unwrap()` outside poisoned-lock guards, no `#[allow]`, no `_`-masked
unused bindings, ≤50 lines per function, one `///` per function.

**Done when:** `cargo clippy --all-targets` is clean and the provider constructs against
the real downloaded model with all four floats set.

### Batch 3 — Reference resolution + model assets (backend, green throughout)

**Depends on:** Batch 1, and on the weights existing. **Build:** green throughout.

| File | Symbol | Change |
|---|---|---|
| `services/tts/zipvoice_assets.rs` (new) | `pub struct ZipvoiceVoiceEntry { slug, label, gender, text, license, attribution }` + `pub fn load_voice_pack(voices_dir) -> Result<Vec<ZipvoiceVoiceEntry>>` | Parses `voices/voices.json`. Rejects duplicate slugs, missing `clip.wav`/`reference.txt`, and a `text` that is empty or has leading/trailing whitespace (a silent rate bug, §5.1) |
| `services/tts/zipvoice_assets.rs` | `pub fn load_reference(entry, voices_dir) -> Result<ZipvoiceReference>` | Reads `clip.wav` → mono f32, returns samples + verbatim text. Single responsibility, no synthesis concerns |
| `services/tts/factory.rs` | `pub fn resolve_zipvoice_reference(voices_dir, slug) -> Result<ZipvoiceReference>` | Slug → entry → reference. Separate from `resolve_reference_audio`, which is Chatterbox-shaped and must stay that way (N5) |
| `services/tts/factory.rs` | `create_tts_provider` | Wire the reference into the `ZipvoiceEngine` before boxing |
| `sandbox/scripts/build_zipvoice_voicepack.py` (new) | — | §6.3. Non-production; `sandbox/` per AGENTS.md §2 |
| `submodules/vox-models/tts/zipvoice/` | — | Extract `sherpa-onnx-zipvoice-distill-int8-zh-en-emilia.tar.bz2` + `vocos_24khz.onnx` here (D5) |
| `manifests/models_manifest.json` | new `tts/zipvoice` group | `id: "zipvoice"`, `category: "tts"`, `subcategory: "main"`, `is_cloud: false`, `is_remote: false`, `is_built_in: false`, `version: "1.0.0"`, and `files[]` entries for `encoder.int8.onnx`, `decoder.int8.onnx`, `vocos_24khz.onnx`, `tokens.txt`, `lexicon.txt`, `espeak-ng-data.tar.gz`. **Individual files, not a `.tar.bz2`** (N2) — exactly the `kokoro` pattern at `:307-358` |
| `manifests/models_manifest.json` | `models_version` (currently `1.6.0`) + `total_size_bytes` | Bump both. `total_size_bytes` is a stored literal, not computed at runtime |
| `submodules/vox-models/models_manifest.json` | — | Mirror; it is byte-identical to `manifests/models_manifest.json` |

`sha256` values must be computed from the **extracted** files, not the tarball, and must
be verified by actually running the download path once before this batch is called done.

**Do not add `bzip2` to `Cargo.toml`.** That is the whole point of D5.

**Done when:** a fresh install downloads, verifies, and enumerates the group, and
`load_voice_pack` returns the seeded slugs.

### Batch 4 — Frontend (green throughout, but N4 is the risk)

**Depends on:** Batch 1. **Build:** green throughout.

| File | Symbol | Change |
|---|---|---|
| `app/src/store/settingsStore.ts` | `TtsActiveProvider` (`:15`) | `+ "zipvoice"` |
| `app/src/store/settingsStore.ts` | `TtsProviderConfig` (`:39-51`) | `+ { kind: "zipvoice"; voice_id?: string; guidance_scale: number }` |
| `app/src/store/settingsStore.ts` | `TtsZipvoiceConfig` + `TtsSettings` (`:199-231`) | New interface; add `zipvoice?:` to `TtsSettings` |
| `app/src/data/settingsCopy.ts` | `SETTINGS_SCOPE_KEYS.tts` (`:54-64`) | `+ "zipvoice"`. While here, note it also omits `threads` and `provider` — a pre-existing dirty-tracking gap; fix only if you want it, and say so |
| `app/src/services/settingsService.ts` | `FALLBACK_CAPS` (`:43-49`) | `+ "zipvoice": { voices: "custom", speed: true, quality_steps: true, clone: false }` (N3) |
| `app/src/services/voiceService.ts` | `listVoices` provider union (`:27-29`) | `+ "zipvoice"` |
| `app/src/shared/components/settings/models/TtsVoiceManager.tsx` | `customConfigKey` (`:147`) | 🔴 **Replace the two-way branch** with a map keyed on `providerId`, so a third clone provider routes to its own sub-struct (N4). The current expression would write `tts.chatterbox.voice_id` when ZipVoice is active |
| `TtsVoiceManager.tsx` | speed / quality controls (`:236-257`) | `caps.quality_steps` is currently consumed by **nothing** (confirmed: no UI reads it). ZipVoice is the first provider where a real quality control exists — add a `num_steps` control gated on `caps.quality_steps`, reusing the existing `quality_steps` setting and `RotaryKnob` |
| `TtsVoiceManager.tsx` | guidance control | Only if Batch 5 step 2 wins (§5.3). Advanced placement |
| `app/src/shared/components/settings/models/TtsModelWorkspace.tsx` | provider grid (`:60-61`) | Hardcoded `grid-cols-2` with a `length <= 2` special case. A **6th** TTS card flows into the scroll-snap branch. Verify layout with 6 cards before calling the batch done |
| `app/src/shared/components/settings/ModelStatusOverlay.tsx` | `activeVoice` (`:82`) | `modelCatalog.voices.find(v => v.id === draftSettings.tts.voice_index)` — an int-index lookup. **Must not** be used for ZipVoice, whose id is a UUID string. Guard on the voice source |

**Explicitly not needed:** there is no hardcoded TTS provider registry in TypeScript.
`modelCatalog.tts` comes from the manifest and the group id *is* `tts.active`, so
`ModelsCard.tsx:95-97`, `InteractionCard.tsx:101-106`, `ProviderSelectorView.tsx:23-84`,
`TtsModelWorkspace.tsx:38-52`, and `ModelSetupStep.tsx:215-222` are all manifest-driven
and need no change.

**Done when:** selecting ZipVoice in the desk shows a working voice carousel backed by
the seeded pack, changing voice hot-swaps the reference without a worker restart, and
`pnpm test` (5/5 in `app/src/test/invariants.test.ts`) still passes.

### Batch 5 — Measurement and the quality gate (green throughout)

**Depends on:** Batches 2, 3, 4. **Build:** n/a.

The prior draft referenced `/opt/vox/samples/tools/render_set.py` and `/opt/vox/agents.md`.
Neither path exists in this workspace. This batch defines the measurement in terms of
artifacts that actually exist here.

| Step | What | Output |
|---|---|---|
| 5.1 | `benches/tts_bench.rs`: add `run_zipvoice` + a construction block (`:211-218`, `:244-247`); `benches/common/tts_harness.rs:134-138` gets a zipvoice voice-cycling branch; `benches/pipeline_bench.rs:144-148` gets a `"zipvoice"` arm | `--model zipvoice` works |
| 5.2 | `num_steps ∈ {4, 6, 8}` at the settled `guidance_scale` | RTF p50/p90, length-stratified, **plus** the paper's CPU anchor (RTF 1.2202 at 4 NFE on one Xeon 8457C thread) as a sanity bound |
| 5.3 | Guidance/steps sweep in the §5.3 order | Decides `DEFAULT_TTS_ZIPVOICE_GUIDANCE_SCALE` and whether the control ships |
| 5.4 | **Time-to-first-audio** measured end-to-end through `PlaybackEngine`, at the *existing* `ClauseChunker` thresholds — not by varying `min_char_in_sentence` (§4.2) | p50/p90 in ms, ZipVoice vs Kokoro on the same prompts |
| 5.5 | **Seam audibility** at the realised chunk count per turn. Render with and without the Batch 2 cross-fade | Blind A/B |
| 5.6 | Clipping rate and peak distribution over the corpus | Confirms whether the Batch 2 peak guard needs to normalise |
| 5.7 | **Reference-transcript regression test:** perturb a stored transcript (add a word, drop a word) and assert the output speaking rate shifts. This is the empirical proof of §4.4's rate law and the guard against silent transcript rot | A failing-if-broken test, not a passing-if-absent one |
| 5.8 | Voice distinctness: all 8–10 slugs synthesise audibly different speakers. Use WavLM+ECAPA-TDNN cosine distance — the same model the paper uses for SIM-o (§4.5) | Pairwise distance matrix |
| 5.9 | **Speed calibration.** The prior draft's "~11% longer than Kokoro at equal speed" is **UNVERIFIED**. Measure mean utterance duration for both at `speed = 1.0` and set the ZipVoice default accordingly. Do not inherit `DEFAULT_TTS_SPEED = 1.05` on faith | Measured ratio + chosen default |
| 5.10 | **Blind listening set**, ≥40 samples per candidate, opaque ids | **The primary gate.** RTF is a constraint, not a score |

**Done when:** the guidance/steps default is chosen from data, the speed default is
calibrated, the transcript-regression test exists and is proven to catch a mutation, and
you have listened to the blind set and made a call.

### Batch 6 — Tests, benches, evals (green throughout)

**Depends on:** Batches 1–5. **Build:** green throughout.

| File | Change |
|---|---|
| `tests/common/paths.rs` | `+ "zipvoice"` to the import at `:10`; add `get_zipvoice_model_dir()` following `get_supertonic_model_dir()` (`:57-75`). Note there is no `get_kokoro_model_dir()` — do not add one |
| `app/src-tauri/tests/tts_to_playback_test.rs` | New ZipVoice case alongside `:36` and `:211`. Needs new golden clips in `tests/assets/` |
| `app/src-tauri/tests/tts_transition_test.rs` | New voice-switch case alongside `:48` — the direct test of §7 Batch 2's reference swap |
| `app/src-tauri/tests/settings_persistence_test.rs` | Round-trip a `TtsZipvoiceConfig` and a `zipvoice_voice_*` id |
| `app/src-tauri/tests/model_eviction_test.rs` | ZipVoice in the eviction lifecycle alongside `:115-118` |
| `evals/agentic_tool_eval.rs` | `+ "zipvoice"` arm (`:143-154`). ⚠️ `:158-161` calls `create_tts_provider(&settings, &model_path, None)` with `reference_audio = None` — **ZipVoice cannot be constructed that way.** The eval needs the reference threaded through, or ZipVoice is simply unsupported under `--tts-provider zipvoice`. State which, in the code |
| `docs/specs/integration-test-spec.md` | Seam 7 (`:842-928`) and Seam 8 (`:961-1040`) hardcode `SupertonicEngine` fixtures. Record the new ZipVoice seam cases |

**Test commands require your explicit approval** (AGENTS.md §3.0). The runner is:

```bash
RAYON_NUM_THREADS=$(nproc) OMP_NUM_THREADS=$(nproc) \
  cargo nextest run --release --test-threads=1 --no-fail-fast
```

~45.5 s warm. Run cargo commands strictly one at a time.

---

## 8. Requirements coverage — spec → batch

| Requirement | Batch |
|---|---|
| New `TtsProviderKind` / `TtsActiveProvider` / `TtsProviderConfig` variant | 1 |
| `TtsZipvoiceConfig` on `TtsSettings` + mutation path | 1 |
| Explicit `caps_for_id` arm + test | 1 |
| Health check arm | 1 |
| Provider implementing `TtsProvider` | 2 |
| All ten model-config fields set explicitly (C1) | 2 |
| `silence_scale = 1.0` (§4.1.3) | 2 |
| Inter-chunk equal-power cross-fade (§4.3) | 2 |
| Devanagari guard preserved | 2 |
| Peak/clipping observability | 2 |
| RTF telemetry parity with Kokoro | 2 |
| `voice_dir` sidecar resolution | 3 |
| Voice pack parse + validation | 3 |
| Normalisation script with WER = 0 gate | 3 |
| Manifest group, individual files, real sha256s | 3 |
| `min_char_in_sentence` **not** exposed (§4.2) | 1 (by omission), verified in 5.4 |
| `num_steps` control in the UI | 4 |
| `guidance_scale` control (conditional) | 4, gated on 5.3 |
| `customConfigKey` map replacing the two-way branch (N4) | 4 |
| `FALLBACK_CAPS` entry (N3) | 4 |
| Voice-source guard in `ModelStatusOverlay.tsx:82` | 4 |
| 6-card grid layout verified | 4 |
| Bench / pipeline-bench / eval arms | 5, 6 |
| Speed calibration | 5.9 |
| Transcript-mismatch regression test | 5.7 |
| Blind listening gate | 5.10 |
| Spec amendments | 0 |

**Deliberately dropped, with reasons:**

| Dropped | Why |
|---|---|
| `min_char_in_sentence` user setting | §4.2 — inert given `ClauseChunker`. A setting that does nothing is worse than no setting |
| `target_rms` user setting | `> 0` is mandatory (C2); there is no useful range to expose, and §5.2 normalises the clips so it never fires |
| `sid`, `feat_scale`, `t_shift`, `max_char_in_sentence` | No user-facing meaning; `feat_scale`/`t_shift` fixed at validated values |
| `bzip2` dependency | D5 — individual manifest files instead |
| User-supplied ZipVoice reference clips | §6.6 — no place to store the transcript. `clone: false`. Named follow-up |
| Multiple clips per persona for register variation | Real and interesting, but multiplies the asset count. Deferred; it is the one expressivity lever Kokoro genuinely lacks |
| ZipVoice2 | Unreleased. Re-check before committing further weeks |
| TensorRT / GPU | README §3.2: "Don't use ONNX on GPU." Out of scope |

---

## 9. Risks

| Risk | Severity | Mitigation |
|---|---|---|
| **Weights undeclared; training corpus CC-BY-NC** (N1) | **High, commercial** | Per D1, shipping without a UI warning. Recorded here, in the manifest `description`, and in the plan. If Vox ever goes commercial, ZipVoice must be removed or relicensed — this is not a "get clarification later" item |
| `..Default::default()` on the model config → silent empty audio (C1) | **High** | Batch 2 sets all ten fields; Batch 2's failure paths log at `error!`; `events-spec.md` row 4 (Batch 0) documents the mode |
| `target_rms = 0` → silent empty audio (C2) | **High** | Never set it. Constant with the rationale attached |
| Silent wrong-voice routing in the UI (N4) | **High** | Batch 4 replaces the two-way branch with a map; manual UI check of all three clone providers |
| Silent caps mis-gating (N3) | High | Explicit arm + `test_caps_for_id_matrix` + `FALLBACK_CAPS` entry |
| **No streaming** — first audio waits for a whole Vox clause | High, accepted | Not fixable without retraining. §4.2 shows the lever is `ClauseChunker`, not a ZipVoice knob. **Do not promise a latency win.** Expect parity with Kokoro |
| Chunk-seam clicks | High | Batch 2 cross-fade; Batch 5.5 blind A/B |
| Transcript rot re-tuning a voice's global rate | High | Sidecar storage (§6.4); WER = 0 gate at build (§6.3 step 7); mutation-proven regression test (Batch 5.7) |
| OOV proper nouns espeak'd with the wrong voice (N8) | Medium | §6.3 step 8 reports lexicon coverage per clip |
| Prompt longer than 3 s → slower **and** worse | Medium | §6.3 step 3 enforces ≤ 3.0 s |
| Thread oversubscription | Low | `DEFAULT_TTS_THREADS = 4`; measure before raising |
| int8 quality loss | Low | README §3.2 warns of it. fp32 Distill is 477.8 MB if the blind gate fails on quality |
| ZipVoice2 lands and obsoletes this | Low | Re-check before Batch 5; do not pre-optimise |

---

## 10. Named follow-ups (not in this plan)

1. **User-supplied ZipVoice reference clips.** Needs either a `voices` table schema
   migration (and `db-spec.md` currently does not document that table at all — a
   pre-existing spec gap) or a new `add_voice_from_file` transcript argument plus a
   `pre_bake_zipvoice_reference` sibling to the Chatterbox-hardcoded
   `voice.rs:234-266`. Unblocks by raising `clone: true`.
2. **Derive the provider axis from the manifest.** `TtsActiveProvider`,
   `TtsProviderConfig`, `caps_for_id`, `FALLBACK_CAPS`, and `SETTINGS_SCOPE_KEYS.tts`
   are five hand-maintained mirrors of the same axis. The manifest already carries
   `is_cloud`/`is_remote`/`is_built_in` and the UI already trusts them
   (`TtsModelWorkspace.tsx:37` says so in a comment). Deriving all five at boot would
   make the next provider a zero-frontend-diff change and would kill N3 permanently.
3. **Consolidate `caps.speed` / `caps.quality_steps`.** Both are fetched and shipped; the
   first is read by nothing (it renders the speed knob even for Edge TTS where
   `caps.speed === false`) and the second was read by nothing until Batch 4. Dead
   capability flags rot into wrong gating.
4. **`voice_index: i32` → per-provider slug.** D3 sidesteps this. When the first
   multi-voice-per-person provider or the Kokoro fine-tune path lands, migrate.
5. **AMI-derived conversational pack**, if the blind gate rejects LibriSpeech's
   narration register. The only CC-BY conversational option with persistent speaker
   identity; needs its own crop + ASR pipeline.

---

## 11. Checklist

### Batch 0 — specs (no code)
- [ ] `docs/specs/ipc-spec.md` §2.6 header path corrected — depends on: —
- [ ] `docs/specs/ipc-spec.md` `get_model_catalog()` / `get_provider_caps()`: provider id set, `ProviderCaps` shape, `TtsProviderConfig` `kind` set, no-fall-through rule — depends on: —
- [ ] `docs/specs/ipc-spec.md` `get_settings()`: `tts.zipvoice` key — depends on: —
- [ ] `docs/specs/ipc-spec.md` `list_voices()`: provider set corrected, ZipVoice sidecar read, no user clips — depends on: —
- [ ] `docs/specs/events-spec.md` §4: ZipVoice in the local-TTS offload set — depends on: —
- [ ] `docs/specs/events-spec.md` §8 row 4: ZipVoice + the empty-audio failure mode — depends on: —

### Batch 1 — backend registration (red until complete)
- [ ] `services/tts/mod.rs` — new `ZIPVOICE_*` / `MODEL_FILE_TTS_ZIPVOICE_*` / `MAX_QUALITY_STEPS_ZIPVOICE` constant block — depends on: 0
- [ ] `services/tts/mod.rs:7-11` — re-export `ZipvoiceEngine` — depends on: 0
- [ ] `services/tts/providers/mod.rs:1-5` — `pub mod zipvoice;` — depends on: 0
- [ ] `services/tts/providers/mod.rs:24-30` — `TtsProviderKind::Zipvoice` — depends on: 0
- [ ] `core/settings.rs:557-566` — `TtsActiveProvider::Zipvoice` — depends on: 0
- [ ] `core/settings.rs` — new `TtsZipvoiceConfig { voice_id, guidance_scale }` + `Default` — depends on: 0
- [ ] `core/settings.rs:692-721` — `TtsSettings.zipvoice` field + default — depends on: 0
- [ ] `core/settings.rs:618-642` — `TtsProviderConfig::Zipvoice` — depends on: 0
- [ ] `core/settings.rs:723-747` — `to_provider_config()` arm — depends on: 0
- [ ] `core/settings.rs:661-688` — explicit `caps_for_id("zipvoice")` arm — depends on: 0
- [ ] `core/settings.rs:1153-1173` — `test_caps_for_id_matrix` assertion — depends on: 0
- [ ] `core/defaults.rs:43-46` — `DEFAULT_TTS_ZIPVOICE_GUIDANCE_SCALE` — depends on: 0
- [ ] `ipc/settings/mutation.rs:345` — `"provider"` arm — depends on: 0
- [ ] `ipc/settings/mutation.rs:329-344` — `"zipvoice"` key arm — depends on: 0
- [ ] `services/health.rs:170-223` — `check_tts_health` arm — depends on: 0
- [ ] `services/tts/factory.rs:51-106` — `create_tts_provider` arm — depends on: 0
- [ ] `cargo clippy --all-targets` clean — depends on: all above

### Batch 2 — provider (green throughout)
- [ ] `services/tts/providers/zipvoice.rs` — file, grammar order per style guide §2.1 — depends on: 1
- [ ] `services/tts/providers/zipvoice.rs` — `AtomicF32` newtype (mirror `kokoro.rs:35-53`) — depends on: 1
- [ ] `services/tts/providers/zipvoice.rs` — `ZipvoiceEngine` struct + fields — depends on: 1
- [ ] `services/tts/providers/zipvoice.rs` — `ZipvoiceReference` struct — depends on: 1
- [ ] `services/tts/providers/zipvoice.rs` — `new()` sets all ten config fields — depends on: 1
- [ ] `services/tts/providers/zipvoice.rs` — `set_reference()` slug swap — depends on: 1
- [ ] `services/tts/providers/zipvoice.rs` — `synthesize_chunk` — depends on: 1
- [ ] `services/tts/providers/zipvoice.rs` — `synthesize_chunk` — Devanagari guard + error event — depends on: 1
- [ ] `services/tts/providers/zipvoice.rs` — `synthesize_chunk` — empty-text early return — depends on: 1
- [ ] `services/tts/providers/zipvoice.rs` — `synthesize_chunk` — `GenerationConfig` builder (C3) — depends on: 1
- [ ] `services/tts/providers/zipvoice.rs` — free `fn` — inter-chunk equal-power cross-fade — depends on: 1
- [ ] `services/tts/providers/zipvoice.rs` — `synthesize_chunk` — peak guard + warning log — depends on: 1
- [ ] `services/tts/providers/zipvoice.rs` — `synthesize_chunk` — RTF telemetry parity — depends on: 1
- [ ] `services/tts/providers/zipvoice.rs` — `synthesize_chunk` — `None` / empty / short failure logs — depends on: 1
- [ ] `services/tts/providers/zipvoice.rs` — `impl TtsProvider` — 6 methods — depends on: 1
- [ ] `cargo clippy --all-targets` clean; construct against the real model — depends on: all above

### Batch 3 — resolution + assets (green throughout)
- [ ] `services/tts/zipvoice_assets.rs` — `ZipvoiceVoiceEntry` — depends on: 1
- [ ] `services/tts/zipvoice_assets.rs` — `load_voice_pack()` — depends on: 1
- [ ] `services/tts/zipvoice_assets.rs` — `load_reference()` — depends on: 1
- [ ] `services/tts/factory.rs` — `resolve_zipvoice_reference()` — depends on: 1
- [ ] `services/tts/factory.rs` — `create_tts_provider` — wire the reference — depends on: 1
- [ ] `sandbox/scripts/build_zipvoice_voicepack.py` — 10 steps per §6.3 — depends on: 1
- [ ] `submodules/vox-models/tts/zipvoice/` — extracted assets + `vocos_24khz.onnx` — depends on: —
- [ ] `manifests/models_manifest.json` — new `tts/zipvoice` group, individual files — depends on: —
- [ ] `manifests/models_manifest.json` — `models_version` + `total_size_bytes` bump — depends on: —
- [ ] `submodules/vox-models/models_manifest.json` — mirror — depends on: —
- [ ] `persistence/voices.rs` — `seed_zipvoice_voices` (id prefix `zipvoice_voice_`), idempotent — depends on: 1
- [ ] `persistence/schema.rs:260` — call the seeder — depends on: 1
- [ ] download + verify + enumerate verified end-to-end; sha256 from extracted files — depends on: all above

### Batch 4 — frontend (green throughout)
- [ ] `app/src/store/settingsStore.ts:15` — `TtsActiveProvider` — depends on: 1
- [ ] `app/src/store/settingsStore.ts:39-51` — `TtsProviderConfig` — depends on: 1
- [ ] `app/src/store/settingsStore.ts:199-231` — `TtsZipvoiceConfig` + `TtsSettings` — depends on: 1
- [ ] `app/src/data/settingsCopy.ts:54-64` — `SETTINGS_SCOPE_KEYS.tts` — depends on: 1
- [ ] `app/src/services/settingsService.ts:43-49` — `FALLBACK_CAPS` — depends on: 1
- [ ] `app/src/services/voiceService.ts:27-29` — `listVoices` union — depends on: 1
- [ ] `TtsVoiceManager.tsx:147` — 🔴 `customConfigKey` → providerId map — depends on: 1
- [ ] `TtsVoiceManager.tsx` — `num_steps` control gated on `caps.quality_steps` — depends on: 1
- [ ] `TtsVoiceManager.tsx` — `guidance_scale` control — depends on: 1, 5.3
- [ ] `TtsModelWorkspace.tsx:60-61` — 6-card grid verified — depends on: 1
- [ ] `ModelStatusOverlay.tsx:82` — voice-source guard, no int lookup for ZipVoice — depends on: 1
- [ ] `pnpm test` green; voice carousel + hot-swap verified manually — depends on: all above

### Batch 5 — measurement (green throughout)
- [ ] `benches/tts_bench.rs:211-218,244-247` — `run_zipvoice` + construction block — depends on: 2, 3
- [ ] `benches/common/tts_harness.rs:134-138` — voice-cycling branch — depends on: 2, 3
- [ ] `benches/pipeline_bench.rs:144-148` — `"zipvoice"` arm — depends on: 2, 3
- [ ] 5.2 — `num_steps ∈ {4,6,8}` RTF, vs paper's 1.2202 anchor — depends on: 5.1
- [ ] 5.3 — guidance/steps sweep in §5.3 order → default decided — depends on: 5.2
- [ ] 5.4 — time-to-first-audio through `PlaybackEngine`, existing chunker — depends on: 5.1
- [ ] 5.5 — seam blind A/B, cross-fade on/off — depends on: 5.1
- [ ] 5.6 — clipping rate + peak distribution — depends on: 5.1
- [ ] 5.7 — transcript-mismatch regression test, mutation-proven — depends on: 2, 3
- [ ] 5.8 — ECAPA-TDNN distinctness matrix over 8–10 slugs — depends on: 2, 3
- [ ] 5.9 — speed calibration vs Kokoro → default set — depends on: 5.1
- [ ] 5.10 — ≥40-sample blind set, opaque ids — depends on: 5.2–5.9
- [ ] **Gate: blind listening decides ship / no-ship** — depends on: 5.10

### Batch 6 — tests + evals (green throughout)
- [ ] `tests/common/paths.rs:10,57-75` — import + `get_zipvoice_model_dir()` — depends on: 3
- [ ] `tests/assets/` — ZipVoice golden clips — depends on: 3
- [ ] `tests/tts_to_playback_test.rs` — ZipVoice case — depends on: 3, 4
- [ ] `tests/tts_transition_test.rs` — voice-switch case — depends on: 3, 4
- [ ] `tests/settings_persistence_test.rs` — config + id round-trip — depends on: 1
- [ ] `tests/model_eviction_test.rs` — eviction lifecycle case — depends on: 3, 4
- [ ] `evals/agentic_tool_eval.rs:143-154` — `"zipvoice"` arm — depends on: 3
- [ ] `evals/agentic_tool_eval.rs:158-161` — resolve the `reference_audio = None` blocker, or document ZipVoice as unsupported there — depends on: 3
- [ ] `docs/specs/integration-test-spec.md` — Seam 7 / Seam 8 — depends on: all above
- [ ] Full `cargo nextest` suite green (**requires explicit approval**) — depends on: all above
