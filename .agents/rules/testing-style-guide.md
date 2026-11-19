---
trigger: manual
description: Vox testing standard for unit, component, integration, end-to-end, benchmark, and evaluation suites. Defines the governing test question, seam boundaries, falsifiable assertions, the no-false-green rule, project paths, and the local execution tiers.
---

# Vox — Testing, Evaluation & Benchmark Standards

Read before authoring, revamping, or reviewing any test. This document is binding. A test that does not follow it is not complete, even if it passes.

The governing question, from `/create-test`:

> If the production logic between this test's entry point and its observable output silently broke — dropped data, ignored a flag, mis-routed a message, or failed to adapt to real-world skew — would this test fail?

If the answer is not an immediate yes with a named assertion, the test is not a test. It is decoration.

---

## 1. Taxonomy

Levels are defined by what they prove, not by where the file lives.

| Level | Proves | Entry point |
| :--- | :--- | :--- |
| **Unit** | One algorithm, state machine, or boundary arithmetic is correct in isolation. | The function itself |
| **Component** | One production component behaves correctly through its own public API, with no upstream actor. | The component's public interface |
| **Integration (seam)** | A real upstream trigger drives a real handoff across a real boundary, and the observable exit is correct. | The production trigger |
| **End-to-end** | The user-visible loop works: input → pipeline → output → idle. | The harness entry |
| **Evaluation** | Model output quality meets a threshold. Never inside an integration test. | Project eval harness |
| **Benchmark** | Latency or throughput meets a budget. | Benchmark harness |

Rules:

1. A test lives at the lowest level that can still reach its real trigger. If calling a leaf is correct, it is a unit test — do not fake a pipeline around it.
2. If the behavior is driven in production by an actor, channel, event, request, or user action, a unit test of the downstream leaf is not a substitute. That is a false green.
3. Never assert model quality inside an integration test. Integration asserts wiring: right payload, right channel, correct order, non-empty output, proper exit condition.
4. A test that only asserts a row count or a callback count without proving the state transition is a storage or plumbing contract test, not a user-facing behavior test.

---

## 2. File Layout: One Binary Per Seam

Every integration test file compiles and links as its own binary. Compilation is a dominant cost, so file count is a budget, not a preference.

**Rule:** one binary per production seam, containing 10–20 subtests.

- A subtest is a test function. It covers one behavior, one scenario, one invariant.
- A new behavior does not get a new file. It gets a new subtest in the seam's file.
- Do not split a seam across files because the file is long. Split only when the seam genuinely splits into two production boundaries.
- Do not merge two seams into one binary because their tests look similar. The seam is the unit of ownership.

A seam file that cannot reach 10 subtests is either under-tested or not a seam. Investigate before accepting it.

---

## 3. Intent Block — Mandatory in seam binaries

Every test function in a seam binary carries an intent block. It is the durable output of `/create-test` and what a reviewer reads instead of guessing.

