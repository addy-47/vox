# Vox Test Suite — Audit and Revamp Proposal

**Date:** 2026-10-22
**Scope:** `.agents/rules/testing-style-guide.md`, the `create-test` skill, and the non-remote test suite under `app/src-tauri/{src,tests}`.
**Excluded:** everything remote (12 files, 51 tests) — WIP, per instruction.
**Status:** audit only. No test, style-guide, or skill file has been modified.

---

## 0. Verdict

1. **The style guide is a policy document with no enforcement.** It states the right principles (§2 "route events through actual actor channels") and every test violates them when convenient. Nothing checks it.
2. **The mandatory header is decorative.** 39/39 integration files carry the format. 0/39 carry a per-test statement of intent. Headers claim metrics the file never asserts.
3. **The absence-assertion primitive is a false-green generator.** `assert_channel_empty_after` sleeps then calls `try_recv` once. It is endorsed by the normative test spec. Every suppression test built on it proves nothing.
4. **The suite is not uniformly a golden path.** Memory, persistence, settings, and provider catalogues have real edge-case coverage. The gap is concentrated in the **live voice pipeline state machine** and the **harness loop** — the two things users touch every session.
5. **Backend tests do not run in CI.** The `backend` job in `.github/workflows/quality.yml` runs only `./scripts/verify-targets.sh`. Everything above can regress on `main` with a green check.

The revamp should be **targeted, not blanket**. Rewriting the 14 model-manager tests buys nothing. Rewriting the pipeline and harness tiers buys the product.

---

## 1. What I ran and read

| Item | Result |
| :--- | :--- |
| `cargo nextest run --lib --release` | **136 passed, 0 failed, 0 skipped**, 0.705 s (compile 2m08s) |
| Full integration suite | **not run** — models live at `/home/hypr4/models`, `VOX_MODELS_DIR` required |
| Integration files | 39 |
| Integration test functions | 172 total / 51 remote / **121 non-remote** |
| Unit test functions | 136 across 30 `src` files |
| `#[ignore]` markers | 4 |
| Raw `thread::sleep` / `sleep(Duration)` in tests | 65 |
| Silent `while rx.try_recv().is_ok() {}` drains | 10 |
| Swallowed worker panics (`let _ = ...join()`) | 8 |

Read in full: `testing-style-guide.md`, `test-engineer.md`, `qa-engineer.md`, `harness-spec.md`, `dictation-spec.md`, `events-spec.md`, `ownership-spec.md`, `vad-asr-pipeline-bugs-rca.md`, `integration-test-spec.md` §0.1–3, `seam_audit_report.md`, `mutation_report.md`.

---

## 2. Style guide audit

### 2.1 What is right

- §2 integration rule ("must route events through actual actor channels") is exactly correct.
- §3 ground-truth rule (Levenshtein ≥ 0.90, never single-keyword) is correct for accuracy tests.
- §5.2 worker teardown and panic capture is correct, and currently violated 8 times.
- §5.3 native-backend re-warm warning is a real, hard-won constraint. Keep it.
- §7.3 two-attempt rule is good discipline.

### 2.2 What is wrong

