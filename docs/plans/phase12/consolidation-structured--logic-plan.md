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

---

# Part 3 — Third approach (content-element index addressing): implementation, evidence, and open defects

> **Status:** Investigation report from the test-engineering pass. Records what was built,
> what was measured, and what remains broken. Sprints 2 and 3 (14-case run, then judged run)
> have **not** been executed.
>
> **Run context:** executor `qwen3.5:9b` on the remote Ollama GPU server
> (`100.67.98.126:11434`), fresh eval database per run, 14-case shared-DB ladder.
> Sprint 1 ran a 3-case ladder prefix (cases 01–03).

## 3.1 What the third approach actually is

Full-document regeneration (§Part 1) dropped or rewrote prior anchors. The second
approach — a structured delta protocol addressed by `section` + retyped `target_text`
(§Part 2) — failed structurally: nameless `## ` headings minted by
`apply_insert_op`, an append-only bullet dump, and silently skipped operations.

The third approach replaces prose targeting with **deterministic positional addressing**.
`parse_content_elements` (`services/memory/personal.rs`) parses the active document into a
sequence of content elements (headings and bullets, blank lines stripped), numbered from 1.
The LLM receives the document as `[N] <text>` lines and returns
`{op, index, text}` with `op ∈ {insert_after, replace, delete}`. The engine applies operations
in descending index order so earlier positions stay stable, then re-renders the document.

Four passes exist: cold generation (Prompt 1, whole document), incremental fact integration
(Prompt 2, index-addressed edits staged for review), comment-directed edits (Prompt 3, same
engine), and regeneration (whole document).

**The core hypothesis was correct.** Across every cycle measured, no operation was ever
silently skipped for a malformed target, and no operation was ever lost to fuzzy string
matching. The §2.9 / §2.10 failure class is genuinely eliminated. What remains is a
different, narrower problem: the model's arithmetic over those indices.

## 3.2 Changes made in this pass

### Spec (`docs/specs/memory-spec.md`) — changed before code, per the non-drift hook

- **§5.1 Structure Contract**, promoted from unspecified shape to four enforced conditions:
  descriptive headings, unique headings, at least one heading, non-empty body. Rationale
  recorded inline, referencing §2.4–§2.6.
- **§5.3 step 3, Structure Gate** (new): acceptance validates the patched document against
  the §5.1 contract and must not commit a candidate that fails it.
- **§5.3 Op Semantics / Addressing Limits** (new): documented the descending-index
  application order, its consequences for opening a section, and the clamp/drop asymmetry.
- **§5.3 Generation Settings** (new): reasoning **disabled**, temperature **0.2**, output
  ceiling 4096, each with the measurement that justifies it.

### Production

| Change | Location | Rationale |
| --- | --- | --- |
| Accept path now calls `validate_document_structure` before commit; new `MemorySuggestionError::StructureGate` variant | `personal.rs`, mapped in `ipc/memory.rs` | §2.4 root cause: a patch could mint a nameless or duplicate heading, and the malformation cannot self-repair |
| Consolidation reasoning set to `Disabled` | `personal.rs::execute_personal_llm_pass` | Measured: reasoning ON never emits an answer (§3.4) |
| Consolidation temperature `0.2` via new `PERSONAL_CONSOLIDATION_TEMPERATURE` | `personal.rs` | Kept separate from the compaction temperature so compaction calibration is untouched |
| Incremental prompt: valid index range stated, element count supplied in the user message, single-line `text` rule, N+1 rule for opening a section, heading-aware `replace` rules, kind-check instruction | `personal.rs` | Targets the observed off-by-one and out-of-range classes |
| Comment prompt: index range supplied | `personal.rs` | Same addressing model |

**The reasoning change reverses a Part 2 conclusion, and that reversal is the single most
important finding in this pass.** §2.2 established that consolidation must run with
reasoning ON, because under the *prose-targeting* protocol the model degenerated into
whole-document echo operations without reasoning first. The indexed protocol removes that
failure mode at its root: an operation carries only a short `text` and never restates the
document, so there is nothing to echo. The §2.2 constraint did not survive the migration.

### Evaluation harness

