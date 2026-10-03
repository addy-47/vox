# Vox Dictation Speech Refinement Engine & Architectural Specification

**Status:** Living Architectural Specification & Research Contract  
**Scope:** Dictation v2 (Upstream Acoustic STT + Native Deterministic Tier + Non-Autoregressive Neural Refinement Engine)  
**Audience:** ML Research Engineer, Backend Systems Engineer, Test Engineer  

---

## 1. Executive Summary & The Architectural Synthesis

Vox's speech refinement pipeline has undergone an essential convergence: moving from heavy autoregressive language models to a **fast, non-autoregressive, single-stage bi-encoder span extractor**.

### The Upstream Discovery
Empirical profiling on native audio clips revealed that **Nemotron-3.5 natively outputs full truecasing (proper nouns, sentence beginnings) and internal clause punctuation (commas, mid-turn periods, question marks, exclamation marks, and Hindi dandas).** 
The observed 0.4% terminal punctuation in early benchmarks was an **artifact of abrupt stream termination**: calling `OnlineStream::input_finished()` without acoustic padding cuts off the FastConformer transducer horizon before the joiner registers post-speech silence. 
By feeding **250–300ms of trailing acoustic silence** (4,000–4,800 zero samples at 16 kHz) prior to `input_finished()`, Nemotron commits closing sentence-final punctuation (`.`, `?`, `!`, `।`) natively at the acoustic source with negligible latency (~15ms CPU).

### The Architecture Pivot: Collapsing to Single-Stage Span Extraction
Prior drafts specified a two-stage cascade: Stage A Span Tagger (`DELETE` vs `REPAIR`) paired with a Stage B Micro-SLM Resolver (`SmolLM2-360M` / `Qwen-0.6B`) to rewrite "Type 2 generative repairs."

Empirical audit of spoken disfluency datasets (Switchboard, Nyra) and rigorous linguistic analysis revealed that **99.5%+ of all real dictation disfluencies are purely subtractive (Type 1)**:
When speakers misspeak, they audibly utter the replacement word or clause (e.g. `"Tuesday, wait no, Wednesday"` or `"three, actually four"`). Excising the reparandum and the cue restores the speaker's intended utterance 100% deterministically. 

Hypothetical "Type 2 generative repairs" (where a speaker expects a listener to infer unsaid inflections or syntactic reordering without speaking them) are virtually non-existent in dictation, while attempting to train an SLM to handle them introduced severe risks: autoregressive CPU latency (200–500ms), hallucination of entities, dropped numbers, and training data ambiguity.

**Decision:**
- **Retired:** Stage B Micro-SLM Resolver is completely dropped.
- **Unified Engine:** A single **Non-Autoregressive Bi-Encoder Span Extractor** (GLiNER 2.5 / Convai Laya / ModernBERT) querying a single semantic label: **`"speech disfluency"`**.
- **Execution:** Spans flagged as `"speech disfluency"` are excised via a deterministic slice cut with whitespace/comma collapse. Clean sentences naturally emit `[]` (empty list) and pass through in <1ms without modification.

---

## 2. Invariants (Non-Negotiable)

1. **Diagnosis precedes intervention:** Never download a model or design a training loop for an unmeasured failure mode.
2. **One variable per experiment:** Never bundle data changes with model changes or prompt tweaks.
3. **Model proposes → deterministic layer executes & validates:** The span extractor predicts `[start, end]` boundaries; deterministic code applies slice excision, safe whitespace/comma collapse, and guards meaning.
4. **Never block dictation:** Any error, timeout, or malformed refiner output must immediately fall through to raw STT text (fail-open).
5. **Zero-LLM in dictation path:** Refinement is local non-autoregressive neural/deterministic text processing. Never route dictation through generative conversational LLM channels (`llm_rx`).
6. **No fabricated metrics:** No latency or accuracy claim enters without a reproducible measurement on this box.
7. **Single-line pruning policy:** If disk free space drops below 10 GB (or 5 GB hard stop), immediately prune disposable cache (`temp/target`, `temp/downloads`, non-finalized candidate weights) before downloading new assets.
8. **No brittle heuristics on speech:** Rules must be 100% deterministic (e.g. WFST mathematics or exact lexical equivalence). Linguistic ambiguity is resolved by calibrated models, not speculative regex hacks.
9. **Single Semantic Label (`"speech disfluency"`):** The neural model queries a single label. Unannotated spans are the implicit negative background. Never introduce an explicit `"KEEP"` label.
10. **Zero Generative Models on Dictation Path:** No autoregressive SLMs in the refinement loop.

---

## 3. Two-Tier System Architecture

