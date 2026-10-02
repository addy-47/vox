# Vox Dictation Speech Refinement Engine & Architectural Specification

**Status:** Living Architectural Specification & Research Contract  
**Scope:** Dictation v2 (Upstream Acoustic STT + Native Deterministic Tier + Neural Refinement Engine)  
**Audience:** ML Research Engineer, Backend Systems Engineer, Test Engineer  

---

## 1. Executive Summary & The Upstream Eureka

A prior evaluation of Vox's Nemotron-3.5 STT engine concluded that the model suffered from a severe structural defect: *"Sentence-final punctuation is absent in 99.6% of turns, and capitalization is missing."* This false diagnosis threatened to burden downstream systems with unnecessary neural models (e.g. 784 MB token taggers and 0.6B LLMs) just to restore commas, periods, and uppercase characters.

### The Upstream Discovery
Empirical profiling on native audio clips revealed that **Nemotron-3.5 natively outputs full truecasing (proper nouns, sentence beginnings) and internal clause punctuation (commas, mid-turn periods, question marks, exclamation marks, and Hindi dandas).** 

The observed 0.4% terminal punctuation was an **artifact of abrupt stream termination**: calling `OnlineStream::input_finished()` without acoustic padding cuts off the FastConformer transducer horizon before the joiner registers post-speech silence. 

By feeding **250–300ms of trailing acoustic silence** (4,000–4,800 zero samples at 16 kHz) prior to `input_finished()`, Nemotron commits closing sentence-final punctuation (`.`, `?`, `!`, `।`) natively at the acoustic source with negligible latency (~15ms CPU).

---

## 2. Invariants (Non-Negotiable)

1. **Diagnosis precedes intervention:** Never download a model or design a training loop for an unmeasured failure mode.
2. **One variable per experiment:** Never bundle data changes with model changes or prompt tweaks.
3. **Model proposes → deterministic layer executes & validates:** Models flag edits and predict repairs; deterministic code applies safe formatting, handles ITN, and guards meaning.
4. **Never block dictation:** Any error, timeout, or malformed refiner output must immediately fall through to raw STT text (fail-open).
5. **Zero-LLM in dictation path:** Refinement is local neural/deterministic text processing. Never route dictation through conversational LLM channels (`llm_rx`).
6. **No fabricated metrics:** No latency or accuracy claim enters without a reproducible measurement on this box.
7. **Single-line pruning policy:** If disk free space drops below 10 GB (or 5 GB hard stop), immediately prune disposable cache (`temp/target`, `temp/downloads`, non-finalized candidate weights) before downloading new assets.
8. **No 90% heuristics / brittle regexes on speech:** Rules must be 100% deterministic (e.g. WFST mathematics or exact lexical equivalence). Linguistic ambiguity must be resolved by calibrated models, not speculative string hacks.

---

## 3. Two-Tier System Architecture

The pipeline decomposes into a strict, two-tier contract separating **100% deterministic mathematical execution** from **calibrated neural speech refinement**:

```
[Raw Audio (16 kHz f32 Mono)]
            │
            ▼
┌────────────────────────────────────────────────────────────────────────────────────────┐
│ UPSTREAM STT ENGINE (Nemotron-3.5 + 250ms Trailing Silence Pad)                        │
│ • Full capitalization (proper nouns, sentence starts)                                 │
│ • Internal clause punctuation (commas, colons, exclamation marks)                      │
│ • Sentence-final punctuation (periods, question marks, dandas)                         │
└────────────────────────────────────────────────────────────────────────────────────────┘
            │
            ▼ (Verbatim punctuated text: "Hey Vox, I need, um, twenty five boxes on Friday... wait no, Saturday.")
┌────────────────────────────────────────────────────────────────────────────────────────┐
│ TIER 1: 100% DETERMINISTIC & EXACT LOCAL LAYER (< 1 ms, 0 MB extra RAM)                │
│ 1. Inverse Text Normalization (ITN):                                                   │
│    - Engine: Native Rust WFST (`text-processing-rs` / `sherpa-onnx` `rule_fsts`)      │
│    - Spoken numbers → digits ("twenty five" → "25", "ten dollars" → "$10", "10:30 AM")│
│ 2. Unambiguous Non-Lexical Filler Strip:                                              │
│    - Strips isolated non-semantic acoustic markers (`um`, `uh`, `er`, `ah`, `hmm`)    │
│ 3. Verbatim Stutter Collapse:                                                          │
│    - Deduplicates consecutive identical word tokens (`the the` → `the`, `I I` → `I`)  │
└────────────────────────────────────────────────────────────────────────────────────────┘
            │
            ▼
┌────────────────────────────────────────────────────────────────────────────────────────┐
│ TIER 2: NEURAL SPEECH REFINEMENT ENGINE (Confidence-Gated Cascade)                     │
│ Target: Ambiguous Conversational Fillers ("like", "you know") & Speech Repairs         │
│                                                                                        │
│ ┌────────────────────────────────────────────────────────────────────────────────────┐ │
│ │ Stage A: System-1 Decision Model / Token Tagger (e.g. Laya / GLiNER / ModernBERT)  │ │
│ │ • Non-autoregressive single-pass forward classification (~20–35 ms)                │ │
│ │ • Emits typed spans: `INTJ` (filler), `EDITED` (repair), with calibrated confidence │ │
│ └────────────────────────────────────────────────────────────────────────────────────┘ │
│                                  │                                                     │
│         ┌────────────────────────┴────────────────────────┐                            │
│         ▼                                                 ▼                            │
│   [TYPE 1: Subtractive Repair]                      [TYPE 2: Generative Repair]        │
│   (High Confidence Span)                            (Low Confidence or Rewrite Needed) │
│         │                                                 │                            │
│         ▼                                                 ▼                            │
│ ┌───────────────────────────────┐           ┌───────────────────────────────┐          │
│ │ Deterministic Slice Cut       │           │ Stage B: Micro-SLM Resolver   │          │
│ │ Deletes marked tokens without │           │ (SmolLM2-360M / Qwen-0.6B)    │          │
│ │ changing remaining token order│           │ Rephrases repair span only    │          │
│ └───────────────────────────────┘           └───────────────────────────────┘          │
│         │                                                 │                            │
│         └────────────────────────┬────────────────────────┘                            │
│                                  ▼                                                     │
│ ┌────────────────────────────────────────────────────────────────────────────────────┐ │
│ │ Stage C: Deterministic Guard Layer                                                 │ │
│ │ • Number & entity conservation check (no dropped digits)                           │ │
│ │ • Conjunction protection (`and`, `but`, `so`)                                      │ │
│ │ • Fail-open fallback: Return raw Tier 1 text on validation failure                 │ │
│ └────────────────────────────────────────────────────────────────────────────────────┘ │
└────────────────────────────────────────────────────────────────────────────────────────┘
            │
            ▼
[Committed OS Written Text]
```

