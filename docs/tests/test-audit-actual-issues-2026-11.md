# Vox Test Suite — Actual-Issues Report (Voice, Session & Remote)

**Date:** 2026-11-07
**Source:** Senior test-engineer transcript (Parts 1–4) — decoupled as architect. This file lists **only** actual test gaps. All retro recipe fixes live in the three recipe files listed in §8.
**Scope:** `app/src-tauri/tests/` seam binaries named in the scorecard, plus `benches/pipeline_bench.rs` / `benches/common/pipeline_harness.rs` harness wiring.
**Canonical traces (now present):**

* `docs/plans/phase14/voice_pipeline_scenarios_trace.md` — 34 steps, 5 branch points (1A–1F, 2A–2B, 3A–3C, 4A–4B, 5A–5B)
* `docs/plans/phase14/dynamic_logic_and_magic_numbers_audit.md` — all VAD/AEC/STT timer horizons & thresholds

This report does not contain recipe text. It is the hand-off baseline for the next agent: fold / harden / rewrite / create E2E.

---

## 1. Scorecard (verbatim from lead, §1)

Graded 1–5 on Entry / Wiring / Realism / Contention / Spec (total /25).

| Test File | Seam / Domain | Entry | Wiring | Realism | Contention | Spec | **Total (/25)** | Verdict |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| `audio_capture_aec_test.rs` | Capture → AEC Worker | 3 | 3 | 2 | 1 | 3 | **12/25** | **Fold** into `aec_pipeline.rs` |
| `audio_capture/aec_pipeline.rs` | Playback Tap → AEC3 | 3 | 2 | 3 | 2 | 3 | **13/25** | **Harden** — Calls `process_frame` sync; bypasses `PlaybackStreamContext` |
| `audio_capture/barge_in.rs` | AEC → VAD → Router | 4 | 2 | 3 | 3 | 3 | **15/25** | **Rewrite Setup** — 80 lines manual thread wiring; `_dummy_stt_tx` drops STT |
| `vad_segmentation_test.rs` | VAD → STT Worker | 4 | 2 | 3 | 1 | 3 | **13/25** | **Harden** — Hardcodes `800ms`; Router is completely absent |
| `stt_turn_identity_test.rs` | STT Turn State Machine | 4 | 4 | 3 | 4 | 5 | **20/25** | **Keep** — Cleanest component test in repo |
| `transcript_ingestion_test.rs` | Transcript → Accumulator → LLM | 2 | 2 | 3 | 4 | 4 | **15/25** | **Fix Entry Seam** — Calls `on_transcript_final` directly instead of `VoxEvent` router |
| `transcript_to_llm_test.rs` | Transcript → Harness → Qwen | 2 | 3 | 3 | 2 | 3 | **13/25** | **Fold & Un-ignore** — `#[ignore]`, banned `_matrix` name, duplicates ingestion |
| `llm_to_tts_test.rs` | LLM Stream → Clause → TTS | 2 | 2 | 3 | 1 | 3 | **11/25** | **Rewrite** — `#[ignore]`; header claims unit tests that do not exist |
| `playback/tts_to_playback.rs` | TTS Worker → Playback Pre-roll | 4 | 3 | 4 | 2 | 4 | **17/25** | **Harden** — Stops at `PlaybackStarted`; never drains to `PlaybackFinished` |
| `harness_loop_test.rs` | Harness Turn Loop | 2 | 3 | 3 | 2 | 4 | **14/25** | **Misclassified** — Guide calls it E2E, but it is a stubbed component test |
| `agentic_tool_loop_test.rs` | Tool Execution & Wire Mapping | 2 | 2 | 4 | 2 | 4 | **14/25** | **Fix Entry Seam** — Calls `step6_handle_*` directly; live network in `web_search` |
| `session_lifecycle_test.rs` | Session Start/Continue/Pause/End | 5 | 4 | 4 | 3 | 5 | **21/25** | **Keep & Expand** — Drives real `spawn_router` via `VoxEvent` |
| `dictation_ptt_test.rs` | PTT → VAD Window → STT Sink | 4 | 3 | 4 | 4 | 5 | **20/25** | **Harden** — Custom `test-vad-forwarder` thread not in `engine.rs` |
| `dictation_output_test.rs` | Dictation Transcript → Output Sink | 4 | 4 | 4 | 3 | 5 | **20/25** | **Keep** — Clean router-driven seam test |
| `realtime_transport_test.rs` | Realtime WS Framing & Reconnect | 4 | 4 | 4 | 4 | 4 | **20/25** | **Harden** — Misses VAD `StreamPassthrough` pre-roll seam |
| `remote_security_test.rs` | Noise_XX, Pairing Tokens, TrustStore | 4 | 4 | 5 | 4 | 5 | **22/25** | **Keep** — High-signal cryptographic & boundary tests |
| `remote_link_test.rs` | LoopbackLink & WsLink Framing | 4 | 4 | 4 | 4 | 4 | **20/25** | **Harden** — Runs `run_link_conformance` on `LoopbackLink`; skips `WsLink` |
| `remote_connection_test.rs` | Pairing, Dial, Heartbeat, Revoke | 4 | 4 | 4 | 4 | 5 | **21/25** | **Keep & Expand** — Needs Tier 3 audio uplift/downlink seam coverage |
| `settings_persistence_test.rs` | Config Mutation, JSONC, Corruption | 4 | 4 | 4 | 3 | 5 | **20/25** | **Keep** — Solid contract & recovery coverage |

