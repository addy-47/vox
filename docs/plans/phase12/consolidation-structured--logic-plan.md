# Personal Memory Consolidation — Evaluation Findings

> **Status:** Investigation report. The structured-delta implementation plan that
> previously lived in this file has been **removed**: the plan was executed, and
> the evaluation of that execution is recorded here instead.
> **Contents:** Part 1 — the failure analysis that triggered the refactor.
> Part 2 — failures observed in the refactored delta logic during the
> re-evaluation run.

---

# Part 1 — What failed in the original full run

The pipeline itself was mechanically healthy:

- 5,200 turns persisted.
- 14 sessions created.
- 33 compactions completed.
- Zero incomplete compactions.
- Zero failed queue items.
- 1,625 facts persisted.
- Ingestion accounting passed for every case.

The 11 failed cases came from two separate problems.

## 1. Compaction-count classification: 6 cases

- Case 04: expected 1, actual 0.
- Cases 10–14: expected 3, actual 4 or 5.
- Actual total: 33 compactions versus 28 expected.

This is not a broken compactor. The expected counts were calibrated to a different model's compaction output size. That model's generated context has a different size, so later sessions cross the threshold earlier or later.

**Conclusion:** compaction counts are model-specific calibration data, not a correctness specification. The model-independent assertion is **trigger correctness** — every compaction fires at a critical turn, with a contiguous ledger and a valid watermark.

## 2. Personal-memory continuity: 8 cases

Failed cases: 03, 05, 06, 07, 08, 11, 13, 14.

The model consolidated successfully, but rewrote or dropped prior memory anchors. The original prompt said "preserve existing memory", but that is only a behavioral instruction; the model can still regenerate the whole document and violate it.

This is what motivated replacing full-document regeneration with an audited patch protocol.

---

# Part 2 — Failures observed in the structured-delta logic

Re-evaluation of the shipped delta implementation surfaced **eleven** defects. Several are upstream generation/capability failures; the majority trace to a single structural root cause documented in §2.4.

Run context: executor and judge both `qwen3.5:9b` on the remote Ollama server, 14-case shared-DB ladder, fresh database.

## 2.1 Truncated model output aborts the entire consolidation

**Severity: blocking.** The 14-case matrix could not complete.

```
case_02: status=FAILED error=Production personal-memory consolidation failed
  Failed to parse consolidation JSON patch output:
  EOF while parsing a string at line 170 column 316
```

The response was cut off mid-string by the 4096-token output ceiling
(`token_limit: options.num_predict`). `consolidate_personal_memory` treats
unparseable output as fatal: the whole cycle is lost, candidate facts remain
`active`, and the calling evaluation aborts. There is no retry and no salvage of
the operations that were already syntactically complete.

A second, distinct fatal path was also observed: `LLM generated empty personal
memory document!` when the model returned an empty payload.

## 2.2 Root cause of the degenerate output: reasoning force-disabled

The model was emitting whole-document echo operations — every `target_text` and
`proposed_text` was a near-verbatim multi-line copy of the entire document
(445 characters) — against a base document of only 445 characters. Five full
copies exhausted the output budget and produced the truncation in §2.1.

Cause: `execute_personal_llm_pass` hard-set `request.options.reasoning =
ReasoningMode::Disabled`. Via the Ollama wire policy
(`reasoning_off: {path: "think", value: false}`) this sent `think: false`.

This setting was inherited, not chosen. Memory compaction carries an explicit,
documented invariant that reasoning is always disabled because it is a
voice-latency summarization pass. Consolidation is a different class of task —
background, correctness-critical, once per session — but it shared
compaction's `GenerationPurpose`, so it silently inherited compaction's
invariant.

Measured directly against the server on an equivalent input:

| Reasoning | Output | Operation shape |
|---|---|---|
| disabled | 1,244 chars | 4-line and 3-line quoted targets (degenerate) |
| enabled | 488 chars | 2 clean single-line operations |

With reasoning enabled the model reasons about the diff first and then emits
minimal, correctly-targeted edits. Omitting `think` is sufficient to enable it —
the server defaults to thinking on.

**Temperature was already correct** at `0.2`, matching compaction. No change was
needed there.

## 2.3 Strict JSON schema was silently disabled by a catalog defect

**Severity: blocking, and app-wide.** Both memory compaction and personal
consolidation were falling back from a strict JSON schema to bare
`format: "json"`, on every run, with this warning:

```
[Memory::Personal] Model qwen3.5:9b lacks structured-output support;
using JSON-object baseline.
```

`get_baseline_spec` resolved unknown models by substring and family-prefix
matching over 4,595 catalog entries, returning whichever entry the `HashMap`
iterator happened to visit first. `qwen3.5:9b` matched `family = "qwen"`, a
family tag shared by 66 models whose capabilities disagree with each other:

| `supports_structured` | `context_window` | model count |
|---|---|---|
| `false` | 32768 | 6 |
| `false` | 131072 | 26 |
| `false` | 262144 | 12 |
| `true` | 131072 | 4 |
| `true` | 262144 | 22 |
| `true` | 1000000 | 6 |
| (plus `false` at 1000000 and 1048576) | | |

Consequences:

- `supports_structured` resolved to `false`, disabling strict schema output.
- The answer was **non-deterministic**, dependent on hash iteration order.
- `context_window` and `supports_tools` were equally arbitrary.
- The correct entry for a qwen3.5 model exists and reports
  `supports_structured: true`.

**Status: fixed.** `get_baseline_spec` now performs exact-match lookup only.
Unknown models return `None`, which callers already treat as "capabilities
unknown" and handle with optimistic defaults plus negotiate-down on a provider
400. Two regression tests were added to lock the behaviour in.

## 2.4 `section` was empty in 100% of operations — the structural root cause

Every operation emitted across every case carried an unusable section name:

```
case_01  op=insert   section=''
case_02  op=replace  section='## '   (x5)   op=insert section='## '
case_03  op=insert   section='## '   (x3)
case_05  op=insert   section='##'    (x4)
```

The document at that point:

```
## 
- The user has decided to slow down on Japanese studies while focusing more on Spanish instead.

## 
- The user has decided to focus more on their Spanish studies while slowing down on Japanese for a while.

## 
- The user wants hard sci-fi books for reading on weekends.

## 
- The user likes to read hard sci-fi books on weekends and specifically enjoys 'Children of Memory'.
```

Every heading is a bare `##` with no title, and the content is a flat,
append-only bullet list.

**Mechanism — self-perpetuating corruption:**

1. Case 01 starts from an empty document. The model has no heading to copy and
   emits `section: ''`.
2. `apply_insert_op` finds no match and mints a heading. Because
   `op.section.trim_start().starts_with('#')` is false, it writes
   `format!("## {}", heading_title)` where `heading_title` is
   `normalize_heading("")` = `""` — a nameless `## ` heading.
3. From v2 onward the document's only headings are literally `## `. The model
   is faithfully copying them, exactly as the prompt instructed.
4. `find_section_range("## ")` → `normalize_heading` → `""` → returns `None`. A
   nameless section can never be matched, by construction.
5. Every subsequent insert therefore appends yet another nameless heading.

There is no code path that appends a bullet under the last existing heading, and
none that seeds a default section into an empty document.

## 2.5 The document degenerates into an append-only list of bullets

Version history from a partial ladder run:

```
v2 (51 ch)  ⊂  v3 (618 ch)  ⊂  v5 (731 ch)  ⊂  v7 (932 ch)  ⊂  v9 (1361 ch)
```

Strictly append-only. Nothing is ever replaced, deleted, deduplicated, or
reorganised. Observed consequences:

- **Duplicates:** "slow down on Japanese / focus on Spanish" appears twice in
  near-identical wording; hard sci-fi appears twice; sourdough appears three
  times.
- **Stale facts survive supersession:** "trip to Mexico City in approximately 20
  days" persists alongside a newer "in approximately 3 weeks".
- **No structure:** headings never form, so the document cannot be read as a
  coherent profile of the user.

## 2.6 The "anchor erosion" framing was incorrect

`memory-spec.md §5.1` defines the artifact only as "an evolving markdown document
capturing consolidated knowledge about the user". It never specifies a shape, so
nothing prevented the drift to a bullet dump.

The document has therefore **never had structure**. Anchor erosion was a symptom
of a malformed document, not an independent defect. Converting consolidation
from a full rewrite into a delta protocol converted a silent full-document
rewrite into a safe append — which **masked** the malformation rather than
fixing it. The symptom changed shape; the cause was untouched.

The current document is a bare fact list, not the semantic memory of the user
that the feature is supposed to produce.

## 2.7 Loss-only assertions are structurally blind to this failure class

The new deterministic preservation check reports `unexplained_lost_lines`,
computed by diffing base-document lines against the accepted document. In this
failure mode it is **always empty**, because nothing is ever lost — only
accumulated. Every case passed the preservation check while the document was
visibly degrading.

This is the same class of false green the seam audit flagged: an assertion that
cannot fail for the defect it appears to cover. Detecting this class requires
*accumulation* assertions — nameless-heading count, duplicate-bullet count,
monotonic-growth detection — not loss detection.

The judge was intended as the second layer and was also insufficient: it returned
`PASS` on cases whose documents had nameless headings and heavy duplication,
because the prompt weighted anchor survival heavily and did not score structure
or duplication. It also returned `UNKNOWN` (no parseable `VERDICT` line) on 2 of
4 judged cases, so the semantic signal was both incomplete and unreliable.

