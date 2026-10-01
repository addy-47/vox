# Phase 12.7 — Memory Pipeline Prompt Hardening & Eval Harness Repair

> **Author role:** QA/Test Auditor (`.agents/rules/qa-engineer.md`) — this document is a plan, not an implementation. No production code is authored here.
>
> **Trigger:** Independent audit of eval run [`20261001_064506_59adeb37`](file:///home/addy/projects/apps/vox/app/src-tauri/evals/results/memory_eval/20261001_064506_59adeb37/master_synthesis_report.md) — 90 judge claims audited, 25 verified true, **7 verified false**.
>
> **Scope:** Consolidation prompts, LLM wire op vocabulary, three production bug fixes, and the eval harness that failed to catch any of them.
>
> **Governance:** Per `AGENTS.md` §4.3, [`memory-spec.md`](file:///home/addy/projects/apps/vox/docs/specs/memory-spec.md) currently *mandates* the behaviour this plan changes (op names, `INVARIANT 5.3-C`, `INVARIANT 5.3-A`, `auto_apply` semantics). **Spec artifacts are updated FIRST, before any code.** Per `AGENTS.md` §4.2, `.agents/rules/backend-style-guide.md` must be read before modifying Rust.

---

## 1. Framing — What Counts as a Failure Here

The pipeline model under test is **`qwen3.5:9b`**, the floor of the supported range. **97–98% accuracy is the success bar, not 100%.** Chasing exact coverage produces prompt bloat that degrades a small model's instruction-following.

This plan is therefore gated on **clear red flags only**:

| Red flag | Definition | Why it is disqualifying |
|---|---|---|
| **RF-1 Hallucination** | A stored attribute the user never stated | Becomes permanent false fact about a real person |
| **RF-2 Unjustified deletion** | Memory removed with no contradicting observation | Silent, irreversible loss of user knowledge |
| **RF-3 Omission** | Observation silently dropped | Fact the user gave never stored |
| **RF-4 Fabricated attribute** | Inference from context (a city becomes a home) | Same harm as RF-1, subtler |

Everything else — block-count drift, section-title wobble, prose density — is **cosmetic and explicitly out of gate scope.**

### 1.1 The measurement paradox this plan resolves

The audit found the eval harness **scored 100% on both consolidation cases while the memory was losing data underneath it.** Three independent causes, all in the harness:

1. **Judge scorecards had no independent denominator.** Case_02 and case_03 reported `Observation Coverage: 7/7 = 100%` and `6/6 = 100%` — while their own §3 sections listed the dropped observation. Two of seven refutations are exactly this.
2. **Judge prompts never asked about `deletes`.** `consolidation_eval.rs:203` asks about "rewritten/deleted" only in prose. The case_02 judge never used the word *delete* while two blocks were destroyed.
3. **Judge requests were never persisted.** `llm_client.rs:247` returns only the report body. **6 of 9 reports had no recoverable input** — every audit verification required template reconstruction.

**Conclusion: the prompts cannot be evaluated until the instrument can measure them.** Batches 1–2 fix the instrument; Batch 3–4 fix the prompts; Batch 5 re-measures.

---

## 2. Corrections to the Original Audit

Recorded so they are not reintroduced. Both were errors in [`master_synthesis_report.md`](file:///home/addy/projects/apps/vox/app/src-tauri/evals/results/memory_eval/20261001_064506_59adeb37/master_synthesis_report.md).

| Original claim | Status | Correction |
|---|---|---|
| **F3** — delete revisions show "only an opaque ID" to the user | ❌ **RETRACTED** | **Already implemented.** `MemoryRevisionView.old_text` ([`revisions.rs:30`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/memory/personal/revisions.rs#L30)) resolves prior block text; `render_revision_preview` ([`:282-290`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/memory/personal/revisions.rs#L282)) renders `"Delete block: <text>"`; frontend keys off it ([`reviewModel.ts:57,191`](file:///home/addy/projects/apps/vox/app/src/shared/components/memory/staging/reviewModel.ts#L57)). The audit read `stage_revisions` and stopped before reading the display path. **No change needed.** |
| **F2** — refuse deletes that would empty a section | ❌ **INVERTED** | **Auto-prune on last-block delete is correct** and [`memory-spec.md §5.1:202`](file:///home/addy/projects/apps/vox/docs/specs/memory-spec.md#L202) mandates it. The real defect is the **unused** `is_non_destructive()` ([`operations.rs:82-87`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/memory/personal/operations.rs#L82)) — zero call sites outside its own tests, doc comment promising an `auto_apply` guarantee the code never provides. **F2 is redefined in §5.2.** |

---

## 3. Resolved Decisions

| # | Question | Resolution | Source |
|---|---|---|---|
| D1 | Op vocabulary | `new_sections`→`new`, `creates`→`add`, `updates`→`update`, `deletes`→`delete`. **LLM wire only.** Persisted `op` strings (`create_block` etc.) and `ResolvedOp` variants unchanged. | User directive |
| D2 | Umbrella sections | Principle + examples in the worked example. **No hardcoded vocabulary, no closed list.** | User directive: *"dont define or hardcode umberallas, just mention exmaples"* |
| D3 | `dispositions` array | **Rejected.** | *"dispositions ... makes no sense i cant see any poitn in it"* |
| D4 | `reason` + `observation` on `delete` | **Rejected.** | *"adding reason is lso pointless"* |
| D5 | `auto_apply` semantics | Applies everything, simply. Deletions are the one exception (see F2). | *"auto apply means auto apply simply"* |
| D6 | `is_non_destructive()` | **remove this entirely**  |
| D7 | Compaction prompt | **Out of scope** for this plan. Deferred — see §9. | User scope |
| D8 | Quality bar | 97–98% is good on the floor model. Gate on RF-1..RF-4 only. | *"97-98 is good"* |
| D9 | F1 coverage floor | Calibrate from measured distribution before shipping. See §5.1. | QA recommendation |

---

## 4. Spec Updates (Batch 1 — MUST land first)

> [!IMPORTANT]
> `AGENTS.md` §4.3 **Mandatory Spec Alignment Hook**: specifications must never quietly drift from code. These edits precede all code.

### 4.1 `docs/specs/memory-spec.md`

| Location | Current | New |
|---|---|---|
| §4 schema, `:247-250` | `"new_sections"`, `"creates"`, `"updates"`, `"deletes"` | `"new"`, `"add"`, `"update"`, `"delete"` |
| §4 bullet list, `:253-256` | per-key descriptions | Renamed to match; `new` description notes it is **rare** — prefer `add` |
| §5.3.1 `INVARIANT 5.3-C`, `:226` | *"On commit, precisely that snapshot transitions `active → integrated`"* | **Only observations demonstrably represented in the resulting memory transition.** Unrepresented observations remain `active` and are re-presented next pass |
| §5.3-A, `:294` | *"**All** candidate personal observations … transition … immediately upon staging"* | Aligned to 5.3-C. Add: *"A candidate with no representing operation stays `active`; it is never marked integrated on the strength of an unrelated operation landing."* |
| §5.4 `auto_apply`, `:322` | *"**All** operations (`create_block`, `create_section`, `update_block`, `delete_block`) … automatically commit"* | *"Additive operations (`create_block`, `create_section`, `update_block`) automatically commit. `delete_block` is destructive and is **staged as a pending revision** for user review, per `ResolvedOp::is_non_destructive()`."* |
| §5.5 prompts, `:271` | *"Propose the minimal set of semantic operations"* | Removed — this phrasing is the proximate cause of RF-2 |
| §5.5 prompts, `:271-272`, `:279`, `:287` | old op names | New names |
| §5.5 `:265` | "No artificial delta arrays (`creates`, `updates`, `deletes`)" | Renamed |

### 4.2 `docs/specs/ipc-spec.md`

| Location | Change |
|---|---|
| `:112` | `new_sections`-only grouped schema for regeneration → `new`. (Regeneration still uses `run_whole_memory_pass`; only the key name moves.) |
| `:124` | `MemoryRevisionView` — **unchanged** (already carries `old_text`). Correct the `op` enumeration only if §4.1 touches it. |

### 4.3 `docs/specs/db-spec.md`

**No change.** `:179` op strings stay `create_section|create_block|update_block|delete_block` — persisted data, untouched by the wire rename.

---

## 5. Production Bug Fixes

### 5.1 F1 — Unlanded observations marked `integrated` , keep as it as for now 

**Location:** [`consolidate.rs:130-142`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/memory/personal/consolidate.rs#L130)

**Current behaviour:**
```rust
if !pass_result.landed { /* bail */ }
let ids: Vec<String> = candidates.iter().map(|o| o.id.clone()).collect();
mark_observations_integrated(request.conn, &ids).await?;
```
If **any** operation lands, **every** candidate is marked `integrated`. There is no reconciliation between observations supplied and operations returned.

**Empirical proof** (run `20261001_064506_59adeb37`):

| Observation | Trace evidence | v4 memory | DB status | Recoverable by incremental pass? |
|---|---|---|---|---|
| `fact_…770_5edd` *"sets a reminder for Japanese at 9 PM daily"* | `"deletes":[{"block":"b1"},{"block":"b2"}]` | absent | `integrated` | **No** |
| `fact_…871_5b3e` *"User studies Spanish and Japanese languages."* | no referencing op | absent | `integrated` | **No** |
| `fact_…559_7f02` *"User has a roommate who wants to try new recipes together."* | in `<new_observations>`, no op | absent | `integrated` | **No** |

Verified: `instr(content,'9 PM')` → `1` at v2, `0` at v3, `0` at v4. `instr(content,'roommate')` → `0` at v1–v4.

**Fix:** after the pass lands, test each candidate observation for representation in the resulting memory. Only represented ones transition. The rest return to `active` and retry.

**Representation test — word coverage, not cosine.** Deliberately chosen over embedding similarity:

| Criterion | Word coverage | Cosine |
|---|---|---|
| Threshold cliff | None — monotone fraction | Yes |
| Embedder dependency | None | Requires warm ONNX model in the commit path |
| Interpretability | "7 of 9 content words present" | Opaque float |
| Calibrated distribution | **⚠️ Missing — see below** | Available (measured) |

**Blocker — the floor is uncalibrated.** Known cosine anchors from the run: paraphrase duplicates **0.9300**, related-but-distinct **0.8078–0.8387**. No word-coverage distribution exists. Per **D9**, calibration must precede implementation:

1. Extract all 19 `personal` observations from the audited DB.
2. For each, compute coverage against its case's final memory (represented) and against the other cases' memories (unrepresented).
3. Inspect the distribution; choose the floor at the widest separation.
4. If no separation exists, word coverage is the wrong instrument — escalate, do not guess.

**Mandatory:** `log::warn!` prints each observation's coverage score so the first eval run produces a fresh distribution.

### 5.2 F2 — `is_non_destructive()` rmeoved  

**Location:** [`operations.rs:82-87`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/memory/personal/operations.rs#L82)

```rust
/// True when the operation only adds or rewrites content and therefore cannot lose user data.
/// Used by the `auto_apply` revision policy, which commits these directly and holds deletions.
pub fn is_non_destructive(&self) -> bool {
    !matches!(self, Self::DeleteBlock { .. })
}
```

`grep` across `src/`, `evals/`, `tests/`, `benches/` finds **zero call sites outside its own unit tests.** `auto_apply_operations` ([`consolidate.rs:401-424`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/memory/personal/consolidate.rs#L401)) passes every resolved op straight into `apply_operations`.

**Why it matters.** Production default is `manual_review` ([`defaults.rs:55`](file:///home/addy/projects/apps/vox/app/src-tauri/src/core/defaults.rs#L55)), so deletes normally stage for review. The eval **forced** `auto_apply` ([`consolidation_eval.rs:73-76`](file:///home/addy/projects/apps/vox/app/src-tauri/evals/common/consolidation_eval.rs#L73)), and per user confirmation that forcing is intentional. On the `auto_apply` path — user-selectable in settings — deletes commit silently and irreversibly.

**Fix:** partition the op batch on `is_non_destructive()`. Additive ops commit to a new version; destructive ops stage via `stage_revisions`. Requires `apply_operations` to report which ops it applied (Batch 3).

### 5.3 Explicitly NOT changing

| Item | Reason |
|---|---|
| `prune_empty_sections` | **Correct.** `memory-spec.md §5.1` mandates it. |
| `MemoryRevisionView.old_text` | Already implemented (§2, F3 retraction). |
| Persisted `op` strings | DB data + asserted by [`personal_memory_test.rs:604`](file:///home/addy/projects/apps/vox/app/src-tauri/tests/personal_memory_test.rs#L604). |
| `deactivate_observation` cascade | Verified correct — also sets `memory_facts_vectors.status='inactive'` ([`facts.rs:317-320`](file:///home/addy/projects/apps/vox/app/src-tauri/src/persistence/facts.rs#L317)). Confirmed empirically: 0 inactive facts, 0 inactive vectors. |

---

## 6. Corrected Prompts

> **Applied per D2–D4:** umbrella sections as principle-plus-examples with **no closed list**; **no `dispositions`**; **no `reason`/`observation`** on `delete`.

### 6.1 `PERSONAL_COLD_GENERATION_SYSTEM_PROMPT`

```
<role>
You build a user's personal memory from a numbered list of observations about them.
You are restating what is already known. You are not composing a profile, inferring background,
or filling gaps. The output is a reorganization of the input and nothing beyond it.
</role>

<input_format>
Observations arrive as [O1], [O2], ... Each is a durable fact already established about the user.
</input_format>

<sections>
Sections are UMBRELLAS, not topics. An umbrella is a broad area of someone's life — for example
"Career", "About Them", "Habits", "Interests", "Health", or "Plans". A subject like baking, Rust,
or language study is never its own section: it lives inside an umbrella, and the block carries
the subject.

1. Pick an umbrella by the SUBJECT NOUN of the facts it holds, not by the conversation they came
   up in. "bakes sourdough every weekend" and "sets a 9 PM reminder" are both routine habits;
   "likes sourdough" is an interest.
2. If a fact fits no existing umbrella, extend the closest one.
3. Keep the number of umbrellas small. One section per subject is the failure mode: it is what
   lets a later pass destroy a whole subject by deleting its few blocks.
4. Every section holds at least one block. Omit an umbrella entirely if nothing belongs in it.
5. A fact belongs to exactly one umbrella. Never restate one under two.
6. Never split one subject across two umbrellas.
</sections>

<rules>
1. Restate, do not extend. Every sentence traces back to an observation. A detail that is not in
   an observation is not in the output.
2. Never infer an attribute from a context. A city is not a home. A place visited is not a
   residence. A second person mentioned is not a household member, partner, or colleague. A project
   is not a job title. A scheduled item is not a standing habit. State only what is stated.
3. Every observation appears in exactly one block. Observations saying the same thing merge into
   one block. None is dropped.
4. Keep the specificity of the observations. "A study reminder set for 9 PM" is not "a routine".
   "Walnut rolls next week" is not "baking".
5. A block is 1 to 3 sentences of coherent prose on a single subject. Never bullets or fragments.
6. If two observations conflict, keep both, with the more recent one stated last.
7. Fewer, denser blocks beat more, thinner ones. Never restate the same content in two blocks.
8. Output only the raw JSON object. No markdown, no code fences.
</rules>

<output_format>
{ "sections": [ { "title": "...", "blocks": ["...", "..."] } ] }
</output_format>

<example>
Observations:
[O1] The user says the farmer's market is closer to their place.
[O2] The user says they will walk in the city park this weekend.
[O3] The user bakes sourdough bread with walnuts.
[O4] The user is working on a Rust project involving ownership and lifetimes.
[O5] The user sets a study reminder for 9 PM.

Correct:
  About Me        -> "The user lives in a city with a nearby farmer's market."
  Habits & Routine-> "The user bakes sourdough bread with walnuts."
  Work & Skills   -> "The user is working on a Rust project involving ownership and lifetimes."
  Plans           -> "The user plans to walk in a city park this weekend."

Wrong, and why:
  "The user lives in an apartment near the park."
      No observation says apartment. No observation says the park is near their home. Both are
      invented, and both become stored fact about this user.

  Sections "Baking", "Rust", "Languages":
      Subjects are not sections. These belong inside umbrellas, and one subject given its own
      section is what loses whole subjects on later passes.
</example>
```

**Rule-2 provenance.** The `apartment`/`house` failure is real: `grep -in "apartment"` across all 200 turns of cases 01–03 returns **0 hits.** `"house"` returns 0. Only `live` matches are Rust lifetimes (*"how long the reference needs to live"*). User's sole residential statement is *"my place in Chicago."*

### 6.2 `PERSONAL_INCREMENTAL_INTEGRATION_SYSTEM_PROMPT`

```
<role>
You maintain a user's personal memory. You receive the current memory (umbrella sections with
blocks) and a list of new observations. You return the edits that fold those observations in.

Priority order, highest first:
1. Nothing currently in the memory is removed unless a new observation directly contradicts it.
2. Every new observation ends up represented in the memory.
3. No block ever claims more than its observations support.
</role>

<memory_view>
[s1] Umbrella Title
  [b1] One block: 1-3 sentences of prose about a single subject.
  [b2] Another block.

Handles are positional labels for THIS request only. They are the only way to refer to existing
content. Never invent a handle that is not shown.
</memory_view>

<observations>
Numbered [O1], [O2], ... Each is a durable statement already established about the user.
</observations>

<procedure>
For each observation, decide exactly one:
  no-op   the memory already states this — write nothing
  update  the memory states something this refines or supersedes — one `update` on that block
  add     new, and an existing umbrella covers the subject — one `add` on that umbrella
  new     new, and no existing umbrella covers the subject — one `new`
  delete  the memory states something this proves false — see <retirement>

Then check: is there any observation you skipped without deciding? Place it, or leave it in an
umbrella it fits. Never silently skip one.
</procedure>

<operations>
new     {"title": "...", "blocks": ["...", "..."]}
        An umbrella that does not exist yet, with its first blocks. Rare: prefer `add`, since
        reaching for `new` is how umbrellas fragment back into one-section-per-subject.
add     {"section": "s2", "text": "..."}
        Append one block to an existing umbrella. The block's subject must match that umbrella.
update  {"block": "b3", "text": "..."}
        Replace one block's text. The new text must describe the SAME subject as the block it
        replaces. If the subject changed, use `add` and leave the old block alone.
delete  {"block": "b3"}
        Remove one block. See <retirement>.
</operations>

<retirement>
`delete` permanently discards everything that block said. It is not a cleanup tool, not a way to
keep the operation count low, and not a way to shorten or tidy text.

Use it only when an observation states the block is no longer true, and the replacement is written
in the same pass.

Prefer `update` whenever a block is being extended, refined, reorganized, or partly wrong. Never
delete because a new observation is about a related topic.
</retirement>

<rules>
1. A block may state only what its observations state. Do not add housing type, relationship
   status, proximity ("near", "close to"), quantities, motivations, or timeframes that no
   observation supplied.
2. An `update` keeps the subject of the block it replaces. Different subject -> `add`.
3. Never move content between sections. Content stays in the umbrella that holds it.
4. Never restate what the memory already says. Redundant text is worse than no text.
5. Every operation traces to at least one observation. None exists to tidy or shorten the memory.
6. Empty arrays are correct and expected for operations you do not need.
7. Output only the raw JSON object. No markdown, no commentary.
</rules>

<example>
Current memory:
[s1] About Me
  [b1] The user studies Spanish and Japanese daily, with a study reminder set for 9 PM.
  [b2] The user is a software developer.
[s2] Habits & Routine
  [b3] The user bakes sourdough bread with walnuts.

New observations:
[O1] The user is slowing down Japanese and focusing on Spanish.
[O2] The user plans walnut rolls for an upcoming bake.

Correct:
  update: [{ "block": "b1", "text": "The user studies Spanish and Japanese daily with a 9 PM
             study reminder, and is currently focusing more on Spanish while slowing down
             Japanese." }]
  add:    [{ "section": "s2", "text": "The user plans walnut rolls for an upcoming bake." }]
  new: []  delete: []

Two wrong outputs, and why:

  "delete": [{ "block": "b1" }]
      Nothing contradicts the 9 PM reminder. O1 adds to b1, so O1 is an `update`. Deleting also
      removes the only block holding language study — a whole subject lost over phrasing.

  "update": [{ "block": "b3", "text": "The user bakes sourdough bread with walnuts. The user is a
             software developer working on a Rust project." }]
      Software work does not belong under Habits & Routine, and b2 already covers the profession.
      Rewriting a correct block to carry content that belongs nowhere is churn.
</example>
```

### 6.3 Prompt-change → failure mapping

| Change | Failure it addresses | Evidence |
|---|---|---|
| `<retirement>` replaces peer-verb `delete` | RF-2 | `"deletes":[{"block":"b1"},{"block":"b2"}]`, nothing contradicting either |
| *"smallest set" role line removed (`:43`, `:76`)* | RF-2 | That line is what the destructive minimalism optimised |
| Umbrella-vs-topic guidance, both prompts | RF-2, RF-3 | `sec_1a0f63610e1_c3e5` deleted wholesale; v3→v4 split *Location* across two sections |
| Rule 2 attribute-inference ban + example | RF-1, RF-4 | `"lives in an apartment or house near Grant Park"` — both fabricated |
| Rule 2 `update` keeps subject | RF-2 | Rust project written into the Baking block |
| Rule 3 no inter-section migration | RF-2 | same |
| Rule 4 no restating what memory holds | cosmetic | Children-of-Memory duplicated into two blocks |
| `[O1]…` numbering | RF-3 | enables "which observation justifies this?" review |

### 6.4 Not changing

`COMMENT_DIRECTED_EDIT_SYSTEM_PROMPT` — op-key renames only, behaviour untouched.

---

## 7. Eval Harness Fixes

### 7.1 `evals/common/llm_client.rs` — persist judge requests **and** responses

**Finding H1/H2.** `NvidiaJudgeClient::evaluate` returns only the report body; the request is discarded. It also bypasses `RecordingLlmProvider` entirely, so `raw_llm_traces.json` holds **6 pipeline calls and 0 judge calls.**

**Fix:** write `case_dir/judge_traces.json` — full prompt, model, params, raw response, per judge call. Every §5 verification in the master report required template reconstruction; this removes that tax permanently.

### 7.2 `evals/common/consolidation_eval.rs` — make deletion visible and countable

| Change | Addresses |
|---|---|
| Replace `X / Y = Z%` coverage ask with an **explicit omitted-observation list** | Circular denominator — judge scored 7/7 while listing the drop |
| Add **`Deletions: N`** as a countable scorecard field | `:203` never asks about `deletes`; case_02 judge never said the word |
| Feed the **block-level disappearance list** into the prompt | Deletion currently must be *inferred* from markdown diffs |
| Update op vocabulary in the `:203` reference | Rename |

### 7.3 `evals/common/compaction_eval.rs` — force fact-to-turn citation

**Finding:** `"lives in an apartment or house near Grant Park"` scored `Fact Precision 17/17 = 100%` and *"There are no hallucinated statements or ungrounded claims."* The judge had all 90 turns in its prompt and never checked one fact against the dialogue.

**Fix:** require a **source-turn citation per fact**. A fact that cannot cite a turn is ungrounded by construction. Add an explicit *"attributes not stated by the user"* check.

### 7.4 `evals/common/ingestion_eval.rs` — same-type similarity table

**Finding (Refutation D).** In case_02 the judge compared two `personal` facts against **`objective`** facts and dismissed both via the cross-type invariant — never running the `personal`↔`personal` comparison that mattered (measured **0.9300**, a genuine paraphrase duplicate).

**Fix:** harness computes the **same-type cosine table** from `memory_facts_vectors` and injects it. The judge reads numbers instead of guessing.

### 7.5 `evals/memory_eval.rs`

Update op vocabulary in the consolidation judge prompt reference. Flag the forced-`auto_apply` deviation in the run summary so reports cannot be mistaken for default-policy behaviour.

---

## 8. Metrics — What the Eval Must Cover

### 8.1 Red-flag gates (pass/fail)

| ID | Metric | Instrument | Target |
|---|---|---|---|
| **G-1** | Unjustified deletions per pass | LLM op trace + `memory_facts`/`personal_memory` diff | **0** |
| **G-2** | `personal` facts with no source-turn citation | Turn-level string trace | **0** |
| **G-3** | Observations silently dropped | DB: candidate count vs `integrated` count vs op-referenced count | **0** |
| **G-4** | Sections lost between versions | `personal_memory` section-ID diff | **0** |

**G-4 note:** a section legitimately disappears when its last block is deleted *and a replacement section is created in the same pass.* The gate counts **net** loss, not mechanical disappearance — else it would flag correct auto-prune.

### 8.2 Calibration metrics (measure, never gate)

Per **D8** — reported on a 97–98% success bar.

| ID | Metric | Why it is not a gate |
|---|---|---|
| M-1 | Deduplication precision / recall | Run `…59adeb37` had **0 merges** across 54 facts; max same-type cosine **0.9300**. Undefined, not good. |
| M-2 | Consolidation observation coverage | 2 of 3 cases landed 5–6 of 6. |
| M-3 | Section-title stability | Cosmetic. |
| M-4 | Block-count drift | Cosmetic. |
| M-5 | Prose density / coherence | Subjective. |

### 8.3 Threshold sensitivity — carry forward, still unresolved

Sweep over the 341 same-type pairs (embeddings as stored):

| Threshold | Pairs merged | Assessment |
|---:|---:|---|
| 0.99 – 0.95 | 0 | Shipped. 1 duplicate survives. |
| **0.94 – 0.93** | **1** | Catches the paraphrase duplicate. Zero false merges. |
| 0.88 – 0.85 | 1 | Identical. |
| 0.80 | 5 | First information-losing merges (0.8387, 0.8238). |

**Only evidence-supported window is 0.92–0.95.** Per `qa-engineer.md` — *"A constant doesn't move because a handful of cases looked wrong"* — **n=1 duplicate is not a confusion matrix.** No threshold change until a labelled duplicate set exists (§10).

### 8.4 Required additions to the harness

| # | Addition | Unblocks |
|---|---|---|
| E-1 | Judge traces persisted | Every future audit (H1/H2) |
| E-2 | Section-ID + block-ID diff between versions, emitted as telemetry | G-4 |
| E-3 | Candidate vs represented observation counts | G-3 |
| E-4 | Source-turn string-trace per `personal` fact | G-2 |
| E-5 | Same-type cosine table precomputed | M-1, Refutation D |
| E-6 | Pipeline `seed` fixed; **k ≥ 3** runs, report mean ± spread | All — current run is unseeded n=1, so **no claim is repeatable** |

---

## 9. Batches

> [!IMPORTANT]
> **Build expectation key:** 🟢 green throughout · 🟡 red mid-batch, green on completion

### Batch 1 — Spec Synchronization 🟢
**Files:** `memory-spec.md`, `ipc-spec.md`
**Depends:** nothing. **Blocks everything else** (`AGENTS.md` §4.3).
Per §4. **No `db-spec.md` change.**

### Batch 2 — Eval Instrument Repair 🟢
**Files:** `llm_client.rs`, `consolidation_eval.rs`, `compaction_eval.rs`, `ingestion_eval.rs`, `memory_eval.rs`
**Rationale:** instrument before measuring. Pure harness work — no production behaviour change, so it can land and be reviewed independently of the prompt rewrite.

### Batch 3 — Production Bug Fixes 🟡
**Dependency:** Batch 2's telemetry (E-2/E-3) validates F1's floor.

### Batch 4 — Prompt Rewrite 🟢
**Files:** `prompts.rs`
Per §6. Schema keys renamed in lockstep with Batch 3.

### Batch 5 — Re-measure 🟡
Same three cases, identical command. Report **G-1..G-4 only** as gates; §8.2 as measured context.

---

## 10. Risk Register

| Risk | Impact | Mitigation |
|---|---|---|
| F1 coverage floor has no separating distribution | Observations silently retried forever, or loss persists | Calibrate first (§5.1). **If no separation, escalate — do not guess.** |
| Op rename breaks a JSON-schema consumer | `OutputConstraint::JsonSchema` 400s → transport degrades | Rename + prompt land in one commit; check `catalog` structured-output support |
| `#[serde(rename)]` drifts from `prompts.rs` schema | Model emits `creates`, parse yields empty | Single source of truth: derive schema keys from the serde attributes |
| F2 partitioning changes `auto_apply` commit shape | Duplicate versions, or staged ops applied twice | Partition on `apply_operations`' **applied** set, never on the requested set |
| Prompts grow past small-model attention | RF-1..RF-4 regress | Word-count budget; track scorecard movement, not prose volume |
| Re-run still unseeded n=1 | Unrepeatable | E-6 before Batch 5 |
| Judge trace persistence writes sensitive dialogue | Data exposure | `temp/`-class path discipline per `storage-spec.md`; redact before any commit |

---

## 11. Spec Coverage

| Requirement | Batch |
|---|---|
| Wire op names `new`/`add`/`update`/`delete` | 1, 4 |
| `INVARIANT 5.3-C` + 5.3-A → only represented observations integrate | 1, 3 |
| `auto_apply` stages `delete_block` | 1, 3 |
| Umbrella section principle (no closed vocabulary) | 4 |
| `<retirement>` delete gating | 4 |
| Attribute-inference ban (RF-1/RF-4) | 4 |
| Observation numbering `[O1]…` | 3, 4 |
| Judge trace persistence | 2 |
| Countable deletion + omission metrics | 2 |
| Source-turn citation gate (G-2) | 2 |
| Same-type cosine table | 2 |
| Seeding + k≥3 | 2, 5 |

### Deferred

| Item | Reason |
|---|---|
| `COMPACTION_SYSTEM_PROMPT` hardening | **Upstream of every RF-1/RF-4 instance.** The `apartment or house` fabrication is born here; its rule `:31` *already* forbids location→residence and a 9B model walked past it. Fixing consolidation stores the fabrication more tidily. **Highest-value deferred item — recommend a follow-up plan.** |
| 0.95 → 0.93–0.94 threshold change | n=1 duplicate. Needs a labelled set (§8.3) |
| Cross-case continuity assertion | `dataset_record.md §4` requires reporting "invalid cross-case memory continuity"; harness implements none. G-4 is a partial proxy |
| Cases 04–14 execution | 11 of 14 unrun; `context_window: 8192` vs ~41k tokens for `case_13` — **unverified transport behaviour** |
| Harness contract compliance | `dataset_record.md`: LLM judges forbidden, ≥1 simulated day between cases, production budgeting, crossing cases. All violated |
| `regenerate_personal_memory` hardening | Correctly reads `status='integrated'`; would recover F1 losses — but must not become a silent workaround for the bug |

---

## 12. QA Sign-off Criteria

Per `qa-engineer.md` — *"you say it because you checked."*

**Approval requires:**

1. **Spec (Batch 1) merged before code** — no silent drift.
2. **G-1..G-4 = 0** on a seeded k≥3 re-run.
3. **Every judge claim independently verifiable** from `judge_traces.json` without template reconstruction.
4. **F1 coverage floor calibrated from a measured distribution**, not chosen.
5. **Existing suite green** — op rename must not regress `tests/personal_memory_test.rs`.

**Not approval criteria:** coverage %, threshold change, section-title quality, prose density.

> **Current status: NOT CERTIFIED.** Run `…59adeb37` cannot certify the memory pipeline. It carries a vacuous dedup evaluation, a fabricated fact scored 100% precision, and a documented permanent-loss path. Re-audit after Batch 5.

---

*QA-authored plan. No production code modified. Every file reference and metric traced to verified source or to the audited run's database and traces.*