| # | Finding | Severity | Evidence |
| :--- | :--- | :--- | :--- |
| G1 | **No taxonomy beyond Unit/Integration.** Component, end-to-end, evaluation, and benchmark are not distinguished by *what they prove*. A "benchmark" and an "eval" both look like tests; a release gate cannot tell them apart. | High | §1 |
| G2 | **Header is file-level only.** It describes the file, not the test. A reader cannot tell what any individual function proves, or which failure it catches. This is the root cause of the `_matrix`/overstated-name problem. | Critical | §4; `dictation_window_test.rs:6-8` |
| G3 | **Headers assert metrics that are not asserted.** `dictation_window_test.rs:8` claims "Latency, Levenshtein ≥ 0.90"; the file contains no `elapsed()` and no Levenshtein call. Same in `passive_streaming_test.rs:8`, `ptt_window_modular_test.rs:8`, `transcript_to_llm_test.rs:8`. | Critical | grep: `elapsed()` appears in 3 files only; Levenshtein in 3 |
| G4 | **No rule for absence/suppression assertions** — the single most important class for a voice assistant. The guide says "verify worker panics" but never says how to prove *nothing happened*. | Critical | §5.1 covers timeouts only |
| G5 | **"20-word cap" is wrong.** The chunker uses a 22-word compound seam, a 28-word comma gate, and a 38-word hard ceiling split at 30. The header states a threshold the code does not implement. | High | `chunking_determinism_test.rs:9` |
| G6 | **No naming rule.** Result: `test_dictation_matrix`, `*_matrix`, `*_validation`, `test_` prefixes. Names describe shape, not behavior. | Medium | 172 test fns |
| G7 | **No traceability requirement.** No mapping from spec rule (e.g. `RULE-EVT-08A`) to test to mutant. Reviewers cannot detect a missing rule. | High | no such file |
| G8 | **No flake policy.** 65 raw sleeps, wall-clock latency assertions (`audio_capture_aec_test.rs` `<15ms`), `sleep(50ms)` then assert `Idle`. These fail on shared CI for reasons unrelated to correctness. | High | grep |
| G9 | **No tiering.** Model-loading tests, pure unit tests, and live-provider tests share one command. There is no way to run a fast, deterministic gate. | High | §1 |
| G10 | **The guide is 105 lines and unenforced.** Nothing in CI reads it. Docs do not fail builds; tests do. | Critical | `quality.yml:34-44` |

### 2.3 The header problem, stated plainly

The current header answers "what is this file?" It does not answer the only question a reviewer has: **"if this production logic broke, would this test fail, and how?"**

That question is the entire thesis of the `create-test` skill, and it is absent from the artifact the repository actually keeps. The skill's reasoning is discarded at the moment the test is committed. This is the largest structural defect in the whole system.

---

## 3. `create-test` skill audit

### 3.1 What is right (and better than most test guidance)

- The governing question (line 8–11) is correct and should survive verbatim.
- Phase 1 direction check ("if the function you call is the same function production calls to deliver a result, you are testing the consumer, not the pipeline") is the best single paragraph in the repo's process docs.
- Phase 2b upstream-bypass check is the correct falsification test.
- The "production reimplementation signal" (line 111) correctly identifies the most common way integration tests become theatre.
- Phase 3 negative-assertion mandate (line 126) correctly names the trap: "never use a short timeout as a proxy for nothing happened."

### 3.2 Where it fails

| # | Finding | Severity |
| :--- | :--- | :--- |
| S1 | **It produces no durable artifact.** The Phase 1 trace and Phase 2b table are described as "the contract" and "keep this table", but there is no file, schema, or link from the test to the table. The next agent cannot see it. `/mutate` cannot consume it. | Critical |
| S2 | **Nothing forces the reasoning to match the code.** A test can claim a mutant and never be checked against it. The skill has no output that a linter or reviewer can verify. | Critical |
| S3 | **"One test file per seam" is asserted, not derived.** In practice it produces grab-bag files (`dictation_window_test.rs`, 8 subtests, one of which claims to test paste and does not). The rule should be *one behavior per test function*, with files grouped by subsystem, not a quota. | High |
| S4 | **No naming rule**, so names drift to `_matrix`. | Medium |
| S5 | **No tiering or cost model.** It does not say where a test runs (default gate, model tier, nightly). This is why the suite has 4 orphan `#[ignore]` tests with no owner, no expiry, no nightly job. | High |
| S6 | **No "delete" phase.** Every audit produces additions. Nothing in the skill asks "which existing test does this replace, and does the old one now lie?" | Medium |
| S7 | **No subtest honesty check.** A subtest named "Paste" that never touches the clipboard passes review because the skill only audits the top-level seam. | High |

### 3.3 The fix, in one line

Make the skill emit a **persisted test intent block** above every test function, and make CI reject a block that does not match the body. The reasoning becomes the artifact; the artifact becomes checkable.

---

