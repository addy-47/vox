# Product Requirements Document (PRD)
# Vox Realtime Dictation Speech Refinement Engine

---

## 1. Problem Statement & User Experience

### 1.1 Context & Core Need
Vox's dictation subsystem captures spoken audio via VAD and transcribes it using an acoustic STT model (Nemotron-3.5 / Qwen3-ASR). However, raw acoustic transcripts reflect spoken disfluencies, phonetic numbers, unformatted punctuation, and mid-sentence backtracking that make spoken dictation unusable without manual editing.

Vox requires a **neural refinement layer** between acoustic STT and the OS paste injection (`output_router`). This engine must transform spontaneous spoken thoughts into clean, publication-ready text that reads as if it were typed with care.

### 1.2 Scope & Domain
This is a **universal everyday dictation tool** across all desktop applications (chat, email, documents, terminals, code editors). It must handle conversational prose, technical acronyms, file names, company names, currency, and spoken voice commands simultaneously.

---

## 2. Functional Requirements & Transformation Matrix

| Capability | Raw Spoken Input (from STT) | Required Cleaned Output | Edge-Case / Hazard to Guard |
| :--- | :--- | :--- | :--- |
| **1. Backtracking & Self-Correction** | *"Let's do this on Friday or not actually on Saturday"* | *"Let's do this on Saturday."* | Do NOT truncate when no repair exists. |
| **2. Multi-word Repair** | *"Send three wait no actually send four copies"* | *"Send 4 copies."* | Preserve the verb and surrounding grammar. |
| **3. Inverse Text Normalization (ITN)** | *"twenty five dollars and fifty cents"* | *"$25.50"* | Never alter numeric values; convert spoken units to symbols. |
| **4. Dates & Times** | *"meet at four thirty p m on the twenty first"* | *"meet at 4:30 PM on the 21st"* | Handle relative context (AM/PM, ordinal suffixes). |
| **5. File Names & Extensions** | *"open dot e n v or package dot json"* | *"open `.env` or `package.json`"* | Recognize file extensions (`.tsx`, `.py`, `.rs`, `.env`). |
| **6. Acronyms & Real-world Entities** | *"deploy the v ram to a w s and open a i"* | *"deploy the VRAM to AWS and OpenAI"* | Truecase technical acronyms and company names. |
| **7. Spoken Quotes & Direct Speech** | *"he said quote I will return unquote immediately"* | *'he said "I will return" immediately'* | Balance quotation marks; handle unquote boundaries. |
| **8. Structural Voice Formatting** | *"first paragraph period new line second paragraph"* | *"First paragraph.\nSecond paragraph"* | Convert spoken punctuation commands to typography. |
| **9. Conversational Filler Removal** | *"Um, I think we should, you know, proceed"* | *"I think we should proceed"* | Strip hesitation crutches (*"um"*, *"uh"*, *"er"*). |
| **10. Semantic Preservation (Identity)** | *"I like drinking tea in the morning"* | *"I like drinking tea in the morning"* | **CRITICAL**: Never strip semantic words like *"like"*, *"umami"*. |

---

## 3. Architectural Evaluation: Encoder-Only Tagger vs. Full SLM

The core architectural dilemma is whether to use a **Token-Classification Tagger** (`ModernBERT` / `LFM2.5-Encoder-230M`) or a **Compact Generative SLM** (`LFM2.5-230M` / `Qwen3.5-0.8B`).

