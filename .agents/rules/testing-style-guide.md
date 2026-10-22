---
trigger: manual
description: Test taxonomy, seam-level file layout, mandatory per-test intent blocks, assertion rules, execution tiers, and the deliberate no-CI test policy for Vox (`app/src-tauri/tests/`, `benches/`, `evals/`).
---

# Vox — Testing, Evaluation & Benchmark Standards

Read before authoring, revamping, or reviewing any test. This document is binding. A test that does not follow it is not complete, even if it passes.

The governing question, from `/create-test`:

> If the production logic between this test's entry point and its observable output silently broke — dropped data, ignored a flag, mis-routed a message — would this test fail?

If the answer is not an immediate yes with a named assertion, the test is not a test. It is decoration.

---

## 1. Taxonomy

Levels are defined by **what they prove**, not by where the file lives.

| Level | Proves | Location | Entry point |
| :--- | :--- | :--- | :--- |
| **Unit** | One algorithm, state machine, or boundary arithmetic is correct in isolation. | `#[cfg(test)] mod tests` in the owning `src/` file | The function itself |
| **Component** | One production component behaves correctly through its own public API, with no upstream actor. | `#[cfg(test)]` or a seam binary | The component's public interface |
| **Integration (seam)** | A real upstream trigger drives a real handoff across a real boundary, and the observable exit is correct. | `tests/<seam>_test.rs` | The production trigger |
| **End-to-end** | The user-visible loop works: input → pipeline → output → idle. | `tests/harness_loop_test.rs` | The harness entry |
| **Evaluation** | Model output quality meets a threshold. Never inside an integration test. | `evals/<capability>/` | Crate API + models + dataset |
| **Benchmark** | Latency or throughput meets a budget. | `benches/<feature>_bench.rs` | `harness = false` `main()` |

Rules:

1. A test lives at the **lowest** level that can still reach its real trigger. If calling a leaf is correct, it is a unit test — do not fake a pipeline around it.
2. If the behavior is driven in production by an actor, channel, event, or user action, a unit test of the downstream leaf is **not** a substitute. That is a false green.
3. **Never assert model quality inside an integration test.** Integration asserts wiring: right model, right payload, right channel, non-empty output, correct order. Quality belongs in `evals/`.
4. A test that calls a persistence leaf, constructs no state, and asserts a row count is a **storage contract** test. File it in the storage seam, not a feature seam.

---

## 2. File Layout: One Binary Per Seam

Every integration test file compiles and links as its own binary. Compilation is the dominant cost of this suite, so file count is a budget, not a preference.

**Rule:** one binary per production seam, containing **10–20 subtests**.

- A subtest is a test function. It covers one behavior, one scenario, one invariant.
- A new behavior does **not** get a new file. It gets a new subtest in the seam's file.
- Do not split a seam across files because the file is long. Split only when the seam genuinely splits into two production boundaries.
- Do not merge two seams into one binary because their tests look similar. The seam is the unit of ownership.

Why: 39 test binaries link 39 times. Folding them to ~19 halves the link cost and puts related scenarios in one compiled context. It also stops the `*_matrix` grab-bag: one file, one seam, many named behaviors.

A seam file that cannot reach 10 subtests is either under-tested or not a seam. Investigate before accepting it.

---

## 3. Mandatory Intent Block

Every test function carries this block. It is the durable output of `/create-test`, and it is what a reviewer reads instead of guessing.

```rust
/// PURPOSE:  The production defect this test exists to catch, in one sentence.
///           Name a failure mode, not a feature.
/// SCENARIO: Preconditions and the exact user or system action.
/// ASSERTS:  The observable exit and the exact invariant. Every claim here must
///           map to an assertion in the body.
/// CLASS:    happy-path | edge-case | error-path | suppression | regression | contract
/// REQUIRES: models, env vars, network, or time budget. Write "none" if none.
/// MUTANT:   The one-line production change that must turn this test red.
```

Binding rules:

1. `PURPOSE` must state what breaks in production if the test is absent. "Verifies X works" is banned.
2. `ASSERTS` must be falsifiable without reading the body. A reviewer can name the assertion line it maps to.
3. `CLASS` is mandatory. `suppression`, `edge-case`, and `regression` also require `MUTANT`.
4. If the block names a metric, threshold, or count, the body must contain that assertion. A header that claims `Levenshtein ≥ 0.90` while the file never computes Levenshtein is a false statement, not documentation.
5. `MUTANT` is the exact seed for `cargo-mutants`. It must be a concrete one-line edit, not a description.

The old file header is retained, reduced to facts that cannot drift:

```rust
//! ============================================================================
//! <file> — <seam name and the production boundary it covers>
//! ============================================================================
//! Category  : Integration (Seam N) | Component | E2E | Evaluation | Benchmark
//! Component : <production modules in the path>
//! Binary    : <test binary name>
//! Prereqs   : <models / env / services, or "none">
//! Run       : <exact command>
//! ============================================================================
```