The harness could not previously fail for the defect class that killed the first two
approaches. `evals/common/structure.rs` was added, and `memory_pipeline_eval.rs` rewired:

- **Accumulation-side structure measurement** — nameless headings, duplicate headings,
  duplicate bullets, bullets above the first heading, bullets-per-section distribution.
  §2.7 established that the existing preservation check is loss-only and therefore
  structurally blind to an append-only degeneration; these are its counterpart.
- **Real-engine accounting** — the previous `engine_would_match` was a stub that only tested
  for non-empty content and could not fail for an out-of-range index. Replaced with bounds
  accounting derived from the engine's real guards.
- **Section-membership verification** — detects an inserted bullet landing under a section
  other than the one its anchor index belonged to.
- **Re-anchor arithmetic verification** — recomputes INVARIANT 5.3-B independently from the
  suggestion rows instead of inferring it from a version bump.
- **Consolidation-path discriminator** — the cold-start and incremental paths carry different
  invariants; conflating them produced two false reds (§3.5).
- **Section-count preservation** — catches a `replace` aimed at a heading index.
- **Fact-coverage classification** — separates wholesale-absence from partial representation.
- **`--max-cases N`** — runs a ladder prefix, preserving the cumulative document evolution the
  incremental path depends on. `--case N` runs a case in isolation and reaches only cold start.
- **Stale judge prompt rewritten** — `judge_pipeline_anchor_survival.md` still described the
  removed `section`/`target_text` contract, and its Quality section scored nothing about
  structure or duplication (§2.7).

## 3.3 Result: the structural thesis is confirmed

Across cold start and every incremental cycle measured, the §5.1 contract held:

- 4–5 uniquely-titled `##` headings per document
- **0** nameless headings (the §2.4 signature)
- **0** duplicate headings
- **0** bullets above the first heading
- Real accumulation with genuine supersession: an incremental cycle `replace`d a sourdough
  bullet to fold in a roommate's enjoyment of the rolls, rather than appending a near-duplicate
  bullet as §2.5 describes

Two consecutive 3-case runs reproduced
`['passed', 'passed', 'failed_invariant']`, and a clean run reproduced the same structure.
The append-only bullet dump and nameless-heading cascade of Part 2 did not recur. **This is
the first approach whose document shape is correct.**

## 3.4 Experiment: why reasoning had to be turned off

Direct measurement against the server, strict JSON schema, consolidation task shape:

| Config | `done_reason` | eval tokens | thinking chars | content chars |
| --- | --- | --- | --- | --- |
| `num_predict=4096`, reasoning on | `length` | 3843 | 17,874 | **0** |
| `num_predict=1024`, reasoning on | `length` | 1024 | 4,314 | **0** |
| `num_predict=512`, reasoning on | `length` | 512 | 2,070 | **0** |
| `num_predict=4096`, `think:false` | `stop` | 55 | 0 | 163 (valid JSON) |

With reasoning on, the model consumes the entire output budget and **never emits an answer**,
at every ceiling tried. `OllamaChunkMessage` (`llm/transport/ollama.rs`) has no `thinking`
field, so the answer tokens never reach `execute_personal_llm_pass`, which raises
`LLM generated empty personal memory document` and aborts the entire cycle. This is §2.1's
second fatal path, still unfixed at the start of this pass, and enabling reasoning in Part 2
made it *more* likely.

A follow-up sweep over 6 document/fact combinations with `think:false` produced 6/6 valid
minimal JSON, ~1s each, **0 out-of-range indices and 0 multi-line `text`**. The out-of-range
and multi-line operations seen with reasoning on were symptoms of the runaway trace, not an
independent engine defect.

Side effect: disabling reasoning cut per-case wall time from ~17s to ~7s.

## 3.5 Two false reds in the new harness (test defects, not production)

Both were assertions written for the incremental path and scored against cold start:

1. **Engine replay.** Compared the replay of zero operations on an empty base against the
   cold-generated document. Added a `replay_applicable` flag so the comparison is reported as
   not applicable rather than scored as a mismatch.
2. **Staging immutability.** Prompt 1 commits the synthesized document directly by design
   (§5.3), so requiring the document to be untouched asserted the opposite of the spec. Gated
   on the path.