The pipeline decomposes into a strict, two-tier contract separating **100% deterministic mathematical execution** from **non-autoregressive neural span extraction**:

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
            ▼ (Punctuated text on closed sentence boundary: "Hey Vox, I need, um, twenty five boxes on Friday... wait no, Saturday.")
┌────────────────────────────────────────────────────────────────────────────────────────┐
│ TIER 1: DETERMINISTIC LAYER (< 1 ms, 0 MB extra RAM) — BOUNDARY-ONLY                   │
│ INVARIANT: Executes strictly on closed terminal boundaries (`.`, `?`, `!`, `।`).       │
│ NEVER runs on mid-stream audio slices or commas (commas are not syntactic boundaries).  │
│ 1. Inverse Text Normalization (ITN): Native Rust WFST via `text-processing-rs`         │
│ 2. Unambiguous Non-Lexical Filler Strip: Isolated acoustic markers (`um`, `uh`, `ah`)  │
│ 3. Verbatim Stutter Collapse: Identical word token deduplication (`the the` → `the`)   │
└────────────────────────────────────────────────────────────────────────────────────────┘
            │
            ▼
┌────────────────────────────────────────────────────────────────────────────────────────┐
│ TIER 2: NEURAL SPEECH REFINEMENT (Single Non-Autoregressive Bi-Encoder Span Extractor)  │
│ Model: Fastino GLiNER2.5-base / GLiNER2.5-small / Convai Laya (ModernBERT)             │
│ Queried Label: "speech disfluency"                                                     │
│ Output: Sparse spans `[{label: "speech disfluency", span: [s, e], confidence: c}]`     │
│ Default State: Implicit Untouched Passthrough (covers 80–85% of clean turns)           │
│                                                                                        │
│ ┌────────────────────────────────────────────────────────────────────────────────────┐ │
│ │ Bi-Encoder Forward Pass (Single Pass, Non-Autoregressive, 10–25ms CPU)             │ │
│ │ Evaluates candidate spans against semantic embedding of "speech disfluency"        │ │
│ └────────────────────────────────────────────────────────────────────────────────────┘ │
│                                  │                                                     │
│         ┌────────────────────────┴────────────────────────┐                            │
│         ▼                                                 ▼                            │
│   [SPANS EMPTY: `[]`]                         [HAS DISFLUENCY SPAN(S)]                 │
│   (Clean Passthrough)                         (conf >= tau, e.g. 0.90)                 │
│   (80–85% of clean turns)                                 │                            │
│         │                                                 ▼                            │
│         │                                     ┌───────────────────────────────┐        │
│         │                                     │ Deterministic Slice Cut       │        │
│         │                                     │ • Excise `raw[start:end]`     │        │
│         │                                     │ • Post-cut comma/space collapse│       │
│         │                                     └───────────────┬───────────────┘        │
│         │                                                     │                        │
│         └────────────────────────┬────────────────────────────┘                        │
│                                  ▼                                                     │
│ ┌────────────────────────────────────────────────────────────────────────────────────┐ │
│ │ Deterministic Guard Layer                                                          │ │
│ │ • Number & entity conservation check (clean text preserves intended numbers)        │ │
│ │ • Fail-open fallback: Return raw Tier 1 text on validation failure                 │ │
│ └────────────────────────────────────────────────────────────────────────────────────┘ │
└────────────────────────────────────────────────────────────────────────────────────────┘
            │
            ▼
