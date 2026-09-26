---
title: "Phase 12 — Target Integration Test Specification (v2.4)"
audience: "Internal — Test Engineers, Backend Engineers, QA"
last_updated: 2026-09-22
app_version: "0.8.9"
status: "Active Working Draft (Post-Harness v2, Memory v2, Schema v5, Agentic Loop & Tool Taxonomy)"
owners: "test-engineer role"
related_docs:
  - "docs/specs/events-spec.md — Pipeline event routing & state transitions"
  - "docs/specs/harness-spec.md — Harness v2 plugin chassis & Two-Door runtime"
  - "docs/specs/memory-spec.md — Cognitive memory v2 & 2-stage dedup"
  - "docs/specs/notifications-spec.md — Notification 3D matrix & Schema v4"
  - "docs/specs/db-spec.md — Database v2 schema & Turso engine invariants"
  - "docs/specs/ipc-spec.md — Frontend-backend IPC contracts"
  - "docs/specs/tools-spec.md — Agentic tool runtime, taxonomy & capability gating"
---

# Phase 12 — Target Integration Test Specification (v2.5)

> **Specification Ground Truth:**
>
> - **Approved Baseline:** Phase 12 (App Version **0.8.9**).
> - **Architectural SSOT:** Unified across Harness Agentic Loop (`services/harness/`), Tool Taxonomy (`services/harness/stages/tools/`), Memory v2 (`persistence/` & `services/memory/`), **Schema v7** (`personal_memory_suggestions`, `session_tool_calls`), and 6-Domain Event Contracts (`pipeline/assistant/`, `pipeline/dictation/`).
> - **Testing Standard:** Strictly complies with `.agents/rules/testing-style-guide.md` and `.agents/rules/test-engineer.md`. Zero mocks when local models, assets, or API keys exist.
> - **Execution Discipline:** `cargo nextest run --test <file> --release --nocapture --test-threads=1`. Single-thread isolation, release builds only.

> ## ⚠️ v2.5 Amendment — Authority Notice (2026-09-26)
>
> A 139-function audit of the full suite (`docs/tests/seam_audit_report.md`) established that the
> per-seam sections below had drifted from production code. **Section 0.1 is now normative and overrides
> any conflicting statement in Sections 1–21.** The per-seam sections are retained as design intent and
> must be reconciled against Section 0.1 before being cited as current.
>
> Specifically, the following claims in this document are **stale**:
> - Schema version references to **v5** (now **v7** — `SCHEMA_VERSION` in `persistence/schema.rs`).
> - Seam 20's "schema version 5" migration assertions.
> - Seam 12's characterisation of `memory_compaction_test.rs` as covering the `CompactionCoordinator`.
> - Any seam section asserting a coverage property that `docs/tests/seam_audit_report.md` records as
>   `MISCLASSIFIED`, `EYE-CANDY`, or `REFACTOR`.

---

## 0.1 Normative Test-Placement & Evidence Rules (v2.5 — OVERRIDES SECTIONS 1–21)

These rules are binding. They exist because the audit found 11 `MISCLASSIFIED`, 6 `EYE-CANDY` and 2
`REFACTOR` functions, the majority of which were level errors or self-fulfilling assertions rather than
missing coverage.

### 0.1.1 Level Selection (the Level Test)

A test in `tests/` **MUST** cross a real boundary with a real upstream trigger. Classify before writing:

| Level | Entry condition | Location |
| :--- | :--- | :--- |
| **Unit** | Pure/algorithmic fn, no upstream producer, no cross-boundary handoff. Calling it directly is correct. | `#[cfg(test)] mod tests` in the owning `src/` file |
| **Integration** | Driven in production by an upstream actor, queue, channel, event, request, or user action. | `tests/<feature>_test.rs` |
| **Persistence contract** | Exercises a `persistence::*` function against real Turso. No event, no router, no actor. | `tests/database_persistence_boundary_test.rs` |
| **Evaluation** | The assertion is about model output quality, semantic correctness, or LLM judgement. | `evals/<capability>/` |

**Banned:** a test that calls a `persistence::*` leaf, constructs no `AppState`/router/actor, and asserts
only on a returned struct or row count **while living in a feature seam file**. That is a *Persistence
contract* test filed under a feature name. Move it to the Seam 20 file.

### 0.1.2 The Direction Check (binding)

If the function a test calls to *initiate* is the same function production calls to *deliver a result*, the
test exercises the **sink**, not the trigger. Canonical violations, all found in this audit:

- Calling `commit_compaction_output` / `fetch_turns_for_compaction` directly and asserting rows landed.
- Calling `persistence::notifications::resolve_notification_in_place` directly when production reaches it
  via `CompactionCoordinator`.

**Rule:** every feature-seam test MUST name the production trigger it drives in a header comment, and that
trigger MUST be a public function or a real event dispatch.

### 0.1.3 The Self-Execution Ban (binding)

**A test MUST NOT perform a production step itself and then assert the result of that step.** This is the
single highest-severity failure mode found, and it produces tests that pass while production is deleted.

Prohibited patterns, each confirmed present in this codebase before v2.5:

| Anti-pattern | Where it was found | Required instead |
| :--- | :--- | :--- |
| Calling `harness.push_assistant_turn(...)` then asserting history contains the response | `llm_to_tts_test.rs` Exit 6 (production commits at `harness/steps.rs`) | Assert against state production left behind |
| Rebuilding a payload in a loop, then asserting the payload's contents | `memory_compaction_test.rs` test 6 | Call the production assembler (make it `pub`) |
| Re-implementing a production predicate/branch inside the test body, then asserting it | `vad/actor.rs` trimming test; `model_manager_test.rs` tar-slip test | Extract the logic to a callable `pub`/`pub(crate)` fn first |
| Asserting a third-party crate's behaviour instead of Vox's use of it | `model_manager_test.rs` zip-slip test | Call `ModelManager::do_extract` |
| Asserting a file is absent when nothing ever attempted to create it | `model_manager_test.rs` zip-slip test | Assert on the *return value* of the operation that could have created it |

**Visibility:** per `.agents/rules/backend-style-guide.md` §2, `pub` is sanctioned for items the `tests/`
crate must reach. Visibility widening (`pub(crate)` → `pub`) is the expected remedy, not a workaround.

### 0.1.4 Mandatory Negative Assertions (binding)

For every suppression, gate, or exclusion path, at least one test MUST assert the gated output is
**absent**, and MUST do so deterministically. Preferred forms, in order:

1. **Capture-channel interception** (strongest) — attach a sink *before* the boundary and assert nothing
   was sent. Reference: `dictation_window_test.rs` "LLM Zero Invariant".
2. **Storage-layer count** — assert `SELECT COUNT(*) … == 0`. Reference:
   `notifications_crud_test.rs` zero-DB invariant.
3. **Flag + booby-trapped callback** — the callback sets a flag *and* returns an error. Reference:
   `realtime_transport_test.rs` paused-state suppression.
4. `assert_channel_empty_after(&rx, Duration, msg)` — the sanctioned helper in `tests/common/harness.rs`.

**Banned:** a bare `sleep(Nms)` followed by a non-blocking `try_recv` for an absence claim
(`testing-style-guide.md` §6.1). Waits must be deadline polls or event-ordered sentinels.

### 0.1.5 Model Lens (binding for a model-oriented system)

Every test MUST be classified on whether it touches a model, and the classification MUST be recorded in the
file header's `Metrics` line.

| Test touches a model? | Then the assertions must be | Verdict if it asserts quality |
| :--- | :--- | :--- |
| **Yes** | **Wiring** only: right model, right payload, right channel, non-empty output, streaming order | `EVAL-DEFER` |
| **No** | Structural/persistence/routing behaviour | `MISCLASSIFIED` if filed as a feature-seam IT |

A wiring assertion is *"tokens reached the accumulator"*, *"clause was dispatched before `LlmFinished`"*,
*"RMS > 0.001"*. A quality assertion is *"the answer is Paris"*, *"facts were extracted"*, *"the summary is
coherent"*. **Never assert model correctness inside an IT** — that is `evals/` with an LLM judge.

When a seam genuinely has both dimensions, **split it** (reference: `tts_to_playback_test.rs` — real model
for flow, deterministic stub for the threshold boundary).

### 0.1.6 `#[ignore]` Constraint (binding — new in v2.5)

An `#[ignore]` reason MUST enumerate **which specific assertions require the external dependency**.

- **Permitted:** the whole test requires a live third-party service, and the offline surface is covered by a
  sibling test. Reference: `realtime_transport_test.rs` live handshake.
- **Banned:** the test bundles offline-verifiable assertions behind a paid key or unreachable endpoint.
  Reference: `ptt_window_realtime_test.rs` — 6 of 7 assertions need no key; the zero-STT-leak invariant is
  the seam's most valuable property and was dark in CI.

If an external dependency is unavailable, the offline subset MUST run against a real in-process substitute
(TCP listener + WebSocket, or an HTTP server capturing the request body — reference:
`agentic_tool_runtime_test.rs::spawn_mock_wire_server`).

### 0.1.7 Matrix Bundling (reconciles `testing-style-guide.md` §7.3 with `/create-test` Phase 3)

§7.3 mandates consolidated matrix tests **for backend initialisation only** (no ONNX/GGUF re-warming). It
does **not** license merging assertions. Binding rule:

- **Backend initialisation:** one worker/model session per test binary or per consolidated test fn. (§7.3)
- **Scenarios:** separately reported, order-independent, no cross-scenario state. (`/create-test` Ph. 3)
- **Channel drains:** a drain between scenarios MUST NOT silently discard. Either assert on what is drained
  or eliminate the drain by giving each scenario its own actors.

### 0.1.8 Reference Implementations (new tests MUST follow these)

| Concern | Reference file | What to copy |
| :--- | :--- | :--- |
| Provider wire contract, inbound + outbound | `tests/agentic_tool_runtime_test.rs` | `spawn_mock_wire_server` captures the real request body; assert both directions |
| Real persistence, MVCC, float precision | `tests/database_persistence_boundary_test.rs` | Real Turso in tempdir; distinctive inputs, not uniform fills |
| Suppression-gate assertion | `tests/dictation_window_test.rs` | Capture-channel "LLM Zero Invariant" |
| Boundary-gate assertion with a stub | `tests/tts_to_playback_test.rs` | In-file false-green audit note; both gate directions |
| Latch lifecycle | `tests/playback_interrupt_test.rs` | Arm → hold → release → clear, in order |
| Test-harness self-justification | `tests/tts_to_playback_test.rs` | `// NOTE (false-green audit): …` comment on every stub |

### 0.1.9 Mandatory Header (extends `testing-style-guide.md` §4)

Every file in `tests/` MUST carry `(Seam N)` in its `Category` line, and — where a test touches a model —
a `Model: <none|wired-only|quality-asserted>` line. `Category: Integration Test` without a seam number is
non-conforming.

---

## 0. How to Read This Specification

- **Audience:** Test engineers constructing or modifying integration test binaries in `app/src-tauri/tests/`, backend engineers validating subsystem boundaries, and QA verifying release gates.
- **Scope:** Defines every end-to-end subsystem boundary ("seam") across the Vox runtime.
- **Conventions:** Every seam defines:
  1. **Production Path Trace (`/create-test` Phase 1)**: The exact chain of production modules from trigger to exit.
  2. **Testability & Direction Check (`/create-test` Phase 2a)**: Verification that the test calls the upstream trigger, not the sink.
  3. **False-Green Audit Table (`/create-test` Phase 2b)**: 2–5 specific mechanical defects that _must_ fail the test.
  4. **Execution & Evidence Criteria (`/test`)**: Hard timeouts, exact metrics, and Levenshtein thresholds.
  5. **Mutation Sensitivity (`/mutate`)**: Tier 1 production code mutations used to empirically prove test sensitivity.
- **Non-goals:** This document does not specify unit tests (internal helper logic in `src/**/#[cfg(test)]`) or macro benchmarks (`benches/`).
- **SSOT:** Architectural contracts reside in the respective specifications under `docs/specs/`. This document is the SSOT for **cross-boundary testing contracts**.

---

## 1. System Architecture & Cognitive Stage Topology

The integration test specification reflects the true post-refactor system topology (`harness-spec.md §4.1`, `events-spec.md §1`):

```
[Audio In] ──► [VAD] ──► [STT] ──► [HarnessSession (Cognitive Stage)] ──► [TTS] ──► [Playback]
                                            ▲ │
                         Duplex Session Pipe│ │Token Stream
                                            │ ▼
                                       [LlmActor (Pure Model)]
```

### The 4 Cognitive Seams in Harness v2:

1. **`STT ──► HarnessSession` (`prepare_turn`)**:
   `on_transcript_final` enters `HarnessSession`. Evaluates token budget (65% soft / 85% critical). If critical compaction is required, transitions `Thinking` $\to$ `Working`, dispatches `AudioIntent::InterimFiller` to TTS, and triggers inline compaction. If valid text, stays in `Thinking` and issues generation request via duplex pipe. If empty, recovers to `Ready` with Info Toast.
2. **`HarnessSession ◄──► LlmActor` (Duplex Dialogue Pipe)**:
   Session-scoped bidirectional channel. Harness sends generation requests, assembled prompt, and cancellation token. `LlmActor` streams raw tokens back to Harness. Zero direct TTS, CPAL, or UI knowledge in `LlmActor`.
3. **`HarnessSession ──► TTS` (`StreamRoutingStage`)**:
   The Harness owns the stream router. Parses streaming tokens, demuxes `<response>` tag, chunks clauses via `TtsClauseChunker`, and dispatches synthesized clauses directly to `TtsActor`. Trailing orchestration tags (`<title>`, `<action>`) are routed out of band to persistence and notifications.
4. **`TTS ──► Playback` (Audio Intent & State Gating)**:
   Dispatches audio tagged with `AudioIntent::InterimFiller` or `AudioIntent::TurnResponse`. Interim filler playback holds the pipeline in `InteractionState::Working` and **never** emits `PlaybackFinished` $\to$ `Ready`. Only `TurnResponse` audio transitions `Working` $\to$ `Speaking` $\to$ `Ready`.

---

## 2. Test Infrastructure & Shared Harness Contracts

All integration tests reside in `app/src-tauri/tests/` and import shared utilities from `tests/common/`:

```
app/src-tauri/tests/
├── common/
│   ├── mod.rs                  # Harness re-exports
│   ├── audio.rs                # decode_wav_to_mono_16k, stream_audio_to_ring_buffer, stream_silence_frames
│   ├── scoring.rs              # normalize_text, calculate_similarity, assert_similarity_above
│   ├── paths.rs                # Model weight paths & isolated TempPathsGuard (VOX_HOME isolation)
│   └── harness.rs              # Production worker constructors (setup_stt_worker, setup_vad_actor, etc.)
└── assets/
    ├── edgetts_01_en_briefing.wav
    ├── edgetts_07_hi_weather.wav
    ├── supertonic_01_en_briefing.wav
    └── supertonic_07_hi_weather.wav
```

### Shared Harness Invariants:

1. **Zero Production Duplication**: Test helpers must never reimplement production loops, ring buffers, or parsing logic (`create-test Phase 1`).
2. **Filesystem & Database Isolation**: Tests interacting with SQLite or `~/.vox/` must use `TempPathsGuard` to isolate paths to an ephemeral temporary directory, preventing host database contamination.
3. **Hard Timeout Enforcement**: Every async test function must be wrapped in `tokio::time::timeout(Duration::from_secs(N), ...)`; every synchronous test function must use an explicit `Instant::now() + Duration::from_secs(N)` deadline.

---

## 3. Specifications per Integration Seam

```
Seam Status Legend:
- [x] Solid (Verified Green & Validated)
- [ ] Needs Rework (Mock Elimination / Signature Drift)
- [ ] Decommissioned & Replaced (Legacy Memory v1 Replaced with v2)
- [ ] New Critical Seam (Phase 11 Architectural Addition)
```

---

### Seam 1 — Passive Audio Streaming: Ring Buffer ──► ContinuousSegmentation VAD ──► Nemotron STT ──► `TranscriptFinal`

- **Status:** `[x] Solid (Verified Green & Validated)`
- **File:** `tests/passive_streaming_test.rs`
- **Category:** Integration Test
- **Subsystems:** `services/vad/actor.rs`, `services/stt/actor.rs`, `core/state.rs`
- **Prerequisites:** Local Earshot VAD + Nemotron 3.5 STT ONNX weights in `~/.vox/models/`
- **Execution:** `cargo nextest run --test passive_streaming_test --release --nocapture --test-threads=1`
- **Metrics:** Perceived latency, Levenshtein transcript similarity ($\ge 0.90$)

#### 1.1 `/create-test` Phase 0 — Classification

- **Type:** Integration Test.
- **Rationale:** Verifies cross-thread, lock-free audio streaming and actor coordination between CPAL ring buffer, `VadActor`, and `SttActor` without mocking.

#### 1.2 `/create-test` Phase 1 — Production Path Trace

```
SUT: Passive voice detection, speech onset segmentation, silence cutoff detection, and local STT transcription.

Production Entry Seam:
  stream_audio_to_ring_buffer(audio, &mut ring_producer) feeding CPAL audio ring buffer (HeapRb<f32>).
  Direction Check: PASS — entry seam is the upstream audio microphone stream input trigger, NOT the output consumer.

Production Path:
  CPAL audio ring buffer (HeapRb<f32>)
  ──► VadActor (reading lock-free ring buffer in ContinuousSegmentation mode)
  ──► EarshotVadEngine (evaluates 256-sample chunks for speech probability)
  ──► SpeechStart event emitted (speech_onset_ms window validated)
  ──► Speech silence detected exceeding silence_duration_ms
  ──► SttCommand::Final { audio_samples, turn_id } dispatched to STT worker channel
  ──► SttActor (Nemotron 3.5 ONNX CTC greedy decoding)
  ──► VoxEvent::TranscriptFinal { turn_id, text } pushed to central pipeline event queue.

Observable Exit:
  VoxEvent::TranscriptFinal received from the central pipeline event channel.

Production Functions Called:
  setup:   common::harness::setup_stt_worker(&app), common::harness::setup_vad_actor(...)
  entry:   stream_audio_to_ring_buffer(&audio, &mut producer)
  observe: pipeline_event_rx.recv_timeout(deadline)

Helpers Written:
  decode_wav_to_mono_16k (hound WAV decode + 16k linear resample)
  calculate_similarity / assert_similarity_above (character-level Levenshtein similarity)
```

#### 1.3 `/create-test` Phase 2a — Testability Check

1. Entry seam callable with production signature? **Yes** (`RingBuffer` producer).
2. Production constructors used? **Yes** (`setup_stt_worker`, `setup_vad_actor`).
3. Channels/events observable? **Yes** (`pipeline_event_rx`).
4. Real observable exit without mocks? **Yes** (`TranscriptFinal` text).

#### 1.4 `/create-test` Phase 2b — False-Green Audit Table

| If this production defect existed                                                        | Would Seam 1 test fail? | Expected Failure Mode                                                     |
| ---------------------------------------------------------------------------------------- | ----------------------- | ------------------------------------------------------------------------- |
| **Upstream audio producer completely silent / zero frames pushed**                       | **Must fail**           | `pipeline_event_rx.recv_timeout` times out waiting for `TranscriptFinal`. |
| `VadActor` audio passthrough suppression inverted (`should_suppress_audio` stuck `true`) | Must fail               | VAD suppresses all frames; no speech onset; times out.                    |
| `SttActor` drops `SttCommand::Final` or fails to dispatch `TranscriptFinal`              | Must fail               | Channel receives no final event; times out.                               |
| VAD silence cutoff detection broken (`silence_duration_ms` timer never fires)            | Must fail               | Audio window never terminates; test exceeds 60s deadline.                 |
| STT decoder emits empty string or distorted characters                                   | Must fail               | `assert_similarity_above` panics (Levenshtein score $< 0.90$).            |

#### 1.5 `/test` Execution Protocol

- **Command:** `cargo nextest run --test passive_streaming_test --release --nocapture --test-threads=1`
- **Success Criteria:**
  1. English briefing clip (`supertonic_01_en_briefing.wav`): Levenshtein similarity $\ge 0.90$ against reference text.
  2. Hindi weather clip (`supertonic_07_hi_weather.wav`): Levenshtein similarity $\ge 0.90$ with clean transliteration.
  3. Clean worker teardown: background STT worker thread joins cleanly without panic.
  4. Execution finishes within 60s deadline.
- **Failure Signatures:**
  - `RecvTimeoutError`: Indicates deadlock or dropped event on VAD/STT crossbeam channels.
  - `AssertionError: similarity < 0.90`: Indicates acoustic distortion, incorrect resampling, or truncated audio window.

#### 1.6 `/mutate` Mutant Definitions (Tier 1)

- **Mutant 1.1 (Gate Inversion):** In `services/vad/actor.rs:should_suppress_audio`, replace body with `true` unconditionally.  
  _Prediction:_ Test goes RED via `RecvTimeoutError` because VAD suppresses all input frames.
- **Mutant 1.2 (Silent Drop):** In `services/stt/actor.rs`, comment out `vox_event_tx.send(VoxEvent::TranscriptFinal { .. })`.  
  _Prediction:_ Test goes RED via `RecvTimeoutError` on `pipeline_event_rx`.
- **Mutant 1.3 (Boundary Flip):** In `services/vad/actor.rs`, set `speech_threshold = 1.0` (unreachable speech probability).  
  _Prediction:_ Test goes RED because speech onset is never detected.

---

### Seam 2 — PTT Window Validation (Modular): Press ──► VAD Window ──► `SttCommand::Final`

- **Status:** `[x] Solid (Verified Green & Validated)`
- **File:** `tests/ptt_window_modular_test.rs`
- **Category:** Integration Test
- **Subsystems:** `pipeline/assistant/ptt.rs`, `services/vad/actor.rs`, `services/stt/actor.rs`, `core/state.rs`
- **Prerequisites:** Local Earshot VAD + Nemotron 3.5 STT ONNX weights in `~/.vox/models/`
- **Execution:** `cargo nextest run --test ptt_window_modular_test --release --nocapture --test-threads=1`
- **Metrics:** Perceived latency, Levenshtein transcript similarity ($\ge 0.90$), state transitions, ghost gate suppression.

#### 2.1 `/create-test` Phase 0 — Classification

- **Type:** Integration Test.
- **Rationale:** Verifies user PTT interaction lifecycle (`ptt_start`, `ptt_stop`, `ptt_cancel`), non-blocking lock discipline on Tokio threads, VAD windowed buffer accumulation, ghost audio discard, and clean STT handoff.

#### 2.2 `/create-test` Phase 1 — Production Path Trace

