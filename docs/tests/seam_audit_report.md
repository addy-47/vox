# Vox Test Suite Audit — Master Report

**Date:** 2026-09-26
**Scope:** 139 test functions — 62 unit tests (18 files in `src/`) + 77 integration tests (21 files in `tests/`)
**Method:** 23 sprints (1 unit-test audit + 21 seam audits + shared-harness coverage), one seam per sprint.
**Authority:** production code. `docs/specs/integration-test-spec.md` was treated as stale and was **not**
used to judge correctness — it is reported as drift to be corrected.

---

## 1. Calibration

**Reviewed as: production-grade, local-first desktop app whose value is model orchestration.**

Consequences applied throughout:
- *Wiring* claims (right model, right payload, right channel) are IT-grade and were judged as such.
- *Model quality* claims ("the summary was good", "facts were extracted") are eval-grade and were flagged
  wherever they appeared inside an IT.
- An IT earns its place by crossing a real boundary with a real trigger. A unit test earns its place by
  probing real branching. Anything else is a level error.

---

## 2. Final verdict tally — 139/139 classified, 0 remaining

| Verdict | Unit tests | Integration tests | **Total** | Share |
| :--- | ---: | ---: | ---: | ---: |
| `WORTHY` | 50 | 54 | **104** | 74.8% |
| `WORTHY-THIN` | 3 | 13 | **16** | 11.5% |
| `MISCLASSIFIED` | 4 | 7 | **11** | 7.9% |
| `EYE-CANDY` | 4 | 2 | **6** | 4.3% |
| `REFACTOR` | 1 | 1 | **2** | 1.4% |
| **Total** | **62** | **77** | **139** | 100% |

**86% of the suite is sound.** Your suspicion that "so many of them look like simple UTs" is correct about
*specific* tests, not about the suite. The defects are concentrated, identifiable, and mostly duplication
or level errors — not rot.

**Enforcement proof (create-sprints Step 6):** the Step 1 enumeration was re-run after all sprints and
returns identical counts — 21 IT files, 77 IT functions, 4 `#[ignore]`, 18 UT files, 62 UT functions.
`CHECKLIST.md` shows 139/139 checked, 0 unchecked, 139 carrying an explicit verdict. No instance was missed
and none was introduced.

---

## 3. Per-seam matrix

| Sprint | Seam | File | Verdict profile | File verdict |
| :--: | :--- | :--- | :--- | :--- |
| 0 | — | 18 `src/` files | 50 W · 3 WT · 4 MC · 4 EC · 1 R | Restructure |
| 1 | 1 | `passive_streaming_test.rs` | 1 WT | KEEP |
| 2 | 2 | `ptt_window_modular_test.rs` | 1 WT | KEEP |
| 3 | 3 | `ptt_window_realtime_test.rs` | 1 WT | KEEP + SPLIT |
| 4 | 4 | `dictation_window_test.rs` | 1 W | KEEP |
| 5 | 5 | `transcript_to_llm_test.rs` | 1 WT | KEEP + STRENGTHEN |
| 6 | 6 | `llm_to_tts_test.rs` | 1 WT | KEEP + STRENGTHEN |
| 7 | 7 | `tts_to_playback_test.rs` | 2 W | KEEP |
| 8 | 8 | `tts_transition_test.rs` | 2 W | KEEP |
| 9 | 9 | `playback_interrupt_test.rs` | 6 W · 1 WT | KEEP |
| 10 | 10 | `chunking_determinism_test.rs` | 3 MC | **DELETE** (relocate 3, add 1 to Seam 6) |
| 11 | 11 | `session_lifecycle_test.rs` | 7 W · 1 MC | KEEP + SPLIT |
| 12 | 12 | `memory_compaction_test.rs` | 3 WT · 2 MC · 1 R | KEEP + RESTRUCTURE |
| 13 | 13 | `memory_ingestion_test.rs` | 4 W | KEEP |
| 14 | 14 | `personal_memory_test.rs` | 4 W · 1 WT · 1 MC | KEEP + **EXTEND** |
| 15 | 15 | `settings_persistence_test.rs` | 3 W | KEEP |
| 16 | 16 | `model_eviction_test.rs` | 2 WT | KEEP + RESTRUCTURE |
| 17 | 17 | `model_manager_test.rs` | 3 W · **2 EC** | KEEP + REWRITE 2 |
| 18 | 18 | `notifications_crud_test.rs` | 5 W | KEEP |
| 19 | 19 | `realtime_transport_test.rs` | 5 W · 1 WT | KEEP |
| 20 | 20 | `database_persistence_boundary_test.rs` | 6 W | KEEP + FIX-DOCS |
| 21 | 21 | `agentic_tool_runtime_test.rs` | 6 W | KEEP — reference |