```text
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
4. If the block names a metric, threshold, or count, the body must contain that assertion.
5. `MUTANT` is the exact seed for mutation testing. It must be a concrete one-line edit, not a description.

The old file header is retained only where it documents the seam boundary, not the business logic:

```text
//! =====================================================================================
//! <file> — <seam name and the production boundary it covers>
//! =====================================================================================
//! Category  : Integration (Seam) | Component | E2E | Evaluation | Benchmark
//! Component : <production modules in the path>
//! Binary    : <test binary name>
//! Prereqs   : <models / env / services, or "none">
//! Run       : <exact command>
//! =====================================================================================
```

---

## 4. Naming

- `snake_case`, no `test_` prefix.
- Pattern: `<subject>_<scenario>_<outcome>`.
- One behavior per function.
- Banned: `_matrix`, `_validation`, `_works`, `_basic`, `_sanity`, `_check`, `test_*`.
- A subtest is named for what it asserts, not what it touches.

Examples:
- `queue_released_after_timeout_emits_single_final_event`
- `stale_cancel_does_not_kill_next_turn`
- `continuation_merges_second_utterance_into_one_request`
- `route_drops_duplicate_message_before_dispatch`

If the name needs "and", it is two tests.

---

## 5. Assertion Rules

### 5.1 Absence and suppression

Absence is proven only after quiescence is proven. A timer is not proof, and a single `try_recv` is not proof.

Preferred forms, in order:

1. **Capture-channel interception** — attach a sink before the boundary and assert nothing arrived.
2. **Producer closed** — drop the sender, then drain the receiver until `Disconnected`.
3. **Barrier or acknowledgement** — a sentinel that proves the producer finished, then assert the queue is empty.
4. **Storage count** — `SELECT COUNT(*) == 0` after the transaction commits.

Banned:
- `sleep(N)` followed by `try_recv()`.
- Any helper whose implementation is the above.
- "No event within 500 ms" as a negative assertion.

A suppression test that cannot name its quiescence mechanism is not written yet.

### 5.2 Error paths

Every fallible seam asserts both directions: the failure is surfaced, and the happy path still works. An error test that only asserts `is_err()` does not prove the error is the right one. Assert the variant or the message.

### 5.3 No swallowed failures

- `let _ = handle.join()` is banned. Use `.join().expect("worker panicked")`.
- `while rx.try_recv().is_ok() {}` is banned. Drain explicitly and assert on what was drained, or give each scenario its own actors.
- A background worker that dies must fail the test, not be silently absent from the assertions.

### 5.4 Time

> 🛑 **MANDATORY PER-STAGE FIRST-OUTPUT CEILING: 5 SECONDS.**
> This is a **time-to-first-output** budget, not a total test duration. A model-backed stage that produces no first output within 5 seconds has failed; the test must fail immediately and name the stage. A full E2E pass may take longer once all stages have produced their first output.

- **LLM:** first streamed token within $5\text{s}$ of dispatch (TTFT). Assert the first-token event, not the full answer.
- **STT:** first partial transcript within $5\text{s}$ of the first bound audio chunk (or the final transcript when partials are disabled).
- **TTS:** first non-silent audio chunk within $5\text{s}$ of clause dispatch.
- Each stage with a generator must carry one bounded `wait_for_first_*` assertion with the 5s ceiling. Produce the event through the production seam; never synthesize it in the test.
- Every async test still wraps its body in an overall timeout so a deadlock fails rather than hangs. The overall timeout may exceed 5s for full E2E scenes; it must not be used as a substitute for the per-stage first-output assertion.
- Synchronous channel loops use a deadline or timeout primitive.
- No wall-clock **total duration** assertion belongs in the default tier. The per-stage first-output ceilings above are contract assertions, not scheduler benchmarks. Latency measurement belongs in benchmarks or an explicitly marked benchmark subtest.
- `sleep` may not be used to reach a state. Wait on the state transition or the event.

### 5.5 Ground truth

Accuracy assertions must use a real, normalized, agreed metric against labelled fixtures. Single-keyword presence is banned.

---

## 6. Shared Helpers

- Anything used by two tests lives in a shared test-support module or fixture directory.
- A helper is valid only if it could be replaced by a direct production call with no production change. Otherwise it is production logic hiding in the test.
- A helper that reimplements a production predicate is a false green: test and production can drift, and the test still passes. Extract the predicate to production and call it.
- Duplicated setup between seam files is deleted, not tolerated.
- Every helper carries a one-line comment stating what production contract it stands in for.

---

## 7. Execution

### 7.1 Local execution is the default

Production-grade tests must be run locally on demand. CI is a compile guard, not an execution proof.

Consequences:
1. A green CI check does not mean the suite passes.
2. Every agent that changes test-covered code runs the affected seam locally and states the command and result.
3. A seam not run locally in the current session is reported as unverified, never as passing.

### 7.2 Commands

Vox uses `cargo-nextest`. Tests run sequentially in release mode. Set the model root
and the thread counts explicitly.

```bash
cd app/src-tauri
export VOX_MODELS_DIR="${VOX_MODELS_DIR:-$HOME/.vox/models}"
export RAYON_NUM_THREADS=$(nproc) OMP_NUM_THREADS=$(nproc)

# One seam
cargo nextest run --test <seam> --release --test-threads=1 --no-fail-fast

# Full local suite
cargo nextest run --release --test-threads=1 --no-fail-fast

# Isolated failure
cargo nextest run --test <seam> -E 'test(<subtest>)' --release --nocapture --test-threads=1