One judge catch was independently **verified correct**: in the case-03 smoke it
reported a dropped fact ("roomate who enjoys the baked goods and wants to try
new recipes"), which is genuinely absent from the accepted document — confirmed
against `memory_facts` in the run database. The judge is capable of real
detection; the prompt scope was wrong.

## 2.8 `source_fact_ids` provides no hallucination guard

```rust
let filtered_fact_ids: Vec<String> = op.source_fact_ids.into_iter()
    .filter(|id| valid_fact_ids.contains(id))
    .collect();
```

This strips invented ids out of the *recorded list* only. It does not verify
that `proposed_text` derives from the cited facts, and it does not reject the
operation. An operation citing three hallucinated ids is staged and applied with
`source_fact_ids: []`. It is data hygiene, not a grounding guarantee.

## 2.9 Design error: operations are located by retyped prose

`apply_patch_operations` matches targets literally:

```rust
if line.contains(target) { ... }
```

Any `target_text` that is not character-identical to a document line is
**silently skipped** — no error, no counter. This is why the prompt had to
demand verbatim quoting: the design makes the LLM perform the string matching
that the engine should perform, and LLMs are unreliable at exact reproduction of
long strings. It is the direct cause of the malformed targets in §2.2 and the
skipped operations observed in the runs.

Two further incoherences in the same area:

- `section` and `target_text` are two overlapping ways to express "where", and
  the model half-fills both. `section` is the half that is broken.
- There is no operation type for renaming or moving a section, so a wrong heading
  can never be repaired by the protocol.

## 2.10 Latent data-loss hazard in the patch engine (proven by probe)

`normalize_for_match` collapses newlines to spaces, so matching ignores line
structure while application is strictly per-line. A multi-line `target_text` can
therefore match one unrelated line, which is then wholly replaced:

```
BASE:    - Languages: Spanish, Reading Preference: Hard sci-fi
         - Lives in Chicago
         - Owns a bicycle

target:  "- Languages: Spanish\n- Reading Preference: Hard sci-fi"   (2 lines)

AFTER:   - Languages: Spanish, Japanese      <- whole line replaced
         - Lives in Chicago
         - Owns a bicycle
```

Silent corruption: nothing warns, nothing is logged. This was reproduced with a
direct unit-test probe.

Blocking multi-line targets is **not** the fix — a legitimate paragraph replace
must keep working. The actual gap is that the engine has no multi-line-aware
matching, so it silently degrades a multi-line target into "replace some
arbitrary single line".

## 2.11 Compaction counts remain uncalibrated for this executor

Trigger correctness held for every case that ran, and the count assertion was
correctly demoted to report-only calibration data. The executor
(`qwen3.5:9b` at an 8192 context window) has no calibration entry yet, so counts
are reported as raw deltas against an empty baseline and never gate the run.

## 2.12 Open items not reached

- The 14-case matrix never completed. Best partial result: cases 01–05 passing
  mechanically, with case 02 no longer aborting.
- INVARIANT 5.3-B (re-anchor on accept) was exercised successfully in every case
  that staged ≥2 suggestions (`reanchor=Some(true)`), but never in a run that
  completed all 14 cases.
- The reject path was never probed in a live run.
- `settings.memory.suggestion_policy = "auto_apply"` is declared in
  `memory-spec.md §5.4` but has no implementation anywhere in `src/`.

---

# Summary of root causes

Ordered by leverage:

1. **No document structure contract.** The artifact shape is unspecified in the
   spec and unchecked in code, so the document is a bullet dump (§2.4, §2.5,
   §2.6). Everything else compounds this.
2. **Operations located by retyped prose.** Literal string matching against LLM
   output makes every operation fragile and silently skippable (§2.9, §2.10).
3. **Generation request misconfigured for the task class.** Consolidation
   inherited compaction's reasoning-off invariant, and a catalog defect silently
   downgraded schema enforcement (§2.2, §2.3).
4. **Failure handling is fatal-or-nothing.** No retry, no salvage, no
   accumulation-side assertions (§2.1, §2.7).

# Recommendation

Fix the structure contract first. A personal memory document should be a
coherent, sectioned profile of the user, not an append-only fact list, and the
code should assert that (reject nameless headings, detect duplicate bullets)
rather than trusting the model to maintain it.

Then relocate operation targeting from retyped prose to stable identity — the
`memory_facts` table already keys every fact by id, so an operation can name the
fact it revises rather than quoting a line back. That removes the entire class of
malformed-target, skipped-operation, and hallucinated-id failures at once, and
leaves the LLM doing only the two things it is reliable at: deciding which facts
changed, and writing the new wording.