---

## 2. Seven blind spots — why basic voice still breaks in production

Comparing `src/core/engine.rs`, `src/services/vad/segmenter.rs`, `src/services/audio/sink.rs`, `src/services/audio/aec.rs` against the tests exposes seven concrete blind spots.

### BS-1 — No true end-to-end voice test (`harness_loop_test.rs` is not E2E)

> “It has zero audio, zero VAD, zero STT, zero Router, zero TTS, and zero Playback. It calls `Harness::execute_turn` directly against a `stub_llm_worker`.”

Nowhere is there a test that starts at `SessionStart`, streams speech `cap_prod → AEC → VAD → Router (Listening → Thinking) → STT (TranscriptFinal) → Harness → LLM → TTS → PlaybackStreamContext (Speaking → PlaybackFinished → Ready)`. Every test stops at an artificial wall.

### BS-2 — `vad_segmentation_test.rs` bypasses Router AND cheats silence

> “We explicitly configure 800ms here so VAD tests single-turn transcription correctness on this specific legacy test clip without premature segmentation.”

> “Your production default `DEFAULT_VAD_SILENCE_DURATION_MS` is 400ms. In production, users pausing for 400ms mid-sentence were getting cut off prematurely (RCA Finding 4.2)! When the test clip failed with the 400ms production default, the agent changed the test config to 800ms to make the test pass…”

Additionally the test never spawns `spawn_router`, sets `partial_emitter: None`, so `turn_id` sync and `incomplete_clause_hint` (+352ms on conjunctions, `EARLY_UTTERANCE_MIN_END_FRAMES` 50 frames / 800ms protection over first 1.25s) are never exercised.

### BS-3 — `barge_in.rs` uses `_dummy_stt_tx()` and skips `PlaybackStreamContext`

Test passes `stt_tx: _dummy_stt_tx()` whose receiver is immediately dropped. `handle_speech_start` pre-roll send fails with disconnected channel, silently logged. Test pushes samples via `tap.push_sample(s)` 3× synchronously instead of `PlaybackStreamContext::process_output_buffer` (`sink.rs:106`). Production applies volume ramp (`PLAYBACK_VOLUME_RAMP_STEP`), `PLAYBACK_DEFAULT_VOLUME` scaling, and `tap.reset()` on cancel. Tests never cover `AecProcessor::drain_render_reference` (`aec.rs:135-169`). Nightly `aec_adapts_delay_and_suppresses_echo_under_pipewire_latency` panicked at `current: 40 ms` because `delay_samples = 1920` (120ms) vs `DEFAULT_STREAM_DELAY_MS = 40`.