## 4. Suite findings, severity-ranked

### S1 — Critical (false confidence / no gate)

**F1. Backend tests are not in CI.**
`.github/workflows/quality.yml:34-44` runs only `verify-targets.sh`. A PR can break the pipeline, merge green, and ship. This invalidates every other quality claim in the repo.

**F2. `assert_channel_empty_after` cannot prove absence.**
`tests/common/harness.rs:295-307`:

```rust
std::thread::sleep(wait);
if let Ok(item) = rx.try_recv() { panic!(...) }
```

If the producer is merely slow (scheduler, model load, CI contention), the channel is empty at the sample instant and the test passes. If the producer is *early* by more than `wait`, the test fails spuriously. It proves neither direction. It is also blessed by `integration-test-spec.md §0.1.4`, so the defect is normative.

Correct primitive: **close the producer, then drain to `Disconnected`**, or insert a barrier/ack that guarantees the producer has finished, then assert the queue is empty. Absence is provable only after quiescence is proven, not after a timer.

**F3. Upstream bypass in `transcript_to_llm_test.rs`.**
The test calls `on_transcript_final` directly. If the STT actor never emitted a final transcript, the test still passes. This is the exact failure Phase 2b of `create-test` forbids, in a file whose header claims it tests the seam.

**F4. Names overstate coverage.**
`test_dictation_matrix`'s "Paste" subtest never tests paste, clipboard backup/restore, the 350 ms delay, Wayland fallback, 1200 ms auto-stop, or the recovery-cache IPC. The name is the only evidence a reviewer has, and it is false.

**F5. No per-test intent anywhere.** 0/39 files contain `PURPOSE`, `SCENARIO`, or `ASSERTS`.

### S2 — High (the bugs that actually ship)

**F6. Continuation merging is untested.** Zero references to `continuation_prefix` or `awaiting_continuation` anywhere in `tests/`. `RULE-EVT-08A` has no test.

**F7. Stale-cancel guards are untested.** Zero tests exercise cancel-then-new-turn ordering. This is a named RCA failure (stale cancel killing the next turn).

**F8. Duplicate-audio / duplicate-query filtering is untested.** No test for the dedupe path, the tool-loop cap of 5, or the 65% soft debounce.

**F9. `segmenter.rs` state machine is untested.** Only helpers are covered. `process_continuous_segmentation` — hysteresis, pre-roll, seam decisions — has no direct test. The RCA's pre-roll wipe and double-hysteresis bugs live here.

**F10. `stt/actor.rs` has zero tests.** Turn-id assignment and stream reset — the mechanism that prevents cross-turn transcript bleed — is unverified.

**F11. Harness modules have zero tests.** `harness/loop.rs`, `steps.rs`, `chassis.rs`, `session.rs`. This is the product's main loop.

**F12. Worker panics are swallowed 8 times** (`let _ = ...join()`). The style guide forbids it (§5.2). A dead worker looks like a passing test.

**F13. Silent channel drains, 10 sites.** `while rx.try_recv().is_ok() {}` hides unexpected messages, including duplicates — the exact bug class in the RCA.

**F14. Flaky by construction.** Wall-clock `<15ms` assertion; `sleep(50ms)` then assert `Idle`; 65 raw sleeps.

### S3 — Medium (hygiene, cost, drift)

**F15. Orphan `#[ignore]` tests.** Four, with no owner, expiry, or nightly job. Ignored tests rot silently.
**F16. Remote tests inflate the count.** 51 of 172 integration tests are WIP remote. They run in the default suite unless filtered. They should be feature-gated or nextest-filtered so the headline number means something.
**F17. Spec drift.** `integration-test-spec.md` is 2,520 lines and contradicts the style guide in places (§0.1.4 endorses the sleep-based absence helper). Two normative documents, one stale.
**F18. No mutation evidence for new tests.** `cargo-mutants` is not installed and not in CI. The Phase 2b tables, where they exist, were never executed.

---

## 5. Coverage vs. user impact

Priority is the paths a user hits in ~90% of sessions.

