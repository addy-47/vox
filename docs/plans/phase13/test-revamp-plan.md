# Test Suite Revamp — Vertical Slice Plan

**Date:** 2026-10-22
**Audit:** `docs/tests/test-suite-audit-2026-10-22.md`
**Mode:** vertical slices, one seam end to end, stop and report after each.
**Hard rules for this campaign:** no production code fixes. A failing test is a finding, written to `docs/plans/phase13/test-findings.md`, then the next seam starts.

---

## 0. Standing rules

| Rule | Meaning |
| :--- | :--- |
| **One seam per slice** | Explore the seam's production code, its specs, and every test that touches it. Then write. Then run that seam's binary only. Then stop and report. |
| **One binary per seam, 10–20 subtests** | Test files exist to amortise compile and link time. A behavior gets a subtest, not a file. Target: 39 files → ~17 binaries. |
| **No production edits** | If the test exposes a defect, log it and move on. Do not fix it in this campaign. |
| **No CI changes** | CI is compile-only by choice. Test execution is local and manual. This is documented in the style guide. |
| **Helpers are shared** | Anything used twice lives in `tests/common/`. Duplicated setup is deleted, not tolerated. |
| **Fold or prune** | A weaker test never survives beside a stronger one that covers it. Same behavior → merge. Same assertion → keep one. Dead test → delete. |
| **Batch compilation** | Edit the whole seam, then compile and run once: `cargo nextest run --test <binary> --release --test-threads=1 --no-fail-fast`. Never run per-edit. |
| **Mutants at the end** | One `cargo-mutants` pass across all seams after the last slice. Not per slice. |

---

## 1. Scope, enumerated

From the repository, not from memory:

- 39 integration test files; 12 are remote → **27 in scope**.
- 172 integration test functions; 51 remote → **121 in scope**.
- 136 unit test functions across 30 `src` modules.
- 4 orphan `#[ignore]` tests.
- 65 raw sleeps, 10 silent channel drains, 8 swallowed worker joins.

Full item-by-item tracking: `docs/plans/phase13/test-revamp-checklist.md`.

---

## 2. Target binary layout

One binary per seam. Counts are the expected subtest count after folding.

| # | Binary | Absorbs | ~Subtests |
| :--- | :--- | :--- | :--- |
| 1 | `vad_segmentation_test.rs` | `passive_streaming_test.rs` | 14 |
| 2 | `stt_turn_identity_test.rs` | *(new)* | 12 |
| 3 | `dictation_ptt_test.rs` | `ptt_window_modular_test.rs`, `ptt_window_realtime_offline_test.rs` | 16 |
| 4 | `dictation_output_test.rs` | `dictation_window_test.rs` (output half) | 12 |
| 5 | `transcript_ingestion_test.rs` | `transcript_to_llm_test.rs` | 14 |
| 6 | `agentic_tool_loop_test.rs` | `agentic_tool_runtime_test.rs` | 12 |
| 7 | `llm_to_tts_test.rs` | `llm_to_tts_test.rs`, `chunking_determinism_test.rs` | 14 |
| 8 | `tts_catalogue_test.rs` | `tts_provider_catalogue_test.rs`, `tts_voice_catalogue_test.rs`, `tts_transition_test.rs` | 18 |
| 9 | `playback_test.rs` | `playback_interrupt_test.rs`, `tts_to_playback_test.rs` | 14 |
| 10 | `audio_capture_aec_test.rs` | `audio_capture_aec_test.rs`, `aec_pipeline_test.rs`, `aec_barge_in_integration_test.rs` | 14 |
| 11 | `harness_loop_test.rs` | *(new)* | 14 |
| 12 | `session_lifecycle_test.rs` | unchanged name | 10 |
| 13 | `storage_test.rs` | `database_persistence_boundary_test.rs` | 8 |
| 14 | `settings_test.rs` | `settings_persistence_test.rs` | 9 |
| 15 | `memory_test.rs` | `personal_memory_test.rs`, `memory_ingestion_test.rs`, `memory_compaction_test.rs` | 20 |
| 16 | `notifications_test.rs` | `notifications_crud_test.rs` | 6 |
| 17 | `model_manager_test.rs` | `model_manager_test.rs`, `model_eviction_test.rs` | 14 |
| 18 | `ipc_registration_test.rs` | unchanged | 2 |
| 19 | `realtime_transport_test.rs` | unchanged | 6 |

**19 binaries** for 121+ tests, down from 39. Remote tests stay untouched and feature-gated.

Where a seam already has 10–20 healthy subtests and an honest name (e.g. `session_lifecycle_test.rs`), the file is kept and hardened in place — not renamed for the sake of motion.

---

## 3. Slice order

Ordered by user impact first, shared machinery before the seam that depends on it.

### Slice 0 — Standards *(no test files touched)*

Deliverables:

1. `.agents/rules/testing-style-guide.md` — rewritten:
   - taxonomy: Unit, Component, Integration (seam), E2E, Evaluation, Benchmark, each defined by *what it proves*
   - one-binary-per-seam rule with the 10–20 subtest target and the compile-cost rationale
   - mandatory per-test intent block: `PURPOSE / SCENARIO / ASSERTS / CLASS / REQUIRES / MUTANT`
   - naming rule; banned names
   - absence-assertion rule: quiescence must be proven; a timer is not proof
   - no wall-clock latency asserts in the default tier
   - helper ownership: shared in `tests/common/`, no duplication, replaceable by a production call
   - **execution policy: CI is compile-only by deliberate choice.** Test runs are local/manual. Nobody adds a CI test job without an explicit decision.
   - finding protocol: log to `docs/plans/phase13/test-findings.md`, do not fix
   - fold-or-prune rule
   - mutation policy: `MUTANT` feeds `cargo-mutants`, run once at campaign end