Both were the same class of error: a path-specific invariant applied to a path that does not
have it. A third instance was a plain logic inversion in the cold-start branch of that gate.

## 3.6 Scope limit recorded against the harness itself

`engine_replay.matches_committed_document` **cannot** detect a defect inside
`apply_patch_operations`, because the replay calls the same function production used to
commit — both sides move together. It is a divergence and round-trip check (storage
corruption, non-canonical write, post-commit mutation), not an engine-correctness check.
Engine correctness rests on the structure, membership, and drop-count assertions. This is
documented in the report schema so the field cannot be over-read as stronger evidence than
it is.

## 3.7 Mutation evidence

Per the `mutate` discipline, the re-anchor assertion was proven non-vacuous by seeding a
defect into INVARIANT 5.3-B:

**Mutant M-C** — `personal_memory.rs` re-anchor shift `target_index + 1` → `+ 2`.

- **Assert RED:** both incremental cases flipped to `failed_invariant`;
  `reanchor=Some(false)`, `shifted=2` and `shifted=3` (over-shifted), `unexplained_lost=1`
  (a genuinely lost anchor), and a section-membership violation.
- **Revert and assert GREEN:** clean `git diff`, full re-run green.

This is a substantive result beyond the assertion itself: the re-anchor arithmetic does not
merely relocate metadata. Because the shifted indices are then used to apply the remaining
pending operations, an off-by-one shift **corrupts the document**. INVARIANT 5.3-B is
load-bearing in a way its description does not convey.

Mutant score for this seam: 1 attempted, 1 killed, 0 survivors.

## 3.8 Open defects

### D1 — The model does not reliably map intent to element index (primary open issue)

Two distinct symptoms, same root cause:

**Out-of-range targets.** The model emitted `insert_after` at indices 8 and 9 against a
7-element document, and 12 against an 11-element document. The engine clamps these to an
append, so the content is applied but filed under the last section rather than the intended
one. Observed outcome was benign — "append a new section at the end" expressed imprecisely,
landing correctly. **This is imprecision, not a lost edit**, and the harness now reports it
separately from genuine drops.

**Off-by-one onto a heading — this one destroys content.** Against a 13-element document:

```
[9]  ## Technical Projects
[10] - Working on a Rust project involving ownership and memory management logic.
```

The model emitted `replace index=9` with bullet text plus `delete index=10`. It meant to
update the bullet at 10, hit the heading at 9 instead, and **erased the section**. The
remaining bullet was re-parented under `## Culinary Preferences & Habits`, so sourdough and
Rust content ended up under a languages-adjacent heading.

The §5.1 structure gate did not catch this: the document still had uniquely-titled headings
and no nameless ones. **The damage is subtractive, and the contract only forbids malformed
output, not missing sections.** A heading-count check was added to the harness
(`heading_count_valid`, base 5 headings vs accepted 4) and reproduces across runs.

Mitigation applied: prompt rules stating that a `replace` on a heading must supply a heading,
that a bullet must be addressed by its own index, and that the model should verify the
*kind* of element at each index it uses. Prompt rules reduce but cannot eliminate the class.

### D2 — Candidate facts are retired without being written down

INVARIANT 5.3-A transitions *every* candidate fact to `consolidated` when suggestions are
staged, whether or not the model proposed an edit for it. A fact the model declines to write
down is marked consolidated and never reappears. Observed losses: "lives in Chicago" (twice),
"is male", "plans to visit the Art Institute of Chicago".

`candidate_partition_valid` only asserts that no fact is left `active`, so it cannot see
this. A fact-coverage classifier was added and separates wholesale absence (zero shared
content words — unambiguous) from partial coverage (usually faithful paraphrase; gating on it
produced false positives, since "plans to make rolls instead of loaves" scored 0.55 against a
document line reading "Planning to switch from loaves to rolls").

Whether retirement-without-integration is correct is a **product decision, not a defect** — the
document is a lossy compression of a large fact stream, and judging whether the model fitted
the facts adequately is the judge's job. Currently gated; see §3.9.

### D3 — Near-duplicate bullets accumulate