| Path | User impact | Current state | Verdict |
| :--- | :--- | :--- | :--- |
| Hold PTT → speak → release → transcript → LLM → TTS | Critical, every session | One golden path; dictation file claims more than it does | **Rebuild** |
| Passive VAD streaming → segment → final | Critical, hands-free mode | Golden path only; state machine untested | **Rebuild** |
| Barge-in / interrupt / stale cancel | Critical, feels broken when wrong | Tests exist but rely on fake absence | **Rebuild assertions, keep scenarios** |
| Harness loop (wake → turn → reply → idle) | Critical | Zero tests | **Build from scratch** |
| Continuation merging (multi-utterance turn) | High | Zero tests | **Build** |
| Tool loop (cap 5, dedupe, debounce) | High | Zero tests | **Build** |
| Clipboard paste / restore / auto-stop | High | Name-only coverage | **Build** |
| Settings persistence | Medium | 7 tests, real edge cases | Keep, add intent blocks |
| Personal memory | Medium | 9 tests, real edge cases | Keep, add intent blocks |
| Provider catalogues (TTS/STT) | Medium | 15 tests, strong | Keep, merge redundancy |
| Model manager | Medium | 14 tests, strong | Keep |
| Live provider contract (HTTP shape, retries, errors) | Medium | Untested without keys | **Build offline tier** |

**Answer to "is the suite only one golden happy path?"** No. That charge is true for the voice pipeline and the harness. It is false for memory, persistence, settings, and provider catalogues. A blanket rewrite would destroy real coverage and burn weeks. Target the top seven rows.

---

## 6. Cloud keys, ignored tests, cloud models — direct answers

**Do we need a cloud provider API key?**
**No, not for the default suite.** Keys buy live contract verification, and they cost determinism, money, rate-limit flakes, secret handling in CI, and reproducibility for contributors. They also hide request-shaping bugs, because a live 200 response does not prove the JSON body was correct.

What to do instead, in order:
1. **Offline transport tier (default).** Intercept at the HTTP boundary. Assert method, URL, headers, body shape, auth header presence, timeout, retry count, and error mapping for 400/401/429/500. This catches ~90% of integration bugs and runs in milliseconds.
2. **Recorded cassettes.** One real response per provider, replayed. Catches schema drift in the parser.
3. **Live contract tier (`#[ignore]`, nightly, one request per provider).** Only if a key exists. Never in the default loop. Failure pages the owner; it does not block a PR.

**Should `#[ignore]` tests run in regular runs?**
**No.** But "ignored" must mean "scheduled elsewhere", not "forgotten". Require: owner, reason, expiry date, and a nightly job that runs them. A test ignored with no job is deleted, not ignored.

**Should a cloud model join the suite?**
Not as a correctness dependency. Only as the nightly contract tier above. The default gate must pass with no network.

---

## 7. Proposed replacement: the test intent block

Per **test function**, not per file. File headers stay, reduced to ownership and run command.

```rust
/// PURPOSE:  The production defect this test exists to catch, in one sentence.
///           Must name a failure mode, not a feature.
/// SCENARIO: Preconditions + the exact user or system action.
/// ASSERTS:  The observable output and the exact invariant. Every claim here
///           must map to a line in the body.
/// CLASS:    happy-path | edge-case | error-path | suppression | regression | contract
/// REQUIRES: models / env / network / time budget. "none" if none.
/// MUTANT:   The one-line production change that must turn this test red.
```

Rules that make it enforceable:

1. `PURPOSE` may not contain "works", "functions correctly", or "is handled".
2. `ASSERTS` must be falsifiable without reading the body — a reviewer can name the assertion.
3. `MUTANT` is mandatory for `suppression`, `edge-case`, and `regression` classes. It is the `/mutate` seed.
4. If the block names a metric word (`latency`, `accuracy`, `≥ 0.90`, `N-word`), the body must contain the matching assertion token. A grep-based linter enforces this. This kills G3 and G5 permanently.
5. A test with no `PURPOSE` fails CI. No exceptions, no grandfathering at file level.