### BS-4 — Four tests bypass production entry seam

* `transcript_ingestion_test.rs` / `transcript_to_llm_test.rs`: manually `set_state(Thinking)` + call `on_transcript_final` / `on_speech_start` directly. Never span `vox-router`.
* `llm_to_tts_test.rs`: manually constructs `RoutingContext` with `InteractionMode::PTT` (while step 1 configured `Passive`) and calls `on_llm_finished` (`lines 213-218`).
* `agentic_tool_loop_test.rs`: calls `step6_handle_terminal_tool` / `step6_handle_non_terminal_tool` directly instead of `Harness::execute_turn` or Router.

Violates `/create-test` Phase 1 & 2a: bypasses router dispatch, owner filtering, races like `Cancelled` vs `SpeechStart` / `PlaybackFinished` vs `LlmFinished`.

### BS-5 — `tts_to_playback.rs` stops halfway (never `PlaybackFinished -> Ready`)

Subtest 1 (`real_synthesis_is_ingested_and_arms_the_pre_roll`) waits for `Speaking`, then manually drains `consumer.try_pop()` in a `while let` loop, instead of `PlaybackStreamContext::process_output_buffer` (`sink.rs:106`) via `create_headless_playback_with_sink` (`harness.rs:76`). `VoxEvent::PlaybackFinished` (`sink.rs:219`) is never emitted. `Speaking -> Ready` (Stage 5 Scenario 5A) never tested.

### BS-6 — Two integration binaries are `#[ignore]`d and rotten

* `transcript_to_llm_test.rs:33` — `#[ignore = "requires vox-llm sidecar binary"]`, banned name `test_transcript_to_llm_matrix` (violates `testing-style-guide.md §4`).
* `llm_to_tts_test.rs:46` — same `#[ignore]`. Header (`lines 11-15`) claims unit tests without a model, yet file contains **only 1 test function and it is ignored → 0 runnable tests** in default tier.

### BS-7 — Hidden test-only plumbing in `dictation_ptt_test.rs`

Lines 113–131 spawn custom thread `test-vad-forwarder` looping on `vad_event_rx` → `event_tx`. Cause: `common::harness::setup_vad_actor` (`harness.rs:163`) creates its own `(vox_event_tx, vox_event_rx)` instead of accepting router’s `vox_event_tx` like `engine.rs:274`. Every user triggers bespoke forwarder workaround.

---

## 3. Retrospective root causes (Lead §2 — kept factual)

* **P0 Missing headless builder:** no `TestEngineBuilder` mirroring `start_audio_engine` (`engine.rs:171`). `get_test_app_state()` allows un-managed `AppState`; `app.state::<Arc<AppState>>()` panics (`state() called before manage()`). 70–80 lines manual `HeapRb` / `VadActorHandles` wiring in `barge_in.rs:49-115`. Fix direction: delete `get_test_app_state()`, add single canonical builder wiring `PlaybackReferenceTap → PlaybackStreamContext → AecProcessor → VadActor → SttWorker → Router`.
* **P0 Unenforced mechanical rules:** banned names (`test_*`, `_matrix`, `_works`, `_basic`, `_sanity`), missing `PURPOSE / ASSERTS / CLASS` blocks, `while rx.try_recv().is_ok() {}`, `let _ = handle.join()`. Observed live violation in `transcript_to_llm_test.rs`. Fix direction: deterministic lint in `app/scripts/check_invariants.mjs` scanning `tests/**/*.rs`.
* **P0 White-box anchoring** in `create-test` Phase 1 (“Read actual source — not documentation — and write this before touching a compiler”). Result: inputs calibrated to constants (`640`, `800`). Fix direction: **Phase 0.5 Black-Box Scene & Physics Contract** before reading `.rs`.
* **P0 Single-ended mutants:** mutants only mutate the leaf called; never prove upstream wiring. Fix direction: require **Wiring Mutant + Dynamic/Skew Mutant**.
* **P1 Missing navigation:** neither `test-engineer.md` nor `testing-style-guide.md` pointed to the two phase14 traces, so agents guessed scenarios and stopped at `3.0s (48000 samples)` without knowing 6.0s AEC3 horizon / 1.25s early-utterance window.
* **P2 Boilerplate:** `barge_in.rs` copy-pastes 75 lines 3× (`48-115`, `185-251`, `329-396`). Caused edit-tool collision (test pasted twice). Fix direction: `common::harness::AudioSceneRig::new()`.