Exact-match duplicate detection is clean, but semantically redundant bullets accumulate:
"Currently reading 'The Three-Body Problem'" alongside "Currently reading 'The Three-Body
Problem' & enjoying the detailed world-building of 'Children of Memory'". This is §2.5's
duplication failure in a subtler form that exact matching cannot see. The judge layer is the
intended detector.

## 3.9 Open questions for architecture review

1. **Should headings remain addressable, or should the section set be frozen at cold start?**
   A proposal was made to define sections once during cold generation and thereafter address
   only the bullet lines beneath them, removing headings from the index space entirely. This
   would make section erasure structurally impossible and remove the N+1 rule for opening a
   section. Its cost is that the document can never gain a new category, so facts with no
   existing home must be forced into a wrong section or dropped. See §3.10.

2. **Is retirement-without-integration (D2) correct behaviour or a bookkeeping gap?** If the
   document is a lossy compression, 5.3-A is right and the judge should score coverage. If
   every fact is meant to survive, 5.3-A needs amending so only covered facts retire.

3. **Should an out-of-range `replace` on a heading be refused by the engine**, or merely
   surfaced to the user on the staging slate? This is the only mechanical guarantee against
   D1's destructive case; prompt rules are probabilistic.

4. **Should positional addressing be replaced by section-scoped addressing** — e.g.
   `{section: "Technical Projects", after: 2}` meaning "after the 2nd bullet in that section",
   with the engine resolving the global index? This preserves deterministic resolution while
   removing the global element-counting burden that D1 shows to be the model's weak link.

## 3.10 Assessment: the frozen-section proposal

Recorded here because it was raised during this pass and the reasoning matters for the
decision.

The proposal addresses a real and correctly-diagnosed problem: the model cannot count
elements reliably, and every destructive symptom in D1 comes from that. Removing headings from
the index space eliminates the heading case outright.

Two costs argue against it as stated:

- **No new sections, ever.** The measured documents have sections for hobbies, languages,
  culinary, technical, and productivity — and no section for location or basic demographics.
  The facts that went missing in D2 ("lives in Chicago", "is male") are precisely the facts
  with no home in that taxonomy. Freezing the taxonomy forces such facts into wrong buckets or
  drops them, which is the same semantic-misfiling failure D1 already exhibits, made
  systematic and permanent.
- **The document cannot evolve into a profile of the user**, which is the stated purpose of
  the artifact in §5.1. A frozen taxonomy is a schema, not a memory.

A cheaper change targets the same root cause without either cost: **label the kind of each
element in the indexed view** — `[9] (heading) ## Technical Projects` versus
`[10] (bullet) - Working on a Rust project...`. The model did not misuse `replace` because
headings were addressable; it misused it because it was never told what sat at each index and
had to infer element kind from a `#` prefix. Making the kind explicit removes the inference
without changing the addressing model, and it composes with option 4 in §3.9 if section-scoped
addressing is adopted later.

Recommendation: do not freeze sections. Label element kinds, enforce heading→heading on
`replace`, and evaluate section-scoped addressing as the structural fix for the counting
burden.

---

# Part 4 — System-architect review loop, structural decoupling, and the Sprint 2 stall

> **Status:** Investigation report. Covers the architect review loop that ran between Part 3 and
> Sprint 2, the module decoupling performed in parallel, and the blocking defect Sprint 2 exposed.
>
> **Sprint 1 verdict:** PASS (3-case ladder prefix, no judge).
> **Sprint 2 verdict:** FAIL (14-case ladder, no judge) — 10 of 14 cases refused by a production
> gate, with compounding suggestion accumulation. Root cause is a design flaw in the gate added
> in response to the architect review. **Awaiting an architecture decision; see §4.7.**
>
> **Sprint 3 (judged run):** not executed.

## 4.1 The architect review loop

Part 3 ended with an open question about heading addressability. That was escalated to a system
architect agent, who independently read the current `personal.rs` and produced a fix report. Its
triangulation agreed with Part 3's diagnosis: the residual defect was **not** the model, but
what the model is *given* — prompt/context representation — plus one architectural gap (no
mechanical backstop).

The architect confirmed four defects and proposed six fixes. How each resolved:

| Architect item | Disposition |
| --- | --- |
| §2.1 element kind never shown to the model | **Accepted — implemented.** Root cause of D1. |
| §2.2 `ElementKind` discarded before apply | Deferred; superseded by §2.3's cheaper gate. |
| §2.3 production structure gate weaker than the harness | **Accepted — implemented.** The harness already had `heading_count_valid`; it was ported into the commit path. |
| §2.4 reasoning-ON untested on the new protocol | Partially corrected. §3.4 *did* test the new protocol generally; the genuinely untested slice is the section-opening path. |
| §3.1 self-declared `target_kind` field | **Rejected.** The engine holds ground truth at parse time; the declaration is redundant with a self-check the engine performs for free. It also does not cover the *more frequent* symptom (out-of-range indices have no element to compare against), and a new required field on a 9B model under `strict: true` adds retry risk. Diagnostics value is real but belongs later. |
| §3.3 return an `ApplyReport` instead of `log::warn!` | **Rejected at the time — this was a mistake.** See §4.7: per-op granularity turns out to be load-bearing for correctness, not merely observability. |
| §3.4 heading-count gate in production | **Accepted — implemented.** Strictly better than the narrower "refuse heading→bullet replace" that Part 3 proposed, because it catches any section loss while still permitting legitimate renames. |
| §3.5 refined prompt | **Accepted in tightened form.** Reduced to roughly half the architect's draft, one embedded example instead of two, and an explicit "leave it out of this cycle" clause encoding the product stance that the user, not the model, is the final authority. |
| §3.6 reasoning ON vs OFF on section-opening cases | Not run. Deferred behind the cheaper fixes. |
| §5.1 dedicated `insert_section` op | **Deferred**, per the architect's own gating. See §4.6. |

Two points where the review changed my position and are worth recording:

1. **The architect's §3.3 was load-bearing and I initially misjudged it.** I read the returned
   rejected-op list as "just observability." It is in fact the mechanism that lets a bad
   operation be dropped while the rest of the batch is applied — which is precisely what the
   stall in §4.6 requires. My rejection was wrong.
2. **The architect conceded §2.4 was overstated** and narrowed the untested gap to the
   section-opening path specifically. The correction was accepted in both directions.

The loop also surfaced a product decision rather than a defect: whether retiring candidate facts
without writing them down (D2) is correct. The user settled it — the document is a lossy
compression and the LLM's job is to fit the facts, so 5.3-A is correct as specified and the
eval's `absent_candidate_facts` is judge input rather than a gate.

## 4.2 Structural decoupling (performed in parallel)

`personal.rs` had reached ~1290 lines carrying four system prompts, the document model, the
patch engine, generation plumbing, consolidation control flow, suggestion resolution, and 12
unit tests. It was decoupled into `services/memory/personal/`:

```
mod.rs           29   thin facade (pub use + persistence re-exports)
prompts.rs      135   4 system prompts, consolidation_json_schema, 2 numeric constants
document.rs     131   ElementKind, ContentElement, parse/format/render/clean/validate
patch.rs        112   MemoryPatchOperation, PersonalConsolidationOutput, apply_patch_operations
generation.rs   201   execute_personal_llm_pass, consolidation_output_constraint
consolidate.rs  439   ConsolidationConflictPolicy + FromStr, consolidation/regeneration, quiescence
suggestions.rs  136   MemorySuggestionError, batch_resolve, resolve
tests.rs        172   12 unit tests
```

Pure code motion, zero behaviour change. Verified by `cargo clippy --release --all-targets`
(clean), which transitively proves all eight external consumers of `memory::personal::*` still
resolve unmodified, plus a byte-level reassembly diff showing the only deltas are 8 `pub(super)`
visibility prefixes and the test-module dedent. Four unit tests were added afterwards covering
kind labelling, heading-loss acceptance/rejection, and the chained section anchor.

A latent defect was fixed during the move: `GenerationPolicy::from_settings` was seeded with a
bare `4096` literal while `CONSOLIDATION_MAX_OUTPUT_TOKENS` was also 4096, so editing the
constant would have silently stopped affecting the policy. The literal now reads the constant.

Two further observations were surfaced and deliberately not fixed, since they are design
questions rather than defects:

- The incremental branch of `consolidate_personal_memory` and `regenerate_with_comments` are
  ~35 lines of near-identical staging logic. Now adjacent in one file, so the duplication is
  visible and mechanical to extract.
- `MemorySuggestionError::Database(#[from] anyhow::Error)` flattens the persistence error
  taxonomy, partly defeating that enum's stated purpose.

## 4.3 Implemented after the review

- **Kind labels.** `format_indexed_document` now emits `[N] (heading|bullet) text`. The model no
  longer infers element kind from a `#` prefix.
- **Production heading-loss gate.** `validate_no_heading_loss` refuses any commit that would
  reduce the section count, wired into `batch_resolve_memory_suggestions` behind the existing
  §5.1 structure gate. Counting rather than title comparison is deliberate: a rename legitimately
  changes the title while preserving the count.
- **Prompt rewrite** (tightened twice), including the section-opening recipe, kind-matching rule,
  explicit when *not* to file a fact under an existing heading, and permission to omit a fact
  rather than misplace it.
- **Harness fixes:** chained N/N+1 section anchors are no longer counted as index imprecision
  (the prompt's own documented pattern produces an out-of-range anchor at the document end, which
  the engine clamps correctly); fact-coverage classification separates wholesale absence from
  partial coverage; structure-gate refusals are recorded rather than aborting the ladder.

## 4.4 Sprint 1 result (3-case prefix, no judge)

All three cases passed. The §5.1 contract held across cold start and two incremental cycles:
5 uniquely-titled sections, 0 nameless headings, 0 duplicate headings, 0 duplicate bullets,
0 bullets above the first heading, section count preserved. `unexplained_lost_lines` was 0 in
every case and the re-anchor arithmetic probe was non-vacuous.

The destructive D1 symptom did not recur at 3-case scale: the `replace` that had targeted a
heading in Part 3 now targeted a bullet, confirming the kind labels took effect.

**Two defects survived, and both were judged acceptable under the product stance:**

- **Semantic misfiling.** "Lives in Chicago" was written under `## Culinary Interests & Habits`
  rather than opening a `## Location` section, despite the prompt explicitly instructing
  otherwise. No mechanical assertion can detect this class — the index was in bounds and the
  engine did exactly what it was asked. It is visible to the user on the staging slate *before*
  acceptance, which is the intended mitigation.
- **D2 retirement without integration.** Confirmed live and accepted as designed.

**Methodological note.** Sprint 1 was declared PASS on 3 cases. That was too generous: the
destructive patch that the gate exists to catch did not occur in 3 cycles and occurred in 10 of 14.
Three cases could not have surfaced it. Scale, not green, is what establishes the failure rate.

## 4.5 Sprint 2 result (14-case ladder, no judge) — FAIL

267s, 5200 turns, 14 sessions. Cases 01–05 passed. **Cases 06–14 were refused by the new
heading-loss gate**, and the refusals compound:

| Case | Path | Staged | Blocked | Headings | Bullets | Absent facts |
| --- | --- | --- | --- | --- | --- | --- |
| 01 | cold_start | 0 | 0 | 5 | 8 | 0 |
| 02 | incremental | 2 | 0 | 5 | 9 | 0 |
| 03 | incremental | 3 | 0 | 5 | 11 | 0 |
| 04 | no_candidates | — | — | — | — | — |
| 05 | incremental | 4 | 0 | 5 | 9 | 2 |
| 06 | incremental | 2 | **1** | 5 | 10 | 2 |
| 07 | incremental | 2 | **2** | 5 | 11 | 6 |
| 08 | incremental | 3 | **4** | 5 | 10 | 4 |
| 09 | incremental | 2 | **5** | 5 | 10 | 5 |
| 10 | incremental | 3 | **7** | 5 | 11 | 7 |
| 11 | incremental | 3 | **9** | 5 | 10 | 5 |
| 12 | incremental | 2 | **10** | 5 | 10 | 7 |
| 13 | incremental | 2 | **11** | 5 | 10 | 8 |
| 14 | incremental | 7 | **17** | 5 | 9 | 12 |

Three things are true simultaneously, and together they are worse than the Part 2 failure:

1. **The gate works.** It caught 10 section-destroying patches that three cases never surfaced,
   and committed nothing. Kind labels plus a mechanical backstop did eliminate the destructive
   class — it is now *detected and blocked* rather than silently corrupting the document.

