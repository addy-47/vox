---
name: create-test
description: Pre-implementation reasoning protocol for constructing tests that catch real production bugs. Stack- and project-agnostic. Enforces black-box scene contracts, environmental skew, temporal horizon coverage, and wiring parity before a line of test code is written. Trigger on "write a test for", "create test case", "design integration test", or "how should we test this component". Precedes /test and /mutate.
---

A good test is a thin wire connecting a real production entry point to a real observable output, under realistic environmental conditions, such that if anything on that wire broke — dropped data, wrong routing, skipped state transition, uncalibrated adaptation, or premature timeout — the test fails. A good test contains no control-flow logic of its own. It wires up production components, invokes the entry point, and reads the output.

**The governing question:**
If the production logic or handoff between this test's entry point and its observable output silently broke — dropped data, ignored a flag, mis-routed a message, or failed to adapt to real-world skew — would this test fail?

Hold this question throughout every phase. It is the single standard a test must meet. This skill is stack-agnostic; adapt file layout, test-runner invocation, and language idioms to the codebase at hand.

---

## Phase 0 — Classify & Define Black-Box Scene Contract

Before reading the internal implementation of the component under test, classify the test level and define the black-box contract from specifications or user-visible behavior. Reading implementation code first causes white-box anchoring (calibrating test inputs to match hardcoded internal defaults).

State which level this is and the one-sentence reason:
- **Unit test**: pure algorithmic logic or isolated state machine with no upstream producer and no cross-boundary handoff. Calling the function directly is the correct entry point.
- **Integration (seam) test**: behavior driven in production by an upstream actor, queue, channel, event router, request, or user action — not by calling a downstream handler directly.
- **End-to-end (scene) test**: verifies a complete user-visible workflow from initial external trigger through all active states to final idle quiescence.

If difficulty reaching the correct integration boundary tempts a switch to a simpler, more callable leaf component: that difficulty is the finding. Do not relocate the test to a leaf.

Write the Black-Box Scene Contract before reading implementation code:

Scene / Behavior Under Test:
[The user-visible or cross-component behavior being verified — not a function name]

Black-Box Entry Trigger:
[The external stimulus that initiates this scene in production]

Expected State Trajectory:
[Initial state] -> [Active / intermediate states] -> [Terminal quiescent state]