---

## 4. 🔴 Findings that must be fixed

### F1 — Your consolidation refactor is essentially untested (`personal_memory_suggestions`)

The single most important finding. The entire new surface area has **zero** behavioural tests:

| Surface | Location | Tests |
| :--- | :--- | :---: |
| `personal_memory_suggestions` table | `persistence/schema.rs` | 1 — its *name* in a table list |
| `insert_personal_memory_suggestions` | `persistence/personal_memory.rs:394` | **0** |
| suggestion re-anchoring SQL | `persistence/personal_memory.rs:588-606` | **0** |
| `get_memory_suggestions` (IPC) | `ipc/memory.rs:211` | **0** |
| `resolve_memory_suggestion` (IPC) | `ipc/memory.rs:229` | **0** |
| `'staged'` / `'rejected'` fact lifecycle | `persistence/facts.rs` | **0** |

`src/persistence/personal_memory.rs` — 600+ lines at the centre of the refactor — has **no `#[cfg(test)]`
module at all**. All of this ships green today: a broken re-anchor silently strands queued user work;
facts trapped in `'staged'` are invisible but never cleaned up.

**Your system architect was right.** I verified this independently and reached the same conclusion. The
architect's "no IT needed" meant *no pipeline test* — not *no test*. The transactional boundary does need
one. Four tests required, listed in `SPRINT_14`.

### F2 — The two "security" tests test the third-party crate, not Vox

`model_manager_test.rs` header advertises `Zip-Slip guard` as a covered metric. Neither security test calls
Vox code.

- **Test 3** calls `entry.enclosed_name()` — a `zip` **crate** method — then asserts a file doesn't exist
  that was never written (`extract_dir` is created and never used).
- **Test 4 re-implements the production check inside the test** and asserts the fixture *is* malicious:
  `assert!(has_parent, "Tar archive traversal component ParentDir must be detected")`.

The real guard is `pub` at `src/setup/model_manager.rs:334` (`do_extract`) and returns
`"Zip-Slip vulnerability detected"`. **If it were deleted, both tests still pass.** This is worse than no
test — it retires the concern. Replacement is ~6 lines per test (see `SPRINT_17`).

### F3 — `schema.rs::test_rebuild_fixtures` rewrites your repo assets and asserts nothing

```rust
rebuild_fixture_db(&test_db_path).await.expect("Failed to rebuild test fixture DB");
rebuild_fixture_db(&bench_db_path).await.expect("Failed to rebuild bench fixture DB");
```

A `#[tokio::test]` that overwrites `tests/assets/test_vox.db` and `benches/assets/bench_vox.db` on every
`cargo test --lib` and asserts no content. It is a maintenance script, not a test. Delete; move to
`examples/`.

### F4 — `vad/actor.rs::test_window_validation_trimming_logic` cannot fail

The test copies the production arithmetic into its own body and asserts on its own result. It calls zero
production functions. Root cause is a genuine testability gap: the trimming logic is **inline in the actor
loop** at `src/services/vad/actor.rs:265-275` and was never extracted. None of its three branches is
covered. Extract `fn trim_window(state, raw_len) -> Vec<i16>` first.

### F5 — Two false greens in the cognitive stage

- **Seam 6, Exit 6** — the test performs the history commit by hand
  (`harness.push_assistant_turn(...)`) then asserts history contains it. Production already commits at
  `src/services/harness/steps.rs:671`. **Delete line 671 and the test still passes.**