```
SUT: User holds/releases PTT in Modular pipeline_mode; VAD window evaluates speech and dispatches trimmed audio to STT or discards ghost audio.

Production Entry Seam:
  ptt_start(&app, &state) ──► ptt_stop(&app, &state) / ptt_cancel(&app, &state) bracketing ring buffer audio streaming.
  Direction Check: PASS — entry seam is the upstream user PTT trigger, NOT the STT consumer.

Production Path:
  ptt_start(&app, &state) (non-blocking try_lock on audio engine)
  ──► Pipeline state transitions to InteractionState::Listening
  ──► VadCommand::StartWindowValidation dispatched to VadActor
  ──► CPAL ring buffer frames accumulated in VadActor window buffer
  ──► ptt_stop(&app, &state)
  ──► VadCommand::StopWindowValidation dispatched to VadActor
  ──► VadActor evaluates accumulated window energy/speech probability:
      ├─► Valid speech: transitions to InteractionState::Thinking,
      │   dispatches SttCommand::Final { audio_samples, turn_id } to STT channel
      └─► Silence (Ghost Gate): transitions directly to InteractionState::Ready,
          discards audio, emits ZERO STT commands
  ──► SttActor (Nemotron 3.5 ONNX CTC greedy decoding)
  ──► VoxEvent::TranscriptFinal { turn_id, text } pushed to central pipeline event queue.

Observable Exit:
  1. Valid speech: VoxEvent::TranscriptFinal received on pipeline channel with text similarity >= 0.90.
  2. Silence (Ghost Gate): Pipeline state reverts to Ready; pipeline_event_rx remains strictly empty.
  3. Cancel: ptt_cancel reverts state to Ready; cancels turn_token; pipeline_event_rx remains strictly empty.

Production Functions Called:
  setup:   common::harness::setup_stt_worker(&app), common::harness::setup_vad_actor(...),
           common::harness::attach_mock_engine_with_vad_to_state(&app, &state, ...)
  entry:   ptt_start(&app, &state), ptt_stop(&app, &state), ptt_cancel(&app, &state),
           common::audio::stream_audio_to_ring_buffer(&audio, &mut producer)
  observe: state.pipeline.state(), state.pipeline.turn_token().is_cancelled(),
           pipeline_event_rx.recv_timeout(deadline)

Helpers Written:
  common::harness::collect_all_final_transcripts (drains final transcript strings from channel)
  common::harness::assert_channel_empty_after (negative assertion polling channel emptiness)
```

#### 2.3 `/create-test` Phase 2a — Testability Check

1. Entry seam callable with production signature? **Yes** (`ptt_start`, `ptt_stop`, `ptt_cancel`).
2. Production constructors used? **Yes** (`setup_stt_worker`, `setup_vad_actor`, `attach_mock_engine_with_vad_to_state`).
3. Channels/events observable? **Yes** (`pipeline_event_rx`, `state.pipeline.state()`, `turn_token()`).
4. Real observable exit without mocks? **Yes** (Real audio asset, real Nemotron STT, real VAD windowing).

#### 2.4 `/create-test` Phase 2b — False-Green Audit Table

| If this production defect existed                            | Would Seam 2 test fail? | Expected Failure Mode                                                                                           |
| ------------------------------------------------------------ | ----------------------- | --------------------------------------------------------------------------------------------------------------- |
| **Upstream PTT trigger silent / dropped**                    | **Must fail**           | State never transitions to `Listening`; assertion fails immediately.                                            |
| `ptt_start` drops window accumulation buffer                 | Must fail               | STT receives empty buffer; fails similarity $\ge 0.90$.                                                         |
| Ghost gate broken (silence window falsely treated as speech) | Must fail               | Subtest 2 fails: state enters `Thinking` instead of `Ready`, and `pipeline_event_rx` receives unexpected event. |
| `ptt_cancel` fails to cancel `turn_token`                    | Must fail               | Subtest 3 fails: `assert!(turn_token.is_cancelled())` panics.                                                   |
| Blocking lock re-introduced in `assistant/ptt.rs`            | Must fail               | Deadlocks Tokio runtime; test times out on 60s hard deadline.                                                   |

#### 2.5 `/test` Execution Protocol

- **Command:** `cargo nextest run --test ptt_window_modular_test --release --nocapture --test-threads=1`
- **Success Criteria:**
  1. Subtest 1 (Speech): User speech during PTT hold transitions `Ready` $\to$ `Listening` $\to$ `Thinking`, and emits `TranscriptFinal` with similarity $\ge 0.90$ on `supertonic_01_en_briefing.wav`.
  2. Subtest 2 (Ghost Gate): Silence during PTT hold reverts `Listening` $\to$ `Ready`; `pipeline_event_rx` receives zero events after 500ms.
  3. Subtest 3 (Cancel): `ptt_cancel` reverts `Listening` $\to$ `Ready`; `turn_token.is_cancelled()` is `true`; channel receives zero events.
  4. Clean actor teardown: `vad_join` and `stt_join` join cleanly within 60s deadline.
- **Failure Signatures:**
  - `AssertionError: state != Thinking` after `ptt_stop`: VAD window failed to detect speech in the valid audio fixture.
  - `Ghost gate failure`: Channel receives unexpected `TranscriptFinal` during silence hold.
  - `turn_token.is_cancelled() == false`: Cancellation flag not propagated to pipeline atomics.

#### 2.6 `/mutate` Mutant Definitions (Tier 1)

- **Mutant 2.1 (Gate Inversion):** In `services/vad/actor.rs:StopWindowValidation`, force `speech_detected = true` unconditionally for all windows.  
  _Prediction:_ Subtest 2 goes RED because silence hold emits an unexpected `TranscriptFinal` instead of reverting to `Ready`.
- **Mutant 2.2 (Silent Drop):** In `pipeline/assistant/ptt.rs:ptt_cancel`, comment out `state.pipeline.cancel_current_turn()`.  
  _Prediction:_ Subtest 3 goes RED on `assert!(turn_token.is_cancelled())`.
- **Mutant 2.3 (Boundary Flip):** In `services/vad/actor.rs:StopWindowValidation`, invert `speech_detected` boolean (`!speech_detected`).  
  _Prediction:_ Subtest 1 goes RED because valid speech is discarded as ghost audio, never reaching STT.

---

### Seam 3 — PTT Window Validation (Realtime): Press ──► VAD Window ──► Realtime Commit

- **Status:** `[x] Solid (Verified Green & Validated)`
- **File:** `tests/ptt_window_realtime_test.rs`
- **Category:** Integration Test
- **Subsystems:** `pipeline/assistant/ptt.rs`, `services/vad/actor.rs`, `services/realtime/actor.rs`, `services/realtime/providers/deepgram`
- **Prerequisites:** `DEEPGRAM_API_KEY` in `temp/.env`, Earshot VAD weights in `~/.vox/models/`
- **Execution:** `cargo nextest run --test ptt_window_realtime_test --release --nocapture --test-threads=1 -- --ignored`
- **Metrics:** Sample clamping accuracy (`f32` $\to$ `i16` within $[-32768, 32767]$), speech commit payload dispatch, state transitions, ghost gate discard.

> [!NOTE]
> **Rework Requirement Ledger:**  
> The current test file in `tests/ptt_window_realtime_test.rs` uses synthetic in-memory test doubles (`MockRealtimeProvider`, `MockRealtimeSession`) to verify local sample clamping and call counts without external network calls. Under the Zero-Mock invariant and active API key availability (`GEMINI_API_KEY`), this seam must be upgraded in an implementation sprint to exercise the real `GeminiLiveDriver` WebSocket connection harness.

#### 3.1 `/create-test` Phase 0 — Classification

- **Type:** Integration Test.
- **Rationale:** Verifies user PTT interaction lifecycle in `PipelineMode::Realtime`, clamping float PCM samples to integer mono frames, and committing speech turns directly to the realtime WebSocket provider actor without local STT.

#### 3.2 `/create-test` Phase 1 — Production Path Trace

```
SUT: User holds/releases PTT in Realtime pipeline_mode; VAD window evaluates speech, clamps audio to 16-bit PCM integer samples, and dispatches a speech commit frame to the realtime provider actor.

Production Entry Seam:
  ptt_start(&app, &state) ──► ptt_stop(&app, &state) / ptt_cancel(&app, &state) bracketing audio streaming.
  Direction Check: PASS — entry seam is the upstream user PTT trigger, NOT the RealtimeSession::commit sink.

Production Path:
  ptt_start(&app, &state) (non-blocking try_lock on audio engine)
  ──► Pipeline state transitions to InteractionState::Listening
  ──► VadCommand::StartWindowValidation dispatched to VadActor
  ──► CPAL ring buffer frames accumulated in VadActor window buffer
  ──► ptt_stop(&app, &state)
  ──► VadCommand::StopWindowValidation dispatched to VadActor
  ──► VadActor evaluates accumulated window energy/speech probability:
      ├─► Valid speech: transitions to InteractionState::Thinking,
      │   converts f32 audio samples via x.clamp(-1.0, 1.0) * 32767.0 as i16,
      │   calls realtime_engine.try_lock().signal_speech_committed(&i16_samples)
      │   ──► RealtimeActor frames speech commit payload
      │   ──► Dispatches audio packet across live WebSocket connection (Gemini Live / Deepgram)
      └─► Silence (Ghost Gate): transitions directly to InteractionState::Ready,
          discards audio, emits ZERO WebSocket frames
  ──► Note: Pipeline remains in Thinking until realtime WebSocket receives remote server TranscriptFinal.

Observable Exit:
  1. Valid speech: Outbound speech commit frame transmitted over WebSocket; pipeline state enters Thinking.
  2. Silence (Ghost Gate): Pipeline state reverts to Ready; zero audio frames dispatched to WebSocket.
  3. Cancel: ptt_cancel reverts state to Ready; cancels turn_token; zero audio frames dispatched.

Production Functions Called:
  setup:   get_test_app_and_state(), setup_vad_actor(PTT config),
           attach_mock_engine_with_vad_to_state, state.realtime_engine initialization
  entry:   ptt_start(&app, &state), ptt_stop(&app, &state), ptt_cancel(&app, &state),
           stream_audio_to_ring_buffer(&audio, &mut producer)
  observe: state.pipeline.state(), realtime provider session outbound packet counters,
           state.pipeline.turn_token().is_cancelled()
```

#### 3.3 `/create-test` Phase 2a — Testability Check

1. Entry seam callable with production signature? **Yes** (`ptt_start`, `ptt_stop`, `ptt_cancel`).
2. Production constructors used? **Yes** (`RealtimeActor`, `GeminiLiveDriver`, `setup_vad_actor`).
3. Outbound frames observable? **Yes** (WebSocket driver stream / actor event loop).
4. Real provider without synthetic stubs? **Target Requirement:** Uses real `GeminiLiveDriver` with `GEMINI_API_KEY` (tagged `#[ignore]`).

#### 3.4 `/create-test` Phase 2b — False-Green Audit Table

| If this production defect existed                                                         | Would Seam 3 test fail? | Expected Failure Mode                                                                        |
| ----------------------------------------------------------------------------------------- | ----------------------- | -------------------------------------------------------------------------------------------- |
| **Upstream PTT trigger silent / dropped**                                                 | **Must fail**           | Window never activates; state stays `Ready`; zero audio pushed.                              |
| Ghost gate deleted in `assistant/ptt.rs:on_ptt_stop`                                      | Must fail               | Subtest 2 fails: silence window incorrectly commits audio to cloud actor.                    |
| f32-to-i16 clamping removed (`x.clamp(-1.0, 1.0)` deleted in `dispatch_ptt_speech_audio`) | Must fail               | Sample-exact assertion fails: full-scale audio clips/wraps around in integer representation. |
| `signal_speech_committed` call omitted or not awaited in Realtime branch                  | Must fail               | Realtime provider receives zero committed audio frames.                                      |
| Dispatch routes to Modular STT instead of Realtime provider                               | Must fail               | Realtime engine receives zero samples while local mock STT receives spurious commands.       |

#### 3.5 `/test` Execution Protocol

- **Command:** `cargo nextest run --test ptt_window_realtime_test --release --nocapture --test-threads=1 -- --ignored`
- **Success Criteria:**
  1. Subtest 1 (Speech): Real audio window dispatches clamped `i16` PCM samples; transitions state to `Thinking`; provider commits turn.
  2. Subtest 2 (Ghost Gate): Silence hold reverts state to `Ready`; provider receives 0 commit calls.
  3. Subtest 3 (Cancel): `ptt_cancel` cancels `turn_token`, reverts to `Ready`; provider receives 0 commit calls.
- **Failure Signatures:**
  - `SampleMismatchError`: Indicates missing float-to-integer clamping or scaling drift.
  - `GhostGateLeak`: Outbound WebSocket frames detected during silence window.

#### 3.6 `/mutate` Mutant Definitions (Tier 1)

- **Mutant 3.1 (Boundary Flip):** In `pipeline/assistant/ptt.rs:dispatch_ptt_speech_audio`, delete `.clamp(-1.0, 1.0)`.  
  _Prediction:_ Test goes RED on full-scale audio fixtures due to integer overflow / clamping distortion.
- **Mutant 3.2 (Silent Drop):** In `pipeline/assistant/ptt.rs:dispatch_ptt_speech_audio`, comment out `rt_actor.signal_speech_committed(&i16_samples)`.  
  _Prediction:_ Subtest 1 goes RED because provider commit count remains 0.
- **Mutant 3.3 (Routing Swap):** In `pipeline/assistant/ptt.rs:dispatch_ptt_speech_audio`, swap branch condition `ctx.pipeline_mode == PipelineMode::Realtime` to `PipelineMode::Modular`.  
  _Prediction:_ Subtest 1 goes RED because audio is misrouted to STT worker instead of the Realtime actor.

---

### Seam 4 — Dictation PTT + Passive + Ingestion Gate: `VoxEvent::PttStart/PttStop/PttCancel` ──► VAD Window ──► STT ──► `OutputRouter` (Zero LLM)

- **Status:** `[x] Solid (Verified Green & Validated)`
- **File:** `tests/dictation_window_test.rs`
- **Category:** Integration Test
- **Subsystems:** `pipeline/dictation/{mod,ptt,speech,transcript,error}.rs`, `services/dictation/output_router.rs`, `services/vad/actor.rs`, `services/stt/actor.rs`, `core/state.rs`
- **Prerequisites:** Local Earshot VAD + Nemotron 3.5 STT ONNX weights in `~/.vox/models/`
- **Execution:** `cargo nextest run --test dictation_window_test --release --nocapture --test-threads=1`
- **Metrics:** Latency, Levenshtein transcript similarity ($\ge 0.90$), state transitions, zero LLM commands, Option C gate purge.

#### 4.1 `/create-test` Phase 0 — Classification

- **Type:** Integration Test.
- **Rationale:** Verifies dictation hotkey/passive interaction lifecycle across central FIFO router, lock-free 2-layer ingestion gate, VAD windowing, Nemotron STT decoding, OS output routing (Tray, Clipboard, Simulate Paste), and strict isolation from the LLM Cognitive Stage.

#### 4.2 `/create-test` Phase 1 — Production Path Trace

```
SUT: Dictation hotkey/passive accumulation dispatches to STT and routes transcript to OS injection, NEVER to LLM; ingestion gate, owner handover, and error recovery hold.

Production Entry Seam:
  VoxEvent::PttStart -> VoxEvent::PttStop (or PttCancel) sent via state.event_tx (the identical Sender used by the OS global hotkey listener).
  Direction Check: PASS — entry seam is the upstream user hotkey action trigger, NOT the output_router sink or STT command.

Production Path A (PTT Hotkey Window):
  hotkey Press ──► event_tx.send(VoxEvent::PttStart)
  ──► Router OS thread (state.owner == Dictation)
  ──► pipeline::dictation::handle_event ──► dictation::ptt::on_ptt_start
      ├─► If dictation_state == Idle: error::on_error("Dictation is disabled in Settings"), returns
      └─► If Ready: allocates next_turn() bundle, transitions dictation to Listening,
          dispatches VadCommand::StartWindowValidation via non-blocking try_lock()
  ──► stream_audio_to_ring_buffer while Listening (accumulated in VAD window buffer)
  ──► hotkey Release ──► event_tx.send(VoxEvent::PttStop)
  ──► dictation::ptt::on_ptt_stop ──► VadCommand::StopWindowValidation
      ├─► If silence / empty: transitions dictation to Ready, discards audio (GHOST GATE)
      └─► If valid speech: transitions dictation to Thinking,
          dispatches SttCommand::Final { turn_id, audio } to STT actor
  ──► SttActor (Nemotron 3.5 ONNX CTC decoding)
  ──► VoxEvent::TranscriptFinal pushed to central pipeline event channel
  ──► Router ──► dictation::transcript::on_transcript_final
      ├─► If text is empty: transitions dictation to Ready, triggers "No speech recognized" toast
      └─► If text valid: updates state.dictation_last_transcript,
          dispatches output_router::route_transcript (Tray, Clipboard, or Simulate Paste),
          transitions dictation to Ready, emits IpcEvent::TranscriptFinal to WINDOW_TRAY.
  ──► LLM ZERO INVARIANT: Zero messages ever dispatched to LlmActor; llm_rx remains strictly empty.

Production Path B (Passive Speech Dictation):
  Continuous speech onset ──► VoxEvent::SpeechStart ──► dictation::speech::on_speech_start (transitions to Listening)
  Speech end ──► VoxEvent::SpeechEnd ──► dictation::speech::on_speech_end (transitions to Thinking)
  STT decoding ──► TranscriptFinal ──► output_router, never LLM.

Production Path C (Option C 2-Layer Ingestion Gate):
  Gate Closed: (assistant == Idle && dictation == Idle).
  Layer 1 (services/audio/device.rs): CPAL callback returns before push when gate closed.
  Layer 2 (services/vad/actor.rs): Loop head clears pre-roll and window buffers, draining stale audio.
  Gate Reopen: Audio accumulated while gate was closed does NOT trigger speech onset or STT.

Observable Exit:
  1. Valid speech: dictation_last_transcript matches transcript text (similarity >= 0.90); dictation state returns to Ready; llm_rx is empty.
  2. Ghost gate: Dictation state returns to Ready; zero STT events emitted.
  3. Cancel: PttCancel returns state to Ready; zero STT events emitted.
  4. Gate purge: Ingestion gate closed/reopened discards stale frames cleanly.

Production Functions Called:
  setup:   setup_stt_worker(&app), setup_vad_actor(...), attach_mock_engine_with_llm_vad_to_state, spawn_router
  entry:   event_tx.send(VoxEvent::PttStart), event_tx.send(VoxEvent::PttStop), event_tx.send(VoxEvent::PttCancel)
  observe: state.pipeline.dictation_state(), state.dictation_last_transcript, llm_rx, pipeline_event_rx
```

#### 4.3 `/create-test` Phase 2a — Testability Check

1. Entry seam callable with production signature? **Yes** (`state.event_tx.send(VoxEvent::PttStart)`).
2. Production constructors used? **Yes** (`spawn_router`, `setup_stt_worker`, `setup_vad_actor`).
3. Channels/state observable? **Yes** (`dictation_state()`, `dictation_last_transcript`, `llm_rx`).
4. Real observable exit without mocks? **Yes** (Real Nemotron STT, real VAD windowing, real router thread).

#### 4.4 `/create-test` Phase 2b — False-Green Audit Table

| If this production defect existed                                | Would Seam 4 test fail? | Expected Failure Mode                                                                             |
| ---------------------------------------------------------------- | ----------------------- | ------------------------------------------------------------------------------------------------- |
| **Upstream hotkey trigger silent / dropped in router**           | **Must fail**           | Dictation state never leaves `Ready`; assertion fails.                                            |
| Dictation routes to LLM (accidental coupling to Cognitive Stage) | Must fail               | `llm_rx.recv()` succeeds; LLM Zero Invariant assertion panics.                                    |
| Ghost gate deleted in `dictation/ptt.rs:on_ptt_stop`             | Must fail               | Subtest 2 fails: silence hold enters `Thinking` and emits spurious STT transcript.                |
| Option C Layer-2 gate purge omitted in `VadActor`                | Must fail               | Subtest 5 fails: stale audio held while gate was closed triggers STT upon reopening.              |
| Disabled dictation auto-recovers to `Ready` on hotkey press      | Must fail               | Subtest 6 fails: `PttStart` when `Idle` leaves state in `Ready` or `Listening` instead of `Idle`. |

#### 4.5 `/test` Execution Protocol

- **Command:** `cargo nextest run --test dictation_window_test --release --nocapture --test-threads=1`
- **Success Criteria:**
  1. Subtest 1 (PTT Speech): Dictation state transitions `Ready` $\to$ `Listening` $\to$ `Thinking` $\to$ `Ready`; `dictation_last_transcript` has similarity $\ge 0.90$; `llm_rx` is completely empty.
  2. Subtest 2 (Ghost Hold): Silence hold returns state to `Ready`; `pipeline_event_rx` remains empty.
  3. Subtest 3 (Cancel): `PttCancel` returns state to `Ready`; `pipeline_event_rx` remains empty.
  4. Subtest 4 (Passive Speech): `SpeechStart` $\to$ `SpeechEnd` $\to$ `TranscriptFinal` routes to output with zero LLM commands.
  5. Subtest 5 (Gate Purge): Stale audio pushed while gate closed is purged; reopen does not trigger STT.
  6. Subtest 6 (Idle Start): `PttStart` when `Idle` remains in `Idle`; rejects recording.
  7. Teardown: Router, VAD, and STT threads join cleanly without panics within 90s deadline.
- **Failure Signatures:**
  - `LLM Zero Invariant Failure`: Message detected on `llm_rx`.
  - `Gate Purge Failure`: STT event received on reopened gate from stale buffer.

#### 4.6 `/mutate` Mutant Definitions (Tier 1)

- **Mutant 4.1 (Routing Swap):** In `pipeline/dictation/transcript.rs:on_transcript_final`, add call dispatching `LlmCommand::Generate` to LLM worker.  
  _Prediction:_ Subtest 1 and Subtest 4 go RED immediately on `assert_channel_empty_after(&llm_rx)`.
- **Mutant 4.2 (Gate Inversion):** In `services/vad/actor.rs`, invert Layer-2 gate condition `if !handles.ingestion_gate.load(...)` to `if handles.ingestion_gate.load(...)`.  
  _Prediction:_ Subtest 5 goes RED because stale audio is preserved and processed upon gate reopening.
- **Mutant 4.3 (Default Fallback Flip):** In `pipeline/dictation/ptt.rs:on_ptt_start`, delete the `dictation_state == Idle` error guard and let it fall through to recording.  
  _Prediction:_ Subtest 6 goes RED because pressing hotkey while disabled transitions to `Listening` instead of staying in `Idle`.

---

### Seam 5 — STT ──► `HarnessSession::prepare_turn` ──► Duplex Dialogue Pipe (The Cognitive Front Door)

- **Status:** `[ ] Needs Rework (Harness v2 Real Model Upgrade & Contract Alignment)`
- **File:** `tests/transcript_to_llm_test.rs`
- **Category:** Integration Test
- **Subsystems:** `pipeline/assistant/transcript.rs`, `services/harness/session.rs`, `services/harness/plugins/compaction.rs`, `services/llm/actor.rs`, `services/llm/embedded.rs`
- **Prerequisites:** Local Qwen 3.5 GGUF in `~/.vox/models/llm/qwen/` or `NVIDIA_API_KEY` in `temp/.env`
- **Execution:** `cargo nextest run --test transcript_to_llm_test --release --nocapture --test-threads=1`
- **Metrics:** Context preparation latency, token budget utilization, filler dispatch on critical threshold, GenerationRequest assembly, duplex pipe token stream initiation.