[Committed OS Written Text]
```

---

## 4. The Single-Label Formulation

### Why One Label (`"speech disfluency"`) and No Explicit `"KEEP"`?
In bi-encoder architectures like GLiNER:
1. **Implicit Background Negative:** The model evaluates every candidate n-gram span $(i, j)$ in the sentence against label embeddings. Unannotated text is the negative background. An explicit `"KEEP"` label would cause a combinatorial explosion of positive spans (every valid word, bigram, and trigram) and distort training gradients away from disfluencies.
2. **Semantic Alignment:** Labeling the queried string as `"speech disfluency"` aligns directly with the pretrained linguistic embeddings of ModernBERT/DeBERTa, providing strong zero-shot and fine-tuning inductive bias.
3. **Downstream Simplicity:** In production, every detected span is processed with the same robust, deterministic slice-cut runtime:
   $$\text{clean\_text} = \text{clean\_whitespace\_commas}(\text{raw} \setminus \bigcup \text{spans})$$

---

## 5. Scope Allocation & Division of Labor

### Vox Core Backend Scope (Live Codebase)
1. **Upstream Punctuation Priming:** Maintain 250ms trailing silence padding in `finalize_stream` and `transcribe`.
2. **Benchmark Reporting Fix:** Modernize `stt_bench.rs` and `stt_harness.rs` to report dual metrics (`RawSim` & `NormSim`).
3. **Tier 1 Native ITN Integration:** Integrate native Rust WFST (`text-processing-rs`).
4. **Tier 2 Deterministic Slice Cut Engine:** High-performance string excision with whitespace/comma deduplication (`collapse_hesitation_commas`).

### Sandbox / ML Research Agent Scope
1. **Dataset Strategy (Clean + Disfluency Cuts):**
   - **Pilot Dataset (3,000–5,000 pairs):**
     - 50% Clean Negative Controls (`spans: []`): Business dictation, technical text, emails with valid prepositional/content usages of `"like"`, `"so"`, `"well"`, `"you know"`, and numbers to guarantee zero false positives.
     - 50% Speech Disfluency Cuts (`spans: [{label: "speech disfluency", span: [s, e]}]`): Conversational fillers in disfluent contexts, stutters, false starts, and speech retractions (`"Tuesday, wait no, Wednesday"` $\to$ span `[Tuesday, wait no,]`).
   - **Production Dataset (60,000–75,000 pairs):**
     - Scaled 50/50 distribution for final GPU fine-tuning.
   - **Held-out Gold Benchmark (2,500 sentences, frozen):**
     - Zero training leakage. Enforces over-deletion rate $< 1.0\%$.
2. **Span Extractor Evaluation & Fine-Tuning:**
   - Single forward-pass bi-encoder span extractor (`fastino/gliner2.5`, `convaiinnovations/laya`).
   - Calibrate confidence threshold $\tau$ on CPU to strictly enforce over-deletion rate $< 1.0\%$.
   - Export finalized checkpoint to ONNX for embedding in Vox backend.

---

## 6. Architectural Evolution & Historical Pivot Archive

To preserve hard-won systems knowledge and ensure future contributors understand why the pipeline converged to its current state, this section archives the complete history of architectural pivots and failure modes.

| Phase | Architecture Proposed | Core Mechanism | Why It Failed / Why We Pivoted |
|---|---|---|---|
| **Phase 1** | Local Pipeline LLM | Re-route punctuated STT output through the existing conversational model runtime. | **Autoregressive Latency & Hallucination:** Generating tokens sequentially has $O(N)$ time complexity, adding 300–800ms CPU latency. Conversational models hallucinated, rephrased user sentences, dropped digits, and violated the fail-open dictation SLA. |
| **Phase 2** | Standalone Fine-Tuned SLM | Dedicated compact generative model (`SmolLM2-360M`, `Qwen2.5-0.5B`) for turn rewriting. | **Persistent Generation Bottleneck:** While smaller, autoregressive generation on client hardware still took 150–400ms per turn. Even with constrained decoding, generative models risked subtle semantic drift and number alteration. |
| **Phase 3** | 3-Model Cascade (Router + Tagger + Repair) | Fast classifier router $\to$ bi-encoder tagger $\to$ generative repair model. | **Compounding Error & Runtime Bloat:** Maintaining three separate models tripled deployment complexity and memory footprint. Cascading classification errors meant router misclassifications corrupted downstream turns. |
| **Phase 4** | 2-Model Cascade (Span Tagger + Micro-SLM Resolver) | Bi-encoder tags `DELETE` vs `REPAIR`; slice cut excises `DELETE`; Micro-SLM rewrites `REPAIR`. | **The "Type 2" Fallacy & Synthesis Gridlock:** In natural dictation, >99.5% of speech repairs are subtractive (Levelt's reparandum + cue + repair). When prompted to invent "Type 2 generative repairs", LLMs generated proofreading and grammar fixes. The Judge rejected 87% of candidates, halting data synthesis for days. Maintaining the Micro-SLM introduced massive latency/hallucination risk for an unmeasured 0.1% edge case. |
| **Phase 5 (Current)** | Single-Stage Non-Autoregressive Span Extractor | Single bi-encoder (`GLiNER 2.5` / `Convai Laya`) querying `"speech disfluency"`; deterministic slice cut. | **Essential Simplicity & Mathematical Verification:** Clean text naturally emits `[]` (<1ms passthrough). Disfluent spans are excised with 100% fidelity without an SLM. Total CPU latency: 10–25ms. Zero hallucination risk. Collapses training to a single model. |

### The Core Lessons Learned
1. **Never build a subsystem for an unmeasured tail:** Preemptively building a generative SLM resolver for a theoretical 0.1% edge case paralyzed dataset creation and judge calibration for days.
2. **Generative models should never be on the low-latency dictation path:** Non-autoregressive encoder spans + deterministic slice excision guarantee safety, number conservation, and sub-30ms performance.
3. **One label is better than two:** Forcing the encoder to distinguish between "filler" and "self-correction" created artificial boundary error. A single label (`"speech disfluency"`) with implicit background negatives (`[]`) matches the mathematical loss function of bi-encoders.