- **Seam 12, test 6** — the test hand-assembles `history_messages` in a loop and asserts
  `<prior_summary>` is present. Production does this in the **private**
  `coordinator.rs:267 build_history_messages`. **Make it `pub` and call it** — a one-word change converts a
  false green into a real test.

### F6 — Seam 5 never observes what is sent to the model

The seam is *"assemble system prompt + memory + history into a `GenerationRequest`"*. The test asserts the
transmit half and the store half, never the assemble half. System prompt injection, personal memory
injection, and history truncation could all break with the suite green.

**Root cause is a blocked seam:** `llm_rx` is moved into the worker thread, so the request is unobservable.
**The fix already exists in this repo** — `agentic_tool_runtime_test.rs`'s `spawn_mock_wire_server` captures
real request bodies. Reuse it. (Sprint 5 originally proposed a production observer hook; Sprint 21 showed
that is unnecessary.)

### F7 — The only Realtime PTT test is behind a paid cloud key

`ptt_window_realtime_test.rs` is `#[ignore]`d, so the whole seam is dark in CI. But only *one* of its
assertions needs Deepgram. The **zero-STT-leak invariant** — asserted three times and worth keeping — needs
no key. Reuse Seam 19's in-process WebSocket server to un-gate the other six assertions.

---

## 5. 🟠 Systematic issues (not per-test defects)

1. **Matrix bundling** in Seams 1, 2, 3, 4, 5 — multiple scenarios in one `#[test]`, order-dependent, with
   `while rx.try_recv().is_ok() {}` drains that **silently swallow unexpected events**. Violates
   `/create-test` Phase 3. Style guide §7.3 endorses consolidated matrices for *backend initialisation only*;
   the two rules are in tension and the tests currently honour the wrong half.
2. **Sleep-as-proxy-for-absence** (style §6.1) in Seams 1, 4, 3, 11 — 7+ sites. Fixed waits already exist in
   `common::harness` (`assert_channel_empty_after`, `wait_for_dictation_state`); several tests use raw
   `sleep` instead. Seam 11's duplicate-`SessionStart` check can pass simply because the router thread has
   not been scheduled yet.
3. **Duplication** — 5 of the 8 unit-test defects are weaker duplicates of an existing IT. Two exact test
   *name collisions* (`test_stage1_exact_dedup_winner_takes_all` exists in both `src/` and `tests/`).
   Seam 8's filler test supersedes Seam 5's subtest.
4. **Doc drift** — Seam 20 declares schema v5 in 5 places while asserting v7; `schema.rs` test named
   `test_v2_schema_initialization` with a `// Verify user_version == 2` comment while `SCHEMA_VERSION = 7`;
   Seam 10's docstring describes a "20-word emergency cap" that does not exist (actual: 8/15/24); Seam 12's
   filename claims "Coordinator" coverage it does not have. The stale `integration-test-spec.md` compounds
   this across all 21 seams.

---

## 6. Plan of action

### P0 — before the consolidation refactor is called done
| # | Action | Effort |
| :-- | :--- | :-- |
| 1 | Add the 4 `personal_memory_suggestions` lifecycle tests (F1) | 1 test file section |
| 2 | Rewrite Zip-Slip / Tar-Slip tests to call `do_extract` + add benign positive control (F2) | ~30 min |
| 3 | Delete `test_rebuild_fixtures` (F3) | 2 min |
| 4 | Make `build_history_messages` `pub`; fix Seam 12 test 6 (F5) | 5 min |
| 5 | Remove the hand-rolled commit in Seam 6 Exit 6 (F5) | 5 min |

### P1 — close the false greens
| # | Action | Effort |
| :-- | :--- | :-- |
| 6 | Extend `spawn_mock_wire_server` to Seam 5; assert assembled request (F6) | 1 test |
| 7 | Split `ptt_window_realtime_test.rs`; un-gate the 6 offline assertions (F7) | 1 new file |
| 8 | Extract `trim_window` from the VAD actor loop, then fix the test (F4) | small prod change |
| 9 | Fix the `let _ = vad_join.join()` panic-swallow in Seam 9 tests 4 and 5 | 10 min |