> [!NOTE]
> **Rework Requirement Ledger:**
>
> 1. `tests/transcript_to_llm_test.rs` currently instantiates a synthetic in-memory mock provider (`TestMockLlmProvider`) which emits static tokens (`"Hello from mock LLM"`). Under the Zero-Mock invariant, it must be upgraded in an implementation sprint to exercise the real local `EmbeddedProvider` (Qwen GGUF) or remote Nvidia NIM model.
> 2. Subtest 4 asserts legacy Phase 10 logic (`pending_synthesis_jobs.store(1)` in `assistant/transcript.rs:on_transcript_final`), which was removed during the Harness v2 / Realtime refactor where `RealtimeActor` manages its own streaming guard. The assertion expectations must be aligned with Harness v2 contracts.

#### 5.1 `/create-test` Phase 0 — Classification

- **Type:** Integration Test.
- **Rationale:** Verifies the cognitive front door: STT finalized user transcript enters `on_transcript_final`, triggers `HarnessSession::prepare_turn` to evaluate context utilization and compaction thresholds, dispatches interim transition filler if critical, assembles full system prompt + personal memory + history context into a `GenerationRequest`, and transmits it over the duplex dialogue pipe to `LlmActor`.

#### 5.2 `/create-test` Phase 1 — Production Path Trace

```
SUT: Final user transcript triggers context preparation (buffer push + threshold maintenance + prompt assembly) and dispatch of a GenerationRequest to the LLM worker; the real LLM generates tokens into the accumulator and emits LlmFinished.

Production Entry Seam:
  on_transcript_final(turn_id, text, &app, &state, &ctx) with valid non-empty text delivered while in InteractionState::Thinking.
  Direction Check: PASS — entry seam is the upstream STT final transcript event, NOT the LLM command sink.

Production Path:
  on_transcript_final(turn_id, text, app, state, ctx)
  ──► Guard check: state must be InteractionState::Thinking (drops otherwise)
  ──► Transliterate Hindi if enabled (transliterate_if_hi)
  ──► Empty transcript check:
      ├─► If trimmed.is_empty(): clears accumulator, transitions to InteractionState::Ready,
      │   dispatches transient notification "assistant:empty_speech", returns immediately
      └─► If valid: stores query in TurnAccumulator, emits IpcEvent::TranscriptFinal to WINDOW_MAIN
  ──► Branch on ctx.pipeline_mode:
      ├─► PipelineMode::Realtime: transcript logging only; pending handled in Realtime actor stream
      └─► PipelineMode::Modular: spawns spawn_modular_llm_task
          ──► Acquires HarnessSession lock:
              ├─► harness.prepare_turn(&query, turn_id):
              │   ├─► Context utilization >= 0.85 (Critical):
              │   │   transitions to InteractionState::Working,
              │   │   dispatches AudioIntent::InterimFiller to TtsActor,
              │   │   increments pending_synthesis_jobs,
              │   │   executes inline compaction via CompactionStage::run_and_persist,
              │   │   applies compaction summary, falls back to FIFO on failure
              │   └─► Context utilization < 0.85:
              │       appends ChatMessage::user(&query) to working history buffer
              └─► harness.create_generation_request()
          ──► Transmits generation request over session duplex channel (request_tx) to LlmActor
          ──► LlmActor generates tokens ──► streamed back via response_rx to StreamRoutingStage
          ──► StreamRoutingStage demuxes <response> tags and dispatches clauses to TTS.

Observable Exit:
  1. Valid query (<0.85): GenerationRequest dispatched over duplex pipe; LlmActor emits token stream; LlmFinished arrives.
  2. Critical threshold (>=0.85): Pipeline enters Working; interim filler dispatched to tts_tx with AudioIntent::InterimFiller; pending_synthesis_jobs incremented.
  3. Empty query: State transitions directly to Ready; zero generation requests sent to LlmActor.
  4. Non-Thinking drop: Transcript arriving in Listening or Paused is dropped with zero side effects.

Production Functions Called:
  setup:   get_test_app_and_state, EmbeddedProvider::new(2048, 4), HarnessSession initialization,
           attach_mock_engine_with_llm_vad_to_state
  entry:   on_transcript_final(turn_id, text, &app, &state, &ctx)
  observe: state.pipeline.state(), pending_synthesis_jobs, duplex channel responses, tts_rx for filler
```

#### 5.3 `/create-test` Phase 2a — Testability Check

1. Entry seam callable with production signature? **Yes** (`on_transcript_final`).
2. Production constructors used? **Yes** (`HarnessSession`, `EmbeddedProvider`, `AppState`).
3. State and channels observable? **Yes** (`state.pipeline.state()`, `pending_synthesis_jobs`, duplex channel).
4. Real LLM without synthetic mocks? **Target Requirement:** Uses real `EmbeddedProvider` with local Qwen 3.5 GGUF or remote Nvidia NIM.

#### 5.4 `/create-test` Phase 2b — False-Green Audit Table

| If this production defect existed                                 | Would Seam 5 test fail? | Expected Failure Mode                                                                         |
| ----------------------------------------------------------------- | ----------------------- | --------------------------------------------------------------------------------------------- |
| **Upstream `on_transcript_final` never invoked**                  | **Must fail**           | `LlmActor` never receives request; channel assertion fails.                                   |
| `on_transcript_final` drops valid transcript (guard misfires)     | Must fail               | State stays in `Thinking`; zero requests dispatched over duplex pipe.                         |
| Empty transcript fails to guard to `Ready`                        | Must fail               | Subtest 2 fails: empty query passes to LLM instead of recovering to `Ready`.                  |
| Critical threshold ($\ge 0.85$) fails to trigger filler           | Must fail               | Subtest 5 fails: `tts_rx` receives no `InterimFiller` clause; state does not enter `Working`. |
| Duplex pipe `request_tx.send` omitted in `spawn_modular_llm_task` | Must fail               | `LlmActor` never generates tokens; stream routing times out.                                  |

#### 5.5 `/test` Execution Protocol

- **Command:** `cargo nextest run --test transcript_to_llm_test --release --nocapture --test-threads=1`
- **Success Criteria:**
  1. Subtest 1 (Valid Dispatch): Valid transcript in `Thinking` enters `HarnessSession`, dispatches `GenerationRequest`, and yields real generated tokens.
  2. Subtest 2 (Empty Guard): Whitespace transcript reverts state to `Ready` without dispatching to LLM.
  3. Subtest 3 (State Guard): Transcript arriving in `Listening` or `Paused` is dropped without state change.
  4. Subtest 4 (Critical Compaction & Filler): Pre-seeded buffer ($\ge 0.85$) transitions to `Working`, dispatches `AudioIntent::InterimFiller` to TTS, and increments `pending_synthesis_jobs`.
- **Failure Signatures:**
  - `DuplexPipeTimeout`: Indicates stall or missing sender in `spawn_modular_llm_task`.
  - `MissingFiller`: Critical context threshold failed to trigger interim speech filler.

#### 5.6 `/mutate` Mutant Definitions (Tier 1)

- **Mutant 5.1 (Gate Inversion):** In `pipeline/assistant/transcript.rs:on_transcript_final`, delete `if trimmed.is_empty()` guard.  
  _Prediction:_ Subtest 2 goes RED because whitespace transcripts are sent to the LLM.
- **Mutant 5.2 (Silent Drop):** In `pipeline/assistant/transcript.rs:spawn_modular_llm_task`, comment out `harness.create_generation_request()`.  
  _Prediction:_ Subtest 1 goes RED because no request is dispatched over the duplex pipe.
- **Mutant 5.3 (Threshold Flip):** In `services/harness/plugins/budget.rs`, set critical threshold to `1.5` (unreachable).  
  _Prediction:_ Subtest 5 goes RED because critical context never triggers interim filler.

---

### Seam 6 — Cognitive Stage: `HarnessSession` Orchestration ◄──► `LlmActor` Duplex Pipe ──► TTS Dispatch

- **Status:** `[x] Solid (Verified Green & Validated)`
- **File:** `tests/llm_to_tts_test.rs`
- **Category:** Integration Test
- **Subsystems:** `services/harness/`, `services/harness/loop.rs`, `services/harness/steps.rs`, `services/harness/stages/streaming/router.rs`, `services/llm/actor.rs`, `services/llm/embedded.rs`, `pipeline/assistant/llm.rs`, `services/tts/actor.rs`
- **Prerequisites:** Local Qwen 3.5 GGUF model in `~/.vox/models/llm/qwen/`
- **Execution:** `cargo nextest run --test llm_to_tts_test --release --nocapture --test-threads=1`
- **Metrics:** Real token generation, pass-level clause buffering, drop-all-prefix on tool proposals, atomic `pending_synthesis_jobs` accounting, scratchpad context budget tracking, 5-pass loop bound.

#### 6.1 `/create-test` Phase 0 — Classification

- **Type:** Integration Test.
- **Rationale:** Verifies the complete Cognitive Stage orchestration under Harness v2: `HarnessSession` manages turn context and budget, executes the reentrant cognitive loop (`loop.rs`), streams tokens through `StreamRouter` with clause buffering and drop-all-prefix, dispatches speakable clauses to `tts_tx` with `AudioIntent::TurnResponse`, and finalizes the turn via `commit_turn` and quiet watcher scheduling.

#### 6.2 `/create-test` Phase 1 — Production Path Trace

```
SUT: Real local Qwen LLM generation streams tokens across duplex channel into StreamRouter, buffers clauses in buffered_clauses, drops all prefix text on ToolCallReceived, chunks speakable clauses via TtsClauseChunker, and dispatches real TtsCommand::Generate synthesis jobs with AudioIntent::TurnResponse, atomic pending accounting, and remainder flush.

Production Entry Seam:
  Harness::execute_turn(harness_arc, TurnExecutionRequest { query, turn_id, ... })
  Direction Check: PASS — entry seam is the upstream Cognitive Stage orchestration trigger, NOT the TTS sink.

Production Path:
  execute_turn()
  ──► step1_intake() [dedup + initial budget check]
  ──► loop.rs: run_cognitive_loop() ──► execute_loop_iterations()
      ──► check_loop_budget() [tracks history + scratchpad + tool schemas]
      ──► step4_assemble_request() [PromptBuilderStage::build_generation_request]
      ──► step5_dispatch_llm() [duplex pipe send]
      ──► step6_run_stream_pass() ──► StreamRouter::route_stream()
          ├─► Demuxes <response> tags from token stream
          ├─► Buffers clauses in buffered_clauses
          ├─► If ToolCallReceived: drops buffered_clauses, clears chunker, returns call
          └─► If Completed: flushes buffered_clauses to tts_tx
      ──► Loop iteration control:
          ├─► iteration < 5: appends to scratchpad, continues loop
          └─► iteration == 5: sets allow_tools = false, runs final tool-free text pass
  ──► step7_commit_completed() / step7_handle_cancelled()
```

#### 6.3 `/create-test` Phase 2a — Testability Check

1. Entry seam callable with production signature? **Yes** (`harness.prepare_turn`, `harness.execute_turn`, `llm_tx.send`).
2. Production constructors used? **Yes** (`HarnessSession`, `EmbeddedProvider`, `spawn_llm_worker`).
3. Output channels and atomics observable? **Yes** (`tts_rx`, `pending_synthesis_jobs`, `event_rx`, `history`).
4. Real LLM model without mocks? **Yes** (Real local Qwen 3.5 GGUF).

#### 6.4 `/create-test` Phase 2b — False-Green Audit Table

| If this production defect existed | Would Seam 6 test fail? | Expected Failure Mode |
|---|---|---|
| **Upstream `LlmCommand::Generate` dropped / Qwen model fails to load** | **Must fail** | `tts_rx` empty after timeout; `VoxEvent::LlmFinished` never emitted. |
| Prefix text eagerly dispatched to TTS on ToolCall | **Must fail** | `tts_rx` receives unexpected `TurnResponse` audio packets before tool execution. |
| Reentrant loop does not terminate at 5 iterations | **Must fail** | Loop exceeds 5 passes or hangs; hard timeout triggers panic. |
| Scratchpad tokens omitted from reentrant budget check | **Must fail** | Context budget remains nominal despite large scratchpad payload. |
| Final pass still invokes tools | **Must fail** | Tool call executed when `allow_tools == false`. |
| Tail remainder flush deleted on `LlmFinished` (`assistant/llm.rs:47`) | Must fail | `pending_synthesis_jobs` mismatch against dispatched clauses. |
| Dispatched clauses misclassified as `AudioIntent::InterimFiller` | Must fail | `assert_eq!(intent, AudioIntent::TurnResponse)` panics. |

#### 6.5 `/test` Execution Protocol

- **Command:** `cargo nextest run --test llm_to_tts_test --release --nocapture --test-threads=1`
- **Success Criteria:**
  1. `VoxEvent::LlmFinished` arrives within 30s deadline.
  2. `accumulator.assistant_response` captures full generated text.
  3. At least 1 streaming clause dispatched to `tts_rx` before `LlmFinished`.
  4. Flushed remainder dispatched on `on_llm_finished`.
  5. `pending_synthesis_jobs` exactly equals `all_clauses.len()`.
  6. All clauses tagged with `AudioIntent::TurnResponse`.
  7. On tool call proposal, zero prefix audio is emitted to `tts_rx`.
- **Failure Signatures:**
  - `LlmFinished timeout`: Worker stalled or token stream did not close.
  - `pending != clauses.len()`: Atomic accounting discrepancy in stream router.

#### 6.6 `/mutate` Mutant Definitions (Tier 1)

- **Mutant 6.1 (Silent Drop):** In `services/harness/plugins/stream.rs:route_stream`, comment out `tts_tx.send(TtsCommand::Generate { .. })`.  
  _Prediction:_ Test goes RED on `assert!(!streaming_clauses.is_empty())`.
- **Mutant 6.2 (Boundary Flip):** In `pipeline/assistant/llm.rs:on_llm_finished`, delete `flush_modular_tts_remainder` call.  
  _Prediction:_ Test goes RED on `assert_eq!(final_pending, all_clauses.len())` due to unflushed tail remainder.
- **Mutant 6.3 (Intent Swap):** In `services/harness/plugins/stream.rs`, set clause intent to `AudioIntent::InterimFiller` instead of `TurnResponse`.  
  _Prediction:_ Test goes RED on `assert_eq!(intent, AudioIntent::TurnResponse)`.
- **Mutant 6.4 (Prefix Drop Bypass):** In `services/harness/stages/streaming/router.rs`, flush buffered clauses to TTS immediately upon creation rather than holding until outcome.  
  _Prediction:_ Test goes RED on tool-calling test case with unexpected audio in `tts_rx`.

---

### Seam 7 — TTS Synthesis ──► Playback Sample Ingestion & Pre-roll Gates

- **Status:** `[ ] Needs Rework (Manual Handler Invocation & Subtest 2 TTS Bypass Elimination)`
- **File:** `tests/tts_to_playback_test.rs`
- **Category:** Integration Test
- **Subsystems:** `services/tts/actor.rs`, `services/tts/providers/supertonic.rs`, `services/audio/playback.rs`, `pipeline/assistant/playback.rs`
- **Prerequisites:** Local Supertonic ONNX models in `~/.vox/models/tts/supertonic-3/`
- **Execution:** `cargo nextest run --test tts_to_playback_test --release --nocapture --test-threads=1`
- **Metrics:** Real ONNX audio synthesis (RMS > 0.001), 24kHz to 48kHz 2x upsampling, 12,000-sample pre-roll cushion arming, short utterance `flush_pre_roll`, atomic `pending_synthesis_jobs` decrement.

> [!NOTE]
> **Rework Requirement Ledger:**
>
> 1. **Manual Router Handler Invocation in Subtest 1:** In lines 176–187 of `tests/tts_to_playback_test.rs`, the test manually calls `on_playback_started(turn_id, ...)` directly instead of having the central pipeline router receive `VoxEvent::PlaybackStarted` from `event_tx` and execute the state transition `Thinking -> Speaking`.
> 2. **TTS Actor Bypassed in Subtest 2:** In `test_tts_to_playback_short_utterance_flush` (lines 255–270), the test pushes audio directly into `engine.ingest_chunk(&short_chunk)` and manually invokes `engine.flush_pre_roll()`. It completely bypasses `spawn_tts_worker`. The production path under test is that when `spawn_tts_worker` finishes synthesizing a chunk and observes `pending_synthesis_jobs <= 1`, the worker loop itself automatically calls `handles.playback.flush_pre_roll()`. Subtest 2 tests none of the worker loop accounting.
> 3. **Rework Scope:** In an implementation sprint, Subtest 2 must send a short text string to `TtsCommand::Generate`, verifying that the worker thread completes synthesis, decrements `pending_synthesis_jobs`, and automatically triggers `flush_pre_roll()` to arm playback. Modifying test code is out of scope for this classification pass.

#### 7.1 `/create-test` Phase 0 — Classification

- **Type:** Integration Test.
- **Rationale:** Verifies the physical speech synthesis and audio rendering boundary: `TtsCommand::Generate` received by dedicated TTS worker thread, executed against real local Supertonic ONNX engine, PCM audio upsampled 2x and pushed into lock-free playback ring buffer, pre-roll cushion gate evaluated to emit `VoxEvent::PlaybackStarted`, pipeline state transitioned from `Thinking` to `Speaking`, and worker-driven short utterance `flush_pre_roll`.

#### 7.2 `/create-test` Phase 1 — Production Path Trace

```
SUT: Real Supertonic ONNX text-to-speech synthesis jobs generate real PCM audio from text, upsample 2x to 48kHz, ingest into the lock-free playback ring buffer, satisfy the pre-roll cushion threshold gate (12,000 samples for Modular Assistant), transition state to Speaking, and flush unplayed samples on completion.

Production Entry Seam:
  TtsCommand::Generate { turn_id, text, intent } sent to tts_tx.
  Direction Check: PASS — entry seam is the upstream TTS dispatch command from StreamRoutingStage, NOT direct injection into PlaybackEngine.

Production Path A (Standard Utterance > Pre-roll Threshold):
  tts_tx.send(TtsCommand::Generate { turn_id, text, intent })
  ──► spawn_tts_worker loop (services/tts/actor.rs)
  ──► SupertonicEngine::synthesize_chunk (services/tts/providers/supertonic.rs):
      ├─► Tokenizes phonemes / text via ONNX text encoder
      ├─► Runs duration predictor, vector estimator, and vocoder
      └─► Progressively pushes generated 24kHz f32 PCM slices into ctx.playback
  ──► PlaybackEngine::ingest_chunk_with_threshold(chunk_24k, MODULAR_PREROLL_THRESHOLD_SAMPLES = 12000):
      ├─► Checks cancel_flag (aborts if cancelled)
      ├─► upsample_2x_into(chunk_24k, scratch) converts 24kHz to 48kHz
      ├─► prod.push_slice(scratch) pushes samples into SPSC HeapRb
      └─► Gate 1 (Start): If !turn_armed && occupied_len >= 12000:
          ├─► turn_armed.store(true)
          └─► emits VoxEvent::PlaybackStarted { turn_id, intent } on event_tx
  ──► pipeline/assistant/playback.rs:on_playback_started:
      transitions state from InteractionState::Thinking to InteractionState::Speaking
  ──► After chunk synthesis returns in worker loop:
      ├─► if pending_synthesis_jobs Some:
      │   let remaining = jobs.fetch_sub(1, Relaxed);
      │   if remaining <= 1 { handles.playback.flush_pre_roll(); }
  ──► pending_synthesis_jobs decrements to 0.

Production Path B (Short Utterance < Pre-roll Threshold):
  TtsCommand::Generate with short text (<12,000 samples after upsampling)
  ──► ingest_chunk_with_threshold pushes samples; occupied < 12000
  ──► turn_armed remains false; zero events emitted
  ──► Synthesis finishes; remaining <= 1 triggers handles.playback.flush_pre_roll()
  ──► flush_pre_roll checks: occupied > 0 && !turn_armed:
      ├─► turn_armed.store(true)
      └─► emits VoxEvent::PlaybackStarted { turn_id, intent } immediately.

Observable Exit:
  1. Real Supertonic ONNX synthesis generates valid non-silent PCM audio (RMS > 0.001 across drained samples).
  2. Playback ring buffer contains occupied samples (occupied > 0).
  3. VoxEvent::PlaybackStarted arrives on event_rx with matching turn_id and intent.
  4. State transitions from Thinking to Speaking.
  5. pending_synthesis_jobs decrements to 0 upon completion.
  6. Short utterance (<12,000 samples) holds arming until flush_pre_roll, then arms immediately.

Production Functions Called:
  setup:   get_test_app_and_state(), SupertonicEngine::new(&model_dir, voice, steps, speed, threads),
           create_mock_playback_engine_with_handles, spawn_tts_worker
  entry:   tts_tx.send(TtsCommand::Generate { turn_id, text, intent })
  observe: event_rx for PlaybackStarted, playback_engine.buffer_len(), consumer_arc.try_pop(),
           pending_synthesis_jobs, state.pipeline.state()
```

#### 7.3 `/create-test` Phase 2a — Testability Check

1. Entry seam callable with production signature? **Yes** (`TtsCommand::Generate { turn_id, text, intent }`).
2. Production constructors used? **Yes** (`SupertonicEngine`, `spawn_tts_worker`, `PlaybackEngine`).
3. Channels and atomics observable? **Yes** (`event_rx`, `buffer_len()`, `pending_synthesis_jobs`, `state`).
4. Real TTS engine without synthetic mocks? **Yes** (Real local Supertonic-3 ONNX models).

#### 7.4 `/create-test` Phase 2b — False-Green Audit Table

| If this production defect existed                                          | Would Seam 7 test fail? | Expected Failure Mode                                                             |
| -------------------------------------------------------------------------- | ----------------------- | --------------------------------------------------------------------------------- |
| **Upstream TTS worker silent / drops `TtsCommand::Generate`**              | **Must fail**           | `event_rx` receives no `PlaybackStarted`; 30s timeout assertion panics.           |
| Supertonic ONNX inference broken (e.g. invalid model path)                 | Must fail               | Constructor fails or generates 0 samples; buffer remains empty (`occupied == 0`). |
| Vocoder emits pure silence / NaNs                                          | Must fail               | `rms > 0.001` assertion fails.                                                    |
| Pre-roll threshold cushion gate deleted (`turn_armed` set unconditionally) | Must fail               | Subtest 2 fails: short utterance arms immediately before `flush_pre_roll`.        |
| `flush_pre_roll` omitted or disabled                                       | Must fail               | Subtest 2 fails: short utterance (<12,000 samples) never arms and times out.      |
| `pending_synthesis_jobs.fetch_sub` omitted in `spawn_tts_worker`           | Must fail               | Subtest 1 fails: `pending_jobs` remains 1; assertion `pending == 0` fails.        |

#### 7.5 `/test` Execution Protocol

- **Command:** `cargo nextest run --test tts_to_playback_test --release --nocapture --test-threads=1`
- **Success Criteria:**
  1. Subtest 1 (Real Synthesis & Pre-roll): `PlaybackStarted` emitted within 30s; ring buffer occupied; drained samples RMS > 0.001; state transitions to `Speaking`; `pending_synthesis_jobs` reaches 0.
  2. Subtest 2 (Short Utterance Flush): Chunk < 12,000 samples does NOT arm buffer; `flush_pre_roll()` immediately arms and emits `PlaybackStarted`.
  3. Worker joins cleanly upon `TtsCommand::Shutdown`.
- **Failure Signatures:**
  - `PlaybackStarted timeout`: Worker hung, model failed, or pre-roll gate deadlocked.
  - `RMS <= 0.001`: Vocoder failure or model output corrupted.
  - `pending != 0`: Atomic counter leak in worker loop.