---

## 4. The Decision Boundary: Type 1 vs Type 2 Speech Repairs

A central design principle governs when a repair can be handled by direct token excision versus when it requires a generative language model.

### The Canonical One-Liner Test
> **“Can I get the intended final utterance by deleting existing tokens, without changing the order of the remaining tokens?”**
> - **If YES $\to$ Type 1 (Subtractive / Extractive Repair)**
> - **If NO $\to$ Type 2 (Context-Inferred / Generative Repair)**

### Type 1: Subtractive / Extractive Repairs
* **Definition:** The correct utterance is already fully contained as a subsequence of the spoken words. The disfluency consists of a restart, false start, or superseded candidate.
* **Example:**
  - *Spoken:* `"I want to do this on Friday, oh no no, not Friday, Saturday."`
  - *Test:* Can we reach `"I want to do this on Saturday."` purely by deleting tokens? **Yes.**
  - *Deleted Span:* `[Friday, oh no no, not Friday,]`
* **Execution Contract:** 
  The System-1 model flags the span as `EDITED` with confidence $\ge \tau$. The deterministic layer executes the token deletion directly. **No generative SLM is invoked.**

### Type 2: Context-Inferred / Generative Repairs
* **Definition:** The intended utterance requires morphological transformation, word reordering, syntactic restructuring, or context inference that cannot be produced solely by deleting tokens.
* **Example:**
  - *Spoken:* `"Send that to John... actually make that both John and Sarah's managers."`
  - *Test:* Can we obtain `"Send that to both John and Sarah's managers."` purely by deleting tokens? **No** (requires inflection and syntactic reorganization).
* **Execution Contract:**
  The System-1 model flags a Type 2 structural boundary or reports low confidence. The bounded clause is passed to the **Micro-SLM Resolver (SmolLM2-360M)** with strict prompt constraints, followed by Stage C deterministic validation.

---

## 5. Scope Allocation & Division of Labor

### Vox Core Backend Scope (Live Codebase)
1. **Upstream Punctuation Priming:** Maintain 250ms trailing silence padding in [finalize_stream](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/stt/providers/nemotron.rs#L103-L138) and `transcribe`.
2. **Benchmark Reporting Fix:** Modernize [stt_bench.rs](file:///home/addy/projects/apps/vox/app/src-tauri/benches/stt_bench.rs) and [stt_harness.rs](file:///home/addy/projects/apps/vox/app/src-tauri/benches/common/stt_harness.rs) to report dual metrics (`RawSim` preserving casing and punctuation, alongside normalized `NormSim`), printing raw hypothesis strings to stdout.
3. **Tier 1 Native ITN Integration:** Integrate native Rust WFST (`text-processing-rs` or `sherpa-onnx rule_fsts`) in `EmbeddedSttProvider`.

### Sandbox / ML Research Agent Scope
1. **Evaluation Datasets:**
   - **`nvidia/Numb3rs`:** Standardized ASR number-transcription and ITN evaluation.
   - **`kensho/spgispeech`:** Production financial and business dictation with truecased and formatted ground truth.
   - **`vxb-synth`:** Constructed evaluation set with controlled, labeled damage (Type 1 deletions, Type 2 repairs).
   - **`vxb-real`:** Real disfluent audio recordings (`DisfluencySpeech`) for regression and realism audits.
2. **System-1 Model Evaluation:**
   - Benchmark non-autoregressive decision models: **Laya** (ModernBERT-large decision head), **fastino/gliner2.5-base-v1**, and fine-tuned token classifiers.
   - Measure single-pass CPU inference latency (budget: $\le 35\text{ ms}$).
   - Calibrate confidence threshold $\tau$ to enforce **over-deletion rate $< 1.0\%$**.
3. **Micro-SLM Resolver Formulation:**
   - Evaluate compact SLMs (SmolLM2-360M, Qwen-0.6B) on Type 2 repairs under structured JSON/span input-output contracts.