### Naming

- `snake_case`, no `test_` prefix.
- Pattern: `<subject>_<scenario>_<outcome>`.
- One behavior per function.
- Banned: `_matrix`, `_validation`, `_works`, `_basic`, `_sanity`.
- Subtests are named for what they **assert**, not what they touch.

Examples:
- `ptt_release_with_partial_transcript_emits_single_final`
- `stale_cancel_does_not_kill_next_turn`
- `continuation_merges_second_utterance_into_one_request`
- `barge_in_suppresses_pending_tts_chunk`
- `paste_restores_previous_clipboard_after_injection`

---

## 8. Guardrails (proposed, not implemented)

| # | Guardrail | Cost | Kills |
| :--- | :--- | :--- | :--- |
| 1 | **Backend test job in CI**: `cargo nextest run --release --test-threads=1` on the deterministic tier. | ~1 runner + cache | F1 |
| 2 | **`scripts/lint-test-headers.sh`**: every `#[test]`/`#[tokio::test]` has `PURPOSE`/`ASSERTS`/`CLASS`; metric words require matching assertions. | ~80 lines | G2, G3, G5, F5 |
| 3 | **Deterministic absence primitive** replacing `assert_channel_empty_after`: close producer → drain to `Disconnected`. Delete the sleep version and its spec endorsement. | ~40 lines + call-site edits | F2, F13 |
| 4 | **`join().expect()` everywhere**; a grep gate forbids `let _ = ...join()`. | mechanical | F12 |
| 5 | **nextest profiles**: `default` (no models, no network, <60 s), `models` (explicit), `nightly` (live providers). | config only | G9, F15 |
| 6 | **Traceability table** `docs/tests/traceability.md`: spec rule → test → mutant. Generated from the intent blocks, not hand-written. | script | G7 |
| 7 | **Nightly `cargo-mutants`** on changed files only. | ~15 min/night | F18 |
| 8 | **Ban raw `sleep` in tests** via clippy `disallowed_methods` or grep gate. Allow only in `#[ignore]` timing probes. | config | F14 |
| 9 | **Feature-gate remote tests** so 51 WIP tests stop inflating the suite. | one `cfg` | F16 |

---

## 9. Proposed revamp sequence (after approval)

**Phase 0 — Truth.** Add the intent block + linter. Do not change behavior. This makes every later claim checkable and immediately exposes the false headers.

**Phase 1 — Assertion primitive.** Replace the absence helper, fix the 10 silent drains and 8 swallowed joins. Now negative tests can be trusted.

**Phase 2 — Voice pipeline.** Rebuild in this order: `stt/actor` turn identity → `segmenter` state machine → continuation merge → stale cancel → duplicate filter/tool cap → barge-in suppression. Each gets a `MUTANT` and a mutation run before merge.

**Phase 3 — Harness.** First test that ever covers `harness/loop.rs`: wake → turn → reply → idle, plus the cancel and error exits.

**Phase 4 — Dictation output.** Paste, clipboard restore, delays, Wayland fallback, auto-stop, recovery cache. Delete the lying subtest name.

**Phase 5 — Prune and tier.** Merge redundant catalogue tests, delete tests with no `PURPOSE` that duplicate a stronger one, gate remote, wire the nightly tier.

Not in scope: rewriting memory/persistence/model-manager tests beyond adding intent blocks.

---

## 10. Questions requiring your decision

1. **Batch order** — start with Phase 0 (headers + linter, no behavior change) or Phase 2 (pipeline tests, highest product risk)? Recommendation: Phase 0, because without it the rewrites repeat the current problem.
2. **CI runner for model tests** — do you want the deterministic tier in CI now, or after Phase 1? Recommendation: now, `--lib` + non-model integration only, so the gate exists before the rewrite.
3. **Live provider tier** — is there a budget/owner for a nightly contract run, or do we ship offline-only for now? Recommendation: offline-only until the offline tier exists; add nightly later.
4. **Remote tests** — feature-gate now, or leave them running? Recommendation: gate now.