#### 7.6 `/mutate` Mutant Definitions (Tier 1)

- **Mutant 7.1 (Pre-roll Gate Inversion):** In `services/audio/playback.rs:ingest_chunk_with_threshold`, change `if occupied >= preroll_threshold` to `if true`.  
  _Prediction:_ Subtest 2 goes RED because short chunks arm immediately on push instead of waiting for flush.
- **Mutant 7.2 (Flush Deletion):** In `services/audio/playback.rs:flush_pre_roll`, comment out `self.turn_armed.store(true, Ordering::Relaxed)`.  
  _Prediction:_ Subtest 2 goes RED on `assert!(turn_armed.load(...))`.
- **Mutant 7.3 (Accounting Drop):** In `services/tts/actor.rs:spawn_tts_worker`, comment out `jobs.fetch_sub(1, Ordering::Relaxed)`.  
  _Prediction:_ Subtest 1 goes RED on `assert_eq!(pending_jobs.load(Ordering::Relaxed), 0)`.

---

### Seam 8 — TTS Transition & Voice Hot-Swap (`AudioIntent` Gating)

- **Status:** `[x] Solid (Verified Green & Validated)`
- **File:** `tests/tts_transition_test.rs`
- **Category:** Integration Test
- **Subsystems:** `ipc/settings/core.rs`, `services/tts/actor.rs`, `services/tts/providers/supertonic.rs`, `pipeline/assistant/transcript.rs`, `pipeline/assistant/playback.rs`, `services/harness/session.rs`, `services/audio/playback.rs`, `services/harness/steps.rs`
- **Prerequisites:** Local Supertonic ONNX model in `~/.vox/models/tts/supertonic-3/`
- **Execution:** `cargo nextest run --test tts_transition_test --release --nocapture --test-threads=1`
- **Metrics:** Settings IPC voice hot-swap without worker thread restart, critical threshold compaction filler dispatch, Working state gating on InterimFiller, speech normalization of filler clauses, at-most-once filler guard (`has_played_filler`).

#### 8.1 `/create-test` Phase 0 — Classification

- **Type:** Integration Test.
- **Rationale:** Verifies the runtime voice switching and transitional speech gating:
  1. Settings IPC updates `voice_index`, sending `TtsCommand::SetVoice` to update active speaker embeddings in `SupertonicEngine` without worker thread termination.
  2. History context exceeding 85% utilization triggers `on_transcript_final` $\to$ `spawn_modular_llm_task` to transition the pipeline to `InteractionState::Working`, increment `pending_synthesis_jobs`, and dispatch `AudioIntent::InterimFiller` to TTS, holding the pipeline in `Working` through playback until finalized turn response synthesis.
  3. NonTerminal execution (`enter_non_terminal_phase`) enforces `has_played_filler` so compaction cycles within a turn never play duplicate fillers, and applies speech normalization to `spoken_filler` before synthesis.

#### 8.2 `/create-test` Phase 1 — Production Path Trace

```
SUT: TTS voice hot-swap updates speaker embeddings without dropping in-flight synthesis; critical context budget threshold in HarnessSession dispatches interim transition filler with AudioIntent::InterimFiller to mask compaction latency; speech normalization cleans filler text.

Production Entry Seam A (Voice Hot-Swap):
  ipc::settings::core::update_setting("tts", "voice_index", json!(2)) [Production Seam]
  Direction Check: PASS — entry seam is the user/frontend settings mutation trigger, propagating down to tts_tx.

Production Path A:
  User changes voice in settings UI ──► ipc::settings::core::update_setting
  ──► Dispatches TtsCommand::SetVoice(2) to tts_tx
  ──► spawn_tts_worker loop processes serially:
      ├─► In-flight Clause A synthesizes with initial voice 0
      ├─► TtsCommand::SetVoice(2) calls provider.set_voice(2) (SupertonicEngine updates active voice)
      └─► Next clause synthesizes with voice 2 without worker restart
  ──► pending_synthesis_jobs decrements cleanly; audio streams into playback buffer.

Production Entry Seam B (Compaction Transition Filler & Working State):
  on_transcript_final(turn_id, text, &app, &state, &ctx) when context utilization >= 85%.
  Direction Check: PASS — entry seam is finalized STT user query entering cognitive stage.

Production Path B:
  on_transcript_final ──► spawn_modular_llm_task
  ──► harness.prepare_turn(&query, turn_id)
  ──► ContextBudgetStage evaluates utilization (tracked_tokens >= 85% of budget)
  ──► Returns TurnPreparation::NeedsInlineCompaction { filler_phrase, uncompacted_slice }
  ──► transition(InteractionState::Working, &ctx, &app, &state)
  ──► pending_jobs.fetch_add(1)
  ──► enters enter_non_terminal_phase:
      ├─► Normalizes filler text (strips markdown, formats numbers)
      ├─► Checks has_played_filler: skips if already played in this turn
      └─► tts_tx.send(TtsCommand::Generate { turn_id, text: normalized_filler, intent: AudioIntent::InterimFiller })
  ──► TTS synthesizes filler phrase ──► PlaybackEngine::ingest_chunk_with_intent(..., InterimFiller)
  ──► on_playback_started: intent == InterimFiller leaves pipeline locked in Working (NOT Speaking)
  ──► on_playback_finished: intent == InterimFiller leaves pipeline locked in Working (NOT Ready)
  ──► Inline compaction completes ──► Main LLM response streams with AudioIntent::TurnResponse.

Observable Exit:
  1. Path A: Settings mutation triggers voice hot-swap; subsequent synthesis uses new voice; pending drops to 0; RMS > 0.001.
  2. Path B: on_transcript_final transitions to Working; pending_jobs increments; InterimFiller dispatched to tts_tx.
  3. Playback gating: During filler playback, on_playback_started and on_playback_finished preserve Working state.
  4. Non-critical turn (<85%) remains in Thinking, returns TurnPreparation::Ready, and dispatches zero filler.
  5. Filler text sent to TTS contains zero unnormalized markdown symbols.

Production Functions Called:
  setup:   get_test_app_and_state(), SupertonicEngine::new(&model_dir, 0, 2, 1.0, 4),
           HarnessSession::new_modular(...), spawn_tts_worker
  entry:   update_setting("tts", "voice_index", json!(2)), on_transcript_final(turn_id, text, &app, &state, &ctx)
  observe: pending_synthesis_jobs, state.pipeline.state(), tts_rx for InterimFiller, playback gate states
```

#### 8.3 `/create-test` Phase 2a — Testability Check

1. Entry seams callable with production signatures? **Yes** (`tts_tx.send(TtsCommand::SetVoice)`, `harness.prepare_turn`).
2. Production constructors used? **Yes** (`SupertonicEngine`, `HarnessSession::new_modular`).
3. State and channels observable? **Yes** (`pending_synthesis_jobs`, `tts_rx`, `consumer_arc`).
4. Real local models used? **Yes** (Real local Supertonic-3 ONNX model).

#### 8.4 `/create-test` Phase 2b — False-Green Audit Table

| If this production defect existed | Would Seam 8 test fail? | Expected Failure Mode |
|---|---|---|
| **Upstream `TtsCommand::SetVoice` dropped / ignored in worker** | **Must fail** | Second clause synthesizes with old voice; voice mutation fails. |
| `SetVoice` clears pending jobs prematurely | Must fail | `pending_jobs` drops to 0 before synthesis completes; assertion fails. |
| `SetVoice` drops in-flight generation jobs | Must fail | Ring buffer missing second clause audio; sample count / RMS assertion fails. |
| Critical threshold fails to return `NeedsInlineCompaction` | Must fail | Path B panics on `Expected NeedsInlineCompaction`. |
| Filler command tagged as `TurnResponse` instead of `InterimFiller` | Must fail | `assert_eq!(intent, AudioIntent::InterimFiller)` fails. |
| Non-critical turn dispatches spurious filler | Must fail | `tts_rx.try_recv().is_err()` assertion fails. |
| `has_played_filler` does not guard compaction filler | Must fail | Multiple compactions in same turn dispatch duplicate filler audio to `tts_tx`. |
| Unnormalized filler sent to TTS (raw symbols/abbreviations) | Must fail | `TtsCommand::Generate` text does not match normalized speech rules. |

#### 8.5 `/test` Execution Protocol

- **Command:** `cargo nextest run --test tts_transition_test --release --nocapture --test-threads=1`
- **Success Criteria:**
  1. Subtest 1 (`test_tts_voice_switch_without_worker_restart`): Voice 0 $\to$ SetVoice(2) $\to$ Voice 2 completes cleanly; `pending_synthesis_jobs` returns to 0; drained samples RMS > 0.001.
  2. Subtest 2 (`test_compaction_filler_dispatch_and_pending_accounting`): Critical threshold triggers `NeedsInlineCompaction`; filler belongs to `TRANSITION_MESSAGES_EN`; `AudioIntent::InterimFiller` dispatched; normal turn yields `Ready` with zero filler.
  3. Subtest 3 (`test_spoken_filler_speech_normalization`): Raw markdown filler normalized before TTS dispatch.
- **Failure Signatures:**
  - `NeedsInlineCompaction match panic`: Context window <= 4096 or message count < 4 for embedded model.
  - `RMS <= 0.001`: Synthesis failed across voice hot-swap.

#### 8.6 `/mutate` Mutant Definitions (Tier 1)

- **Mutant 8.1 (Voice Switch Deletion):** In `services/tts/actor.rs:spawn_tts_worker`, comment out `provider.set_voice(voice)` in the `TtsCommand::SetVoice` match arm.  
  _Prediction:_ Subtest 1 fails to update engine speaker embedding.
- **Mutant 8.2 (Intent Corruption):** In `pipeline/assistant/transcript.rs` (or test dispatch), change filler intent from `AudioIntent::InterimFiller` to `AudioIntent::TurnResponse`.  
  _Prediction:_ Subtest 2 goes RED on `assert_eq!(intent, AudioIntent::InterimFiller)`.
- **Mutant 8.3 (Threshold Inversion):** In `services/harness/plugins/budget.rs:evaluate_utilization`, invert `utilization >= critical` to `< critical`.  
  _Prediction:_ Subtest 2 goes RED because seeded critical buffer returns `Ready` and normal buffer triggers compaction.
- **Mutant 8.4 (Filler Deduplication Bypass):** In `services/harness/steps.rs`, comment out `has_played_filler` check in `enter_non_terminal_phase`.  
  _Prediction:_ Subtest 2 goes RED on repeated compaction with duplicate filler emission.

---

### Seam 9 — Playback Lifecycle, VAD Ducking & 6-Step Barge-in Sequence

- **Status:** `[x] Solid (Verified Green & Validated)`
- **File:** `tests/playback_interrupt_test.rs`
- **Category:** Integration Test
- **Subsystems:** `services/audio/playback.rs`, `services/audio/sink.rs`, `pipeline/assistant/playback.rs`, `pipeline/assistant/interrupt.rs`, `pipeline/router.rs`, `services/vad/actor.rs`, `pipeline/atomics.rs`
- **Prerequisites:** Local Earshot VAD ONNX model in `~/.vox/models/` + test audio assets in `tests/assets/`
- **Execution:** `cargo nextest run --test playback_interrupt_test --release --nocapture --test-threads=1`
- **Metrics:** Pre-roll cushion threshold (12,000 samples), autonomous sink callback `PlaybackFinished` emission, `pending_synthesis_jobs` deferral, VAD speaker ducking suppression, 6-step barge-in state mutations, Invariant 13 partial persistence, Invariant 14 synthesis guard latch (`drained_while_open`).

#### 9.1 `/create-test` Phase 0 — Classification

- **Type:** Integration Test.
- **Rationale:** Verifies the physical audio playback rendering lifecycle, acoustic echo suppression, barge-in, and Phase 12 synthesis latches:
  1. Playback gating: 12,000-sample pre-roll cushion arms playback and emits `PlaybackStarted` (transitioning `Thinking -> Speaking`).
  2. Sink completion: Audio buffer drain with `pending_synthesis_jobs == 0` emits `PlaybackFinished` (transitioning `Speaking -> Ready`); non-zero pending jobs defers completion.
  3. Acoustic suppression: `Speaker` mode suppresses VAD speech detection during `Speaking`; `Headset` mode preserves full duplex transparency.
  4. 6-step barge-in: Interruption during `Speaking` halts playback, advances turn ID, clears accumulator, cancels turn token, resets pending jobs, persists partial speech (Invariant 13), and transitions to `Listening`.
  5. Invariant 14 synthesis guard latch: If audio drains while the LLM generation stream is still open (`is_turn_open() == true`), latches `drained_while_open = true` and preserves `Speaking` state until `on_llm_finished` arrives.

#### 9.2 `/create-test` Phase 1 — Production Path Trace

```
SUT: PlaybackEngine buffer ingestion and sink callback enforce pre-roll arming and completion gating; VAD actor suppresses mic input during Speaker playback; user interruption triggers the 6-step canonical barge-in sequence; synthesis guard latch preserves Speaking state during active streaming.

Production Path A (Playback Lifecycle & Sink Callback):
  Ingest samples ──► PlaybackEngine::ingest_chunk_with_threshold(chunk, 12000)
  ──► If occupied >= 12000 && !turn_armed:
      ├─► turn_armed.store(true)
      └─► emits VoxEvent::PlaybackStarted { turn_id, intent } on event_tx
  ──► Router (spawn_router) ──► pipeline::assistant::playback::on_playback_started
      transitions Thinking/Working -> Speaking (if intent == TurnResponse)
  ──► CPAL Output Sink Callback (services/audio/sink.rs:137):
      ├─► While consumer has samples: plays out at 48kHz with volume ramps
      └─► When consumer.is_empty():
          ├─► If pending_synthesis_jobs > 0: records underrun, defers completion
          └─► If pending_synthesis_jobs == 0 && turn_armed:
              ├─► turn_armed.store(false)
              └─► emits VoxEvent::PlaybackFinished { turn_id, intent } on event_tx
  ──► Router ──► pipeline::assistant::playback::on_playback_finished:
      ├─► If pending_synthesis_jobs > 0: logs deferred, remains Speaking
      ├─► If pipeline.is_turn_open(): latches drained_while_open = true, remains Speaking
      └─► If pending_synthesis_jobs == 0 && !pipeline.is_turn_open(): transitions Speaking -> Ready.

Production Path B (VAD Speaker Ducking Suppression):
  State transitions to Speaking with AudioOutputMode::Speaker
  ──► VadActor loop checks state_atomic == Speaking && audio_mode == Speaker
  ──► audio_suppressed is set true
  ──► Incoming microphone frames (stream_audio_to_ring_buffer) are drained without evaluating speech onset
  ──► Zero VoxEvent::SpeechStart events emitted.
  When audio_mode is Headset (or state returns to Ready):
  ──► audio_suppressed is false ──► SpeechStart fires normally.

Production Path C (6-Step Canonical Barge-in Sequence):
  User presses PTT hotkey or speaks (Headset mode) while in Speaking
  ──► Router receives VoxEvent::PttStart (or SpeechStart)
  ──► Invokes pipeline::assistant::interrupt::on_interrupt:
      Step 1: engine.playback_engine.cancel() (aborts CPAL stream, clears ring buffer)
      Step 2: state.pipeline.cancel_flag.store(true) & turn_token.cancel()
      Step 3: state.pipeline.pending_synthesis_jobs.store(0)
      Step 4: Persists partial turn via persist_tx (PersistenceEvent::TurnCompleted)
      Step 5: state.pipeline_accumulator.lock().clear()
      Step 6: (new_turn_id, _) = state.pipeline.next_turn();
              transitions state to InteractionState::Listening.

Observable Exit:
  1. Pre-roll cushion >= 12,000 samples emits PlaybackStarted; router transitions to Speaking.
  2. Short utterance flushes via flush_pre_roll and immediately arms.
  3. Sink callback drains buffer: pending == 0 and turn_open == false emits PlaybackFinished; router transitions to Ready.
  4. Audio drain while turn_open == true latches drained_while_open and remains Speaking.
  5. Speaker playback suppresses SpeechStart; Headset playback allows SpeechStart.
  6. Barge-in strictly advances turn_id, cancels old token, clears accumulator, and enters Listening.

Production Functions Called:
  setup:   get_test_app_and_state(), create_mock_playback_engine_with_handles, setup_vad_actor, spawn_router
  entry:   playback_engine.ingest_chunk, sink_callback drain, event_tx.send(VoxEvent::PttStart)
  observe: event_rx for PlaybackStarted/PlaybackFinished, state.pipeline.state(),
           vox_event_rx for SpeechStart, state.pipeline.peek_turn_id(), drained_while_open latch
```

#### 9.3 `/create-test` Phase 2a — Testability Check

1. Entry seams callable with production signatures? **Yes** (`playback_engine.ingest_chunk`, `event_tx.send(PttStart)`).
2. Production constructors used? **Yes** (`PlaybackEngine`, `VadActor`, `spawn_router`).
3. State and channels observable? **Yes** (`event_rx`, `state.pipeline.state()`, `pending_synthesis_jobs`, `drained_while_open`).
4. Real VAD models used? **Yes** (Real local Earshot ONNX model).

#### 9.4 `/create-test` Phase 2b — False-Green Audit Table

| If this production defect existed | Would Seam 9 test fail? | Expected Failure Mode |
|---|---|---|
| **Upstream CPAL sink callback fails to emit `PlaybackFinished` when buffer empty** | **Must fail** | `PlaybackFinished` never emitted; state stays in `Speaking` indefinitely. |
| Pre-roll threshold gate omitted (arms immediately on 1 sample) | Must fail | Subtest 2 fails: short chunk arms before `flush_pre_roll`. |
| `pending_synthesis_jobs > 0` check deleted in `on_playback_finished` | Must fail | Subtest 3 fails: transitions to `Ready` while synthesis jobs in-flight. |
| VAD speaker ducking logic deleted in `VadActor` | Must fail | Subtest 4 fails: mic speech during Speaker playback emits `SpeechStart`. |
| Headset mode incorrectly suppresses mic input | Must fail | Subtest 5 fails: `SpeechStart` never fires in Headset mode. |
| Barge-in fails to reset `pending_synthesis_jobs` to 0 | Must fail | Subtest 6 fails: `pending_jobs` retains stale count from interrupted turn. |
| Barge-in fails to advance `turn_id` strictly monotonically | Must fail | Subtest 6 fails: `new_turn_id > old_turn_id` assertion panics. |
| Accumulator assistant response not cleared on interrupt | Must fail | Subtest 6 fails: accumulator retains text from interrupted turn. |
| Invariant 14: `on_playback_finished` flips to `Ready` while `turn_open=true` | Must fail | Subtest 7 fails: pipeline flips to `Ready` while generation stream active. |

#### 9.5 `/test` Execution Protocol

- **Command:** `cargo nextest run --test playback_interrupt_test --release --nocapture --test-threads=1`
- **Success Criteria:**
  1. Subtest 1: Ingest >= 12,000 samples $\to$ `PlaybackStarted` $\to$ `Speaking`; drain buffer with pending == 0 $\to$ `PlaybackFinished` $\to$ `Ready`.
  2. Subtest 2: Ingest < 12,000 samples does not arm; `flush_pre_roll` arms immediately.
  3. Subtest 3: `PlaybackFinished` with pending > 0 is deferred; state remains `Speaking`.
  4. Subtests 4 & 5: Speaker mode in `Speaking` suppresses `SpeechStart`; Headset mode in `Speaking` allows `SpeechStart`.
  5. Subtest 6: Barge-in cancels playback, advances turn ID, resets pending jobs to 0, cancels token, clears accumulator, and transitions to `Listening`.
  6. Subtest 7: Draining playback while `turn_open = true` latches `drained_while_open` without flipping state; `on_llm_finished` evaluates latch and restores `Ready`.
- **Failure Signatures:**
  - `PlaybackStarted timeout`: Cushion threshold not met or arming gate broken.
  - `State != Ready on finish`: Completion deferral logic inverted or pending counter corrupted.
  - `SpeechStart emitted during Speaker playback`: Ducking suppression failure.

#### 9.6 `/mutate` Mutant Definitions (Tier 1)

- **Mutant 9.1 (Pending Deferral Bypass):** In `pipeline/assistant/playback.rs:on_playback_finished`, delete the `if pending_jobs > 0 { return; }` guard.  
  _Prediction:_ Subtest 3 goes RED because calling finished with `pending == 1` transitions to `Ready` instead of remaining `Speaking`.
- **Mutant 9.2 (Ducking Inversion):** In `services/vad/actor.rs`, invert the speaker ducking condition so Speaker mode does not suppress audio.  
  _Prediction:_ Subtest 4 goes RED because `SpeechStart` is detected during playback.
- **Mutant 9.3 (Barge-in Pending Reset Deletion):** In `pipeline/assistant/interrupt.rs:on_interrupt`, comment out `state.pipeline.pending_synthesis_jobs.store(0, Ordering::Relaxed)`.  
  _Prediction:_ Subtest 6 goes RED on `assert_eq!(pending_synthesis_jobs, 0)`.
- **Mutant 9.4 (Accumulator Clear Omission):** In `pipeline/assistant/interrupt.rs`, comment out `state.pipeline_accumulator.lock().clear()`.  
  _Prediction:_ Subtest 6 goes RED on `Accumulator assistant response must be cleared on interrupt`.
- **Mutant 9.5 (Invariant 14 Latch Bypass):** In `pipeline/assistant/playback.rs:on_playback_finished`, delete `if state.pipeline.is_turn_open() { ... }` check.  
  _Prediction:_ Subtest 7 goes RED on premature `Ready` assertion.

---

### Seam 10 — Clause Chunking Determinism & Prosody Rules

- **Status:** `[x] Solid (Verified Green & Validated)`
- **File:** `tests/chunking_determinism_test.rs`
- **Category:** Integration Test
- **Subsystems:** `services/tts/actor.rs` (`TtsClauseChunker`), `pipeline/assistant/accumulator.rs` (`TurnAccumulator`), `services/harness/plugins/stream.rs`
- **Prerequisites:** None (pure deterministic logic, zero external models/networks)
- **Execution:** `cargo nextest run --test chunking_determinism_test --release --nocapture --test-threads=1`
- **Metrics:** Invariance across token fragmentation, emergency 20-word cap determinism, comma prosody gating stability, clean buffer flush.

#### 10.1 `/create-test` Phase 0 — Classification

- **Type:** Integration Test.
- **Rationale:** Verifies the text boundary extraction and prosody engine: incoming LLM streaming tokens arriving with varying token fragmentation boundaries (simulating LLM TPS and network packet jitter) pass through `TurnAccumulator::push_token`, chunk deterministically at valid sentence and sub-clause punctuation marks, enforce the emergency word cap, and flush cleanly without losing tail characters.

#### 10.2 `/create-test` Phase 1 — Production Path Trace