2. `create-test` skill (global) — amended:
   - persist the trace as the in-code intent block
   - one binary per seam, many subtests; remove the "one file per seam" ambiguity
   - naming rule
   - subtest honesty check
   - "fold or prune" step
   - testability gap → report, not production edit
   - hand-off to the findings file, not to `/mutate` per test
3. `.agents/rules/test-engineer.md` — aligned to the above.
4. `docs/tests/integration-test-spec.md` §0.1 — remove the endorsement of the sleep-based absence helper; reconcile header and section references with the new guide.
5. `app/src-tauri/scripts/test.sh` + `app/src-tauri/tests/tier-default.txt` — one local entry point for `default` / `models` / `all` / `seam <binary>`. Not CI.

Report, then stop.

### Slice 1 — Shared test support

- `tests/common/harness.rs`: replace `assert_channel_empty_after` with a quiescence-based absence primitive (close producer → drain to `Disconnected`; or barrier + empty queue).
- Audit the other 11 `tests/common/*.rs` for duplicates and dead helpers.
- Remove the 10 silent drains and 8 swallowed joins at their call sites.
- Report the sleep inventory by file, with a plan for each.

No seam behavior changes. Report, then stop.

### Slices 2–12 — Voice path

2. VAD segmentation
3. STT turn identity
4. Dictation PTT lifecycle
5. Dictation output (paste / clipboard / auto-stop)
6. Transcript ingestion (continuation, dedupe, stale cancel)
7. Agentic tool loop (cap 5, debounce, non-terminal retrieval)
8. LLM → TTS chunking
9. TTS catalogues and engine transitions
10. Playback, interrupt, barge-in
11. Audio capture and AEC
12. Harness loop

Each slice: explore code + specs + existing tests → write → run that binary → report → stop.

### Slices 13–19 — Supporting seams

13. Session lifecycle
14. Storage boundary
15. Settings
16. Memory (personal + ingestion + compaction)
17. Notifications
18. Model manager and eviction
19. IPC registration
20. Realtime transport

### Slice 21 — Final sweep

- Add intent blocks to remaining `src` unit-test modules.
- Prune and fold pass across the whole suite.
- Feature-gate remote tests.
- One `cargo-mutants` run over the changed files.
- Re-run the full non-remote suite once.
- Final report against the checklist.

---

## 4. Per-seam procedure

Every slice follows the same six steps. No exceptions, no shortcutting.

1. **Enumerate.** List every test in the seam and every production module it touches.
2. **Read.** Read the production code and the governing spec rules. Confirm the real entry seam and the real observable exit.
3. **Map.** Mark each existing test: keep / fold / prune / rewrite. Record the reason.
4. **Write.** One intent block per subtest. Helpers go to `tests/common/`.
5. **Run.** `VOX_MODELS_DIR=... cargo nextest run --test <binary> --release --test-threads=1 --no-fail-fast`. One compile per seam.
6. **Report.** Tests added / folded / pruned, findings logged, checklist updated. Stop.

### Finding record

Appended to `docs/plans/phase13/test-findings.md`:

```
## F-<n> — <one line>
Seam: <slice>
Test: <binary>::<subtest>
Symptom: <what the test observed>
Evidence: <assertion output / log line>
Suspected cause: <file:line, or "unknown">
Impact: <user-visible effect>
Status: open — not fixed (campaign rule)
```

---

## 5. Findings already known before the revamp

These are logged at Slice 1 or the owning seam, not fixed now.

| ID | Finding | Owning slice |
| :--- | :--- | :--- |
| F-1 | `assert_channel_empty_after` cannot prove absence (sleep + one `try_recv`) | 1 |
| F-2 | `transcript_to_llm_test.rs` calls `on_transcript_final` directly, bypassing STT | 6 |
| F-3 | "Paste" subtest never tests paste, clipboard restore, or auto-stop | 5 |
| F-4 | Zero tests for `continuation_prefix` / `awaiting_continuation` | 6 |
| F-5 | Zero tests for stale-cancel ordering | 6 |
| F-6 | `segmenter.rs` state machine untested | 2 |
| F-7 | `stt/actor.rs` turn identity untested | 3 |
| F-8 | Harness modules untested | 12 |
| F-9 | 8 swallowed worker joins; 10 silent drains | 1 |
| F-10 | Wall-clock latency asserts and `sleep`-then-assert-state flake | 1, 11, 12 |

---

## 6. Compilation discipline

- One compile per seam. All edits first, then one run.
- `--test <binary>` only. Never the whole suite mid-campaign.
- Release profile, shared `target/`, models via `VOX_MODELS_DIR=/home/hypr4/models`.
- Non-model seams (catalogue, IPC, settings, storage) still use `--release` per repo rule, but they link fast because the lib is already built.
- The full suite and the mutation pass run exactly twice: once at Slice 0 as a baseline, once at Slice 21.

---

## 7. Definition of done

- Checklist shows 100% of the 27 in-scope integration files resolved (kept, folded, or pruned).
- Every in-scope test function carries a valid intent block.
- Every seam has exactly one binary, 10–20 subtests, helpers imported from `tests/common/`.
- No `assert_channel_empty_after`, no silent drain, no swallowed join, no raw sleep in a default-tier test.
- Findings file contains every defect the campaign discovered, each with evidence, none fixed.
- One mutation run executed and recorded.
- Final report delivered; no production code modified.