```
┌──────────────────────────────────────────────────────────────────────────────────────────────────┐
│                                   TAGGER vs. SLM TRADE-OFF MATRIX                                │
├─────────────────────────┬───────────────────────────────────────┬────────────────────────────────┤
│ Dimension               │ Token Tagger (Seq2Edit)               │ Compact Generative SLM (230M)  │
│                         │ (e.g. ModernBERT / LFM-Encoder)       │ (e.g. Liquid LFM2.5-230M)      │
├─────────────────────────┼───────────────────────────────────────┼────────────────────────────────┤
│ Operation               │ Label per token: KEEP / DELETE / PUNC │ Autoregressive text generation │
│ File names & Extensions │ ❌ Cannot insert dots or join tokens  │ ✅ Native (`dot env` -> `.env`)│
│ Acronyms & Entities     │ ❌ Cannot rewrite `open a i` -> OpenAI│ ✅ Native entity normalization │
│ ITN (Spoken Numbers)    │ ❌ Cannot turn `twenty five` into `25`│ ✅ Native numerical reasoning  │
│ Complex Backtracking    │ ⚠️ Can only delete exact spans        │ ✅ Reformulates syntax & tense │
│ Hallucination Risk      │ 0.0% (Mathematically impossible)      │ Small (Mitigated via guards)   │
│ CPU Latency             │ Ultra-fast (10–15 ms)                 │ Fast on edge (60–90 ms)        │
│ Memory Footprint        │ ~70 MB (INT8 ONNX)                    │ ~130–230 MB (Q8 GGUF)          │
└─────────────────────────┴───────────────────────────────────────┴────────────────────────────────┘
```

### Architectural Verdict
A pure token tagger **cannot scale to real-world dictation**. It cannot generate symbols, file extensions, numeric digits, or proper capitalization of multi-token entities (`"dot t s x"` $\to$ `".tsx"`). 

**Decision**:
1. **Primary Engine**: **Compact Generative SLM** (`LiquidAI/LFM2.5-230M` or `Qwen3.5-0.8B`). Handles full-spectrum normalization, repairs, file names, ITN, and formatting.
2. **Secondary Fallback & Safety Net**: **Deterministic Rule Engine + Validation Guardrails**. If the SLM violates a safety invariant, drops numeric tokens, or exceeds latency thresholds, the system drops the generative output and falls back to deterministic rules.

---

## 4. System Invariants & Execution Boundary

### 4.1 Utterance-Level Injection Contract
- **No In-Cursor Streaming**: Vox injects text into foreign host applications via simulated paste (`Ctrl+V` / `Cmd+V`) through `output_router`. Because simulated keystrokes cannot cleanly retract or backspace over committed words in third-party applications, **refinement runs once per completed utterance** at the `TranscriptFinal` boundary (PTT release or VAD silence auto-stop).
- **Left-Context Continuity (`prev_tail`)**: The engine accepts an optional context parameter containing the trailing 20 characters of Vox's previous utterance. This informs the model whether the upcoming utterance should start capitalized or continue a lowercase sentence.

### 4.2 Script & Language Gating (Hindi/Hinglish Non-Interference)
- Vox contains a dedicated Devanagari transliteration engine (`transliterate_if_hi`).
- **Invariant**: The English refinement model must **never touch Devanagari script** or romanized code-switched Hindi without explicit multilingual training. If Devanagari unicode characters are detected, the neural refiner is bypassed completely, routing directly to transliteration and output.

### 4.3 Runtime Lifecycle & Zero Idle RAM Guarantee
- Like STT and transliteration models in Vox, the refiner model must be an **evictable singleton**:
  - Lazily initialized upon first dictation activation.
  - Automatically evicted from RAM after 5 minutes of dictation inactivity.
  - Must not leak memory or persist background threads when dictation is disabled.

---

## 5. Input / Output Contracts & Runtime Safety Guardrails

### 5.1 Interface Schema
```rust
pub struct RefinementRequest<'a> {
    pub raw_transcript: &'a str,
    pub prev_tail: Option<&'a str>, // Last ~20 chars of preceding utterance
    pub language_tag: Option<&'a str>,
}

pub struct RefinementResponse {
    pub cleaned_text: String,
    pub latency_ms: u32,
    pub fallback_triggered: bool,
}
```

### 5.2 Mandatory Runtime Safety Guardrails (Zero Hallucination Gates)
Every output generated by the neural model must pass through three strict deterministic gates in Rust before being sent to the OS clipboard:

1. **Numeric Invariant Gate**:
   - Extract all spoken numbers from `raw_transcript` (e.g. *"twenty five"*, *"three hundred"*).
   - Verify that the numeric equivalent (`25`, `300`) exists in the cleaned output. If digits were dropped or altered, **reject the output** and fallback.
2. **Length & Edit Ratio Clamp**:
   - The cleaned output must satisfy: $0.35 \le \frac{\text{len}(\text{cleaned})}{\text{len}(\text{raw})} \le 1.15$ (unless resolving spoken punctuation commands).
   - If output length is suspiciously long (hallucinated conversational response like *"Sure, here is your text:"*), **reject and fallback**.