```
SUT: TurnAccumulator and TtsClauseChunker process streaming token slices and extract speakable sentence and sub-clause strings deterministically, independent of upstream tokenization fragmentation.

Production Entry Seam:
  TurnAccumulator::push_token(&token) called repeatedly as tokens stream over duplex channel.
  Direction Check: PASS — entry seam is the streaming token feed from StreamRoutingStage, NOT internal find_split_point leaf logic.

Production Path:
  StreamRoutingStage receives token over duplex channel
  ──► handles.accumulator.lock().push_token(&token)
      ├─► Appends token to assistant_response (full turn text)
      └─► chunker.push_str(&token)
          ├─► Appends token to internal buffer
          └─► extract_chunks() loop:
              find_split_point() evaluates:
              1. Emergency cap: If words >= 25 -> split at word 20
              2. Sentence terminators: '\n', '?', '!' split immediately
              3. Sub-clause boundaries: ',', ';', ':', '—', '–' split ONLY if preceding words >= 5
              4. Period ('.'): splits unless:
                 - Surrounded by digits (decimal guard: "3.14")
                 - Preceded by known honorific/abbreviation ("Dr.", "Mr.", "v1.0")
              Returns completed chunks: Vec<String>
  ──► On stream completion: accumulator.flush_chunker() drains tail remainder.

Observable Exit:
  1. Identical logical text fragmented into fine-grained tokens vs coarse multi-word tokens produces identical clause sequences byte-for-byte and order-preserved.
  2. Unpunctuated stream of 30 words produces exactly 2 chunks: 20 words (emergency split) and 10 words (flushed remainder).
  3. Comma preceded by < 5 words does not split; comma preceded by >= 5 words splits into 2 clauses.
  4. Buffer is completely empty after flush; clear() resets accumulator.

Production Functions Called:
  setup:   TurnAccumulator::new(), TtsClauseChunker::new()
  entry:   acc.push_token(tok), chunker.push_str(tok), acc.flush_chunker()
  observe: returned Vec<String> clauses, acc.chunker.is_empty(), word counts
```

#### 10.3 `/create-test` Phase 2a — Testability Check

1. Entry seam callable with production signature? **Yes** (`acc.push_token(&token)`, `acc.flush_chunker()`).
2. Production constructors used? **Yes** (`TurnAccumulator::new()`, `TtsClauseChunker::new()`).
3. State observable? **Yes** (`clauses`, `acc.chunker.is_empty()`).
4. Real components without mocks? **Yes** (Zero mocks, 100% production code).

#### 10.4 `/create-test` Phase 2b — False-Green Audit Table

| If this production defect existed                                  | Would Seam 10 test fail? | Expected Failure Mode                                            |
| ------------------------------------------------------------------ | ------------------------ | ---------------------------------------------------------------- |
| **Upstream `push_token` fails to append / drops tokens**           | **Must fail**            | `clauses_a` is empty; assertion `!clauses_a.is_empty()` panics.  |
| Split point scan runs right-to-left instead of left-to-right       | Must fail                | Clause order inverted; `assert_eq!(clauses_a, clauses_b)` fails. |
| Emergency word cap threshold off-by-one (splits at 19 or 21 words) | Must fail                | Subtest 2 fails: `chunk_0_word_count == 20` panics.              |
| Comma prosody gate (< 5 words) ignored (splits on all commas)      | Must fail                | Subtest 3 fails: `res1.len() == 1` assertion panics (got 2).     |
| Tail remainder flush loses unpunctuated final words                | Must fail                | Subtest 2 fails: `chunks_a.len() == 2` panics (got 1).           |

#### 10.5 `/test` Execution Protocol

- **Command:** `cargo nextest run --test chunking_determinism_test --release --nocapture --test-threads=1`
- **Success Criteria:**
  1. Subtest 1: `clauses_a == clauses_b` byte-for-byte; accumulator buffer empty after flush.
  2. Subtest 2: 30 unpunctuated words split into exactly 20-word chunk + 10-word remainder across fragmentations.
  3. Subtest 3: Comma with 3 words does not split; comma with 6 words splits.
- **Failure Signatures:**
  - `Determinism mismatch`: Non-deterministic state in token accumulator.
  - `Word count mismatch`: Emergency cap or prosody threshold drift.

#### 10.6 `/mutate` Mutant Definitions (Tier 1)

- **Mutant 10.1 (Comma Gate Inversion):** In `services/tts/actor.rs:find_split_point`, change `if word_count >= 5` to `if word_count >= 1`.  
  _Prediction:_ Subtest 3 goes RED because short comma clause splits prematurely (`res1.len()` is 2 instead of 1).
- **Mutant 10.2 (Emergency Cap Off-By-One):** In `services/tts/actor.rs:find_split_point`, change `target_word_count = 20` to `target_word_count = 21`.  
  _Prediction:_ Subtest 2 goes RED on `assert_eq!(chunk_0_word_count, 20)`.
- **Mutant 10.3 (Decimal Guard Deletion):** In `services/tts/actor.rs:find_split_point`, delete `if prev_is_digit && next_is_digit { continue; }`.  
  _Prediction:_ Test goes RED on inputs containing decimals (e.g. "3.14" splits into "3." and "14").

---

### Seam 11 — Assistant Session Lifecycle & FSM (`tests/session_lifecycle_test.rs`)

#### 11.1 Seam Identifier & Classification

- **Binary:** `app/src-tauri/tests/session_lifecycle_test.rs`
- **Classification:** **Category A (Solid — Verified Green & Validated)**
- **Subsystems:** `pipeline/assistant/session.rs`, `pipeline/router.rs`, `ipc/pipeline.rs`, `persistence/worker.rs`, `services/harness/session.rs`, `services/llm/catalog/probe.rs`
- **Execution Command:** `cargo nextest run --test session_lifecycle_test --release --nocapture --test-threads=1`

#### 11.2 `/create-test` Phase 1 — Production Path Trace

```
Production Entry Seam:
  IPC Command: ipc::pipeline::start_session(sessionId) / pause_session / resume_session / end_session
  OR
  Pipeline Event: VoxEvent::SessionStart { owner, session_id } / PauseSession / ResumeSession / EndSession
  sent over event_tx into spawn_router.
  Direction Check: PASS — entry seam is the IPC command or central router event channel, NOT direct calls to internal handlers.

Production Path — Session Start (Modular & Continuation):
  VoxEvent::SessionStart { owner, session_id }
  ──► router::route_event
      ──► assistant::session::on_session_start
          ├─► Guard: current_state == Idle (drops if already active)
          ├─► owner.store(Assistant), cancel_flag.store(false)
          ├─► conv_id = session_id.unwrap_or(timestamp_now) -> conversation_id.store(conv_id)
          ├─► persist_tx.try_send(PersistenceEvent::SessionStarted { session_id: conv_id, timestamp_ms })
          │   └──► persistence::worker executes: INSERT OR IGNORE INTO sessions (id, ...)
          ├─► resolve_model_tool_support(model):
          │   ├─► Checks model_capabilities.json cache
          │   ├─► If present: initializes harness.supports_tools immediately
          │   └─► If missing: sets false, spawns non-blocking discovery probe in background
          ├─► start_modular_session:
          │   ├─► ensure_modular_workers_sync (spawns STT/LLM/TTS workers if needed)
          │   └─► vad_tx.send(VadCommand::SetOperationalMode(ContinuousSegmentation / WindowedValidation))
          ├─► prompt = state.resolve_base_prompt()
          ├─► Continuation Resolution:
          │   ├─► If session_id == Some(sid):
          │   │   fetch_session_continuation(&state.db, sid) -> (personal_memory, summary, turns)
          │   └─► If session_id == None:
          │       get_personal_memory(&state.db, None) -> personal_memory
          ├─► Mount HarnessSession:
          │   mut harness = HarnessSession::new_modular(session_id, prompt, personal_memory, settings, llm_tx)
          │   If continuation: harness.seed_continuation(summary, turns)
          │   *state.harness.lock() = Some(harness)
          ├─► spawn_idle_monitor(app, state)
          ├─► pipeline_accumulator.lock().clear()
          └─► transition(InteractionState::Ready, ctx, app, state)

Production Path — Pause & Resume:
  VoxEvent::PauseSession ──► assistant::session::on_pause
      ├─► Guard: drops if Idle or Paused
      ├─► cancel_flag.store(true), turn_token.cancel(), pipeline_accumulator.clear()
      ├─► playback_engine.cancel()
      ├─► owner.store(Dictation) (unconditional yield)
      ├─► vad_tx.send(SetOperationalMode(DictationMode))
      └─► transition(InteractionState::Paused)
  VoxEvent::ResumeSession ──► assistant::session::on_resume
      ├─► Guard: must be Paused, Sleeping, or Error
      ├─► owner.store(Assistant), cancel_flag.store(false), rearm_turn_token()
      ├─► vad_tx.send(SetOperationalMode(AssistantMode))
      └─► transition(InteractionState::Ready)

Production Path — Session End & CPAL Gate:
  VoxEvent::EndSession ──► assistant::session::on_end
      ├─► Guard: drops if Idle
      ├─► cancel_flag.store(true), turn_token.cancel(), pipeline_accumulator.clear()
      ├─► state.harness.lock().take() (Zero Harness Instances in Memory Invariant)
      ├─► If ctx.pipeline_mode == Realtime: purge_session_cache()
      ├─► If dictation_state == Idle: stop CPAL audio engine (state.engine = None)
      │   If dictation_state == Ready: keep engine active (Dictation preservation invariant)
      └─► transition(InteractionState::Idle)
```

#### 11.3 `/create-test` Phase 2a — Testability Check

1. Entry seams callable with production signatures? **Yes** (`router.route_event`, `on_session_start`).
2. Production constructors used? **Yes** (`AppState`, `HarnessSession`, `VoxDb`).
3. State and DB observable? **Yes** (`state.pipeline.state()`, `state.owner`, Turso SQLite `sessions` table).
4. Real components used? **Yes** (Real Turso SQLite, real router loop, real harness).

#### 11.4 `/create-test` Phase 2b — False-Green Audit Table

| If this production defect existed | Would Seam 11 test fail? | Expected Failure Mode |
|---|---|---|
| `on_session_start` does not persist session row to Turso SQLite | **Must fail** | Assertion `row.is_some()` panics (session not in DB). |
| `on_session_start` fails to advance turn counter on continuation | Must fail | Turn ID remains 0 instead of matching DB max turn. |
| `on_session_start` capability cache lookup ignored | Must fail | `supports_tools` remains false despite cache containing true. |
| `on_end` fails to unmount `HarnessSession` | Must fail | Assertion `state.harness.lock().is_none()` panics. |
| `on_end` CPAL gate kills engine when dictation is Ready | Must fail | Assertion `state.engine.lock().is_some()` panics (engine was destroyed). |
| `on_pause` fails to yield owner to `Dictation` | Must fail | Assertion `state.owner.load() == InteractionOwner::Dictation` panics. |
| `on_resume` rejects `Sleeping` or `Error` states | Must fail | State remains `Sleeping` or `Error` instead of transitioning to `Ready`. |

#### 11.5 `/test` Execution Protocol

- **Command:** `cargo nextest run --test session_lifecycle_test --release --nocapture --test-threads=1`
- **Success Criteria:**
  1. Subtest 1 (`test_session_start_modular_sets_ready_and_identity`): Starts via `VoxEvent::SessionStart`, state transitions `Idle` $\to$ `Ready`, real Turso DB has session row, identity facts present in harness.
  2. Subtest 2 (`test_session_continuation_seeds_harness`): Resumes existing session ID with turns in Turso; harness seeds continuation summary and turn history.
  3. Subtest 3 (`test_session_start_realtime_wires_audio`): Starts Realtime session; `RealtimeActor` active, VAD sent `StartRealtime`.
  4. Subtest 4 (`test_session_pause_resume_transitions`): Pauses to `Paused`, owner yields to `Dictation`, token cancelled; resumes to `Ready`, owner restored to `Assistant`, token re-armed.
  5. Subtest 5 (`test_session_resume_from_sleeping_and_error`): Validates recovery from `Sleeping` and `Error` into `Ready`.
  6. Subtest 6 (`test_session_end_dictation_gate_keeps_engine`): Validates CPAL engine preservation when dictation is `Ready`, and teardown when dictation is `Idle`.
  7. Subtest 7 (`test_session_end_purges_and_unmounts_harness`): Validates `state.harness` is `None`, cache purged, and accumulator drained.
  8. Subtest 8 (`test_session_boot_capability_cached_lookup`): Validates cached `model_capabilities.json` is read synchronously without probe and hydrates `supports_tools = true`.
- **Failure Signatures:**
  - `Timeout waiting for Ready`: Router event dispatch broken or handler stalled.
  - `Database session row missing`: Persistence worker failed to write session row.
  - `Harness not None after EndSession`: Zero harness in idle invariant violated.

#### 11.6 `/mutate` Mutant Definitions (Tier 1)

- **Mutant 11.1 (Persistence Dispatch Deletion):** In `pipeline/assistant/session.rs:on_session_start`, delete `persist_tx.try_send(PersistenceEvent::SessionStarted { .. })`.  
  _Prediction:_ Subtest 1 goes RED because Turso SQLite contains no row for the session.
- **Mutant 11.2 (Harness Unmount Deletion):** In `pipeline/assistant/session.rs:on_end`, delete `state.harness.lock().take()`.  
  _Prediction:_ Subtest 7 goes RED on `assert!(state.harness.lock().is_none())`.
- **Mutant 11.3 (CPAL Gate Inversion):** In `pipeline/assistant/session.rs:on_end`, change `if state.pipeline.dictation_state() == InteractionState::Idle` to `if true`.  
  _Prediction:_ Subtest 6 goes RED because CPAL engine is prematurely destroyed when dictation is Ready.
- **Mutant 11.4 (Continuation Branch Deletion):** In `pipeline/assistant/session.rs:on_session_start`, replace `if let Some(sid) = session_id` with `if false`.  
  _Prediction:_ Subtest 2 goes RED because seeded database turns are never hydrated into working memory.
- **Mutant 11.5 (Capability Cache Inversion):** In `pipeline/assistant/session.rs:712`, invert cached `supports_tools` boolean.  
  _Prediction:_ Subtest 8 goes RED on `Cached model capabilities must immediately set harness.supports_tools = true`.

---

### Seam 12 — Memory Compaction Coordinator & `CompactionStage` v2 (`tests/memory_compaction_test.rs`)

#### 12.1 Seam Identifier & Classification

- **Binary:** `app/src-tauri/tests/memory_compaction_test.rs`
- **Classification:** **Category C (Decommission Legacy v1 XML & Author Target Memory v2 Subsystem Spec)**
- **Subsystems:** `services/memory/compaction/coordinator.rs`, `services/memory/compaction/runner.rs`, `services/harness/plugins/compaction.rs`, `persistence/compactions.rs`, `persistence/notifications.rs`
- **Execution Command (Local Default):** `cargo nextest run --test memory_compaction_test --release --nocapture --test-threads=1`
- **Execution Command (Cloud Live API):** `cargo nextest run --test memory_compaction_test --release --nocapture --test-threads=1 -- --ignored`

#### 12.2 `/create-test` Phase 1 — Production Path Trace

```
Production Entry Seams:
  Entry Seam A (Boundary / Manual / Auto Compaction):
    CompactionCoordinator::run_compaction_slice(app, state, session_id, trigger_kind, cancel_token)
  Entry Seam B (Inline Turn Budget Compaction):
    CompactionStage::run_and_persist(provider, conn, params)

Direction Check: PASS — entry seams are the Coordinator slice executor and Harness Plugin runner, NOT leaf database helpers or prompt string interpolations.

Production Path — Coordinator Slicing & Database Ledger:
  CompactionCoordinator::run_compaction_slice(app, state, session_id, trigger_kind, cancel)
  ├─► Gating Check:
  │   If trigger_kind == "soft" => requires pipeline state in {Ready, Paused}
  ├─► Mutual Exclusion Check:
  │   fetch_latest_compaction_run(conn, session_id)
  │   If status == "in_progress" => return Ok(None)
  ├─► Turn Range Resolution:
  │   last_compacted_turn = fetch_latest_compaction_run(...).to_turn_id
  │   turns = fetch_turns_for_compaction(conn, session_id, last_compacted_turn + 1, u32::MAX)
  │   If turns.is_empty() => return Ok(None)
  ├─► Atomic Run Registration:
  │   record_compaction_start(conn, session_id, trigger_kind, from_turn_id, to_turn_id)
  │   └──► Enforces Turso partial unique index: rejects duplicate in-progress runs with Ok(None)
  ├─► Execution:
  │   run_compaction(active_provider, history_messages, settings, cancel)
  │   ├─► Builds GenerationRequest with COMPACTION_SYSTEM_PROMPT
  │   ├─► Dispatches to LLM with COMPACTION_SENTINEL_TURN_ID (999_999) (up to 2 attempts)
  │   ├─► parse_unified_compaction_json(&summary_content)
  │   │   Extracts flat 6-key JSON: { personal, objective, workdone, blocker, next_step, pitfall }
  │   └─► Returns CompactionResult { raw_json, session_context, facts }
  ├─► Atomic Commit:
  │   commit_compaction_output(conn, run_id, raw_json, facts, session_id)
  │   ├─► session_compactions row updated: status = 'completed', compaction_output = raw_json
  │   └─► memory_ingestion_queue: inserts each fact with status = 'pending', retry_count = 0
  └─► Notification Management:
      ├─► dismiss_interactive_by_entity(conn, "session_compaction", session_id)
      └─► emit_session_compaction_success_receipt(app, conn, session_id, facts_count)

Production Path — Harness CompactionStage:
  CompactionStage::run_and_persist(provider, conn, params)
  ├─► Invariant check: can_perform_inline_compaction (context_window > 4096, message_count >= 4)
  ├─► Runs run_compaction and commits output to DB
  └─► prune_history_with_summary(system_prompt, user_turn):
      Formats <session_context>...</session_context> into root system prompt and prunes ChatMessage history to [System, User].

Observable Exit:
  1. Turso SQLite `session_compactions`: row with status 'completed', valid turn watermarks, and valid 6-key JSON output.
  2. Turso SQLite `memory_ingestion_queue`: pending facts staged with matching session/compaction provenance.
  3. Concurrency Safety: concurrent run_compaction_slice calls for the same session ID return Ok(None) without panicking.
  4. System Prompt Hydration: HarnessSession system prompt contains structured <session_context>.
  5. Notification Center: interactive card is dismissed and a success receipt is recorded in `notifications`.

Production Functions Called:
  setup:   get_test_app_and_state(), VoxDb::open(), seed_turns_from_dataset(), MockLlmProvider / NvidiaProvider
  entry:   CompactionCoordinator::run_compaction_slice(), CompactionStage::run_and_persist(), CompactionStage::prune_history_with_summary()
  observe: Turso SQLite queries on `session_compactions`, `memory_ingestion_queue`, `notifications`, CompactionExecutionSummary
  teardown: drop TempPathsGuard
```

#### 12.3 `/create-test` Phase 2a — Testability Check

1. Entry seam callable with production signature? **Yes** (`CompactionCoordinator::run_compaction_slice`, `CompactionStage::run_and_persist`).
2. Production constructors used? **Yes** (`CompactionCoordinator`, `CompactionStage::new`, `commit_compaction_output`).
3. State observable? **Yes** (Turso SQLite tables `session_compactions`, `memory_ingestion_queue`, `notifications`).
4. Real components without mocks? **Yes** (Cloud live subtest runs against real Nvidia/OpenAI API; local test runs against real Turso DB with canned LLM provider).

#### 12.4 `/create-test` Phase 2b — False-Green Audit Table

| If this production defect existed                                           | Would Seam 12 test fail? | Expected Failure Mode                                                                                   |
| --------------------------------------------------------------------------- | ------------------------ | ------------------------------------------------------------------------------------------------------- |
| **Upstream coordinator fails to query uncompacted turns**                   | **Must fail**            | Slicing returns `Ok(None)`; `session_compactions` row not created.                                      |
| `commit_compaction_output` skips inserting into `memory_ingestion_queue`    | Must fail                | Query `SELECT COUNT(*) FROM memory_ingestion_queue WHERE session_id = ?` returns 0.                     |
| Partial unique index missing (allows concurrent duplicate in-progress runs) | Must fail                | Concurrent slicing creates two `in_progress` runs instead of rejecting the second with `Ok(None)`.      |
| LLM JSON output missing one of 6 required flat keys                         | Must fail                | Live test validates presence of `personal`, `objective`, `workdone`, `blocker`, `next_step`, `pitfall`. |
| `can_perform_inline_compaction` permits compaction on <= 3 messages         | Must fail                | Plugin subtest asserts `can_perform_inline_compaction(3) == false`.                                     |
| `prune_history_with_summary` fails to wrap context in `<session_context>`   | Must fail                | Assertion on formatted prompt panics (missing tag).                                                     |

#### 12.5 `/test` Execution Protocol

- **Default Suite:** `cargo nextest run --test memory_compaction_test --release --nocapture --test-threads=1` (Runs subtests 2–5 locally in < 1s).
- **Ignored Live Suite:** `cargo nextest run --test memory_compaction_test --release --nocapture --test-threads=1 -- --ignored` (Runs Subtest 1 against real cloud API).
- **Subtests:**
  1. `#[ignore]` `test_memory_compaction_100_turns_live_api`: Loads 100 turns from `sandbox/datasets/dataset_session1.json`, seeds into Turso DB, executes `run_compaction_slice` against Cloud LLM, asserts 6 categories extracted and persisted.
  2. `test_memory_compaction_coordinator_slicing_and_ledger`: Executes slicing with local mock provider, verifies `session_compactions` watermark progression and `memory_ingestion_queue` staging.
  3. `test_memory_compaction_concurrency_and_partial_unique_index`: Proves database-level lock mutual exclusion rejects concurrent runs.
  4. `test_compaction_plugin_preemptive_fifo_and_context_injection`: Verifies message threshold guards and prompt tag formatting.
  5. `test_compaction_notification_lifecycle`: Verifies interactive card creation (`notify_uncompacted_session`), dismissal on slice completion, and receipt emission.

#### 12.6 `/mutate` Mutant Definitions (Tier 1)

- **Mutant 12.1 (Ingestion Queue Staging Deletion):** In `persistence/compactions.rs:commit_compaction_output`, comment out `insert_ingestion_queue_batch`.  
  _Prediction:_ Subtests 1 & 2 go RED because `memory_ingestion_queue` contains 0 pending facts.
- **Mutant 12.2 (Partial Unique Index Inversion):** In `services/memory/compaction/coordinator.rs:run_compaction_slice`, delete the `in_progress` check.  
  _Prediction:_ Subtest 3 goes RED on concurrency check.
- **Mutant 12.3 (Preemptive FIFO Threshold Inversion):** In `services/harness/plugins/compaction.rs:can_perform_inline_compaction`, change `>= MIN_MESSAGES_FOR_COMPACTION` to `>= 1`.  
  _Prediction:_ Subtest 4 goes RED on `assert!(!plugin.can_perform_inline_compaction(3))`.

---

### Seam 13 — Ingestion Queue & 2-Stage Deduplication v2 (`tests/memory_ingestion_test.rs`)

#### 13.1 Seam Identifier & Classification

- **Binary:** `app/src-tauri/tests/memory_ingestion_test.rs`
- **Classification:** **Category C (Decommission Legacy v1 4-Stage & Author Target Memory v2 Subsystem Spec)**
- **Subsystems:** `services/memory/ingestion/runner.rs`, `services/memory/ingestion/stage1_dedup.rs`, `services/memory/ingestion/stage2_embed.rs`, `persistence/queue.rs`, `persistence/facts.rs`, `services/memory/ml/embedder.rs`
- **Execution Command:** `cargo nextest run --test memory_ingestion_test --release --nocapture --test-threads=1`