---

## 4. Pipeline bench addendum (Response 2 §1–§2)

`benches/pipeline_bench.rs` is a benchmark, not an E2E gate:

> “It is not run by `cargo nextest` or `cargo-mutants`, and it does not test multi-branch state trajectories (barge-in during `Speaking`, continuation during `Thinking`, or sub-300ms cough recovery).”

`benches/common/pipeline_harness.rs` has four hidden wiring bugs vs `engine.rs`:

1. **Missing `PlaybackStreamContext`** — manually pops `playback_cons` (`pipeline_bench.rs:265-271, 341-342`). `PlaybackFinished` never fires; `Speaking → Ready` never happens.
2. **Missing `AecProcessor` / `PlaybackReferenceTap`** — `PlaybackEngine::from_parts(..., None)`, `aec_tx: None`, `in_prod` feeds `spawn_vad_actor` directly (`pipeline_harness.rs:140, 206, 255`). Skips `spawn_aec_worker` (`engine.rs:224`).
3. **Missing `partial_emitter`** — `pipeline_harness.rs:163` sets `partial_emitter: None`. `incomplete_clause_hint` never fires.
4. **Direct `on_session_start`** — `pipeline_bench.rs:210` calls `on_session_start` + `set_state(Ready)` instead of `VoxEvent::SessionStart` via `router_tx`.

**Offline claim (verbatim core):**

> “YES — 100% of the tests, blind spots, and seams I pointed out can be tested completely offline using only your local model stack (`~/.vox/models`) and loopback sockets (`127.0.0.1:0`), with ZERO external cloud API keys.”

All layers run locally: HeapRb + `PlaybackStreamContext` / WebRTC AEC3 / Earshot+Silero+Ten VAD / Embedded STT (`nemotron`) / transliteration / Qwen GGUF via `EmbeddedProvider` or scripted `LlmProvider` / `common::wire` loopback (`127.0.0.1:0`) / Supertonic+Kokoro TTS / loopback WS + temp Turso DB. `testing-style-guide.md §8` mandates this; external keys never enter default tier.

---

## 5. Directory layout decision (Lead Part 3 §1)

Keep all tests in `app/src-tauri/tests/` (Cargo compiles each top-level `.rs` as a binary). Use Submodule + Naming Tier:

* **Submodule grouping** for related seams in one binary (e.g. `audio_capture_aec_test.rs` as module root including `audio_capture/aec_pipeline.rs` + `barge_in.rs`) — saves link budget.
* **`e2e_` prefix** for true full-loop scene binaries (`tests/e2e_<scene>_test.rs`).

```bash
cargo nextest run -E 'not binary(/^e2e_/)' --release --test-threads=1   # fast seams
cargo nextest run -E 'binary(/^e2e_/)' --release --test-threads=1       # full E2E scenes
```

---

## 6. Fold / prune / rewrite matrix for the next agent (verbatim Lead table)