2. **The document stopped learning.** Headings never move off 5. Bullets oscillate 8→9→11→9→10
   and finish at **9**, lower than case 3. Across 13 incremental cycles and 5200 turns the
   document absorbed essentially nothing. This is a *new* failure mode: not the Part 2 append-only
   dump, but a **frozen** document.

3. **A positive feedback loop.** Refusing the whole batch means every suggestion in it stays
   `pending`. The next cycle stages more suggestions on top, the batch accept now spans the whole
   pile, the pile is more likely to contain a section-destroying operation, and the gate fires
   again. `blocked_ops` climbs 1→2→4→5→7→9→10→11→17 with no equilibrium. This is a permanent
   stall, not a transient one.

The gate is correct in isolation and catastrophic in aggregate. That is a design flaw, not an
implementation bug: **the refusal granularity is the entire-batch, and it must be per-operation.**

Secondary quality regression, visible in the final document and independent of the stall: the
model files unrelated facts under whatever heading is adjacent ("Lives in Logan Square, Chicago"
under `## Culinary Preferences & Plans`), emits single bullets containing long parenthetical
reasoning traces, and produces near-duplicate bullets ("Has successfully mastered the ownership
component of this Rust project" twice, once standalone and once merged into another bullet).

## 4.6 Why per-operation granularity is the fix

The architect's §3.3 proposed returning an `ApplyReport` carrying the document plus the rejected
operations. Part 4.1 records that this was rejected at the time as observability-only. That
rejection was wrong: the rejected list is exactly what lets the engine **drop the one offending
operation and apply the rest of the batch**, which is the difference between a gate and a stall.

A dedicated `insert_section` op (§5.1 of the architect report) remains deferred per its own
gating advice. If per-operation granularity is adopted, the case for it weakens further: with
the offending op dropped rather than the batch refused, the remaining N/N+1 pattern is a
correctness question rather than a liveness one.

## 4.7 Harness defect found alongside the production defect

`reanchor_valid` reports `false` from case 07 onward. Part of that is a harness bug introduced
when the probe was made non-vacuous: `remaining_before` samples only the current cycle's staged
suggestions, while `remaining_after` comes from `fetch_pending_suggestions`, which returns
suggestions accumulated across *all* cycles. Once a pile exists the two sets stop matching, so
the arithmetic comparison reports spurious shifts. This is test-code only and must be corrected
by keying both samples on the same suggestion-id set. The underlying INVARIANT 5.3-B arithmetic
remains mutation-proven (§3.7).

## 4.8 Open question requiring an architecture decision

**How should a batch containing one unsafe operation be handled?** The current whole-batch refusal
guarantees no section loss and also guarantees the document never grows. Candidate designs:

1. **Per-operation drop with reporting** (architect §3.3). The engine applies the safe operations
   and returns the rejected one with a reason. The accept path commits the good subset and marks
   the rejected suggestion resolved-with-reason so it cannot re-enter the pile. Preserves both
   liveness and the section guarantee, and produces the audit trail §4.6 needs. Cost: changes
   `apply_patch_operations`' return type, which the eval consumes.
2. **Per-operation validation at staging.** Run the heading-loss check when suggestions are
   *staged*, dropping the offending operation before it is ever persisted, so the pile can never
   contain one. Fails fast and keeps the commit path simple. Caveat: INVARIANT 5.3-A marks all
   candidate facts `consolidated` at staging, so a fact whose operation was dropped at staging
   would be retired without being written down. Amending 5.3-A is a product decision, already
   settled once in §4.1 and not to be reopened silently.
3. **Revert the gate to eval-only detection.** Restores integration and leaves section destruction
   to be caught by the judge. Undoes the main win of this pass.
4. **Escalate to per-section anchoring** (§3.9 option 4 of the Part 3 report, and the option this
   report still considers strongest long-term): address operations by section title plus ordinal
   rather than global element index, removing the counting burden that produces the malformed
   operations in the first place. Largest change; addresses cause instead of symptom.

The stall must be resolved before Sprint 3, since a judged run over a frozen document measures
nothing about consolidation quality.