3. **Content Word Containment**:
   - Every proper noun or technical identifier in `cleaned` must have phonetic or lexical grounding in `raw_transcript`. The model must never introduce novel external facts.
4. **Fallback Behavior**:
   - If any gate fails, Vox falls back to a deterministic rule cleaner (regex filler stripper + basic ITN) applied to `raw_transcript`. Dictation output is never blocked.

---

## 6. Dataset Distribution & Curation Requirements

To avoid learning an aggressive "over-editing bias", the training and evaluation corpus must reflect natural speech distributions where the majority of spoken phrases are already grammatical.

### 6.1 Distribution Composition (Target: 80,000 Samples)
| Bucket | Proportion | Purpose / Description |
| :--- | :--- | :--- |
| **Bucket A: Clean Identity Invariants** | **45%** (36,000) | Already-clean sentences. Ground truth output is identical to input. Teaches the model when NOT to edit. |
| **Bucket B: Backtracking & Repairs** | **20%** (16,000) | False starts, explicit corrections (*"wait no"*, *"or rather"*), and implicit restarts. |
| **Bucket C: Numbers, Units & Dates (ITN)** | **15%** (12,000) | Spoken currencies, percentages, times, dates, and large numbers converted to digits. |
| **Bucket D: Entities, Acronyms & File Names**| **10%** (8,000) | File paths (`.tsx`, `.env`), acronyms (`AWS`, `VRAM`, `API`), companies (`OpenAI`, `GitHub`). |
| **Bucket E: Quotes & Spoken Formatting** | **10%** (8,000) | Quotes (*"quote ... unquote"*), line breaks (*"new line"*), and spoken punctuation commands. |

### 6.2 Hard Negatives Requirement
The dataset must explicitly include ambiguous linguistic constructs to prevent naive keyword deletion:
- *"I like the idea"* (Verb *"like"* $\to$ MUST KEEP).
- *"Wait for me outside"* (Imperative *"wait"* $\to$ MUST KEEP).
- *"The Jurassic period was prehistoric"* (Noun *"period"* $\to$ MUST KEEP).
- *"Get a price quote from them"* (Noun *"quote"* $\to$ MUST KEEP).
- *"We are launching a new line of shoes"* (Phrase *"new line"* $\to$ MUST KEEP).
- *"He is actually coming"* (Adverb *"actually"* $\to$ MUST KEEP).

### 6.3 Acoustic Realism Requirement
Pure LLM-generated text lacks realistic acoustic speech characteristics. Synthetic samples must be generated using **Target-First Programmatic Injection** combined with **TTS $\to$ Nemotron ASR round-tripping** to capture realistic speech recognition noise, phonetic homophones, and imperfect tokenization.

---

## 7. Acceptance Criteria & Evaluation Benchmarks

Before any model artifact is accepted into production, it must be evaluated on an independent, hand-audited test set of **500 real human dictation utterances**:

| Metric | Target SLA | Release Gate Threshold | Notes |
| :--- | :--- | :--- | :--- |
| **Identity Invariance Rate (IDR)** | **$\ge$ 99.0%** | $\ge$ 98.0% | Percentage of clean sentences left completely untouched. |
| **Numeric Accuracy** | **100.0%** | 99.8% | Digits, amounts, and dates correctly normalized without loss. |
| **Repair Recall (Backtracking)** | **$\ge$ 93.0%** | $\ge$ 90.0% | Correctly eliminates abandoned reparandums. |
| **Entity / Acronym Casing** | **$\ge$ 95.0%** | $\ge$ 92.0% | Accurately cases acronyms and file extensions. |
| **Hallucination Rate** | **0.0%** | $\le$ 0.05% | Introducing facts/words not present in speech. |
| **P95 Latency on 4-Core CPU** | **$\le$ 85 ms** | $\le$ 110 ms | Total elapsed time for 25-word utterance on client CPU. |
| **RAM Consumption (Active)** | **$\le$ 250 MB**| $\le$ 350 MB | Peak resident memory during inference. |