| Current File(s) | Action | Exact Instruction |
| --- | --- | --- |
| `audio_capture_aec_test.rs` + `audio_capture/aec_pipeline.rs` | **Fold & Harden** | Fold 3 worker subtests from `audio_capture_aec_test.rs` into `aec_pipeline.rs` (leave root as pure module). Drive reference via `PlaybackStreamContext::process_output_buffer`. ≥120ms delay, ≥7s. |
| `audio_capture/barge_in.rs` | **Refactor Setup & Expand** | Extract 3× duplicated 75-line setup into `AudioSceneRig`. Replace `_dummy_stt_tx()` with real STT channel; assert barge-in flushes pre-roll+body to `stt_rx` with incremented `turn_id`. |
| `vad_segmentation_test.rs` | **Harden** | Remove `initial_silence_duration_ms: 800` cheat; run prod default `400ms`; verify `EARLY_UTTERANCE_MIN_END_FRAMES` (800ms / 1.25s) + `incomplete_clause_hint` (+352ms) prevent 420ms splits. Add Scenario 2B (`voiced_samples < 4800` → fallback empty `TranscriptFinal`, `Thinking → Ready`). |
| `transcript_to_llm_test.rs` + `transcript_ingestion_test.rs` | **Fold & Fix Seam** | Fold compaction-filler subtest (Subtest 5) into `transcript_ingestion_test.rs` with stub LLM; delete `#[ignore]`d file. Send `VoxEvent::TranscriptFinal` into `spawn_router`. |
| `llm_to_tts_test.rs` + `playback/tts_to_playback.rs` | **Rewrite & Complete Loop** | `Harness::execute_turn` with scripted streaming `LlmProvider` → `StreamRoutingStage` → `tts_rx` → `VoxEvent::LlmFinished` → `spawn_router`. Use `create_headless_playback_with_sink`; pump `sink.process_output_buffer` until `PlaybackFinished` → `Speaking → Ready`. |
| `agentic_tool_loop_test.rs` | **Fix Entry Seam** | Drive through `Harness::execute_turn`. Mock HTTP in `web_search_runs_three_stages_and_stays_non_terminal` via `common::wire`; no live internet in default tier. |
| `remote_link_test.rs` | **Add `WsLink` Conformance** | Add subtest `run_link_conformance` against real `WsLink` pair (`bind` + `dial`) plus existing `LoopbackLink::pair`. |
| **NEW** `tests/e2e_voice_turn_test.rs` | **Create True E2E Binary** | Full prod scene (`PlaybackStreamContext + AecProcessor + VadActor + SttWorker + spawn_router + Harness + TtsWorker`). Scenes: (1) `Ready → Listening → Thinking → Speaking → Ready`, (2) barge-in `Speaking → Listening → Thinking`, (3) mid-thought continuation `Thinking → Listening → merged`, (4) sub-300ms cough recovery via empty fallback. |

---

## 7. What this report is not

* Not the recipe. Recipe patches live in `.agents/skills/create-test/SKILL.md`, `.agents/rules/testing-style-guide.md`, `.agents/rules/test-engineer.md`.
* Not a test run. No claim of passing / failing binaries. Verdicts above are audit grades, not CI results.
* Not a production patch. Production defects, if any, are recorded as findings by test-engineer; backend-engineer owns fixes per role bounds.

---

## 8. Traceability to recipe fixes

| This report § | Companion recipe fix |
| --- | --- |
| §1–§2 BS-1…BS-7 | Style-guide §5.6/§6.1 + SKILL Phase 0.5/Phase 4 circuit breaker |
| §3 P0/P1/P2 | Linter decision + `AudioSceneRig` spec (deferred to implementation agent — recipe now points to it) |
| §4 bench harness | Bench-reuse note in style-guide §1 E2E tier |
| §5 layout | Style-guide §1–§2 `e2e_` naming tier |
| §6 matrix | `test-engineer.md` pointers to phase14 traces |

---

*Generated by system-architect from senior test-engineer transcript. All verbatim rows are quoted from the transcript. File:line citations are as-given in the transcript; the implementation agent must re-verify each against live `app/src-tauri/src/` before touching tests.*

---

## 9. Re-audit after hardening (2026-11-07)