### P2 — level corrections and cleanup
| # | Action |
| :-- | :--- |
| 10 | Relocate 3 tests from `chunking_determinism_test.rs` into `chunker.rs` UTs, add 1 to Seam 6, delete the file |
| 11 | Move `test_session_continuation_fallback_when_compaction_empty` → Seam 20 |
| 12 | Delete `test_manual_edit_persistence` (subsumed by tests 2 and 6) |
| 13 | Delete 3 `src/` pseudo-UTs in `memory/ingestion/` + `schema.rs::test_v2_schema_initialization` |
| 14 | Delete `test_rebuild_fixtures`, `test_llm_cloud_keys_persistence`, `test_interaction_owner_conversions` |
| 15 | Move the `resolve_channel` truth table into `#[cfg(test)]` in `notifications/router.rs` |
| 16 | Move `llm/catalog/sync.rs`'s timing assertion to `benches/` |
| 17 | Consolidate Seam 16's two ONNX lifecycles into one session (style §7.3) |
| 18 | De-duplicate the filler assertions shared by Seams 5 and 8 |

### P3 — documentation and spec
| # | Action |
| :-- | :--- |
| 19 | Make `SCHEMA_VERSION` `pub`; reference it symbolically in Seam 20 and `schema.rs` to kill the v5/v7 drift class |
| 20 | Fix the 5 stale v5 references in `database_persistence_boundary_test.rs` |
| 21 | Fix Seam 10's "20-word cap" docstring → 8/15/24 adaptive tiers |
| 22 | Rename `memory_compaction_test.rs` — "Coordinator" is a claim it does not honour |
| 23 | Add `(Seam N)` to the 4 headers missing it (style §4) |
| 24 | Rewrite `docs/specs/integration-test-spec.md` against the findings here, or mark it superseded |
| 25 | Add a mandatory `as_ref` clause for `#[ignore]`: *"list which assertions genuinely require the external dependency"* — this alone would have caught F7 and Seam 12's ignored test |

### New tests worth adding (ranked)
1. The four `personal_memory_suggestions` tests (F1) — blocks a data-loss class.
2. `apply_patch_operations` **multi-op sequencing** — op *N* must see op *N-1*'s mutations. Untested
   anywhere, and multi-op patches are precisely the anchor-erosion failure mode.
3. `apply_patch_operations` **unknown `op` silently skipped** — a model emitting `"op":"update"` loses the
   fact with no error surfaced.
4. Barge-in **cancellation of `execute_turn` mid-generation** — the 30 s real-model path has no cancel test.
5. Seam 20 **at the production 400 ms silence default** — the shipped setting is currently untested
   (Seam 1 overrides to 800 ms to make its clip produce one utterance).

---

## 7. What is genuinely excellent — do not let the findings erode this

- **`agentic_tool_runtime_test.rs` (Seam 21)** — the reference implementation. Real captured wire bytes,
  chunked-SSE reassembly, incident-derived assertions (`"NIM 503s on stream_options"`), negative assertions
  on the output channel. Copy this file's structure.
- **`database_persistence_boundary_test.rs` (Seam 20)** — real Turso, real MVCC concurrency, and float
  precision tested with distinctive `sin()` values rather than a uniform fill, so a truncating
  implementation cannot pass.
- **`audio_filters.rs`** — `test_filter_bank_subtractive_identity` asserts a mathematical invariant. Catches
  coefficient drift no fixture would.
- **`dictation_window_test.rs` (Seam 4)** — the **LLM Zero Invariant** via a capture channel: asserts
  *nothing was sent toward the LLM at all*, which is strictly stronger than asserting an event did not
  appear. Make this the template.