#### 13.2 `/create-test` Phase 1 — Production Path Trace

```
Production Entry Seams:
  Entry Seam A (Full Cycle):
    run_ingestion_cycle(conn)
  Entry Seam B (Stage-Isolated Execution):
    run_stage1_exact_dedup(conn)
    run_stage2_cosine_dedup(conn)
  Entry Seam C (Startup Crash Recovery):
    reconcile_crashed_queue_on_boot(conn)

Direction Check: PASS — entry seams operate against the pending queue buffer (`memory_ingestion_queue`), driving lifecycle progression through Stage 1, Stage 2, and Turso persistence, NOT bypassing queue logic with direct row insertions into `memory_facts`.

Production Path — Stage 1 Exact Jaccard Deduplication:
  claim_pending_queue_batch(conn, "pending", "stage1_processing", STAGE1_BATCH_CEILING=128)
  ├─► Queries active facts of the same `fact_type`: fetch_active_facts_by_type(conn, type)
  ├─► Computes Jaccard word-set similarity: jaccard_similarity(item.text, fact.text)
  ├─► Winner-Takes-All Policy:
  │   If similarity >= 1.0 (exact match):
  │   Older matching fact in `memory_facts` is deactivated: deactivate_facts_batch(conn, [older_fact_id])
  └─► Queue status transition: update_queue_item_status(conn, item.id, "stage1_done")
      (On error: record_queue_item_failure -> resets to "pending" with retry_count + 1)

Production Path — Stage 2 Semantic Cosine Deduplication & Vector Storage:
  claim_pending_queue_batch(conn, "stage1_done", "stage2_processing", STAGE2_BATCH_SIZE=16)
  ├─► Batched ONNX Inference (blocking thread offload):
  │   ensure_embedder_loaded(true) ──► generate_embeddings_batch(texts) (MiniLM-L12, 384-dim)
  ├─► Queries active vectors of same type: fetch_active_vectors_by_type(conn, type)
  ├─► Computes cosine similarity: cosine_similarity(new_vec, active_vec)
  ├─► Winner-Takes-All Policy:
  │   If cosine_similarity >= SOFT_VECTOR_DEDUP_THRESHOLD (0.95):
  │   Older fact in `memory_facts` is deactivated: deactivate_fact(conn, older_id)
  ├─► Commit Active Fact & Vector:
  │   insert_fact(conn, &FactRecord { status: "active", ... })
  │   insert_vector(conn, fact_id, type, "active", project_id, embedding)
  └─► Queue status transition: update_queue_item_status(conn, item.id, "completed")
      (On error: record_queue_item_failure -> resets to "stage1_done" with retry_count + 1)

Production Path — Boot Crash Reconciliation:
  reconcile_crashed_queue_on_boot(conn)
  ├─► Identifies in-flight items left in 'stage1_processing' or 'stage2_processing'
  ├─► Items with retry_count < 3:
  │   'stage1_processing' ──► 'pending' with retry_count + 1
  │   'stage2_processing' ──► 'stage1_done' with retry_count + 1
  └─► Items with retry_count >= 3:
      Poison pills transition to 'failed'

Observable Exit:
  1. Queue Lifecycle: items in `memory_ingestion_queue` transition: pending -> stage1_done -> completed.
  2. Winner-Takes-All Invariant: older identical or semantically duplicate facts transition to status = 'inactive'.
  3. Vector Invariant: `memory_facts_vectors` contains 384-dimensional non-zero float vectors for all active facts.
  4. Poison Pill Gate: un-processable items fail safely after 3 retries without blocking subsequent queue cycles.

Production Functions Called:
  setup:   VoxDb::open(), recreate_schema(), enqueue_fact(), ensure_embedder_loaded()
  entry:   run_ingestion_cycle(), run_stage1_exact_dedup(), run_stage2_cosine_dedup(), reconcile_crashed_queue_on_boot()
  observe: fetch_active_facts_by_type(), fetch_active_vectors_by_type(), SQL queries on `memory_ingestion_queue`
  teardown: unload_memory_pipeline_onnx_models(), drop TempPathsGuard
```

#### 13.3 `/create-test` Phase 2a — Testability Check

1. Entry seam callable with production signature? **Yes** (`run_ingestion_cycle`, `run_stage1_exact_dedup`, `run_stage2_cosine_dedup`, `reconcile_crashed_queue_on_boot`).
2. Production constructors used? **Yes** (`VoxDb::open`, `enqueue_fact`, `generate_embeddings_batch`).
3. State observable? **Yes** (Turso tables `memory_ingestion_queue`, `memory_facts`, `memory_facts_vectors`).
4. Real components without mocks? **Yes** (Real local MiniLM ONNX embedder from `~/.vox/models/embedding/minilm-l12-v2`, real Turso SQLite database).

#### 13.4 `/create-test` Phase 2b — False-Green Audit Table

| If this production defect existed                                    | Would Seam 13 test fail? | Expected Failure Mode                                                   |
| -------------------------------------------------------------------- | ------------------------ | ----------------------------------------------------------------------- |
| **`run_stage1_exact_dedup` fails to claim pending items**            | **Must fail**            | Queue items remain `pending`; `summary.processed == 0`.                 |
| Winner-Takes-All deactivation omitted in Stage 1                     | Must fail                | `fetch_active_facts_by_type` returns 2 active facts instead of 1.       |
| Stage 2 fails to generate 384-dim embeddings                         | Must fail                | Query on `memory_facts_vectors` returns 0 rows or vector length != 384. |
| Cosine threshold ignored (treats dissimilar facts as duplicate)      | Must fail                | Dissimilar fact deactivates unrelated active fact; active count drops.  |
| Queue status fails to update to `completed`                          | Must fail                | Queue status query returns `stage2_processing` or `stage1_done`.        |
| Crash reconciliation does not move items with retry >= 3 to `failed` | Must fail                | Poison pill item reset to `pending` instead of `failed`.                |

#### 13.5 `/test` Execution Protocol

- **Command:** `cargo nextest run --test memory_ingestion_test --release --nocapture --test-threads=1`
- **Execution Target:** 4 subtests run locally in < 3.0s using local ONNX weights.
- **Subtests:**
  1. `test_stage1_exact_dedup_winner_takes_all`: Seeds active fact, enqueues exact match with case/punctuation variation, runs Stage 1, asserts older fact inactive and queue item `stage1_done`.
  2. `test_stage2_semantic_cosine_dedup_real_embedder`: Seeds active fact and vector, enqueues semantic paraphrase, runs Stage 2 with local MiniLM ONNX, asserts cosine $\ge 0.95$, older fact deactivated, new fact/vector active, queue item `completed`.
  3. `test_ingestion_cycle_end_to_end`: Enqueues 5 facts across distinct categories (`personal`, `objective`, `workdone`, `blocker`, `next_step`), runs full cycle, asserts 5 items `completed`, 5 active facts, 5 active vectors.
  4. `test_crash_reconciliation_and_poison_pill`: Seeds crashed in-flight items with retry counts 0, 1, and 3, runs `reconcile_crashed_queue_on_boot`, asserts retries incremented and retry 3 marked `failed`.

#### 13.6 `/mutate` Mutant Definitions (Tier 1)

- **Mutant 13.1 (Winner-Takes-All Inversion):** In `services/memory/ingestion/stage1_dedup.rs:process_stage1_item`, delete `deactivate_facts_batch(conn, &duplicate_ids)`.  
  _Prediction:_ Subtest 1 goes RED because older fact remains `active` (`active_facts.len()` is 2 instead of 1).
- **Mutant 13.2 (Cosine Dedup Threshold Drift):** In `services/memory/ingestion/mod.rs`, change `SOFT_VECTOR_DEDUP_THRESHOLD = 0.95` to `SOFT_VECTOR_DEDUP_THRESHOLD = 1.05`.  
  _Prediction:_ Subtest 2 goes RED because paraphrase is treated as new instead of duplicate (`duplicates_deactivated == 0`).
- **Mutant 13.3 (Poison Pill Threshold Deletion):** In `persistence/queue.rs:reconcile_crashed_queue_on_boot`, change `retry_count >= 3` to `false`.  
  _Prediction:_ Subtest 4 goes RED because poison pill item is reset to `pending` instead of `failed`.

---

### Seam 14 — Personal Memory Document & Session Continuation v2 (`tests/personal_memory_test.rs`)

#### 14.1 Seam Identifier & Classification

- **Binary:** `app/src-tauri/tests/personal_memory_test.rs`
- **Classification:** **Category C (Decommission Legacy v1 BFS Retrieval & Author Target Memory v2 Subsystem Spec)**
- **Subsystems:** `services/memory/personal.rs`, `persistence/personal_memory.rs`, `persistence/sessions.rs`, `persistence/facts.rs`, `ipc/memory.rs`
- **Execution Command:** `cargo nextest run --test personal_memory_test --release --nocapture --test-threads=1`

#### 14.2 `/create-test` Phase 1 — Production Path Trace

```
Production Entry Seams:
  Entry Seam A (Personal Memory CRUD / Optimistic Save):
    save_personal_memory(conn, project_id, content, expected_version)
  Entry Seam B (Consolidation Pipeline):
    consolidate_personal_memory(conn, provider, comments, project_id)
  Entry Seam C (File Portability):
    export_personal_memory(conn, path, project_id)
    import_personal_memory(conn, path, project_id)
  Entry Seam D (Session Continuation):
    fetch_session_continuation(conn, session_id)

Direction Check: PASS — entry seams operate at the service and persistence facade, driving business validation (optimistic concurrency, ingestion quiescence gating, fact state transitions, session continuation slicing), NOT raw isolated SQL helper calls.

Production Path — Optimistic Concurrency Control (Direct Manual Edits):
  save_personal_memory(conn, project_id, content, expected_version)
  ├─► UPDATE personal_memory SET content=?, version=version+1, updated_at=?
  │   WHERE project_id IS NULL AND version = expected_version
  ├─► If affected_rows == 0:
  │   └──► Returns Err("Optimistic version conflict for personal memory: expected version {N}")
  └─► If affected_rows == 1:
      └──► Returns Ok(PersonalMemoryRecord { version: N+1, content, ... })

Production Path — Fact Consolidation & Quiescence Gating:
  consolidate_personal_memory(conn, provider, comments, project_id)
  ├─► Quiescence Check: verify_ingestion_quiescence(conn)
  │   ├─► has_in_progress_compaction(conn) => Err("Precondition failed: active compaction is in progress")
  │   └─► has_unfinished_items(conn) => Err("Precondition failed: pending items in memory ingestion queue")
  ├─► Candidate Fact Gathering:
  │   fetch_active_facts_by_type(conn, "personal")
  │   If empty => return Ok(current_record) (no-op)
  ├─► LLM Reasoning Merge:
  │   execute_personal_llm_pass(provider, SYSTEM_PROMPT, current_memory + active_facts)
  ├─► Atomic Save & Provenance Update:
  │   save_consolidated_memory(conn, project_id, updated_markdown, current_version)
  │   └──► Sets last_consolidated_at = now, version = version + 1
  └─► Fact State Transition:
      mark_facts_consolidated(conn, fact_ids)
      └──► UPDATE memory_facts SET status = 'consolidated' WHERE id IN (...)

Production Path — Session Continuation Payload Assembly:
  fetch_session_continuation(conn, session_id)
  ├─► Fetch Personal Memory: get_personal_memory(conn, None) -> Option<String>
  ├─► Fetch Latest Context Summary:
  │   fetch_latest_compaction_run(conn, session_id) (status == 'completed')
  │   └──► parse_unified_compaction_json(&run.compaction_output).context_summary
  ├─► Fetch Uncompacted Recent Turns:
  │   fetch_turns_for_compaction(conn, session_id, last_compacted_turn + 1, u32::MAX)
  └─► Returns SessionContinuationData { personal_memory, latest_summary, turns }

Observable Exit:
  1. Optimistic Concurrency: Stale `expected_version` calls return explicit conflict errors.
  2. Fact Consolidation: Active personal facts transition from `status = 'active'` to `status = 'consolidated'`.
  3. Quiescence Gating: Consolidation is rejected when compactions or ingestion queue tasks are in-flight.
  4. Portability Roundtrip: `import_personal_memory` replaces document content; `export_personal_memory` writes identical text to disk.
  5. Continuation Hydration: `SessionContinuationData` contains personal memory, latest compaction context summary, and strictly uncompacted turns.

Production Functions Called:
  setup:   VoxDb::open(), recreate_schema(), insert_fact(), create_session_with_id(), record_compaction_start()
  entry:   save_personal_memory(), consolidate_personal_memory(), export_personal_memory(), import_personal_memory(), fetch_session_continuation()
  observe: PersonalMemoryRecord, fetch_active_facts_by_type(), SessionContinuationData, disk file contents
  teardown: drop TempPathsGuard
```

#### 14.3 `/create-test` Phase 2a — Testability Check

1. Entry seam callable with production signature? **Yes** (`save_personal_memory`, `consolidate_personal_memory`, `fetch_session_continuation`).
2. Production constructors used? **Yes** (`VoxDb::open`, `get_personal_memory`, `save_consolidated_memory`).
3. State observable? **Yes** (`personal_memory`, `memory_facts`, `session_compactions`, `SessionContinuationData`).
4. Real components without mocks? **Yes** (Real Turso SQLite engine; canned/mock LLM provider for deterministic fact consolidation).

#### 14.4 `/create-test` Phase 2b — False-Green Audit Table

| If this production defect existed                                  | Would Seam 14 test fail? | Expected Failure Mode                                                                    |
| ------------------------------------------------------------------ | ------------------------ | ---------------------------------------------------------------------------------------- |
| **Optimistic concurrency check omitted (unconditional overwrite)** | **Must fail**            | Stale save succeeds instead of returning version conflict error.                         |
| `mark_facts_consolidated` omitted on successful merge              | Must fail                | Merged facts remain `status = 'active'`; `fetch_active_facts_by_type` returns > 0.       |
| Quiescence gate ignores in-progress compaction                     | Must fail                | Consolidation proceeds during active compaction instead of returning Precondition error. |
| Quiescence gate ignores pending ingestion queue items              | Must fail                | Consolidation proceeds with pending queue items instead of returning Precondition error. |
| Continuation turn slicing includes already compacted turns         | Must fail                | `turns.len()` in continuation data contains old compacted turns.                         |
| Export writes empty or corrupted file                              | Must fail                | Exported disk file fails byte-for-byte equality assertion against database content.      |

#### 14.5 `/test` Execution Protocol

- **Command:** `cargo nextest run --test personal_memory_test --release --nocapture --test-threads=1`
- **Execution Target:** 5 subtests run locally in < 1.0s.
- **Subtests:**
  1. `test_personal_memory_optimistic_concurrency`: Validates version incrementing on update and rejection of stale `expected_version`.
  2. `test_personal_memory_consolidation_with_facts`: Seeds active personal facts, runs consolidation, asserts document updated and facts marked `consolidated`.
  3. `test_consolidation_quiescence_precondition_gating`: Verifies rejection when compaction is `in_progress` or queue items are `pending`.
  4. `test_export_and_import_roundtrip`: Verifies document import overwrite and byte-for-byte disk export.
  5. `test_session_continuation_data_assembly`: Seeds 10 turns, completed compaction for turns 1–5, asserts continuation data contains strictly turns 6–10.

#### 14.6 `/mutate` Mutant Definitions (Tier 1)

- **Mutant 14.1 (Optimistic Concurrency Inversion):** In `persistence/personal_memory.rs:save_personal_memory`, delete `AND version = ?`.  
  _Prediction:_ Subtest 1 goes RED because stale update succeeds without conflict.
- **Mutant 14.2 (Quiescence Gate Deletion):** In `services/memory/personal.rs:consolidate_personal_memory`, delete `verify_ingestion_quiescence(conn).await?;`.  
  _Prediction:_ Subtest 3 goes RED because consolidation executes despite in-progress compaction.
- **Mutant 14.3 (Fact Consolidation Status Omission):** In `services/memory/personal.rs:consolidate_personal_memory`, delete `mark_facts_consolidated(conn, &fact_ids).await?;`.  
  _Prediction:_ Subtest 2 goes RED because active personal facts are not transitioned to `consolidated`.
- **Mutant 14.4 (Continuation Turn Watermark Off-By-One):** In `persistence/sessions.rs:fetch_session_continuation`, change `last_compacted + 1` to `last_compacted`.  
  _Prediction:_ Subtest 5 goes RED because turn count is 6 instead of 5 (includes already compacted turn).

---

### Seam 15 — Settings Persistence & Atomic Corrupt Recovery (`tests/settings_persistence_test.rs`)

#### 15.1 Seam Identifier & Classification

- **Binary:** `app/src-tauri/tests/settings_persistence_test.rs`
- **Classification:** **Category A (Solid — Verified Green & Validated)**
- **Subsystems:** `core/settings.rs`, `ipc/settings/mutation.rs`, `utils/paths.rs`
- **Execution Command:** `cargo nextest run --test settings_persistence_test --release --nocapture --test-threads=1`
- **Execution Baseline:** 3 passed in 0.037s (verified green).

#### 15.2 `/create-test` Phase 1 — Production Path Trace

```
Production Entry Seams:
  Entry Seam A (Settings Mutation & Save):
    apply_setting_mutation(&mut settings, domain, field, value) ──► settings.save()
  Entry Seam B (Settings Load & Corruption Recovery):
    VoxSettings::load()

Direction Check: PASS — entry seams operate on real domain mutations, atomic disk serialization, and recovery parsing, NOT bypassing file I/O or mocking path resolution.

Production Path — Atomic Settings Persistence:
  apply_setting_mutation(&mut settings, domain, field, value)
  ├─► Validates field types and applies mutation across any domain
  │   (appearance, audio, vad, stt, llm, tts, interaction, dictation, memory)
  settings.save()
  ├─► Ensures parent directory exists: fs::create_dir_all(parent)
  ├─► Serializes struct: serde_json::to_string_pretty(self)
  ├─► Generates temp file path: settings.<nanos>.tmp
  ├─► Writes content to temp file and calls file.sync_all() (fsync)
  ├─► Atomically replaces target: fs::rename(&tmp_path, &settings_path)
  └─► If rename/write fails: cleans up tmp_path and returns Err

Production Path — Resilient Settings Loading:
  VoxSettings::load()
  ├─► Reads file: fs::read_to_string(&path)
  ├─► Case 1 — Full Valid JSON:
  │   serde_json::from_str(&content) ──► returns loaded VoxSettings
  ├─► Case 2 — Partial / Unknown Section JSON:
  │   Parses serde_json::Value
  │   Recovers valid known sections (appearance, vad, stt, llm, tts, dictation, memory, etc.)
  │   Fills missing sections from VoxSettings::default()
  │   Returns hybrid recovered settings
  └─► Case 3 — Unparseable Malformed JSON:
      Renames corrupt file: settings.corrupt.<ts>.json (preserves user data for diagnosis)
      Logs warning and returns in-memory VoxSettings::default()

Observable Exit:
  1. Physical File: settings.json exists at paths::settings_path(), valid JSON, matches mutated values.
  2. Exact Roundtrip Equality: load() reproduces modified settings field-for-field.
  3. Total Corruption Resilience: malformed JSON backs up to settings.corrupt.<ts>.json and falls back to defaults without panicking.
  4. Partial Section Recovery: valid sections in damaged files are preserved while omitted sections fall back to defaults.

Production Functions Called:
  setup:   TempPathsGuard::new(), VoxSettings::default(), serde_json::json!()
  entry:   apply_setting_mutation(), settings.save(), VoxSettings::load()
  observe: fs::read_to_string(), fs::read_dir(), field-wise equality assertions
  teardown: drop TempPathsGuard
```

#### 15.3 `/create-test` Phase 2a — Testability Check

1. Entry seam callable with production signature? **Yes** (`apply_setting_mutation`, `settings.save`, `VoxSettings::load`).
2. Production constructors used? **Yes** (`VoxSettings::default()`).
3. State observable? **Yes** (`settings.json` file, `settings.corrupt.<ts>.json`, `VoxSettings` fields).
4. Real components without mocks? **Yes** (100% production code, zero mocks).

#### 15.4 `/create-test` Phase 2b — False-Green Audit Table

| If this production defect existed                       | Would Seam 15 test fail? | Expected Failure Mode                                                      |
| ------------------------------------------------------- | ------------------------ | -------------------------------------------------------------------------- |
| **`settings.save()` fails to write file or is a no-op** | **Must fail**            | Assertion `settings_path.exists()` panics.                                 |
| `save()` omits domain fields during serialization       | Must fail                | Raw JSON check `json_val["tts"]["voice_index"] == 42` panics.              |
| `load()` does not preserve mutated domain fields        | Must fail                | Reloaded field assertion `reloaded.tts.voice_index == 42` panics.          |
| Malformed JSON causes `load()` to panic                 | Must fail                | Subtest 2 panics instead of returning defaults.                            |
| Corrupt file backup creation omitted on parse failure   | Must fail                | Subtest 2 assertion `found_corrupt_backup` panics.                         |
| Partial section recovery drops valid sections           | Must fail                | Subtest 3 assertion `recovered.appearance.theme == "nordic_frost"` panics. |

#### 15.5 `/test` Execution Protocol

- **Command:** `cargo nextest run --test settings_persistence_test --release --nocapture --test-threads=1`
- **Success Criteria:** 3 tests pass in < 0.1s.
- **Subtests:**
  1. `test_settings_json_roundtrip_persistence`: 9-domain mutation via `apply_setting_mutation` $\to$ `save()` $\to$ file validation $\to$ `load()` $\to$ exact field equality.
  2. `test_settings_malformed_fallback_to_default`: Malformed JSON $\to$ `load()` $\to$ defaults restored $\to$ backup file `settings.corrupt.<ts>.json` created.
  3. `test_settings_partial_section_recovery`: Partial JSON with valid `appearance` and `tts` $\to$ `load()` $\to$ valid sections recovered, missing sections defaulted.

#### 15.6 `/mutate` Mutant Definitions (Tier 1)

- **Mutant 15.1 (Atomic Write Temp Rename Deletion):** In `core/settings.rs:save`, replace `fs::rename(&tmp_path, &path)` with `Ok(())`.  
  _Prediction:_ Subtest 1 goes RED because `settings.json` is never written/renamed (`settings_path.exists()` fails).
- **Mutant 15.2 (Corrupt Backup Inversion):** In `core/settings.rs:load`, delete `fs::rename(&path, &bak)`.  
  _Prediction:_ Subtest 2 goes RED on `assert!(found_corrupt_backup)`.
- **Mutant 15.3 (Partial Section Recovery Deletion):** In `core/settings.rs:load`, delete the partial JSON recovery loop and always jump to corrupt backup.  
  _Prediction:_ Subtest 3 goes RED because valid sections in partial JSON are not recovered.

---

### Seam 16 — Model Eviction & Zero Idle RAM (`tests/model_eviction_test.rs`)

#### 16.1 Seam Identifier & Classification

- **Binary:** `app/src-tauri/tests/model_eviction_test.rs`
- **Classification:** **Category A (Solid — Verified Green & Validated)**
- **Subsystems:** `services/memory/ml/mod.rs`, `services/memory/ml/embedder.rs`, `services/translit.rs`, `services/tts/actor.rs`, `services/llm/actor.rs`
- **Execution Command:** `cargo nextest run --test model_eviction_test --release --nocapture --test-threads=1`
- **Execution Baseline:** 2 passed in 2.666s (verified green).