Same rubric: Entry / Wiring / Realism / Contention / Spec, each 1–5, total /25.
Scores are re-graded against the current uncommitted tree, not the campaign plan.
"Was" is the §1 score.

| Seam binary / file | Was | Entry | Wiring | Realism | Contention | Spec | **Now** | Verdict |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| `audio_capture/aec_pipeline.rs` | 13 | 4 | 4 | 5 | 3 | 4 | **20/25** | Keep; F-001 open |
| `audio_capture/barge_in.rs` | 15 | 4 | 4 | 5 | 4 | 5 | **22/25** | Keep; F-001 open |
| `vad_segmentation_test.rs` | 13 | 4 | 4 | 4 | 4 | 4 | **20/25** | Keep |
| `transcript_ingestion_test.rs` | 15 | 4 | 5 | 4 | 4 | 4 | **21/25** | Keep |
| `llm_to_tts_test.rs` | 11 | 4 | 4 | 4 | 3 | 4 | **19/25** | Keep; no longer ignored |
| `playback/tts_to_playback.rs` | 17 | 4 | 3 | 4 | 4 | 5 | **20/25** | Keep |
| `harness_loop_test.rs` | 14 | 3 | 4 | 3 | 4 | 4 | **18/25** | Keep; correctly classified |
| `agentic_tool_loop_test.rs` | 14 | 4 | 4 | 4 | 4 | 4 | **20/25** | Keep |
| `dictation_ptt_test.rs` | 20 | 4 | 4 | 4 | 4 | 5 | **21/25** | Keep |
| `remote_link_test.rs` | 20 | 4 | 4 | 4 | 4 | 4 | **20/25** | Unchanged; F-003 open |
| `e2e_voice_pipeline_test.rs` | — | 5 | 5 | 5 | 2 | 4 | **21/25** | New; Scene 3 ignored (F-005) |
| `audio_capture_aec_test.rs` (root) | 12 | — | — | — | — | — | **n/a** | Folded into `audio_capture/` |
| `transcript_to_llm_test.rs` | 13 | — | — | — | — | — | **n/a** | Deleted; folded into ingestion |

### Why each score changed

**`aec_pipeline.rs` 13 → 20.** Entry (+1): the reference now flows through `PlaybackReferenceTap` and the worker, not a synchronous `process_frame` call. Wiring (+2): the real `spawn_aec_worker`, clean-capture ring, and bypass command are exercised in one rig. Realism (+2): the ignored F-001 scene keeps the 120 ms skew and adds an active bypass control that reproduces the echo interruption. Spec (+1): intent blocks now name the production failure and the ERLE threshold. Contention (+1, still 3): the ERLE scene is ignored and flaky — it measured 12.98 dB in one run and passed in another — so it cannot be relied on yet.

**`barge_in.rs` 15 → 22.** Entry (+0): the router-driven path was already correct. Wiring (+2): the duplicated 75-line setup is replaced by `AudioSceneRig`, and the dummy STT sink is replaced by a real STT channel. Realism (+2): the double-talk scene uses `PlaybackStreamContext` output and the real pre-roll/body flush. Spec (+2): the production bug is recorded instead of hidden; F-001 stays red under `#[ignore]` with exact numbers. Contention (+1): no more thread-wiring races in the setup; the AEC skew itself remains the open production defect, not a test defect.

**`vad_segmentation_test.rs` 13 → 20.** Entry (+0): the VAD → STT → router path was already real. Wiring (+2): the production `400 ms` silence default replaces the `800 ms` cheat, and the partial emitter drives the real `incomplete_clause_hint`. Realism (+1): branch points 2A/2B (700 ms pause, sub-300 ms burst) are now explicit scenes against real Nemotron. Contention (+3): the silence guards now use an ordered VAD barrier instead of `sleep(300 ms)` + `try_recv`, so the negative assertions cannot pass by racing. Spec (+1): the intent block and `MUTANT` names match the phase14 trace.