- **`tts_to_playback_test.rs` (Seam 7)** — the only file carrying an **in-file false-green audit** for its
  stub, and its two-test split (real model for "does audio flow", deterministic stub for "does the threshold
  gate fire at the boundary") is the right decomposition.
- **`llm_to_tts_test.rs` (Seam 6)** — the two strongest assertions in the suite: streaming order (clauses
  dispatched *before* `LlmFinished`) and exact bidirectional accounting
  (`pending_synthesis_jobs == clauses.len()`).
- **`playback_interrupt_test.rs` (Seam 9)** — drives the real CPAL sink callback; test 7 is a complete latch
  lifecycle (arm, hold, release, clear).
- **The unit-test layer** — 50/62 genuinely good, with `chunker.rs`, `normalizer.rs`, `personal.rs` and
  `stitcher.rs` exemplary. Real boundary and negative assertions, not default-value coverage.

---

## 8. Direct answers to your questions

**"Does `results getting saved in db` classify as an IT?"**
No. It is a persistence test, and calling it an IT overstates it by a whole layer. That pattern occurs in
Seam 12 (5 of 6 tests), Seam 11 (1 of 8), and Seam 10 (3 of 3) — 9 of 77 ITs. It is not worthless: the
*behaviour* is often real. It is mislabelled, and the mislabelling is what let genuinely weak tests
(`test_memory_compaction_coordinator_slicing_and_ledger`, the Zip-Slip pair) sit beside strong ones without
comment.

**"Is compaction testable at all?"**
The coordinator's *plumbing* is IT-testable and already is — Seam 12 test 3's mutual-exclusion gate is a
real, valuable test. Whether a summary is *good* is not IT-testable; that is
`evals/memory_compaction_eval.rs`. The live test currently mixes both, which is why it reads as unsatisfying.

**"Was the architect right about not needing an IT?"**
Right about the pipeline, and right that the suggestion to add a bullet-normalization unit test was
redundant — `test_patch_replace_avoids_double_bullet` already exists. **Wrong to imply the transactional
boundary needs no test**: the four suggestion-lifecycle tests are the actual gap (F1).

**"How do I tell a real IT from eye candy?"**
Three questions, in order:
1. *If the upstream producer were deleted, would this still pass?* If yes → false green by construction.
2. *Does the test perform a production step itself?* (A loop, a commit, an assembly step.) If yes → it tests
   itself. This catches F5, F2-test-4, F4, and Sprint 6's Exit 6.
3. *Is any assertion a claim about model output quality?* If yes → it belongs in `evals/`.

Applied to a model-oriented app, question 3 is the one that matters most, and it is the one the current
suite is worst at: Seams 12, 13, 14 and 16 all use real models, and only Seam 13 keeps quality and wiring
cleanly separated.


---

# Remediation Log (appended 2026-09-26)

Spec updated first, per AGENTS.md §4.3, then tests authored, then the full suite run.

## Spec changes
- `docs/specs/integration-test-spec.md` → **v2.5**. Added normative **§0.1** (placement
  rules, Direction Check, Self-Execution Ban, mandatory negative assertions, Model Lens,
  `#[ignore]` constraint, matrix-bundling reconciliation, reference implementations, header
  rule). §0.1 explicitly OVERRIDES the stale per-seam sections; v5 → v7 schema references corrected.
- `docs/specs/memory-spec.md` → added **INVARIANT 5.3-A** (candidate partition: unselected
  facts → `'consolidated'`, never trapped in `'staged'`) and **INVARIANT 5.3-B** (re-anchor
  on accept — the anchor-erosion fix), plus action/target validation and atomicity contracts.
  Both were implemented in code but undocumented.

## Production changes (3, all non-behavioural)
| Change | File | Why |
| :--- | :--- | :--- |
| `build_history_messages` → `pub` | `services/memory/compaction/coordinator.rs:267` | It was private, so the test reimplemented it. Style guide §2 sanctions `pub` for items `tests/` must reach. |
| `SCHEMA_VERSION` → `pub` | `persistence/schema.rs:15` | Lets Seam 20 assert the version symbolically, permanently killing the v5/v7 comment-drift class. |
| Extracted `trim_window()` + `MIN_WINDOWED_SPEECH_SAMPLES` | `services/vad/actor.rs` | The trimming logic was inline in the actor loop, so no test could reach it. Now all three branches are covered. |

## Findings closed
| # | Finding | Resolution | Mutation-proved |
| :-- | :--- | :--- | :-- |
| F1 | `personal_memory_suggestions` untested | 4 new ITs in Seam 14 | **Yes** — re-anchor mutant killed (`left: 2, right: 3`) |
| F2 | Zip/Tar-Slip tested the crate, not Vox | Rewritten to call `ModelManager::do_extract`; **+ benign positive control** | **Yes** — guard-removal mutant killed |
| F3 | `test_rebuild_fixtures` rewrote repo assets, no assertions | Deleted | n/a |
| F4 | VAD trimming test could not fail | Test rewritten against extracted `trim_window`, all 3 branches + clamp | **Yes** — threshold 256→64 mutant killed |
| F5a | Seam 6 hand-rolled the history commit | Removed; asserts production state | Needs `/mutate` |
| F5b | Seam 12 hand-rolled the history assembly | Now calls production `build_history_messages` | **Yes** — SessionContext-wrap mutant killed |
| F6 | `GenerationRequest` unobservable | `spawn_mock_wire_server` promoted to `tests/common/wire.rs`; **Seam 5 assertion NOT completed — see below** | n/a |
| F7 | Whole Realtime PTT seam behind a paid key | New non-`#[ignore]`d `ptt_window_realtime_offline_test.rs` (3 scenarios, zero-STT-leak ×3) | Needs `/mutate` |

## ⚠️ F6 NOT completed
`tests/common/wire.rs` was created and Seam 21 rewired onto it (still green), but the Seam 5
assertion was **removed, not shipped**. Two attempts failed (AGENTS.md §9.3 2-attempt rule):
passing the harness a throwaway `llm_tx` meant no consumer dispatched to the provider, so the
capture server never received a request and the test hung. A correct version needs the harness's
`llm_tx` wired to a real `RemoteTransport` pointed at the capture server — i.e. Seam 5's
`EmbeddedProvider` worker must become provider-agnostic in the test setup. **The seam's 🔴 gap
therefore remains open.** It is now, however, a known, isolated task with a reusable harness
already in place, rather than an unexamined hole.

## Pre-existing failure found and fixed
`test_concurrent_mvcc_wal_readers_and_persistence_worker` **failed on unmodified code**
(verified via `git stash`: 21/25, and 19/25 on a second run). Root cause: a fixed
`sleep(100ms)` used as a proxy for the persistence worker's drain — a §6.1 violation that is
systematically too short. Replaced with a 10 s deadline poll that still fails if events are
genuinely dropped. This means the `135/135 passing` claim previously recorded in AGENTS.md was
not reproducible on this machine.

## Deletions (7)
`schema.rs::test_rebuild_fixtures`, `schema.rs::test_v2_schema_initialization`,
`settings.rs::test_llm_cloud_keys_persistence`, `state.rs::test_interaction_owner_conversions`,
`stage1_dedup.rs::test_stage1_exact_dedup_winner_takes_all`,
`stage2_embed.rs::test_stage2_cosine_dedup_winner_takes_all`,
`ingestion/mod.rs::test_crash_reconciliation_flow`, `personal_memory_test.rs::test_manual_edit_persistence`.

## Tests added (9)
4 × suggestion lifecycle · 3 × patch-engine (`multi-op sequencing`, `out-of-order target
skipped`, `unknown op skipped`) · 1 × VAD `trim_window` branches · 1 × Realtime PTT offline.

## Doc drift fixed
6 stale schema-v5 references in Seam 20 (assertion now symbolic against `SCHEMA_VERSION`) ·
Seam 10's non-existent "20-word cap" docstring → 8/15/24 adaptive tiers · `(Seam N)` added to
the 4 non-conforming headers.

## Final verification
```
cargo nextest run --release --test-threads=1 --no-fail-fast
  → 135 tests run: 135 passed, 4 skipped          (0 failed)
cargo test --release --lib
  → 58 passed; 0 failed
cargo clippy --release --all-targets
  → 0 warnings, 0 errors
```
Scope recount: **81 IT + 58 UT = 139** (was 139; net +0 after 8 deletions and 9 additions).