#### 16.2 `/create-test` Phase 1 — Production Path Trace

```
Production Entry Seams:
  Entry Seam A (ONNX Singleton Lifecycle):
    ensure_embedder_loaded(true) ──► generate_embedding() ──► unload_memory_pipeline_onnx_models() ──► unload_all_onnx_models()
  Entry Seam B (Worker Cool Down & Thread Join):
    warm_up_tts() ──► cool_down_tts(&mut tts_tx) ──► thread.join()

Direction Check: PASS — entry seams operate on real singleton lazy loaders, worker thread life cycles, and heap trim allocators, NOT synthetic memory counters or mock handles.

Production Path — ONNX Singleton Eviction & Heap Trimming:
  ensure_embedder_loaded(true) + init_transliteration_engine()
  ├─► Loads MiniLM ONNX session + tokenizer into RwLock singleton
  ├─► Loads transliteration ONNX session into RwLock singleton
  ├─► generate_embedding("query") returns Some(Vec<f32>) (384-dim)
  unload_memory_pipeline_onnx_models()
  ├─► embedder::unload_embedder() resets embedder singleton to None
  ├─► trim_heap("MemorySubsystem::unload_memory_pipeline_onnx_models")
  │   └──► Linux: libc::malloc_trim(0); Windows: EmptyWorkingSet()
  └─► generate_embedding("query") returns Ok(None) safely without SIGSEGV
  unload_all_onnx_models()
  ├─► Unloads embedder + transliteration engine
  ├─► trim_heap("MemorySubsystem::unload_all_onnx_models")
  └─► Idempotent: repeated calls do not panic

Production Path — TTS Worker Cool Down:
  warm_up_tts(handles, &settings, &model_dir, None, event_tx)
  ├─► Spawns dedicated OS worker thread: tts_handle = Some(JoinHandle)
  ├─► Sets tts_tx = Some(Sender<TtsCommand>)
  cool_down_tts(&mut tts_tx)
  ├─► Takes tts_tx (resets caller's Option to None)
  ├─► Dispatches TtsCommand::Shutdown into channel
  └─► Worker loop receives Shutdown, drops TTS model, and terminates
  handle.join()
  └─► Caller joins worker thread cleanly without timeout or deadlock

Observable Exit:
  1. Singleton State: is_embedder_loaded() and is_transliteration_engine_loaded() transition true -> false.
  2. Safe Unloaded API: generate_embedding returns Ok(None) when unloaded, never faults.
  3. Worker Teardown: tts_tx is None after cool_down_tts; tts_handle joins cleanly (is_ok).
  4. Heap Trim: malloc_trim(0) returns pages to OS; model reloads cleanly afterwards.

Production Functions Called:
  setup:   paths::init(), VoxSettings::default(), create_mock_playback_engine()
  entry:   ensure_embedder_loaded(), init_transliteration_engine(), generate_embedding(), unload_memory_pipeline_onnx_models(), unload_all_onnx_models(), warm_up_tts(), cool_down_tts()
  observe: is_embedder_loaded(), is_transliteration_engine_loaded(), tts_tx.is_none(), handle.join().is_ok()
  teardown: unload_all_onnx_models()
```

#### 16.3 `/create-test` Phase 2a — Testability Check

1. Entry seam callable with production signature? **Yes** (`ensure_embedder_loaded`, `unload_all_onnx_models`, `warm_up_tts`, `cool_down_tts`).
2. Production constructors used? **Yes** (Real ONNX sessions via `ort`, real threads via `std::thread::Builder`).
3. State observable? **Yes** (`is_embedder_loaded()`, `tts_tx.is_none()`, thread join result).
4. Real components without mocks? **Yes** (Real local MiniLM and Supertonic model weights; playback engine mock used only to sink audio).

#### 16.4 `/create-test` Phase 2b — False-Green Audit Table

| If this production defect existed                           | Would Seam 16 test fail? | Expected Failure Mode                                                |
| ----------------------------------------------------------- | ------------------------ | -------------------------------------------------------------------- |
| **`ensure_embedder_loaded` fails to load model**            | **Must fail**            | Assertion `is_embedder_loaded()` panics.                             |
| `unload_memory_pipeline_onnx_models` fails to drop embedder | Must fail                | Assertion `!is_embedder_loaded()` panics (still true).               |
| Partial eviction inadvertently drops transliteration engine | Must fail                | Assertion `is_transliteration_engine_loaded()` panics (was evicted). |
| Calling `generate_embedding` when unloaded panics / faults  | Must fail                | Subtest 1 panics instead of returning `Ok(None)`.                    |
| `cool_down_tts` fails to take `tts_tx`                      | Must fail                | Assertion `tts_tx.is_none()` panics (still `Some`).                  |
| TTS worker thread ignores `TtsCommand::Shutdown`            | Must fail                | `handle.join()` hangs until test timeout expires.                    |

#### 16.5 `/test` Execution Protocol

- **Command:** `cargo nextest run --test model_eviction_test --release --nocapture --test-threads=1`
- **Success Criteria:** 2 tests pass in < 5.0s.
- **Subtests:**
  1. `test_onnx_model_singleton_lifecycle_eviction`: Starts clean $\to$ lazy load embedder + translit $\to$ generate embedding $\to$ partial eviction $\to$ safe `Ok(None)` query $\to$ full eviction $\to$ idempotency $\to$ reload.
  2. `test_tts_worker_cool_down_clears_handles_and_joins`: Real Supertonic warm-up $\to$ `cool_down_tts` $\to$ `tts_tx.is_none()` $\to$ thread joins Ok.

#### 16.6 `/mutate` Mutant Definitions (Tier 1)

- **Mutant 16.1 (Embedder Unload Deletion):** In `services/memory/ml/mod.rs:unload_memory_pipeline_onnx_models`, comment out `embedder::unload_embedder()`.  
  _Prediction:_ Subtest 1 goes RED on `assert!(!is_embedder_loaded())`.
- **Mutant 16.2 (TTS Shutdown Send Deletion):** In `services/tts/actor.rs:cool_down_tts`, comment out `tx.send(TtsCommand::Shutdown)`.  
  _Prediction:_ Subtest 2 hangs or fails to join thread cleanly.
- **Mutant 16.3 (Channel Take Omission):** In `services/tts/actor.rs:cool_down_tts`, replace `tts_tx.take()` with `tts_tx.as_ref()`.  
  _Prediction:_ Subtest 2 goes RED on `assert!(tts_tx.is_none())`.

---

### Seam 17 — Model Manager: Manifest, Verification & Archive Traversal Defense (`tests/model_manager_test.rs`)

#### 17.1 Seam Identifier & Classification

- **Binary:** `app/src-tauri/tests/model_manager_test.rs`
- **Classification:** **Category A (Solid — Verified Green & Validated)**
- **Subsystems:** `setup/model_manager.rs`, `setup/manager_ops.rs`, `setup/manifest.rs`
- **Execution Command:** `cargo nextest run --test model_manager_test --release --nocapture --test-threads=1`
- **Execution Baseline:** 5 passed in 0.060s (verified green).

#### 17.2 `/create-test` Phase 1 — Production Path Trace

```
Production Entry Seams:
  Entry Seam A (Model Presence & Verification):
    is_model_file_present(entry, models_dir)
  Entry Seam B (Archive Extraction & Defense):
    ModelManager::do_extract(archive_path, archive_type, dest_dir)
  Entry Seam C (Model & Marker Deletion):
    delete_model_file(entry, models_dir)

Direction Check: PASS — entry seams operate on production verification, extraction, and cleanup functions, NOT isolated leaf hash algorithms.

Production Path — Verification & Marker Fast-Path:
  is_model_file_present(entry, models_dir)
  ├─► Check .verified Marker:
  │   If verified_path.exists() => VerifiedMarker::load(&verified_path)
  │   If marker.sha256 == entry.sha256 && dest_path.exists() && size_matches:
  │   └──► Returns true immediately (fast-path avoids expensive hashing)
  ├─► Check File & Generate Marker:
  │   If dest_path.exists() && metadata(dest_path).len() == entry.size_bytes:
  │   └──► VerifiedMarker::save(&verified_path) with model_id, sha256, verified_at, expected_size
  │   └──► Returns true
  └─► If missing or size mismatch => returns false

Production Path — Path Traversal Defense (ZipSlip & TarSlip):
  ModelManager::do_extract(archive, type, dest_dir)
  ├─► Zip Archives ("zip"):
  │   Iterates archive entries
  │   entry.enclosed_name() ──► checks for parent directory traversal ("../")
  │   If None => returns Err("Zip-Slip vulnerability detected: illegal file path in archive")
  ├─► Tar Archives ("tar.gz" | "tgz"):
  │   Iterates archive entries
  │   path.components().any(|c| c == Component::ParentDir)
  │   If true => returns Err("Tar-Slip vulnerability detected: illegal path traversal in archive")
  └─► Unpacks file only within dest_dir

Production Path — Clean Model Teardown:
  delete_model_file(entry, models_dir)
  ├─► Removes verified marker: remove_file(&verified_path)
  └─► Removes model file / directory: remove_file or remove_dir_all(&dest_path)

Observable Exit:
  1. Verified Marker: `.verified` JSON file created on disk matching entry metadata.
  2. Corrupt Payload Rejection: tampered files fail validation; stale/corrupted markers rejected.
  3. Traversal Defense: zip entries with `../` return `enclosed_name().is_none()`; tar entries with `ParentDir` are rejected.
  4. Cleanup: `delete_model_file` removes both model payload and `.verified` marker from disk.

Production Functions Called:
  setup:   tempdir(), create_synthetic_model_entry(), create_test_zip_archive(), create_test_tar_gz_archive()
  entry:   is_model_file_present(), delete_model_file(), ModelManager::do_extract()
  observe: VerifiedMarker::load(), verified_path.exists(), dest_path.exists(), zip::ZipArchive::by_index()
  teardown: drop tempdir, drop TempPathsGuard
```

#### 17.3 `/create-test` Phase 2a — Testability Check

1. Entry seam callable with production signature? **Yes** (`is_model_file_present`, `delete_model_file`, `do_extract`).
2. Production constructors used? **Yes** (`VerifiedMarker`, `ZipArchive`, `tar::Archive`).
3. State observable? **Yes** (`.verified` JSON marker, physical file existence, error results).
4. Real components without mocks? **Yes** (100% production code, zero mocks).

#### 17.4 `/create-test` Phase 2b — False-Green Audit Table

| If this production defect existed                               | Would Seam 17 test fail? | Expected Failure Mode                                         |
| --------------------------------------------------------------- | ------------------------ | ------------------------------------------------------------- |
| **`is_model_file_present` returns true for non-existent files** | **Must fail**            | Assertion on absent model file panics.                        |
| Verification marker creation omitted on valid payload           | Must fail                | Assertion `verified_path.exists()` panics.                    |
| Size mismatch silently accepted without hash validation         | Must fail                | Subtest 2 assertion `!present` panics.                        |
| Stale marker with wrong SHA-256 accepted                        | Must fail                | Subtest 2 assertion on bad marker panics.                     |
| ZipSlip defense omitted (allows parent directory traversal)     | Must fail                | Subtest 3 assertion `entry.enclosed_name().is_none()` panics. |
| TarSlip defense omitted (allows `ParentDir` extraction)         | Must fail                | Subtest 3 assertion `has_parent` panics.                      |
| `delete_model_file` leaves `.verified` marker on disk           | Must fail                | Subtest 4 assertion `!verified_path.exists()` panics.         |

#### 17.5 `/test` Execution Protocol

- **Command:** `cargo nextest run --test model_manager_test --release --nocapture --test-threads=1`
- **Success Criteria:** 5 tests pass in < 0.2s.
- **Subtests:**
  1. `test_model_manager_valid_payload_verification`: Valid payload $\to$ size match $\to$ `.verified` marker generated $\to$ fast-path reuse.
  2. `test_model_manager_corrupted_payload_detection`: Corrupted payload fails size $\to$ marker not created $\to$ corrupted hash marker rejected.
  3. `test_model_manager_zip_slip_rejection`: Synthetic evil zip with `"../../"` rejected by `enclosed_name()`.
  4. `test_model_manager_tar_slip_rejection`: Synthetic evil tar.gz with `"../../"` rejected by `Component::ParentDir`.
  5. `test_model_manager_removal_cleans_marker`: `delete_model_file` deletes payload and `.verified` marker.

#### 17.6 `/mutate` Mutant Definitions (Tier 1)

- **Mutant 17.1 (Marker Fast-Path Hash Check Deletion):** In `setup/manager_ops.rs:is_model_file_present`, change `marker.sha256 == file.sha256` to `true`.  
  _Prediction:_ Subtest 2 goes RED because bad marker with wrong hash passes validation.
- **Mutant 17.2 (Marker File Deletion Omission):** In `setup/manager_ops.rs:delete_model_file`, comment out `remove_file(&verified_path)`.  
  _Prediction:_ Subtest 4 goes RED on `assert!(!verified_path.exists())`.
- **Mutant 17.3 (ZipSlip Enclosed Name Inversion):** In `setup/model_manager.rs:do_extract`, replace `entry.enclosed_name()` with `Some(PathBuf::from(entry.name()))`.  
  _Prediction:_ Subtest 3 goes RED because Zip-Slip traversal error is not triggered.

---

### Seam 18 — Notification Center: 3D Matrix Routing, Category Aggregation & Action Execution (`tests/notifications_crud_test.rs`)

#### 18.1 Seam Identifier & Classification

- **Binary:** `app/src-tauri/tests/notifications_crud_test.rs`
- **Classification:** **Category B (Needs Expansion — 3D Matrix Routing & Action Execution Integration)**
- **Subsystems:** `services/notifications/mod.rs`, `services/notifications/channels.rs`, `persistence/notifications.rs`, `persistence/compactions.rs`, `core/notifications.rs`, `ipc/notifications/mod.rs`
- **Execution Command:** `cargo nextest run --test notifications_crud_test --release --nocapture --test-threads=1`
- **Execution Baseline:** 3 passed in 0.038s (testing raw SQLite persistence layer).