**`transcript_ingestion_test.rs` 15 → 21.** Entry (+2): every event now enters through `spawn_router`; the direct `on_transcript_final` calls are gone. Wiring (+3): the folded compaction-filler and continuation scenes drive the real accumulator and harness stub. Realism (+1): continuation, duplicate suppression, and stale-cancel scenes are now explicit. Contention (+0, already 4): the two absence tests now use an ordered `Rig::quiesce()` (router and LLM shutdown + join) instead of `sleep(250 ms)`. Spec (+0, already 4).

**`llm_to_tts_test.rs` 11 → 19.** Entry (+2): `Harness::execute_turn` plus `spawn_router` replaces the manual `on_llm_finished` call. Wiring (+2): the scripted and real `LlmProvider` both feed the real clause dispatch and TTS channel. Realism (+1): the real Qwen scene runs through the fixed UDS sidecar. Contention (+2): the file is no longer `#[ignore]`; three deterministic scenes run in the default-capable tier, and `SeamRig::shutdown` now kills the sidecar. Spec (+1): the stale "broken in-tree" claim is removed. Remaining gap: the real-model scene is still model-gated and stretched to 42 s by contention.

**`tts_to_playback.rs` 17 → 20.** Entry (+0): already real. Wiring (+0): already real. Realism (+0): already real. Contention (+2): the scene now pumps `PlaybackStreamContext::process_output_buffer` until `PlaybackFinished` and `Speaking → Ready`, instead of stopping at `PlaybackStarted` and popping the consumer directly. Spec (+1): the exit condition is asserted, not assumed.

**`harness_loop_test.rs` 14 → 18.** Entry (+1): the file is classified as a component seam, not E2E, so its stub boundary is honest. Wiring (+1): the rig actually spawns the harness loop. Realism (+1): the scenes assert the loop's own state transitions. Contention (+1): joins are tightened to `.expect(...)`, so a dead worker fails the test. It remains a stubbed component test, so Realism stays at 3.

**`agentic_tool_loop_test.rs` 14 → 20.** Entry (+2): scenes go through `Harness::execute_turn`, not `step6_handle_*`. Wiring (+2): the wire mapping and terminal/non-terminal branches are driven through the harness. Realism (+1): the `web_search` scene is intercepted with `common::wire` loopback; no live network in the default tier. Contention (+1): the tool registry and turn lifecycle are asserted at the seam, not at the leaf.

**`dictation_ptt_test.rs` 20 → 21.** Contention (+1): the bespoke `test-vad-forwarder` thread is gone; the shared harness forwards VAD events with the router's own channel. Everything else was already strong.

**`remote_link_test.rs` stays 20.** Entry/Wiring/Realism/Contention are unchanged. Spec stays 4 because the generic suite is still not applicable to `WsLink` (F-003); the six `ws_link_*` subtests remain the valid coverage.

**`e2e_voice_pipeline_test.rs` new, 21/25.** Entry 5: starts at `SessionStart` and ends at `Ready` through AEC → VAD → router → STT → harness → LLM → TTS → playback. Wiring 5: real Kokoro, real Nemotron, real Qwen over the fixed UDS sidecar. Realism 5: scenes cover full turn, double-talk barge-in, mid-thought merge, and sub-300 ms recovery. Contention 2: the combined run passes 3/4. Scene 3 fails on the merged turn because the cancelled first inference keeps the sidecar busy and turn 3 hits the 30 s idle timeout; it is now `#[ignore = "PROD-BUG: LLM-CANCEL-STARVES …"]` with the exact assertion preserved (F-005). `std::process::exit(0)` still suppresses leak detection on the Kokoro scenes. Spec 4: `MUTANT` seeds are present and the failed scene is recorded, not weakened.

**`audio_capture_aec_test.rs` root and `transcript_to_llm_test.rs` are n/a.** The first is now a pure module root; its three worker subtests live in `audio_capture/aec_pipeline.rs`. The second is deleted; its compaction subtest is folded into `transcript_ingestion_test.rs`. Keeping either would have been a weak duplicate seam.
