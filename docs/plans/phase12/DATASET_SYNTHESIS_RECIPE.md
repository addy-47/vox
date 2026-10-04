# Vox Dictation Disfluency Refinement: Dataset & Synthesis Recipe

> **Canonical SSOT for Dictation Cleanup Dataset Assembly & Synthetic Generation.**
> Governed by [dictation-cleanup-engine.md](file:///home/addy/projects/apps/vox/docs/plans/wip/dictation-cleanup-engine.md) and [GOAL.md](file:///opt/vox/GOAL.md).

---

## 1. Core Architecture & Label Contract

The Vox Dictation Refinement engine operates as a **single-stage non-autoregressive span extractor** (`GLiNER 2.5` / `Convai Laya`) evaluating raw spoken text against **one label**:
- **Positive Label:** `"speech disfluency"`
- **Negative Background:** Implicit empty list `[]` (clean speech produces zero spans)
- **Zero Target Output:** The model identifies substrings to excise. Excising those spans via deterministic slice-cut and whitespace/orphan comma absorption restores the clean written text. No replacement strings or secondary SLM resolvers are used.

---

## 2. Canonical JSON Schema (`schema.py`)

Each JSONL example follows this canonical structure:

```json
{
  "id": "vxb_v2_00001",
  "category": "DISFLUENCY",
  "raw_text": "Because I thought, well anyway, we need to upgrade the database version.",
  "clean_text": "We need to upgrade the database version.",
  "spans": [
    {
      "label": "speech disfluency",
      "span": [0, 32],
      "text": "Because I thought, well anyway, ",
      "confidence": 1.0,
      "origin": "synthetic:false_start"
    }
  ]
}
```

For **CLEAN** negative controls:
```json
{
  "id": "vxb_v2_00002",
  "category": "CLEAN",
  "raw_text": "Any sort of food would be fine, as long as it is a bit expensive.",
  "clean_text": "Any sort of food would be fine, as long as it is a bit expensive.",
  "spans": []
}
```

---

## 3. Disfluency Taxonomy (Beyond Simple Fillers)

Human speech disfluency is **not** just filler words (`like`, `you know`). Synthetic generation and validation sample across 4 core archetypes:

1. **Repetition & Redundancy (30%)**: Lexical echoes, repeated words, conjunctions, prepositions, or noun/verb phrases.
   - Raw: *"Sure, we have we have Chinese, Indian, Italian or Mexican that you could choose from."* $\rightarrow$ Clean: *"Sure, we have Chinese, Indian, Italian or Mexican that you could choose from."* (Span: `"we have "`)
   - Raw: *"Vegetable vegetable glycerin or Carrier Oil can be used in place of aloe vera."* $\rightarrow$ Clean: *"Vegetable glycerin or Carrier Oil can be used in place of aloe vera."* (Span: `"vegetable "`)
   - Raw: *"I have no, I have no background in history and do not pretend to know."* $\rightarrow$ Clean: *"I have no background in history and do not pretend to know."* (Span: `"I have no, "`)

2. **False Starts & Abandoned Stems (30%)**: Abandoned clauses or thought transitions, frequently starting with conjunctions or prepositions.
   - Raw: *"Because I thought, well anyway, we need to upgrade the database."* $\rightarrow$ Clean: *"We need to upgrade the database."* (Span: `"Because I thought, well anyway, "`)
   - Raw: *"If we could, but actually he was educated as an engineer..."* $\rightarrow$ Clean: *"He was educated as an engineer..."* (Span: `"If we could, but actually "`)

3. **Retractions & Spoken Self-Corrections (25%)**: Verbal corrections with audible cues (`wait no`, `sorry`, `I mean`, `scratch that`).
   - Raw: *"What are some common mistakes made when writing Python, wait no, object oriented Lua programs?"* $\rightarrow$ Clean: *"What are some common mistakes made when writing object oriented Lua programs?"* (Span: `"Python, wait no, "`)
   - Raw: *"Have a great day and enjoy your flight, wait no, trip."* $\rightarrow$ Clean: *"Have a great day and enjoy your trip."* (Span: `"flight, wait no, "`)
   - Raw: *"I picked up The Great Gatsby, wait no, Beauty & Submission on a whim..."* $\rightarrow$ Clean: *"I picked up Beauty & Submission on a whim..."* (Span: `"The Great Gatsby, wait no, "`)

4. **Discourse Markers & Parentheticals (15%)**: Conversational discourse hesitations.
   - Raw: *"Is it possible, you know, for Windows machines to use the printer?"* $\rightarrow$ Clean: *"Is it possible for Windows machines to use the printer?"* (Span: `", you know, "`)
   - Raw: *"What day are you, I mean, looking to book it?"* $\rightarrow$ Clean: *"What day are you looking to book it?"* (Span: `", I mean, "`)

---

## 4. Unified In-VRAM Generation & Judging Architecture

### Models & In-VRAM Zero-Swap Residency
Both Generator and Auditor run on the local GPU via Ollama (`http://127.0.0.1:11434`):
- **Model:** `gemma4:12b` (8.0 GB resident VRAM out of 16.3 GB).
- **Zero Swapping:** Keeping a single model resident for both roles eliminates the 6–8s model-swap penalty between generation and judging rounds.
- **Why Qwen 3.5 9B was rejected as Judge:** Diagnostic auditing revealed that Qwen 3.5 9B suffered from severe false-rejection bias. It held an implicit rule that only filler words (`um`/`uh`) count as disfluencies, explicitly rejecting natural repetitions (`we have we have`) and verbal self-corrections (`Python, wait no, Lua`) as "grammar errors". Gemma 4 12B scored 100% on identical controlled tests.

### Generator Inference Configuration (`call_model`)
- `model`: `"gemma4:12b"`
- `think`: `False` (explicitly disabled)
- `temperature`: `0.1` (low, deterministic precision)
- `top_p`: `0.9`
- `num_predict`: `256`
- `concurrency`: 3 concurrent worker streams (`--gen-workers 3`)

### Auditor Inference Configuration (`BatchJudge`)
- `model`: `"gemma4:12b"`
- `batch_size`: `15` (`--judge-batch 15`)
- `temperature`: `0.0`
- `num_predict`: `2048`
- `prompt`: Multi-archetype auditor guidelines explicitly classifying repetitions, retractions, false starts, and discourse parentheticals as valid.

---

## 5. Compiler & Slice-Cut Contract (`compiler.py`)

The compiler converts raw LLM outputs into canonical dataset examples:
1. **Raw Candidate Input:** ONLY `corrupted_text` and `disfluent_spans`.
2. **Deterministic Offset Computation:** Matches verbatim substrings and absorbs orphan hesitation commas.
3. **Sentence-Initial Capitalization Parity:** Matches Vox Tier 1 `itn.rs` normalizer. When leading false starts or repetitions are excised, the initial character of the reconstructed sentence is capitalized, ensuring 100% byte-level identity with clean text.
4. **Roundtrip Assertion:** Asserts `clean_punctuation_whitespace(reconstructed) == clean_punctuation_whitespace(clean_text)`. Any pair failing this check is immediately rejected.

---

## 6. Execution Commands & Proven Benchmarks

### Generate Synthetic Disfluency Pairs
```bash
/opt/vox/venv/bin/python -u /opt/vox/sandbox/scripts/corrupt_from_clean.py \
  --clean /opt/vox/sandbox/corpus/clean_50k.jsonl \
  --output /opt/vox/sandbox/corpus/disfluency_synthetic.jsonl \
  --target 500 \
  --backends ollama \
  --judge ollama \
  --judge-model gemma4:12b \
  --chunk 100 \
  --gen-workers 3 \
  --judge-batch 15 \
  --judge-workers 1 \
  --deadline 600
```

### Verified Benchmark Performance
- **Throughput:** 99 verified, committed, judge-audited pairs in **131.0 seconds** (~1.3s per audited pair).
- **Yield:** **77.3%** end-to-end yield (99 committed out of 128 attempts).
- **Compiler Pass Rate:** **89.8%** (115/128).

### Quality Assurance Gate
```bash
/opt/vox/venv/bin/python /opt/vox/sandbox/scripts/qa_gate.py /opt/vox/sandbox/corpus/pilot_v2.jsonl
/opt/vox/venv/bin/python /opt/vox/sandbox/scripts/qa_gate.py /opt/vox/sandbox/corpus/pilot_v2_train.jsonl
/opt/vox/venv/bin/python /opt/vox/sandbox/scripts/qa_gate.py /opt/vox/sandbox/corpus/pilot_v2_val.jsonl
```
Every committed dataset must print `QA REPORT: CLEAN — all deterministic checks pass`.