Environmental & Temporal Realism Requirements:
Parameter / Skew Independence:
[What real-world variance, offset, delay, or unaligned input must be injected so the test does not merely mirror the SUT's default constants?]

Temporal Horizon:
[If the SUT has warm-up windows, debounce periods, decay timers, heartbeats, or timeouts, how long must the test run to cross those horizons?]

Concurrency / Contention:
[What overlapping event, cancellation, or mid-flight state change must be present if testing asynchronous coordination?]

Observable Terminal Exit:
[What external side effect, emitted event, or persisted state proves the scene completed all the way to quiescence — not stopping halfway at an intermediate stage?]

**Anti-Calibration Principle:** Never override production default configuration values in a test just to make a test fixture pass, and never calibrate test inputs to match the SUT's hardcoded internal defaults. If a system claims to adapt, debounce, or recover dynamically, the test must inject uncalibrated skew that forces that adaptation to work.

---

## Phase 1 — Production Path & Wiring Parity Trace

Now read the actual source code and trace the real production path before touching a compiler or test runner:

SUT (System Under Test):
[The connected production behavior being verified]

Production Wiring Reference:
[The production bootstrap / supervisor function that wires these components together]

Production Entry Seam:
[The real boundary where this behavior enters production and where production-owned state begins — never an internal callback or downstream handler invoked directly]

Direction & Bypass Check:
Is the Production Entry Seam the upstream trigger of this behavior, or is it an internal handler / output consumer?

If production routes this trigger through a central dispatcher, event bus, or actor channel, does the test enter before that dispatcher rather than calling the handler function directly?

Production Path:
[upstream trigger: user action / API call / emitted event / stream input]
-> [ingestion producer / dispatcher / router]
-> [production-owned buffer / queue / channel / state]
-> [worker / processor / state machine]
-> [downstream sink / consumer]
-> [observable terminal exit: event emitted, state persisted, side effect]

Wiring Parity Check:
Are all downstream channels, callbacks, and feedback loops that production attaches to this path also connected in the test? Attaching a disconnected dummy channel that silently drops downstream messages is a false-green bypass.

Production functions the test will call:
setup:   [production constructor, factory, or shared harness builder]
entry:   [the real upstream entry seam invocation]
observe: [channel / event / callback / persisted state the test reads at quiescence]

Functions I will write in the test file:
[List each helper — a helper is valid only if it can be replaced by a direct call to an existing production function with no changes to production code; if it cannot, it belongs in production or shared test infrastructure, not the test file]

This trace is the contract for Phases 2–4. Nothing written in the test may contradict it.

---

## Phase 2 — Pre-Implementation Checkpoint

Two checks, both required before any test code is written.

### 2a — Testability & Wiring Gate

Answer all five. If any answer is No, the test cannot be validly constructed yet:
1. Can the Production Entry Seam be invoked from the test using its real production trigger (without calling internal downstream handlers directly)?
2. Can production-owned state and actor topologies be assembled by calling real production constructors and wiring, rather than by manually reconstructing equivalent state in the test?
3. Can the required channels, queues, callbacks, or persisted state be observed from the test?
4. Is there a real observable exit that proves the system reached quiescence, not merely an intermediate stage?
5. Does the test enter before any central dispatcher / router / bus that production uses, so it observes the real routing behavior?

A blocked entry seam is a testability gap. Record what is blocked, why it is blocked, and what seam or hook would make a valid test constructible. Relocating the test to a more accessible but different entry point is not a resolution; it is a different test for a different thing.

### 2b — False-Green

For the proposed test, identify 2–5 ways it could pass while production is broken. The first row is mandatory for every integration test:

| If this production defect existed | Would this test fail? |
|---|---|
| **[upstream producer completely silent / removed]** | **must fail** |
| [message / event dropped] | must fail |
| [wrong destination / mode routed] | must fail |
| [production buffer / state never populated] | must fail |
| [add SUT-specific defect] | must fail |

> Mandatory upstream bypass check: if the upstream producer were completely silent or removed, would this test still pass? A yes means the test is wired to the output of the system, not the input. It is a false-green by construction. Redesign before proceeding.

A maybe or no means the test design does not cover what it claims to cover. Redesign before proceeding.

Persist this table. Do not discard it. The reasoning must survive the session, because a reviewer cannot judge a test whose justification lived only in chat.

Two durable artifacts:

1. The intent block above the test function, for seam binaries. This is the trace, compressed and committed.
2. The full table in the project's findings artifact when the seam has more than one row worth keeping.

---

## Phase 3 — Implementation

Write the test using the Phase 1 trace as the spec and the Phase 2 checkpoint as the constraint.

- Call real production constructors and factory functions to assemble components.
- Invoke the real production entry seam with real input — not a leaf method chosen for accessibility.
- Feed data the way production feeds it: in the operating mode being tested, at the correct granularity.
- Each distinct operating mode is its own subtest in the seam's binary. A mode difference does not justify a new file; a different production boundary does.
- Assert on the real observable exit: emitted events, persisted state, channel output, side effects.
- When the test fails because production dropped data, misrouted a message, or skipped a state transition: that failure is a finding. Record it in the project's findings artifact. Do not patch production. The test is working correctly.

**Negative assertions are mandatory for suppression, gate, and guard paths.** For any SUT that includes a suppression gate, at least one test must assert that the gated output is absent when the gate is active.

Absence is provable only after quiescence is proven. Use, in order: a capture sink attached before the boundary; a closed producer drained to disconnected; a barrier or acknowledgement proving the producer finished; a storage count of zero after commit. A `sleep` followed by `try_recv`, or "no event within N ms", is banned because it conflates "didn't happen yet" with "will never happen".

**Shared test infrastructure rules:**
- Helpers shared across test files belong in a shared test-support location used by the project.
- A valid helper function is one that can be replaced by a direct production call with no production code changes. If it cannot be, it belongs in production or shared infrastructure, not the test.
- Tests requiring external credentials or live services must be explicitly marked skip/ignore per the project's framework conventions, with a comment on how to run them manually.

**One binary per seam, 10–20 subtests.** Put all behaviors for one production seam in one file. A subtest covers one scenario and one invariant. Do not create a new file per behavior, and do not merge two seams because their tests look alike.

**Naming.** `snake_case`, no `test_` prefix, pattern `<subject>_<scenario>_<outcome>`. One behavior per function. Banned: `_matrix`, `_validation`, `_works`, `_basic`, `_sanity`. A subtest is named for what it asserts, not what it touches.

**Fold or prune — the revamp step.** Do not only add. For every new or strengthened test, decide the fate of the old one:

| Old test | Action |
| :--- | :--- |
| Asserts a subset of the new one | Fold into the new subtest; delete the old function |
| Asserts the same thing through a weaker seam | Prune |
| Name overstates the body | Rename, or fix the body |
| Passes if production is deleted | Delete, and record the finding |
| Covers a behavior nothing else covers | Keep; add the intent block |

A test does not survive because it already passes. Passing is not coverage.

---

## Phase 4 — Circuit Breaker

Before submitting, verify the test against the Phase 1 contract.

A test passes this circuit breaker when:
- Its entry point matches the Production Entry Seam in the trace (not a downstream leaf).
- The Direction Check passes: the entry point is the upstream trigger, not the output receiver.
- Every node in the Production Path is exercised, not bypassed.
- Every function written in the test file passes the Phase 1 validity check.
- The test level matches what was requested.
- All suppression / gate paths have at least one negative-assertion test.
- The intent block is present, and every `ASSERTS` claim maps to an assertion in the body.
- `MUTANT` is present for suppression, edge-case, and regression classes, and is a concrete one-line edit.

Then answer the governing question one final time, concretely:
If [specific handoff X] in the production path silently broke, this test would fail because [specific assertion Y] would not receive [expected value Z].

If you cannot complete that sentence with a real assertion, the test does not yet meet the governing question. Return to Phase 1.

---

## Handoff

When Phase 4 passes, run the test. If it fails because a production boundary is broken or disconnected, that is a successful outcome — the test found a real bug. Do not patch production to make it pass. Record the defect in the project's findings artifact with the test name, the observed symptom, the evidence, and the suspected `file:line`, then continue with the next seam.

Once the seam is green, its `MUTANT` line becomes the input to `/mutate`. Mutation runs are batched per campaign, not per test: finish the seams, then run the mutation step once over the changed files and record the result.