The `Metrics` line is gone. Metrics are asserted per test, in `ASSERTS`.

---

## 4. Naming

- `snake_case`, no `test_` prefix.
- Pattern: `<subject>_<scenario>_<outcome>`.
- One behavior per function.
- Banned: `_matrix`, `_validation`, `_works`, `_basic`, `_sanity`, `_check`, `test_*`.
- A subtest is named for what it **asserts**, not what it touches. A function named `paste_*` must touch the clipboard.

Examples:

- `ptt_release_with_partial_transcript_emits_single_final`
- `stale_cancel_does_not_kill_next_turn`
- `continuation_merges_second_utterance_into_one_request`
- `barge_in_suppresses_pending_tts_chunk`
- `paste_restores_previous_clipboard_after_injection`

If the name needs "and", it is two tests.

---

## 5. Assertion Rules

### 5.1 Absence and suppression

Absence is proven only after **quiescence is proven**. A timer is not proof, and a single `try_recv` is not proof.

Preferred forms, in order:

1. **Capture-channel interception** — attach a sink before the boundary and assert nothing arrived.
2. **Producer closed** — drop the sender, then drain the receiver until `Disconnected`. The channel cannot produce another item after disconnection.
3. **Barrier or acknowledgement** — a sentinel that proves the producer finished the work, then assert the queue is empty.
4. **Storage count** — `SELECT COUNT(*) == 0` after the transaction commits.

Banned:

- `sleep(N)` followed by `try_recv()`.
- Any helper whose implementation is the above. `assert_channel_empty_after` is deleted.
- "No event within 500 ms" as a negative assertion.

A suppression test that cannot name its quiescence mechanism is not written yet.

### 5.2 Error paths

Every fallible seam asserts both directions: the failure is surfaced, and the happy path still works. An error test that only asserts `is_err()` does not prove the error is the right one. Assert the variant or the message.

### 5.3 No swallowed failures

- `let _ = handle.join()` is banned. Use `.join().expect("worker panicked")`.
- `while rx.try_recv().is_ok() {}` is banned. Drain explicitly and assert on what was drained, or give each scenario its own actors.
- A background worker that dies must fail the test, not be silently absent from the assertions.

### 5.4 Time

- Every `#[tokio::test]` wraps its body in `tokio::time::timeout`. A deadlock must fail, not hang.
- Synchronous channel loops use `recv_timeout` or an explicit `Instant` deadline.
- **No wall-clock latency assertion in the default tier.** `< 15 ms` on a shared machine measures the scheduler, not the product. Latency belongs in `benches/` or an explicitly marked benchmark subtest.
- `sleep` may not be used to reach a state. Wait on the state transition or the event.

### 5.5 Ground truth

Accuracy assertions use normalized Levenshtein ≥ 0.90 or WER ≤ 0.10 against labelled fixtures. Single-keyword presence is banned.

---

## 6. Shared Helpers

- Anything used by two tests lives in `tests/common/`.
- A helper is valid only if it could be replaced by a direct production call with no production change. Otherwise it is production logic hiding in the test.
- A helper that reimplements a production predicate is a false green: test and production can drift, and the test still passes. Extract the predicate to production and call it.
- Duplicated setup between seam files is deleted, not tolerated. If two seams need the same fixture, the fixture moves to `tests/common/`.
- Every helper carries a one-line comment stating what production contract it stands in for.

---

## 7. Execution

### 7.1 CI is compile-only, by deliberate choice

**Vox does not run its test suite in CI.** The `backend` job in `.github/workflows/quality.yml` runs the dual-target compile gate only. The frontend job runs lint, build, and its own unit tests.

This is intentional. Model weights, audio devices, and long-running pipelines make a reliable hosted test job expensive and flaky. Test execution is a **local, on-demand** action.

Consequences, which are binding:

1. Nobody adds a CI test job without an explicit decision. A green CI check does not mean the suite passes.
2. Every agent that changes test-covered code runs the affected seam locally and states the command and result.
3. A seam that has not been run locally in this session is reported as **unverified**, never as passing.

### 7.2 Commands

All runs are sequential and in release mode:

```bash
cd app/src-tauri
VOX_MODELS_DIR=/path/to/models cargo nextest run --test <seam> --release --test-threads=1 --no-fail-fast
```

Full local suite:

```bash
RAYON_NUM_THREADS=$(nproc) OMP_NUM_THREADS=$(nproc) \
  cargo nextest run --release --test-threads=1 --no-fail-fast
```

Isolated failure:

```bash
cargo nextest run --test <seam> -E 'test(<subtest>)' --release --nocapture --test-threads=1
```