> [!NOTE]
> **Expansion Requirement Ledger:**
>
> 1. Under the [Notification Center Behavioral & Interface Specification (v2)](file:///home/addy/projects/apps/vox/docs/specs/notifications-spec.md), notifications route across a 3D matrix (`impact`, `severity`, `action`), mapped to 7 closed categories (`AudioHardware`, `ModelLifecycle`, `Inference`, `ContextBudget`, `Network`, `Memory`, `SystemHealth`).
> 2. `tests/notifications_crud_test.rs` currently verifies only raw SQLite schema persistence in `persistence/notifications.rs` and `persistence/compactions.rs` (CRUD, pagination, filters, and cascades).
> 3. The target integration test must expand coverage to the active routing service:
>    - **3D Matrix Routing & Zero-DB Invariant:** `services::notifications::notify` routes `NotificationAction::Transient` to HUD stream with **zero database writes** (`persistence::get_notifications` count remains unchanged).
>    - **Idempotent Group-Key Upsert:** `NotificationAction::Interactive` with duplicate `group_key` executes in-place `UPDATE` incrementing `occurrence_count` without inserting duplicate rows.
>    - **Action Execution Engine:** `services::notifications::execute_notification_action` dispatches polymorphic payloads (`TriggerCompaction`, `SwitchModel`, `OpenSettings`, `Dismiss`) and executes targeted mutations cleanly.

#### 18.2 `/create-test` Phase 1 — Production Path Trace

```
Production Entry Seams:
  Entry Seam A (System Notification Ingestion & Routing):
    services::notifications::notify(&state, event)
  Entry Seam B (Interactive Action Execution):
    services::notifications::execute_notification_action(&state, action_id, payload)
  Entry Seam C (Database Persistence & Aggregation):
    persistence::notifications::create_notification(&conn, new_notif)
    persistence::notifications::list_notifications(&conn, filter)

Direction Check: PASS — entry seams operate on high-level notification dispatch and action resolution engines, NOT isolated database queries.

Production Path A — 3D Matrix Routing & Channel Resolution:
  services::notifications::notify(&state, event)
  ├─► resolve_channel(event.impact, event.severity, event.action)
  ├─► Channel Resolution:
  │   ├─► Action::Transient =>
  │   │   ├─► Emits IpcEvent::HudNotification to frontend webview
  │   │   └─► Zero-DB Invariant: does NOT invoke persistence::create_notification
  │   ├─► Action::Receipt =>
  │   │   ├─► Ingests into SQLite notifications table (is_read = false, occurrence_count = 1)
  │   │   ├─► Emits IpcEvent::NotificationReceipt to status badge
  │   │   └─► Does NOT display blocking HUD popup
  │   └─► Action::Interactive(action_payload) =>
  │       ├─► Evaluates group_key correlation:
  │       │   If existing pending notification has matching group_key:
  │       │   └──► Executes in-place UPDATE: increments occurrence_count, updates updated_at timestamp
  │       │   Else:
  │       │   └──► Ingests new record with action metadata
  │       ├─► Emits IpcEvent::InteractiveAlert to user interface
  │       └─► Persists to SQLite notifications table
  └─► Emits VoxEvent::NotificationCreated to internal pipeline bus

Production Path B — Action Execution Engine:
  services::notifications::execute_notification_action(&state, action_id, payload)
  ├─► Fetches notification by action_id; verifies not expired or already executed
  ├─► Match payload:
  │   ├─► ActionPayload::TriggerCompaction { session_id } =>
  │   │   Triggers services::memory::compactor::trigger_compaction(&state, session_id)
  │   ├─► ActionPayload::SwitchModel { model_id } =>
  │   │   Triggers services::models::manager::switch_active_model(&state, &model_id)
  │   ├─► ActionPayload::OpenSettings { section } =>
  │   │   Dispatches IPC event to open client view
  │   └─► ActionPayload::Dismiss =>
  │       Marks notification as dismissed / archived in SQLite
  └─► Updates notification state: marks action_status = Executed, executed_at = now()

Observable Exit:
  1. Zero-DB Invariant: Transient notifications emit IPC events but leave notifications table count at 0.
  2. Occurrence Rollup: Repeated interactive notifications with identical group_key update the existing row (`occurrence_count = 2`) rather than creating a duplicate row.
  3. Action Execution: `execute_notification_action` updates `action_status` to `Executed` and triggers the corresponding subsystem mutation.
  4. Query Filtering: `list_notifications` filters correctly by category, severity, and read status.

Production Functions Called:
  setup:   get_test_app_and_state(), init_test_db()
  entry:   services::notifications::notify(), services::notifications::execute_notification_action(),
           persistence::notifications::create_notification(), persistence::notifications::list_notifications()
  observe: IpcEvent receiver, conn.query_row(SELECT occurrence_count ...), get_notification_by_id()
  teardown: drop conn, drop TempPathsGuard
```

#### 18.3 `/create-test` Phase 2a — Testability Check

1. Entry seam callable with production signature? **Yes** (`notify(&state, event)`, `execute_notification_action(...)`, `list_notifications(...)`).
2. Production constructors used? **Yes** (`NotificationEvent`, `ActionPayload`, real in-memory SQLite connection via Turso engine).
3. State observable? **Yes** (SQLite database state, `occurrence_count`, `action_status`, IPC event emission).
4. Real components without mocks? **Yes** (Real notification service and Turso persistence engine).

#### 18.4 `/create-test` Phase 2b — False-Green Audit Table

| If this production defect existed                                                    | Would Seam 18 test fail? | Expected Failure Mode                                                |
| ------------------------------------------------------------------------------------ | ------------------------ | -------------------------------------------------------------------- |
| **Transient notification written to database (violating Zero-DB Invariant)**         | **Must fail**            | Assertion `list_notifications().is_empty()` panics.                  |
| Group-key duplicate creates new row instead of in-place `occurrence_count` increment | Must fail                | Assertion `notifications.len() == 1` panics (returns 2).             |
| Occurrence count not incremented on deduplicated notification                        | Must fail                | Assertion `notification.occurrence_count == 2` panics (remains 1).   |
| `execute_notification_action` fails to mark action as `Executed`                     | Must fail                | Assertion `action_status == ActionStatus::Executed` panics.          |
| Filter by `NotificationCategory` returns wrong category entries                      | Must fail                | Subtest assertion filtering by category receives unexpected records. |

#### 18.5 `/test` Execution Protocol

- **Command:** `cargo nextest run --test notifications_crud_test --release --nocapture --test-threads=1`
- **Success Criteria:**
  1. Subtest 1 (Persistence CRUD & Pagination): Create, read, update read status, soft delete, and category filtering.
  2. Subtest 2 (Compaction Association): Link compaction record to notification; cascading integrity verified.
  3. Subtest 3 (3D Routing & Zero-DB Invariant): Dispatch `Action::Transient` $\to$ IPC emitted $\to$ database remains 0 rows.
  4. Subtest 4 (Group-Key Rollup): Dispatch 2 identical `Action::Interactive` events with same `group_key` $\to$ single database record with `occurrence_count = 2`.
  5. Subtest 5 (Action Execution): Dispatch `execute_notification_action` with `TriggerCompaction` $\to$ action status transitions to `Executed`.

---

### Seam 19 — Realtime S2S WebSocket Driver Transport Lifecycle (`tests/realtime_transport_test.rs`)

#### 19.1 Seam Identifier & Classification

- **Binary:** `app/src-tauri/tests/realtime_transport_test.rs`
- **Classification:** **Category D (Brand New Seam — Target Specification for Integration Test)**
- **Subsystems:** `services/realtime/transport/connection.rs`, `services/realtime/transport/mod.rs`, `services/realtime/transport/health.rs`, `services/realtime/session.rs`, `services/realtime/providers/gemini/session.rs`, `services/realtime/providers/deepgram/session.rs`
- **Execution Command:** `cargo nextest run --test realtime_transport_test --release --nocapture --test-threads=1`
- **Cloud Ignored Suite:** `cargo nextest run --test realtime_transport_test --release --nocapture --test-threads=1 -- --ignored`

> [!NOTE]
> **Architectural Invariant Ledger:**
>
> 1. **Zero External Cloud Dependency by Default:** All standard CI subtests run against a local in-process mock WebSocket server (`tokio-tungstenite`) to verify wire framing, reconnect backoff, keepalive pings, and GoAway handling deterministically with zero network flakiness.
> 2. **Cloud E2E Gated Behind `#[ignore]`:** Subtests connecting to live Gemini Live or Deepgram endpoints require real API keys and must be annotated `#[ignore]` per Rule 3.4.
> 3. **Terminal Error Invariant:** When `max_reconnect_attempts` (default 3) is exhausted, the harness must cleanly set `terminated = true`, emit `RealtimeProviderEvent::Error` with `PipelineImpact::SessionHalted`, abort background tasks, and drop `ws_sender`.

#### 19.2 `/create-test` Phase 1 — Production Path Trace

```
Production Entry Seams:
  Entry Seam A (Transport Harness Spawning & Outbound Transmission):
    services::realtime::transport::connection::spawn_harness(driver, config, init)
    outbound_tx.send(OutboundCommand::Audio(pcm))
    outbound_tx.send(OutboundCommand::ActivityStart)
  Entry Seam B (Session Cache Token Lifecycle):
    services::realtime::session::create_realtime_provider(&state)
    services::realtime::session::purge_session_cache()

Direction Check: PASS — entry seams operate on high-level connection harness and session factory, NOT raw TCP sockets or tungstenite write loops.

Production Path A — Normal Duplex Streaming & Outbound Task:
  spawn_harness(driver, config, init)
  ├─► Spawns run_outbound_encoder:
  │   Receives OutboundCommand on outbound_rx
  │   Calls driver.encode(cmd) -> returns Option<Message>
  │   Dispatches Message to ws_sender (write_tx)
  ├─► Spawns optional keepalive_task (periodically emits OutboundCommand::KeepAlive)
  └─► Spawns spawn_connection_tasks:
      ├─► write_task: reads Message from write_rx and transmits across WsWriter
      └─► receiver_task: reads Frame from WsReader, passes to driver.handle_frame(&event_tx)

Production Path B — Disconnect / GoAway Reconnect Loop:
  receiver_task detects socket closure / receives FrameAction::GoAway
  ├─► If state_rx == InteractionState::Paused:
  │   └──► Silently disconnects without triggering reconnect loop
  ├─► Else:
  │   ├─► Signals reconnect_notifier oneshot
  │   ├─► Main loop locks ws_sender = None, aborts stale write_handle and recv_handle
  │   ├─► Loops attempt in 0..config.max_reconnect_attempts:
  │   │   ├─► Sleeps delay = (attempt * factor + base_delay)
  │   │   ├─► Invokes reconnect_fn()
  │   │   ├─► On Success: spawns fresh connection tasks; reconnected = true; breaks loop
  │   │   └─► On Error / Timeout: logs warning, increments attempt
  │   └─► If !reconnected (Terminal Failure):
  │       ├─► Emits RealtimeProviderEvent::Error { impact: PipelineImpact::SessionHalted }
  │       ├─► Sets terminated.store(true, SeqCst)
  │       ├─► Locks ws_sender = None
  │       └─► Aborts outbound_task and keepalive_task

Production Path C — Session Cache TTL Management:
  create_realtime_provider(&state)
  ├─► Checks cache_dir().join("realtime_session.json")
  ├─► Reads expires_at:
  │   ├─► If now_ms < expires_at: injects cached handle into settings.resume_handle
  │   └─► If now_ms >= expires_at: calls purge_session_cache(), deletes file from disk

Observable Exit:
  1. Frames sent via outbound_tx arrive on mock server in expected wire format.
  2. Inbound server frames trigger corresponding RealtimeProviderEvent on provider_event_tx.
  3. Forced socket termination initiates reconnect; reconnection succeeds after backoff delay.
  4. Exhaustion of max reconnect attempts emits PipelineImpact::SessionHalted and sets terminated = true.
  5. Paused interaction state suppresses reconnection attempts.
  6. Expired cache file (> 2 hours) is purged automatically on create_realtime_provider.

Production Functions Called:
  setup:   spawn_mock_ws_server(), HarnessConfig { max_reconnect_attempts: 3, ... }, GeminiDriver::new()
  entry:   spawn_harness(), outbound_tx.send(), purge_session_cache(), create_realtime_provider()
  observe: provider_event_rx.recv(), handles.terminated.load(), mock_server.received_messages()
  teardown: handles.shutdown_tx.lock().take().unwrap().send(()), drop mock_server
```

#### 19.3 `/create-test` Phase 2a — Testability Check

1. Entry seam callable with production signature? **Yes** (`spawn_harness`, `create_realtime_provider`, `purge_session_cache`).
2. Production constructors used? **Yes** (`HarnessConfig`, `HarnessInit`, `GeminiDriver`, real `tokio` runtime).
3. State observable? **Yes** (`provider_event_rx`, `terminated` atomic flag, mock server packet logs).
4. Deterministic local execution? **Yes** (Runs in-process on `127.0.0.1` in < 2.0s without external API calls).

#### 19.4 `/create-test` Phase 2b — False-Green Audit Table

| If this production defect existed                                           | Would Seam 19 test fail? | Expected Failure Mode                                                          |
| --------------------------------------------------------------------------- | ------------------------ | ------------------------------------------------------------------------------ |
| **`spawn_harness` drops outbound commands (`run_outbound_encoder` stalls)** | **Must fail**            | Mock server receives 0 frames; timeout assertion panics.                       |
| Reconnect loop does not back off / spams reconnect immediately              | Must fail                | Elapsed time assertion before reconnect attempt panics.                        |
| Reconnect attempts exceed `max_reconnect_attempts` without halting session  | Must fail                | Subtest fails: `terminated` remains `false`; no `SessionHalted` error emitted. |
| Disconnect during `InteractionState::Paused` triggers reconnect             | Must fail                | Subtest fails: reconnect callback executed despite `Paused` state.             |
| Expired session resumption token not purged from disk                       | Must fail                | Subtest fails: cache file still exists after `create_realtime_provider`.       |

#### 19.5 `/test` Execution Protocol

- **Command:** `cargo nextest run --test realtime_transport_test --release --nocapture --test-threads=1`
- **Success Criteria:**
  1. Subtest 1 (Duplex Wire Framing): Outbound commands encoded and delivered; inbound audio frames decoded into `RealtimeProviderEvent::AudioChunk`.
  2. Subtest 2 (Successful Reconnect): Drop server socket $\to$ harness triggers `reconnect_fn` $\to$ stream seamlessly resumes.
  3. Subtest 3 (Terminal Reconnect Failure): Server stays down $\to$ after 3 attempts, harness emits `PipelineImpact::SessionHalted` and marks `terminated = true`.
  4. Subtest 4 (Paused State Guard): Drop connection while `state_rx == Paused` $\to$ harness exits cleanly without reconnecting.
  5. Subtest 5 (Session Cache TTL): Write synthetic cache file with expired timestamp $\to$ `create_realtime_provider` removes file.
  6. Subtest 6 (Live Gemini / Deepgram E2E) `#[ignore]`: Connects to live endpoint with API key and verifies handshake.

#### 19.6 `/mutate` Mutant Definitions (Tier 1)

- **Mutant 19.1 (Terminal Flag Deletion):** In `services/realtime/transport/connection.rs`, comment out `terminated_clone.store(true, Ordering::SeqCst)`.  
  _Prediction:_ Subtest 3 goes RED on `assert!(handles.terminated.load(Ordering::SeqCst))`.
- **Mutant 19.2 (Paused State Reconnect Inversion):** In `services/realtime/transport/connection.rs:spawn_connection_tasks`, remove `if *state_rx.borrow() == InteractionState::Paused { break; }`.  
  _Prediction:_ Subtest 4 goes RED because reconnect notifier is signaled during Paused state.
- **Mutant 19.3 (Cache Expiry Logic Inversion):** In `services/realtime/session.rs:create_realtime_provider`, change `now_ms < expires_at` to `now_ms > expires_at`.  
  _Prediction:_ Subtest 5 goes RED because unexpired tokens are purged and expired tokens are kept.

---

### Seam 20 — Database v2 Persistence Boundary & Turso MVCC (`tests/database_persistence_boundary_test.rs`)

#### 20.1 Seam Identifier & Classification

- **Binary:** `app/src-tauri/tests/database_persistence_boundary_test.rs`
- **Classification:** **Category D (Brand New Seam — Target Specification for Integration Test)**
- **Subsystems:** `persistence/db.rs`, `persistence/schema.rs`, `persistence/worker.rs`, `persistence/mod.rs`, `persistence/compactions.rs`, `persistence/facts.rs`, `persistence/personal_memory.rs`, `persistence/projects.rs`, `persistence/sessions.rs`, `persistence/notifications.rs`
- **Execution Command:** `cargo nextest run --test database_persistence_boundary_test --release --nocapture --test-threads=1`

> [!NOTE]
> **Architectural Invariant Ledger:**
>
> 1. **Turso Native MVCC & WAL Invariant:** `VoxDb::open` configures `PRAGMA journal_mode = WAL`, `PRAGMA busy_timeout = 5000`, and `PRAGMA foreign_keys = ON`. Concurrent multi-threaded reads must never block or be blocked by background persistence worker writes.
> 2. **Relational Cascade & Restriction Invariants:**
>    - `projects` deletion is blocked (`ON DELETE RESTRICT`) when child `sessions` exist.
>    - `sessions` deletion cascades (`ON DELETE CASCADE`) to `turns`, `notifications`, `session_compactions`, and safely decouples (`ON DELETE SET NULL`) historical `memory_facts` and `memory_ingestion_queue` records.
>    - `memory_facts` deletion cascades (`ON DELETE CASCADE`) to `memory_facts_vectors`.
> 3. **Single In-Progress Compaction Index Invariant:** Unique partial index `idx_compactions_one_in_progress` enforces at the SQLite engine level that no session can have more than one compaction in `in_progress` state simultaneously.
> 4. **`F32_BLOB(384)` Vector Fidelity:** Embeddings stored via `encode_f32_blob` and loaded via `decode_f32_blob` retain exact IEEE 754 float representations across 384 dimensions.

#### 20.2 `/create-test` Phase 1 — Production Path Trace

```
Production Entry Seams:
  Entry Seam A (Database Initialization & Schema Migration):
    persistence::db::VoxDb::open(&db_path)
    persistence::schema::run_migrations(&conn)
  Entry Seam B (Async Persistence Worker Pipeline):
    persistence::worker::spawn_persistence_worker(&state, &conn)
    persistence_tx.send(PersistenceEvent::TurnCompleted { ... })
  Entry Seam C (Relational Operations & Integrity Queries):
    persistence::projects::delete_project(&conn, project_id)
    persistence::sessions::delete_session(&conn, session_id)
    persistence::compactions::record_compaction_start(&conn, session_id, ...)
    persistence::facts::insert_vector(&conn, fact_id, fact_type, &embedding)

Direction Check: PASS — entry seams operate on production database connection factory, schema runner, worker thread, and domain repositories.

Production Path A — Clean-Slate Migration & Schema Initialization:
  VoxDb::open(db_path)
  ├─► Builder::new_local(path).experimental_index_method(true).build()
  ├─► Sets PRAGMA journal_mode = WAL
  ├─► Sets PRAGMA busy_timeout = 5000
  ├─► Sets PRAGMA foreign_keys = ON
  run_migrations(conn)
  ├─► Checks PRAGMA user_version: if < 4:
  │   ├─► Drops obsolete legacy tables (DROP_LEGACY_TABLES)
  │   ├─► Executes V2_TABLE_STATEMENTS (10 tables, indexes, unique partial indexes)
  │   ├─► Inserts seed project ('default') and default personal_memory (project_id = NULL)
  │   ├─► Sets PRAGMA user_version = 4
  │   └─► Seeds packaged voices (seed_packaged_voices)

Production Path B — Concurrency & MVCC Worker Stream:
  spawn_persistence_worker(state, conn)
  ├─► Background thread listens on persistence_rx (capacity 128)
  ├─► Dispatches PersistenceEvent::TurnCompleted:
  │   Inserts turn into turns table; updates sessions.updated_at
  ├─► Reader threads concurrently execute:
  │   fetch_active_facts_by_type, list_sessions, get_personal_memory
  └─► Zero locks / zero database busy errors (WAL MVCC isolation)

Production Path C — Relational Constraint Enforcement:
  1. RESTRICT check: Attempt delete_project('default') with child sessions -> Returns Err(ForeignConstraintViolation)
  2. CASCADE check: delete_session(session_id) -> Deletes session; turns and notifications disappear; facts retain record with session_id = NULL
  3. PARTIAL INDEX check: record_compaction_start on session with active in_progress compaction -> Returns Err(UniqueConstraintViolation)

Observable Exit:
  1. Database file initialized with PRAGMA user_version = 4 and 10 v2 tables.
  2. Project deletion restricted when sessions present; session deletion cascades to turns and compactions.
  3. Duplicate in_progress compaction rejected by engine.
  4. Concurrent readers and worker writer execute 100 iterations without SQLiteBusy error.
  5. 384-dimensional float vector stored and retrieved with 100% precision.

Production Functions Called:
  setup:   tempdir(), VoxDb::open(), run_migrations()
  entry:   record_compaction_start(), delete_project(), delete_session(), insert_vector(), spawn_persistence_worker()
  observe: conn.query(), conn.execute(), decode_f32_blob()
  teardown: drop conn, drop tempdir
```

#### 20.3 `/create-test` Phase 2a — Testability Check

1. Entry seam callable with production signature? **Yes** (`VoxDb::open`, `run_migrations`, `record_compaction_start`, etc.).
2. Production constructors used? **Yes** (`turso::Builder`, real temporary SQLite file on disk).
3. State observable? **Yes** (Database tables, PRAGMA queries, return `Result` values).
4. Real components without mocks? **Yes** (100% production Turso engine and repositories).

#### 20.4 `/create-test` Phase 2b — False-Green Audit Table

| If this production defect existed                                                | Would Seam 20 test fail? | Expected Failure Mode                                                              |
| -------------------------------------------------------------------------------- | ------------------------ | ---------------------------------------------------------------------------------- |
| **`PRAGMA foreign_keys = ON` omitted in `VoxDb::open`**                          | **Must fail**            | RESTRICT test allows deleting project with active sessions; CASCADE fails.         |
| Schema migration fails to create partial index `idx_compactions_one_in_progress` | Must fail                | Subtest 3 succeeds in creating 2 concurrent `in_progress` compactions (must fail). |
| WAL mode not enabled (`PRAGMA journal_mode` remains DELETE)                      | Must fail                | Subtest 4 concurrent worker/reader test deadlocks or hits `database locked`.       |
| Session cascade configured as `CASCADE` on `memory_facts` instead of `SET NULL`  | Must fail                | Subtest 2 fails: memory facts deleted when session is deleted (history lost).      |
| `F32_BLOB` vector encoding truncates or corrupts float alignment                 | Must fail                | Subtest 5 assertion `retrieved_vec == original_vec` panics.                        |

#### 20.5 `/test` Execution Protocol

- **Command:** `cargo nextest run --test database_persistence_boundary_test --release --nocapture --test-threads=1`
- **Success Criteria:**
  1. Subtest 1 (Migration & Seeding): Migration sets `user_version = 5`, creates 11 tables (including `session_tool_calls`), seeds default project and personal memory.
  2. Subtest 2 (Relational Cascades & Restrict): `ON DELETE RESTRICT` protects project; `ON DELETE CASCADE` purges turns/notifications; `SET NULL` preserves facts.
  3. Subtest 3 (Unique Partial Index): Duplicate `in_progress` compaction on same session returns unique constraint error.
  4. Subtest 4 (Concurrent MVCC Under WAL): Background persistence worker writes 50 turns while 4 reader threads continuously query sessions and facts without contention errors.
  5. Subtest 5 (F32_BLOB 384-dim Vector Roundtrip): Encodes 384 floats, inserts into `memory_facts_vectors`, selects blob, decodes and asserts exact IEEE 754 equality.
  6. Subtest 6 (Schema v5 Tool Calls & Private Mode): Verifies `session_tool_calls` schema and indexes, self-healing parent session creation (`ensure_session_exists`), and private mode tool call suppression.

#### 20.6 `/mutate` Mutant Definitions (Tier 1)

- **Mutant 20.1 (Foreign Keys Pragma Deletion):** In `persistence/db.rs:open`, comment out `conn.execute("PRAGMA foreign_keys = ON;", ()).await`.  
  _Prediction:_ Subtest 2 goes RED because `delete_project` succeeds despite child sessions.
- **Mutant 20.2 (Partial Index WHERE Clause Deletion):** In `persistence/schema.rs:V2_TABLE_STATEMENTS`, remove `WHERE status = 'in_progress'` from `idx_compactions_one_in_progress`.  
  _Prediction:_ Subtest 3 goes RED because multiple completed compactions are erroneously rejected.
- **Mutant 20.3 (Vector Float Little-Endian Swap):** In `persistence/mod.rs:encode_f32_blob`, replace `f.to_le_bytes()` with `f.to_be_bytes()`.  
  _Prediction:_ Subtest 5 goes RED on `assert_eq!(retrieved, original)`.
- **Mutant 20.4 (Private Mode Event Drop Bypass):** In `persistence/worker.rs:run_event_loop`, bypass `is_private_mode` check for `PersistenceEvent::ToolCallExecuted`.  
  _Prediction:_ Subtest 6 goes RED on `Tool call record must NOT be inserted into SQLite when private mode is active`.

---

### Seam 21 — Agentic Tool Runtime, Taxonomy & Scratchpad Isolation (`tests/agentic_tool_runtime_test.rs`)

#### 21.1 Seam Identifier & Classification

- **Binary:** `app/src-tauri/tests/agentic_tool_runtime_test.rs`
- **Classification:** **Category A (Solid — Verified Green & Validated)**
- **Subsystems:** `services/harness/stages/tools/`, `services/harness/steps.rs`, `services/harness/stages/prompt.rs`, `persistence/sessions.rs`, `pipeline/assistant/session.rs`
- **Execution Command:** `cargo nextest run --test agentic_tool_runtime_test --release --nocapture --test-threads=1`

#### 21.2 `/create-test` Phase 1 — Production Path Trace

```
SUT: ToolExecutor dispatches canonical tool calls; Terminal tool delivers voice directly and updates session title in DB; NonTerminal tool executes hybrid RRF retrieval into scratchpad; Turn completion drops scratchpad ensuring 100% memory/DB parity.

Production Entry Seam:
  ToolExecutor::execute_tool(registry, call, tool_ctx)
  and Harness::execute_turn(harness_arc, req)

Direction Check: PASS — entry seam is the orchestrator tool dispatch, NOT direct SQL insertion.

Production Path A — Terminal Tool Flow (respond_and_set_title):
  Model proposes CanonicalToolCall { name: "respond_and_set_title", args: { title, spoken_response } }
  ──► step6_handle_terminal_tool()
      ├─► Sets accumulator.assistant_response = spoken_response (Guarantees DB turns parity!)
      ├─► Dispatches spoken_response through ClauseChunker to tts_tx as TurnResponse
      ├─► Emits LlmFinished to event_tx
      ├─► ToolExecutor runs RespondAndSetTitleTool::execute()
      │   ├─► Checks is_private_mode (skips DB if private)
      │   └─► Writes set_session_title to SQLite
      ├─► Records title_set = true on Harness
      └─► Commits only spoken_response to history.messages() (zero tool syntax in prompt history)

Production Path B — NonTerminal Tool Flow (search_memory):
  Model proposes CanonicalToolCall { name: "search_memory", args: { query, spoken_filler } }
  ──► step6_handle_non_terminal_tool()
      ├─► Discards prefix text; enters NonTerminalPhase with spoken_filler
      ├─► ToolExecutor runs MemorySearchTool::execute()
      │   ├─► Computes vector embedding + full-text lexical search
      │   └─► RRF fusion (k=60) ranks top candidate facts (excludes personal memory)
      ├─► Appends ToolCall and Tool observation to ephemeral scratchpad
      └─► Loop re-enters step4_assemble_request with scratchpad included

Production Path C — Turn Finalization & Scratchpad Drop:
  ──► Terminal completion commits assistant_response to history
  ──► Ephemeral scratchpad dropped at function exit
```

#### 21.3 `/create-test` Phase 2a — Testability Check

1. Entry seam callable with production signature? **Yes** (`execute_tool`, `execute_turn`).
2. Production constructors used? **Yes** (`ToolRegistry::with_default_tools()`, `ToolExecutor`).
3. State observable? **Yes** (SQLite `sessions.title`, SQLite `session_tool_calls`, `TurnAccumulator.assistant_response`, `harness.history.messages()`).
4. Real components without mocks? **Yes** (Real tool implementations, real SQLite instance, real token accounting).

#### 21.4 `/create-test` Phase 2b — False-Green Audit Table

| If this production defect existed | Would Seam 21 test fail? | Expected Failure Mode |
|---|---|---|
| `respond_and_set_title` does not update `TurnAccumulator` | **Must fail** | `accumulator.assistant_response` remains empty; assertion fails. |
| Tool syntax/scratchpad committed to working history | **Must fail** | `harness.history.messages()` contains `Role::Tool` or `tool_calls`; assertion fails. |
| Title tool offered on Turn 2 of a session | **Must fail** | `active_definitions(&filter)` on Turn 2 contains `respond_and_set_title`. |
| Private mode leaks session title to SQLite | **Must fail** | `sessions.title` in DB updated despite private mode active. |
| `search_memory` returns personal facts | **Must fail** | Retrieval observation contains facts tagged with `personal` scope. |

#### 21.5 `/test` Execution Protocol

- **Command (Local Tests):** `cargo nextest run --test agentic_tool_runtime_test --release --nocapture --test-threads=1`
- **Command (Cloud Model Test):** `cargo nextest run --test agentic_tool_runtime_test --release --nocapture --test-threads=1 -- --ignored`
- **Success Criteria:**
  1. Subtest 1 (`test_terminal_tool_title_and_accumulator_parity`): Verifies `respond_and_set_title` updates SQLite `sessions.title`, sets `TurnAccumulator.assistant_response`, emits `LlmFinished`, and dispatches `TurnResponse` audio clauses to TTS.
  2. Subtest 2 (`test_tool_filter_session_local_turn1_gate`): Verifies dynamic turn gating: Turn 1 provides `respond_and_set_title`, Turn 2+ suppresses title tool and provides only memory tools.
  3. Subtest 3 (`test_scratchpad_isolation_and_drop_on_commit`): Verifies NonTerminal `search_memory` executes hybrid RRF retrieval, returns structured observation to `scratchpad`, re-enters generation, and drops scratchpad at turn exit without leaking tool syntax into working history.
  4. Subtest 4 (`test_private_mode_title_leak_prevention`): Verifies private mode suppresses DB writes to `sessions.title` during terminal tool execution.
  5. Subtest 5 (`test_live_cloud_model_tool_calling_single_pass`): Live cloud model tool calling test marked `#[ignore]`.

#### 21.6 `/mutate` Mutant Definitions (Tier 1)

- **Mutant 21.1 (Terminal Flow Contract Inversion):** In `services/harness/stages/tools/title.rs:41`, change `RespondAndSetTitleTool::flow()` from `ToolFlow::Terminal` to `ToolFlow::NonTerminal`.  
  _Prediction:_ Subtest 1 goes RED because orchestrator treats terminal title tool as multi-pass reentrant observation.
- **Mutant 21.2 (Turn 2+ Title Filter Inversion):** In `services/harness/stages/tools/registry.rs:51`, bypass `respond_and_set_title` suppression filter on Turn 2+.  
  _Prediction:_ Subtest 2 goes RED with `Turn 2 must suppress respond_and_set_title`.
- **Mutant 21.3 (Title Emptiness Validation Bypass):** In `services/harness/stages/tools/title.rs:64`, bypass `title.is_empty()` error validation check.  
  _Prediction:_ Negative test goes RED on empty title argument.
- **Mutant 21.4 (Turn Accumulator Parity Omission):** In `services/harness/steps.rs:440`, omit `stream_handles.accumulator` assignment in `step6_handle_terminal_tool`.  
  _Prediction:_ Subtest 1 goes RED on `TurnAccumulator must capture spoken_response for DB parity`.
- **Mutant 21.5 (Scratchpad Observation Push Corruption):** In `services/harness/steps.rs:556`, push empty string observation to `scratchpad` in `step6_handle_non_terminal_tool`.  
  _Prediction:_ Subtest 3 goes RED on `Scratchpad observation must contain memory search output`.