# External-dependency tests (`#[ignore]`) only with explicit user approval
cargo nextest run -- --ignored
```

Use the tier wrapper in the crate scripts directory with tiers `default`, `models`, `all`, `remote`, and `seam <binary>`. The default tier is the allowlist of seams that need neither model weights nor a full voice rig; maintain that allowlist in the test-support tree, not in this standard.

### 7.3 Vox project profile

| Item | Stable location |
| :--- | :--- |
| Integration tests | crate `tests/` tree, one binary per seam |
| Shared harness and assets | test-support subtree under `tests/` |
| Unit tests | beside production code in the crate `src/` tree |
| Benchmarks | crate benchmarks directory |
| Model root | `$HOME/.vox/models` (override with `VOX_MODELS_DIR`) |
| All test reports | `docs/tests/` only |
| Behavioral contracts | `docs/specs/` |
| Active plan and work log | `docs/plans/<curr_phase>/` |

### 7.4 `docs/tests/` layout — closed set

All test reports live in `docs/tests/` and nowhere else. The directory holds exactly one report for each of these stable purposes: benchmark results, the rubric test review, production findings, the mutation campaign, and the suite baseline. Any other file there is legacy and is deleted, not archived. Do not encode the phase or a transient file name in a rule.

Do not infer timer horizons (AEC3 ~6.0 s, early-utterance 1.25 s, hangover) from memory. No separate timer-audit document exists. Read the production constants in the VAD and audio modules first.

**CI is compile-only by deliberate choice.** Never report a suite as passing because
CI is green. Report the local command and its result, or report `unverified`.

---

## 8. External Dependencies & Ignored Tests

> 🛑 **ZERO UNGROUNDED IGNORED TESTS POLICY:**
> **No test may be marked `#[ignore]` unless it strictly requires an external cloud provider API key (`GEMINI_API_KEY`, `DEEPGRAM_API_KEY`) or an external remote GPU server. Offline transports, local models (GGUF, Kokoro, Silero, Nemotron), and loopback network interfaces (WebSocket, localhost UDS) MUST NEVER be ignored. A local test that fails is a defect or broken contract—it must be fixed or pruned, never swept under an `#[ignore]` marker.**

- Cloud provider keys are not required by the default suite. Live keys buy contract verification; they cost determinism, money, rate-limit flakes, and secret handling.
- Offline transport tests intercept at the HTTP boundary and assert method, URL, headers, body shape, auth presence, timeout, retry count, and error mapping.
- Recorded cassettes cover parser drift.
- Live provider tests requiring external cloud credentials or remote hardware are `#[ignore]` or framework equivalents, run manually, and must carry owner, reason, and expiry date.
- An ignored test with no owner, no reason, and no expiry is deleted, not ignored.

---

## 9. Mutation

- `MUTANT` in the intent block is the seed. It must be a concrete one-line edit to production code.
- Mutation runs are batched per campaign, not per test.
- **Remote Build Server Priority:** When remote credentials exist outside version control, execute mutation runs on the remote server using the generalized mutation runner in local or remote mode. The remote server has ample cores and zero local cargo lock contention. Never name credential files or runner scripts in this standard.
- **Local Fallback (Orthogonal Batching):** If remote is unavailable, never run sequential 16-recompile loops of individual mutants on the primary working tree. Batch orthogonal, non-overlapping mutants across separate modules simultaneously and verify failures in a single test binary run.
- **Why Worktrees Fail with Cargo:** Cargo acquires exclusive file locks on the target directory; multiple worktrees sharing `target/` block each other and corrupt incremental compiler cache.
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

1. Do not patch production to make the test pass.
2. Do not weaken the assertion.
3. Record the defect in the project's findings artifact with test name, symptom, evidence, and suspected `file:line`.
4. Continue with the current seam.

A red test that found a real bug is a successful outcome. Hiding it is the failure.

---

## 12. Blocker Protocol

### 12.1 Single-subtest isolation

When a test fails, isolate immediately with the project's runner and a single-subtest filter. Never loop the whole suite while debugging a local failure.

### 12.2 Instrumented tracing

Place explicit markers before and after locks, channel operations, and async boundaries.

### 12.3 Two-attempt rule

If a failure persists after two targeted attempts without a confirmed new hypothesis, stop and report:

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
- [ ] Helpers live in a shared test-support area and could be replaced by production calls.
- [ ] The name states the behavior, not the file's shape.
- [ ] `MUTANT` is present for suppression, edge-case, and regression classes.
- [ ] The seam binary was run locally; the command and result are reported.