`scripts/test.sh <tier>` is the single local entry point. Tiers: `default` (no models), `models`, `all`.

### 7.3 Tiers

| Tier | Contents | Network | Models | Budget |
| :--- | :--- | :--- | :--- | :--- |
| `default` | unit + component + structural seam tests | no | no | < 60 s |
| `models` | seams that load ONNX/GGUF weights | no | yes | minutes |
| `all` | everything, sequentially | no | yes | long |

External providers never enter `default` or `models`. See §8.

### 7.4 Batch compilation

One compile per seam, not per edit. Make every change in the seam, then run its binary once. Never loop the whole suite while iterating on one seam.

---

## 8. External Dependencies

- Cloud provider keys are **not** required by the default suite. Live keys buy contract verification; they cost determinism, money, rate-limit flakes, and secret handling.
- Offline transport tests intercept at the HTTP boundary and assert method, URL, headers, body shape, auth presence, timeout, retry count, and error mapping for 400/401/429/500.
- Recorded cassettes cover parser drift.
- Live provider tests are `#[ignore]`, run manually, one request per provider, and must carry:
  - the owner,
  - the exact assertions that need the live dependency,
  - an expiry date.
- An `#[ignore]` with no owner, no reason, and no expiry is deleted, not ignored.

---

## 9. Mutation

- `MUTANT` in the intent block is the seed. It must be a concrete one-line edit to production code.
- Mutation runs are **batched**. Do not run `cargo-mutants` after every test. Run it once per revamp campaign, over the changed files, and record the result in `docs/tests/mutation_report.md`.
- A test whose predicted mutant survives is not done. Either the test is weak or the mutant was wrong; resolve before merge.
- Production code is reverted after every mutation run. A mutant must never be committed.

---

## 10. Fold, Prune, Rename

A revamp is not additive. For every new or strengthened test, decide the fate of the old one:

| Situation | Action |
| :--- | :--- |
| Old test asserts a subset of the new one | **Fold** into the new subtest, delete the old function |
| Old test asserts the same thing through a weaker seam | **Prune** |
| Old test name overstates what it does | **Rename** to the real behavior, or fix the body |
| Old test passes if production is deleted | **Delete**, and log the finding |
| Old test covers a behavior no other test covers | **Keep**, add the intent block |

No test survives on the grounds that it is already passing. Passing is not coverage.

---

## 11. Findings: Do Not Fix Production

When a test exposes a production defect:

1. Do **not** patch production to make the test pass.
2. Do **not** weaken the assertion.
3. Record it in `docs/plans/phase13/test-findings.md`:

```
## F-<n> — <one line>
Seam: <slice>
Test: <binary>::<subtest>
Symptom: <what was observed>
Evidence: <assertion output or log line>
Suspected cause: <file:line, or "unknown">
Impact: <user-visible effect>
Status: open — not fixed
```

4. Continue with the current seam.

A red test that found a real bug is a successful outcome. Hiding it is the failure.

---

## 12. Blocker Protocol

### 12.1 Single-subtest isolation

When a test fails, isolate immediately:

```bash
cargo nextest run --test <seam> -E 'test(<subtest>)' --release --nocapture --test-threads=1
```

Never loop the whole suite while debugging a local failure.

### 12.2 Instrumented tracing

Place explicit markers before and after locks, channel operations, and async boundaries:

```rust
eprintln!("[checkpoint X] ...");
```

### 12.3 Two-attempt rule

If a failure persists after **2 targeted attempts** without a confirmed new hypothesis, stop and report:

1. exact test and failing line,
2. logs and checkpoints,
3. both hypotheses and their results,
4. the blocker, missing dependency, or deadlock.

Never mock, stub, or delete an assertion to manufacture a pass.

---

## 13. Model Lifecycle Constraints

- Do not repeatedly initialise and tear down Sherpa-ONNX, llama.cpp, or ONNX Runtime in one process. Re-warm causes memory corruption and segfaults. Group scenarios that need the same backend into one test binary and one initialisation.
- Resampling must use valid interpolation or native-rate fixtures. Stride slicing is banned.
- Save every worker `JoinHandle` and join it on teardown with `.expect(...)`.

---

## 14. Checklist Before a Test Is Called Done

- [ ] It is at the lowest level that can still reach the real trigger.
- [ ] The entry point is the upstream trigger, not the output sink.
- [ ] The intent block is present and every `ASSERTS` claim maps to a line.
- [ ] Every suppression path has an absence test with a named quiescence mechanism.
- [ ] No swallowed join, no silent drain, no bare sleep, no wall-clock latency assert.
- [ ] Helpers live in `tests/common/` and could be replaced by production calls.
- [ ] The name states the behavior, not the file's shape.
- [ ] `MUTANT` is present for suppression, edge-case, and regression classes.
- [ ] The seam binary was run locally; the command and result are reported.
